use anyhow::Result;

use super::client::{ChatMessage, OllamaClient};

pub async fn ask(
    client: &OllamaClient,
    model: &str,
    system_context: &str,
    messages: &[ChatMessage],
) -> Result<ChatMessage> {
    let mut full = vec![ChatMessage {
        role: "system".to_string(),
        content: system_context.to_string(),
        tool_calls: None,
    }];
    full.extend_from_slice(messages);

    let resp = client
        .chat_with_tools(model, full, &[crate::ai::tools::search_news_tool()])
        .await?;

    Ok(resp.message)
}
