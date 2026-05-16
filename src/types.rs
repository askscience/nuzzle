use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Feed {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub feed_type: String,
    pub category: Option<String>,
    pub icon_url: Option<String>,
    pub last_fetched_at: Option<DateTime<Utc>>,
    pub error_count: i64,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: i64,
    pub feed_id: i64,
    pub guid: String,
    pub title: Option<String>,
    pub link: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub is_read: bool,
    pub is_starred: bool,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Highlight {
    pub id: i64,
    pub entry_id: i64,
    pub text: String,
    pub note: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct AISession {
    pub id: i64,
    pub name: String,
    pub model: String,
    pub description: String,
    pub session_type: SessionType,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionType {
    Chat,
    Code,
    Search,
}

impl SessionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionType::Chat => "chat",
            SessionType::Code => "code",
            SessionType::Search => "search",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "code" => SessionType::Code,
            "search" => SessionType::Search,
            _ => SessionType::Chat,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionFile {
    pub id: i64,
    pub session_id: i64,
    pub filename: String,
    pub file_type: String,
    pub filepath: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SessionEmbedding {
    pub id: i64,
    pub session_id: i64,
    pub embedding: Vec<f32>,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct AIMessage {
    pub id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Embedding {
    pub id: i64,
    pub entry_id: i64,
    pub embedding: Vec<f32>,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: Entry,
    pub feed_title: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Feeds,
    Articles,
    Reading,
    Search,
    Ask,
    Digest,
    Tag,
    Highlight,
    Help,
    ModelSelect,
    SessionSelect,
}

#[derive(Debug, Clone)]
pub enum Action {
    NavigateUp,
    NavigateDown,
    OpenFeed,
    OpenArticle,
    Back,
    Refresh,
    Summarize,
    Ask,
    Search,
    ToggleStar,
    Tag,
    AddTag,
    ShowDigest,
    ShowHighlights,
    ShowHelp,
    Export,
    ScrollUp,
    ScrollDown,
    Quit,
}
