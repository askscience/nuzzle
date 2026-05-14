use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::db::repository::Repository;
use crate::feed::fetcher;
use crate::feed::parser;
use reqwest::Client as HttpClient;

pub struct FeedManager {
    repo: Arc<Mutex<Repository>>,
    http: HttpClient,
}

impl FeedManager {
    pub fn new(repo: Arc<Mutex<Repository>>) -> Self {
        Self { repo, http: HttpClient::new() }
    }

    pub async fn add_feed_url(&self, url: &str) -> Result<String> {
        let repo = self.repo.lock().await;
        let existing = repo.list_feeds()?;
        if existing.iter().any(|f| f.url == url) {
            let t = existing.iter().find(|f| f.url == url).map(|f| f.title.clone()).unwrap_or_default();
            return Ok(format!("{} (already added)", t));
        }
        drop(repo);

        let xml = fetcher::fetch_feed(&self.http, url).await?;
        let feed = feed_rs::parser::parse(xml.as_bytes())?;
        let title = feed.title.map(|t| t.content).unwrap_or_else(|| url.to_string());

        let repo = self.repo.lock().await;
        repo.add_feed(&title, url, "rss")?;
        // Get the id of the new feed
        let feed_id = repo.list_feeds()?.iter().find(|f| f.url == url).map(|f| f.id).unwrap_or(0);
        drop(repo);
        // Fetch articles for the new feed immediately
        if feed_id > 0 {
            match self.refresh_single(feed_id, url).await {
                Ok(count) => return Ok(format!("{} (+{} articles)", title, count)),
                Err(e) => return Ok(format!("{} (fetch err: {})", title, e)),
            }
        }
        Ok(title)
    }

    pub async fn refresh_all(&self, _config: &Config) -> Result<(usize, Vec<String>)> {
        let mut total = 0;
        let mut errors = vec![];
        let feeds = {
            let repo = self.repo.lock().await;
            repo.list_feeds()?
        };
        for feed in &feeds {
            match self.refresh_single(feed.id, &feed.url).await {
                Ok(count) => total += count,
                Err(e) => {
                    errors.push(format!("{}: {}", feed.title, e));
                    let repo = self.repo.lock().await;
                    let _ = repo.increment_feed_error(feed.id);
                }
            }
        }
        Ok((total, errors))
    }

    pub async fn refresh_single(&self, feed_id: i64, url: &str) -> Result<usize> {
        let xml = fetcher::fetch_feed(&self.http, url).await?;
        let entries = parser::parse_feed_xml(feed_id, &xml)?;
        let repo = self.repo.lock().await;
        let mut count = 0;
        for entry in &entries {
            if !repo.entry_exists(&entry.guid)? {
                repo.insert_entry(entry)?;
                count += 1;
            }
        }
        repo.update_feed_fetch_time(feed_id, &Utc::now().to_rfc3339())?;
        Ok(count)
    }

    pub async fn load_feeds_from_config(&self, config: &Config) -> Result<Vec<String>> {
        let repo = self.repo.lock().await;
        let mut added = vec![];
        for url in &config.feeds.urls {
            let existing = repo.list_feeds()?;
            if existing.iter().any(|f| f.url == *url) { continue; }
            let xml = match fetcher::fetch_feed(&self.http, url).await {
                Ok(x) => x,
                Err(e) => { added.push(format!("{} (error: {})", url, e)); continue; }
            };
            let feed = match feed_rs::parser::parse(xml.as_bytes()) {
                Ok(f) => f,
                Err(e) => { added.push(format!("{} (parse: {})", url, e)); continue; }
            };
            let title = feed.title.map(|t| t.content).unwrap_or_else(|| url.clone());
            repo.add_feed(&title, url, "rss")?;
            added.push(title);
        }
        Ok(added)
    }
}
