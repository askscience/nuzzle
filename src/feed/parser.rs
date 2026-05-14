use anyhow::Result;
use chrono::Utc;
use feed_rs::parser;

use crate::types::Entry;

pub fn parse_feed_xml(feed_id: i64, xml: &str) -> Result<Vec<Entry>> {
    let feed = parser::parse(xml.as_bytes())?;

    let entries: Vec<Entry> = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let published = match entry.published {
                Some(dt) => dt.with_timezone(&Utc),
                None => entry.updated?,
            };

            Some(Entry {
                id: 0,
                feed_id,
                guid: entry.id.clone(),
                title: entry.title.map(|t| t.content),
                link: entry.links.first().map(|l| l.href.clone()),
                summary: entry.summary.map(|s| s.content),
                content: entry.content.and_then(|c| c.body),
                author: entry.authors.first().map(|a| a.name.clone()),
                published_at: Some(published),
                fetched_at: Some(Utc::now()),
                is_read: false,
                is_starred: false,
            })
        })
        .collect();

    Ok(entries)
}
