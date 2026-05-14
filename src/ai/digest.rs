use anyhow::Result;

use crate::types::Entry;

use super::client::OllamaClient;

const DIGEST_PROMPT: &str = "You are a personal news briefing assistant. \
    Below are recent articles from the user's feeds. \
    Create a concise daily digest with:\n\
    1. A 1-sentence overview of the day's main theme\n\
    2. Grouped summaries by topic (3-5 bullet points per group)\n\
    3. A 'why this matters' takeaway for the most important story\n\n\
    Articles:\n\n";

pub async fn generate_digest(
    client: &OllamaClient,
    model: &str,
    entries: &[Entry],
) -> Result<String> {
    let articles: Vec<String> = entries
        .iter()
        .filter_map(|e| {
            let title = e.title.as_deref().unwrap_or("Untitled");
            let summary = e.summary.as_deref().unwrap_or("");
            if summary.is_empty() { None } else { Some(format!("- {}: {}", title, summary)) }
        })
        .collect();

    if articles.is_empty() {
        return Ok("No articles with summaries to digest.".to_string());
    }

    let prompt = format!("{}{}", DIGEST_PROMPT, articles.join("\n"));
    client.generate(model, &prompt).await
}
