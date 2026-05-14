use std::collections::HashMap;

use anyhow::Result;

use crate::ai::embedding::cosine_similarity;
use crate::db::repository::Repository;
use crate::types::{Entry, SearchResult};

pub struct EmbeddingIndex {
    entries: Vec<(Entry, Vec<f32>)>,
}

impl EmbeddingIndex {
    pub fn new() -> Self {
        Self { entries: vec![] }
    }

    pub fn load(&mut self, repo: &Repository) -> Result<()> {
        let embeddings = repo.load_all_embeddings()?;
        let mut entry_map: HashMap<i64, Entry> = HashMap::new();
        for entry in repo.list_all_entries()? {
            entry_map.insert(entry.id, entry);
        }

        self.entries = embeddings
            .into_iter()
            .filter_map(|emb| {
                entry_map
                    .remove(&emb.entry_id)
                    .map(|entry| (entry, emb.embedding))
            })
            .collect();

        Ok(())
    }

    pub fn search(&self, query_embedding: &[f32], top_k: usize, feed_titles: &HashMap<i64, String>) -> Vec<SearchResult> {
        let mut scored: Vec<(f64, &Entry)> = self
            .entries
            .iter()
            .map(|(entry, emb)| (cosine_similarity(query_embedding, emb), entry))
            .filter(|(score, _)| *score > 0.5)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(top_k)
            .map(|(score, entry)| SearchResult {
                entry: entry.clone(),
                feed_title: feed_titles.get(&entry.feed_id).cloned().unwrap_or_default(),
                score,
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
