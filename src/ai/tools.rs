use serde_json::Value;
use crate::types::Entry;

pub const SEARCH_NEWS_TOOL: &str = r#"
{
  "type": "function",
  "function": {
    "name": "search_news",
    "description": "Search through the user's RSS feed articles to find relevant stories. If no articles match, suggest RSS feed URLs the user can add.",
    "parameters": {
      "type": "object",
      "properties": {
        "query": {
          "type": "string",
          "description": "Search keywords to find relevant articles"
        },
        "max_results": {
          "type": "integer",
          "description": "Maximum number of results (default 5)",
          "default": 5
        }
      },
      "required": ["query"]
    }
  }
}
"#;

pub const ADD_FEED_TOOL: &str = r#"
{
  "type": "function",
  "function": {
    "name": "add_feed",
    "description": "Add a new RSS/Atom feed URL to the user's collection. The feed will be fetched and its articles included in future searches. Use this when the user wants to add a suggested feed.",
    "parameters": {
      "type": "object",
      "properties": {
        "url": {
          "type": "string",
          "description": "The complete RSS feed URL to add (e.g. https://hnrss.org/frontpage)"
        }
      },
      "required": ["url"]
    }
  }
}
"#;

pub fn search_news_tool() -> Value { serde_json::from_str(SEARCH_NEWS_TOOL).unwrap() }
pub fn add_feed_tool() -> Value { serde_json::from_str(ADD_FEED_TOOL).unwrap() }
pub fn all_tools() -> Vec<Value> { vec![search_news_tool(), add_feed_tool()] }

pub fn execute_search_news(entries: &[Entry], query: &str, max_results: usize) -> Vec<Entry> {
    let query_lower = query.to_lowercase();
    let terms: Vec<&str> = query_lower.split_whitespace().collect();
    let mut scored: Vec<(usize, &Entry)> = entries.iter().filter_map(|e| {
        let haystack = [
            e.title.as_deref().unwrap_or(""),
            e.summary.as_deref().unwrap_or(""),
            e.content.as_deref().unwrap_or(""),
        ].join(" ").to_lowercase();
        let score: usize = terms.iter().filter(|t| haystack.contains(*t)).count();
        if score > 0 { Some((score, e)) } else { None }
    }).collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().take(max_results).map(|(_, e)| e.clone()).collect()
}

pub fn format_search_results(entries: &[Entry]) -> String {
    if entries.is_empty() {
        return "RESULTS: No matching articles in your feeds yet.\n\n\
            The user has zero articles for this topic. You MUST suggest 2-3 specific, high-quality \
            RSS feed URLs the user can add. Use add_feed tool if the user asks you to.\n\n\
            Suggested feeds:\n\
            • https://example.com/rss — description".to_string();
    }
    let mut r = vec!["RESULTS: Found matching articles:".to_string()];
    for (i, e) in entries.iter().enumerate() {
        let title = e.title.as_deref().unwrap_or("Untitled");
        let summary = e.summary.as_deref().unwrap_or("");
        r.push(format!("\n{}. {} — {}", i + 1, title, summary));
    }
    r.join("\n")
}
