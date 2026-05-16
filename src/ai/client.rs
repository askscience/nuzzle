use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use bytes::Bytes;
use reqwest::Client as HttpClient;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};

use std::sync::Arc;

use crate::tools::{executor, protocol, registry};

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

    /// Chat with tool-calling loop using text-based protocol.
    /// Returns the final response text (after all tool calls resolved).
    /// Optionally sends status updates through status_tx for UI feedback.
    pub async fn chat_with_text_tools(
        &self,
        model: &str,
        system: &str,
        user_prompt: &str,
        history: &[ChatMessage],
        tool_registry: Arc<Mutex<registry::ToolRegistry>>,
        _session_type: &str,
        env_vars: HashMap<String, String>,
        status_tx: Option<mpsc::UnboundedSender<String>>,
    ) -> Result<String> {
        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage { role: "system".to_string(), content: system.to_string(), tool_calls: None },
        ];
        for m in history {
            messages.push(ChatMessage { role: m.role.clone(), content: m.content.clone(), tool_calls: None });
        }
        messages.push(ChatMessage { role: "user".to_string(), content: user_prompt.to_string(), tool_calls: None });

        let max_loops = 8;
        let mut tool_summary = Vec::new();

        for _ in 0..max_loops {
            if let Some(ref tx) = status_tx {
                let _ = tx.send("thinking...".to_string());
            }

            let resp = self.chat(model, messages.clone()).await?;

            if let Some(ref tx) = status_tx {
                let _ = tx.send("analyzing...".to_string());
            }

            if !protocol::has_tool_calls(&resp) {
                // Send final summary
                if !tool_summary.is_empty() {
                    if let Some(ref tx) = status_tx {
                        let _ = tx.send(format!("done: {}", tool_summary.join(", ")));
                    }
                } else if let Some(ref tx) = status_tx {
                    let _ = tx.send("done".to_string());
                }
                return Ok(resp);
            }

            let calls = protocol::parse_tool_calls(&resp);
            let clean_response = protocol::strip_tool_blocks(&resp);

            // Add assistant's text (without tool blocks) first
            if !clean_response.is_empty() {
                messages.push(ChatMessage { role: "assistant".to_string(), content: clean_response, tool_calls: None });
            }

            // Execute all tool calls and add results
            let mut tool_output = String::new();
            let reg = tool_registry.lock().await;
            for call in &calls {
                let label = format!("{} {}", call.name, call.args);
                if let Some(ref tx) = status_tx {
                    let _ = tx.send(label.clone());
                }

                tool_output.push_str(&format!("Tool: {} {}\n", call.name, call.args));
                match executor::execute_tool(&reg, call, &env_vars).await {
                    Ok(output) => {
                        let trimmed = if output.len() > 8000 {
                            format!("{}...\n[truncated]", &output[..8000])
                        } else {
                            output.clone()
                        };
                        let output_lines = output.lines().count();
                        let output_bytes = output.len();
                        tool_summary.push(format!("{} ({} lines)", call.name, output_lines));
                        if let Some(ref tx) = status_tx {
                            let _ = tx.send(format!("{}: {} lines, {} bytes", call.name, output_lines, output_bytes));
                        }
                        tool_output.push_str(&trimmed);
                    }
                    Err(e) => {
                        tool_summary.push(format!("{} failed", call.name));
                        if let Some(ref tx) = status_tx {
                            let _ = tx.send(format!("{}: ERROR", call.name));
                        }
                        tool_output.push_str(&format!("ERROR: {}\n", e));
                    }
                }
                tool_output.push_str("\n---\n");
            }
            drop(reg);

            messages.push(ChatMessage {
                role: "user".to_string(),
                content: format!("Tool execution results:\n{}", tool_output),
                tool_calls: None,
            });
        }

        // Final response after max loops
        if !tool_summary.is_empty() {
            if let Some(ref tx) = status_tx {
                let _ = tx.send(format!("done: {}", tool_summary.join(", ")));
            }
        } else if let Some(ref tx) = status_tx {
            let _ = tx.send("done".to_string());
        }
        self.chat(model, messages).await
    }

    /// Generate a short session description from the first exchange
    pub async fn generate_description(&self, model: &str, messages: &[ChatMessage]) -> String {
        let mut prompt = String::from("Summarize this conversation topic in ONE short sentence (max 100 characters).\n\n");
        for m in messages {
            prompt.push_str(&format!("[{}]: {}\n", m.role, m.content));
        }
        prompt.push_str("\nDescription: ");

        match self.generate(model, &prompt).await {
            Ok(desc) => {
                let desc = desc.trim().trim_matches('"').trim();
                if desc.len() > 120 {
                    format!("{}...", &desc[..117])
                } else {
                    desc.to_string()
                }
            }
            Err(_) => String::new(),
        }
    }
}
