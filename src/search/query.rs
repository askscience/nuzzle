use std::collections::HashMap;

use anyhow::Result;

use crate::ai::client::OllamaClient;
use crate::search::index::EmbeddingIndex;
use crate::types::SearchResult;

pub async fn semantic_search(
    client: &OllamaClient,
    embed_model: &str,
    index: &EmbeddingIndex,
    query: &str,
    top_k: usize,
    feed_titles: &HashMap<i64, String>,
) -> Result<Vec<SearchResult>> {
    let query_embedding = client.embed(embed_model, query).await?;
    Ok(index.search(&query_embedding, top_k, feed_titles))
}
