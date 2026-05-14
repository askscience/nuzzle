use anyhow::Result;

use super::client::OllamaClient;

const SUMMARY_PROMPT: &str = "Summarize the following article in 3-5 concise bullet points. \
    Focus on the key arguments, findings, and conclusions. \
    Be factual and objective.\n\nArticle:\n";

pub async fn summarize(client: &OllamaClient, model: &str, content: &str) -> Result<String> {
    let prompt = format!("{}{}", SUMMARY_PROMPT, content);
    client.generate(model, &prompt).await
}
