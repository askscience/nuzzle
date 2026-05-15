use anyhow::{Context, Result, bail};
use bytes::Bytes;
use reqwest::Client as HttpClient;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct OllamaClient {
    http: HttpClient,
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateResponse {
    pub response: String,
    pub done: bool,
    #[serde(default)]
    pub context: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub done: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolCall {
    pub function: ToolFunction,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: Value,
}

impl OllamaClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            http: HttpClient::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Send a POST request + deserialize, handling Ollama error responses.
    async fn post_json<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.post(&url).json(&body).send().await?;
        let status = resp.status();
        let raw = resp.text().await?;

        #[derive(Debug, Deserialize)]
        struct ErrorBody { error: String }

        if let Ok(eb) = serde_json::from_str::<ErrorBody>(&raw) {
            if !eb.error.is_empty() {
                bail!("Ollama: {}", eb.error);
            }
        }

        if !status.is_success() {
            let preview = &raw[..raw.len().min(200)];
            bail!("Ollama HTTP {}: {}", status.as_u16(), preview);
        }

        serde_json::from_str::<T>(&raw)
            .with_context(|| format!("parse error for {} — raw: {}", path, &raw[..raw.len().min(300)]))
    }

    pub async fn generate(&self, model: &str, prompt: &str) -> Result<String> {
        let body = json!({"model": model, "prompt": prompt, "stream": false});
        let resp: GenerateResponse = self.post_json("/api/generate", body).await?;
        Ok(resp.response)
    }

    pub async fn generate_with_system(&self, model: &str, system: &str, prompt: &str) -> Result<String> {
        let body = json!({"model": model, "system": system, "prompt": prompt, "stream": false});
        let resp: GenerateResponse = self.post_json("/api/generate", body).await?;
        Ok(resp.response)
    }

    pub async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<String> {
        let body = json!({"model": model, "messages": messages, "stream": false});
        let resp: ChatResponse = self.post_json("/api/chat", body).await?;
        Ok(resp.message.content)
    }

    pub async fn chat_with_tools(
        &self, model: &str, messages: Vec<ChatMessage>, tools: &[Value],
    ) -> Result<ChatResponse> {
        let mut body = json!({"model": model, "messages": messages, "stream": false});
        if !tools.is_empty() { body["tools"] = json!(tools); }
        self.post_json("/api/chat", body).await
    }

    pub async fn generate_stream(
        &self, model: &str, system: &str, prompt: &str,
        tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        let body = json!({"model": model, "system": system, "prompt": prompt, "stream": true});
        let resp = self
            .http
            .post(format!("{}/api/generate", self.base_url))
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let stream = resp.bytes_stream();
        futures::pin_mut!(stream);
        let mut buf = Vec::new();
        while let Some(item) = futures::StreamExt::next(&mut stream).await {
            let chunk: Bytes = item?;
            buf.extend_from_slice(&chunk);
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line = buf.drain(..=pos).collect::<Vec<_>>();
                let line_str = String::from_utf8_lossy(&line).trim().to_string();
                if line_str.is_empty() { continue; }
                if let Ok(val) = serde_json::from_str::<Value>(&line_str) {
                    if let Some(err) = val.get("error").and_then(|v| v.as_str()) {
                        tx.send(format!("Error: {}\n", err)).ok();
                        tx.send("__DONE__".to_string()).ok();
                        return Ok(());
                    }
                    if val.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                        tx.send("__DONE__".to_string()).ok();
                    } else if let Some(t) = val.get("response").and_then(|v| v.as_str()) {
                        if !t.is_empty() { tx.send(t.to_string()).ok(); }
                    }
                }
            }
        }
        if !status.is_success() {
            tx.send(format!("Error: Ollama HTTP {}\n", status.as_u16())).ok();
        }
        tx.send("__DONE__".to_string()).ok();
        Ok(())
    }

    pub async fn embed(&self, model: &str, input: &str) -> Result<Vec<f32>> {
        let body = json!({"model": model, "input": input});
        let resp: EmbeddingResponse = self.post_json("/api/embeddings", body).await?;
        Ok(resp.embedding)
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        #[derive(Debug, Deserialize)]
        struct ModelsResp { models: Vec<Model> }
        #[derive(Debug, Deserialize)]
        struct Model { name: String }

        let url = format!("{}/api/tags", self.base_url);
        let resp_text = self.http.get(&url).send().await?.text().await?;

        #[derive(Debug, Deserialize)]
        struct ErrorBody { error: String }

        if let Ok(eb) = serde_json::from_str::<ErrorBody>(&resp_text) {
            if !eb.error.is_empty() {
                bail!("Ollama: {}", eb.error);
            }
        }

        let resp: ModelsResp = serde_json::from_str(&resp_text)
            .with_context(|| format!("parse error: {}", &resp_text[..resp_text.len().min(200)]))?;
        Ok(resp.models.into_iter().map(|m| m.name).collect())
    }

    pub async fn health_check(&self) -> Result<bool> {
        match self.http.get(format!("{}/api/tags", self.base_url)).send().await {
            Ok(r) => Ok(r.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}
