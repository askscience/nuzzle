use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;
use crossterm::event::{self as cr_event, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::text::{Line, Span};
use ratatui::Terminal;
use tokio::sync::{mpsc, Mutex};
use tui_textarea::TextArea;

use crate::ai::client::{ChatMessage, OllamaClient};
use crate::ai::{digest, qa, summarizer, tools};
use crate::config::Config;
use crate::db::repository::Repository;
use crate::feed::manager::FeedManager;
use crate::highlight;
use crate::search::index::EmbeddingIndex;
use crate::search::query;
use crate::tui::animations::{BrailleSpinner, BraillePulse};
use crate::tui::{layout, widgets};
use crate::types::*;

pub struct App {
    // core
    repo: Arc<Mutex<Repository>>,
    ai: OllamaClient,
    config: Config,
    feed_manager: Arc<Mutex<FeedManager>>,
    embedding_index: EmbeddingIndex,

    // navigation
    mode: AppMode,
    prev_mode: AppMode,
    feeds: Vec<Feed>,
    entries: Vec<Entry>,
    all_entries: Vec<Entry>,
    selected_feed: usize,
    selected_entry: usize,
    scroll_offset: u16,

    // animations
    spinner: BrailleSpinner,
    pulse: BraillePulse,

    // search
    search_results: Vec<SearchResult>,
    search_selected: usize,
    search_input: TextArea<'static>,

    // ask bar — always visible
    ask_input: TextArea<'static>,

    // sessions
    session_id: i64,
    session_name: String,

    // conversation blocks — newest first
    current_block: String,   // being streamed
    history: String,         // completed blocks
    answer_scroll: u16,
    is_streaming: bool,
    streaming_rx: Option<mpsc::UnboundedReceiver<String>>,

    // overlays
    loading_message: Option<String>,
    summary_text: Option<String>,
    digest_text: Option<String>,
    highlights: Vec<Highlight>,
    current_tags: Vec<String>,
    status_message: Option<String>,
    status_time: Instant,

    // model selection
    available_models: Vec<String>,
    model_list_selected: usize,

    // layout
    narrow_mode: bool,
    should_quit: bool,
    save_answer_needed: bool,
}

impl App {
    pub fn new(
        repo: Arc<Mutex<Repository>>,
        ai: OllamaClient,
        config: Config,
        feed_manager: FeedManager,
    ) -> Self {
        let mut ask_input = TextArea::default();
        ask_input.set_placeholder_text("");

        Self {
            repo,
            ai,
            config,
            feed_manager: Arc::new(Mutex::new(feed_manager)),
            embedding_index: EmbeddingIndex::new(),
            mode: AppMode::Feeds,
            prev_mode: AppMode::Feeds,
            feeds: vec![],
            entries: vec![],
            all_entries: vec![],
            selected_feed: 0,
            selected_entry: 0,
            scroll_offset: 0,
            spinner: BrailleSpinner::default(),
            pulse: BraillePulse::default(),
            search_results: vec![],
            search_selected: 0,
            search_input: TextArea::default(),
            ask_input,
            session_id: 0,
            session_name: String::new(),
            current_block: String::new(),
            history: String::new(),
            answer_scroll: 0,
            is_streaming: false,
            streaming_rx: None,
            loading_message: None,
            summary_text: None,
            digest_text: None,
            highlights: vec![],
            current_tags: vec![],
            status_message: None,
            status_time: Instant::now(),
            available_models: vec![],
            model_list_selected: 0,
            narrow_mode: false,
            should_quit: false,
            save_answer_needed: false,
        }
    }

    pub async fn init(&mut self) -> Result<()> {
        self.load_feeds_from_db().await?;
        // Auto-fetch articles on startup
        self.loading_message = Some("Fetching...".to_string());
        let (count, _errors) = {
            let fm = self.feed_manager.lock().await;
            fm.refresh_all(&self.config).await?
        };
        self.loading_message = None;
        if !_errors.is_empty() { log::warn!("fetch errs: {:?}", _errors); }
        {
            let repo = self.repo.lock().await;
            self.all_entries = repo.list_all_entries()?;
            self.session_id = repo.create_session("default", &self.config.ollama.model)?;
            self.session_name = String::new();
        }
        self.load_embedding_index().await;
        self.set_status(&format!("{} articles loaded", count));
        Ok(())
    }

    async fn load_feeds_from_db(&mut self) -> Result<()> {
        let repo = self.repo.lock().await;
        self.feeds = repo.list_feeds()?;
        if self.feeds.is_empty() {
            self.loading_message = Some("Loading feeds...".to_string());
            drop(repo);
            let added = {
                let fm = self.feed_manager.lock().await;
                fm.load_feeds_from_config(&self.config).await?
            };
            if !added.is_empty() {
                self.set_status(&format!("+{} feeds", added.len()));
            }
            let repo = self.repo.lock().await;
            self.feeds = repo.list_feeds()?;
        }
        self.loading_message = None;
        Ok(())
    }

    async fn load_embedding_index(&mut self) {
        let repo = self.repo.lock().await;
        if let Err(e) = self.embedding_index.load(&repo) {
            log::warn!("embedding index: {}", e);
        }
    }

    fn set_status(&mut self, msg: &str) {
        self.status_message = Some(msg.to_string());
        self.status_time = Instant::now();
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
        loop {
            self.spinner.update();
            self.pulse.update();

            // process streaming tokens
            self.drain_stream();

            if self.save_answer_needed {
                self.save_answer_needed = false;
                let answer = self.current_block.clone();
                // Save only the AI response part (after "│ ")
                let ai_text = answer
                    .lines().filter(|l| !l.starts_with("⟩ ")).collect::<Vec<_>>().join("\n");
                let sid = self.session_id;
                let repo = self.repo.lock().await;
                let _ = repo.add_message(sid, "assistant", &ai_text);
                drop(repo);
                // Move completed block to history
                if !self.history.is_empty() {
                    self.history = format!("\n\n{}", self.history);
                }
                self.history = format!("{}{}", self.current_block, self.history);
                self.current_block.clear();
            }

            terminal.draw(|f| {
                self.narrow_mode = f.area().width < 90;
                self.render_ui(f)
            })?;

            if self.status_message.is_some() && self.status_time.elapsed() > Duration::from_secs(5) {
                self.status_message = None;
            }

            if !cr_event::poll(Duration::from_millis(50))? { continue; }

            let ev = cr_event::read()?;
            if let Event::Key(key) = ev {
                if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat { continue; }
                self.handle_key_event(key).await?;
                if self.should_quit { break; }
            }
        }
        Ok(())
    }

    fn drain_stream(&mut self) {
        if let Some(rx) = &mut self.streaming_rx {
            while let Ok(tok) = rx.try_recv() {
                if tok == "__DONE__" {
                    self.is_streaming = false;
                    self.streaming_rx = None;
                    self.save_answer_needed = true;
                    break;
                }
                self.current_block.push_str(&tok);
            }
        }
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            AppMode::Search => {
                if self.search_input.input(key) && key.code == KeyCode::Enter && !self.search_input.is_empty() {
                    let t = self.search_input.lines().join(" ");
                    self.search_input = TextArea::default();
                    self.execute_search(&t).await?;
                }
                return Ok(());
            }
            AppMode::Ask => {
                if self.ask_input.input(key) {
                    if key.code == KeyCode::Enter && !self.ask_input.is_empty() {
                        let t = self.ask_input.lines().join(" ");
                        self.ask_input = TextArea::default();
                        self.ask_input.set_placeholder_text("");
                        self.answer_scroll = 0;
                        self.handle_ask_submit(&t).await?;
                    }
                    return Ok(());
                }
                match key.code {
                    KeyCode::Down if key.kind != KeyEventKind::Release => {
                        self.answer_scroll = self.answer_scroll.saturating_add(1);
                    }
                    KeyCode::Up if key.kind != KeyEventKind::Release => {
                        self.answer_scroll = self.answer_scroll.saturating_sub(1);
                    }
                    _ => {}
                }
                return Ok(());
            }
            AppMode::Digest | AppMode::Help | AppMode::Highlight | AppMode::Tag => return Ok(()),
            AppMode::ModelSelect => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down if key.kind == KeyEventKind::Press => {
                        if self.model_list_selected + 1 < self.available_models.len() {
                            self.model_list_selected += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up if key.kind == KeyEventKind::Press => {
                        if self.model_list_selected > 0 {
                            self.model_list_selected -= 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(m) = self.available_models.get(self.model_list_selected) {
                            self.config.ollama.model = m.clone();
                            let _ = self.config.save();
                            self.set_status(&format!("Model: {}", m));
                        }
                        self.mode = self.prev_mode.clone();
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.mode = self.prev_mode.clone();
                    }
                    _ => {}
                }
                return Ok(());
            }
            _ => {}
        }
        if !matches!(self.mode, AppMode::Ask) {
            if self.ask_input.input(key) {
                if key.code == KeyCode::Enter {
                    if !self.ask_input.is_empty() {
                        let t = self.ask_input.lines().join(" ");
                        self.ask_input = TextArea::default();
                        self.ask_input.set_placeholder_text("");
                        self.handle_ask_submit(&t).await?;
                        return Ok(());
                    }
                    // Empty Enter — let action mapping open feed/article
                } else {
                    return Ok(()); // Textarea absorbed non-Enter key
                }
            }
        }
        match key.code {
            KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => { self.should_quit = true; }
            KeyCode::Char('j') | KeyCode::Down if key.kind == KeyEventKind::Press => {
                if self.mode == AppMode::Feeds && self.selected_feed + 1 < self.feeds.len() { self.selected_feed += 1; }
                else if self.selected_entry + 1 < self.entries.len() { self.selected_entry += 1; }
            }
            KeyCode::Char('k') | KeyCode::Up if key.kind == KeyEventKind::Press => {
                if self.mode == AppMode::Feeds && self.selected_feed > 0 { self.selected_feed -= 1; }
                else if self.selected_entry > 0 { self.selected_entry -= 1; }
            }
            KeyCode::Down if key.kind == KeyEventKind::Repeat => { self.scroll_offset += 1; }
            KeyCode::Up if key.kind == KeyEventKind::Repeat => { self.scroll_offset = self.scroll_offset.saturating_sub(1); }
            KeyCode::Enter => {
                if self.mode == AppMode::Feeds { self.open_feed().await?; }
                else if self.mode == AppMode::Articles { self.open_article().await?; }
                else if self.mode == AppMode::Search && !self.search_results.is_empty() {
                    let e = self.search_results[self.search_selected].entry.clone();
                    self.entries = vec![e.clone()];
                    self.selected_entry = 0;
                    self.scroll_offset = 0;
                    self.mode = AppMode::Reading;
                }
            }
            KeyCode::Right if self.mode == AppMode::Feeds => { self.open_feed().await?; }
            _ => {}
        }
        Ok(())
    }

    fn render_ui(&mut self, f: &mut ratatui::Frame) {
        let area = f.area();
        let (main_area, ask_area, status_area) = layout::app_layout(area);
        let showing_answer = self.mode == AppMode::Ask || self.is_streaming || !self.current_block.is_empty() || !self.history.is_empty();
        let showing_summary = self.summary_text.is_some();

        let showing_modal = self.mode == AppMode::ModelSelect;

        if !showing_answer && !showing_summary && self.mode != AppMode::Search {
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([ratatui::layout::Constraint::Percentage(25), ratatui::layout::Constraint::Percentage(75)])
                .split(main_area);
            f.render_widget(widgets::FeedList { feeds: &self.feeds, selected: self.selected_feed }, chunks[0]);
            self.render_content(f, chunks[1]);
        } else if showing_answer {
            // Single scrollable view: current block (newest) + history
            let view = format!("{}{}", self.current_block, self.history);
            let mut text = ratatui::text::Text::default();
            for line in view.lines() {
                if line.starts_with("⟩ ") {
                    text.lines.push(Line::from(Span::styled(line, ratatui::style::Style::new().dim().cyan())));
                } else {
                    text.lines.push(Line::from(Span::raw(line)));
                }
            }
            let p = Paragraph::new(text).wrap(Wrap { trim: false }).scroll((self.answer_scroll, 0));
            f.render_widget(p, main_area);
        } else if showing_summary {
            if let Some(s) = &self.summary_text {
                f.render_widget(Paragraph::new(s.as_str()).wrap(Wrap { trim: false }).scroll((0, 0)), main_area);
            }
        } else {
            self.render_content(f, main_area);
        }

        f.render_widget(widgets::AskBar { input: &self.ask_input, spinner: self.spinner.current(), is_streaming: self.is_streaming }, ask_area);
        f.render_widget(widgets::MiniBar { text: "" }, status_area);

        if showing_modal {
            let popup_area = layout::centered_rect(f.area(), 60, 70);
            f.render_widget(widgets::ModelList {
                models: &self.available_models,
                selected: self.model_list_selected,
                current: &self.config.ollama.model,
            }, popup_area);
        }
    }

    fn render_content(&self, f: &mut ratatui::Frame, area: Rect) {
        match self.mode {
            AppMode::Feeds if !self.entries.is_empty() => {
                f.render_widget(widgets::ArticleList { entries: &self.entries, selected: self.selected_entry }, area);
            }
            AppMode::Feeds => {
                let hint = Paragraph::new("  No articles — type /feed to refresh, or ask a question below").dim();
                f.render_widget(hint, area);
            }
            AppMode::Articles if !self.entries.is_empty() => {
                f.render_widget(widgets::ArticleList { entries: &self.entries, selected: self.selected_entry }, area);
            }
            AppMode::Reading => {
                let e = self.entries.get(self.selected_entry);
                f.render_widget(widgets::ArticleView {
                    title: e.and_then(|e| e.title.as_deref()),
                    content: e.and_then(|e| e.content.as_deref().or(e.summary.as_deref())),
                    scroll: self.scroll_offset,
                }, area);
            }
            AppMode::Search => {
                let t = if self.search_results.is_empty() { "Type query, Enter to search".to_string() }
                else { self.search_results.iter().enumerate().map(|(i,r)| format!("{}{}", if i==self.search_selected{"▸ "}else{"  "}, r.entry.title.as_deref().unwrap_or(""))).collect::<Vec<_>>().join("\n") };
                f.render_widget(Paragraph::new(t).wrap(Wrap { trim: false }), area);
            }
            AppMode::Digest => {
                if let Some(d) = &self.digest_text { f.render_widget(Paragraph::new(d.as_str()).wrap(Wrap { trim: false }), area); }
            }
            AppMode::Help => { f.render_widget(widgets::HelpOverlay, area); }
            AppMode::Tag => {
                let t = if self.current_tags.is_empty() { "No tags".to_string() } else { format!("Tags: {}", self.current_tags.join(", ")) };
                f.render_widget(Paragraph::new(t), area);
            }
            AppMode::Highlight => {
                let lines: Vec<String> = self.highlights.iter().map(|h| format!("- {}", h.text)).collect();
                let text = if lines.is_empty() { "No highlights".to_string() } else { lines.join("\n") };
                f.render_widget(Paragraph::new(text).wrap(Wrap{trim:false}), area);
            }
            _ => {}
        }
    }

    async fn open_feed(&mut self) -> Result<()> {
        if self.feeds.is_empty() { return Ok(()); }
        let fid = self.feeds[self.selected_feed].id;
        let repo = self.repo.lock().await;
        self.entries = repo.list_entries(fid)?;
        self.selected_entry = 0;
        self.mode = AppMode::Articles;
        Ok(())
    }

    async fn open_article(&mut self) -> Result<()> {
        if self.entries.is_empty() || self.selected_entry >= self.entries.len() { return Ok(()); }
        let eid = self.entries[self.selected_entry].id;
        let repo = self.repo.lock().await;
        let was_read = repo.list_entries(self.feeds[self.selected_feed].id).ok()
            .and_then(|es| es.get(self.selected_entry).map(|e| e.is_read)).unwrap_or(true);
        if !was_read { repo.mark_read(eid)?; }
        drop(repo);
        if let Some(e) = self.entries.get_mut(self.selected_entry) { e.is_read = true; }
        self.scroll_offset = 0;
        self.mode = AppMode::Reading;
        Ok(())
    }

    fn go_back(&mut self) {
        match self.mode {
            AppMode::Articles | AppMode::Reading | AppMode::Tag | AppMode::Highlight => {
                self.summary_text = None;
                self.mode = AppMode::Feeds;
            }
            AppMode::Search | AppMode::Ask | AppMode::Digest => {
                self.mode = AppMode::Reading;
            }
            AppMode::Help | AppMode::ModelSelect => self.mode = self.prev_mode.clone(),
            _ => {}
        }
    }

    // ── refresh ──

    async fn refresh_all(&mut self) -> Result<()> {
        self.loading_message = Some("Fetching...".to_string());
        let (count, errors) = {
            let fm = self.feed_manager.lock().await;
            fm.refresh_all(&self.config).await?
        };
        self.loading_message = None;
        self.load_feeds_from_db().await?;
        {
            let repo = self.repo.lock().await;
            self.all_entries = repo.list_all_entries()?;
        }
        if !errors.is_empty() {
            self.set_status(&format!("{} new, {} errors", count, errors.len()));
        } else {
            self.set_status(&format!("{} articles fetched", count));
        }
        Ok(())
    }

    // ── summarise ──

    async fn summarize_current(&mut self) -> Result<()> {
        if let Some(e) = self.entries.get(self.selected_entry).cloned() {
            let content = e.content.as_deref().or(e.summary.as_deref()).unwrap_or("");
            if content.is_empty() { self.set_status("Nothing to summarize"); return Ok(()); }
            self.loading_message = Some("Summarizing...".to_string());
            let r = summarizer::summarize(&self.ai, &self.config.ollama.model, content).await;
            self.loading_message = None;
            match r {
                Ok(s) => { self.summary_text = Some(s); self.set_status("Esc to close"); }
                Err(e) => self.set_status(&format!("Error: {}", e)),
            }
        }
        Ok(())
    }

    // ── ask with tool calling + streaming ──

    async fn execute_ask(&mut self, question: &str) -> Result<()> {
        self.mode = AppMode::Ask;
        self.answer_scroll = 0;
        // Build new block: question on top, answer marker below
        self.current_block = format!("⟩ {}\n\n│ ", question);

        // Save user message
        {
            let repo = self.repo.lock().await;
            repo.add_message(self.session_id, "user", question)?;
        }

        let (tx, rx) = mpsc::unbounded_channel();
        self.streaming_rx = Some(rx);
        self.is_streaming = true;

        let client = self.ai.clone();
        let model = self.config.ollama.model.clone();
        let all_entries = self.all_entries.clone();
        let q = question.to_string();
        let fm = self.feed_manager.clone();
        let repo = self.repo.clone();
        let sid = self.session_id;

        tokio::spawn(async move {
            let system = "You are Nuzzle, a personal AI news assistant in a terminal RSS reader.\n\
                You have tools to search articles (search_news) and add feeds (add_feed).\n\
                Call search_news to find articles. If none found, suggest RSS feed URLs.\n\
                If the user asks to add a feed, call add_feed.\n\
                Be brief (2-4 sentences). Cite article titles.";

            // Build full message history including system prompt
            let hist = {
                let r = repo.clone();
                let r = r.lock().await;
                r.session_messages(sid, 20).unwrap_or_default()
            };
            // Format history as text for streaming prompt
            let history_text = hist.iter()
                .map(|m| format!("[{}]: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n");

            let mut msgs: Vec<ChatMessage> = vec![
                ChatMessage { role: "system".to_string(), content: system.to_string(), tool_calls: None },
            ];
            for m in hist {
                msgs.push(ChatMessage { role: m.role, content: m.content, tool_calls: None });
            }
            msgs.push(ChatMessage { role: "user".to_string(), content: q.clone(), tool_calls: None });

            let tr = client.chat_with_tools(&model, msgs, &tools::all_tools()).await;

            let final_answer = match tr {
                Ok(resp) => {
                    let mut tool_results = String::new();
                    let mut last_search: Vec<crate::types::Entry> = vec![];
                    if let Some(tcs) = &resp.message.tool_calls {
                        for tc in tcs {
                            if tc.function.name == "search_news" {
                                let query = tc.function.arguments["query"].as_str().unwrap_or("");
                                let max = tc.function.arguments["max_results"].as_u64().unwrap_or(5) as usize;
                                let found = tools::execute_search_news(&all_entries, query, max);
                                tool_results.push_str(&tools::format_search_results(&found));
                                tool_results.push_str("\n\n");
                                last_search = found;
                            } else if tc.function.name == "read_article" {
                                let idx = tc.function.arguments["index"].as_u64().unwrap_or(0) as usize;
                                let title = tc.function.arguments["title"].as_str().unwrap_or("");
                                let content = tools::execute_read_article(&last_search, idx, title);
                                tool_results.push_str(&format!("ARTICLE CONTENT:\n{}\n\n", content));
                            } else if tc.function.name == "add_feed" {
                                let url = tc.function.arguments["url"].as_str().unwrap_or("");
                                let fm = fm.lock().await;
                                match fm.add_feed_url(url).await {
                                    Ok(t) => tool_results.push_str(&format!("+ {}\n", t)),
                                    Err(e) => tool_results.push_str(&format!("Error: {}\n", e)),
                                }
                            }
                        }
                    }
                    let prompt = format!(
                        "Conversation so far:\n{}\n\nUser asked: \"{}\"\n\nTool results:\n{}\n\nAnswer the user's latest message concisely, using the conversation above for context.",
                        history_text, q, tool_results
                    );
                    // Stream directly to the UI channel
                    let _ = client.generate_stream(&model, &system, &prompt, tx).await;
                }
                Err(e) => {
                    tx.send(format!("Error: {}\n", e)).ok();
                    tx.send("__DONE__".to_string()).ok();
                }
            };
        });

        Ok(())
    }

    async fn handle_ask_submit(&mut self, text: &str) -> Result<()> {
        let t = text.trim();
        if t.starts_with('/') {
            self.handle_slash_command(t).await?;
            return Ok(());
        }
        self.execute_ask(text).await
    }

    async fn handle_slash_command(&mut self, cmd: &str) -> Result<()> {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        match parts[0] {
            "/exit" => { self.should_quit = true; }
            "/feed" => {
                self.mode = AppMode::Feeds;
                self.current_block.clear();
                self.history.clear();
                self.answer_scroll = 0;
                self.load_feeds_from_db().await?;
                {
                    let repo = self.repo.lock().await;
                    self.all_entries = repo.list_all_entries()?;
                }
                // Also load first feed's entries
                if !self.feeds.is_empty() {
                    let feed_id = self.feeds[self.selected_feed].id;
                    let repo = self.repo.lock().await;
                    self.entries = repo.list_entries(feed_id)?;
                }
            }
            "/new" => {
                let name = format!("session-{}", Utc::now().format("%H%M%S"));
                let repo = self.repo.lock().await;
                self.session_id = repo.create_session(&name, &self.config.ollama.model)?;
                drop(repo);
                self.session_name = name;
                self.current_block = "New session.".to_string();
                self.mode = AppMode::Ask;
            }
            "/session" => {
                if parts.len() > 1 {
                    let n = parts[1].trim();
                    let repo = self.repo.lock().await;
                    self.session_id = repo.create_session(n, &self.config.ollama.model)?;
                    self.session_name = n.to_string();
                    drop(repo);
                    self.current_block = format!("Session \"{}\".", n);
                } else {
                    let repo = self.repo.lock().await;
                    let s = repo.list_sessions()?;
                    drop(repo);
                    self.current_block = s.iter().map(|s| format!("  {}", s.name)).collect::<Vec<_>>().join("\n");
                    if self.current_block.is_empty() { self.current_block = "No sessions.".to_string(); }
                }
                self.mode = AppMode::Ask;
            }
            "/models" => {
                let m = self.ai.list_models().await.unwrap_or_default();
                if m.is_empty() {
                    self.current_block = "No models.".to_string();
                    self.mode = AppMode::Ask;
                } else {
                    self.available_models = m;
                    self.model_list_selected = self.available_models.iter()
                        .position(|n| n == &self.config.ollama.model)
                        .unwrap_or(0);
                    self.prev_mode = self.mode.clone();
                    self.mode = AppMode::ModelSelect;
                }
            }
            "/model" => {
                if parts.len() > 1 {
                    let n = parts[1].trim();
                    self.config.ollama.model = n.to_string();
                    self.config.save()?;
                    self.current_block = format!("Model: {}", n);
                } else {
                    self.current_block = format!("Current: {}", self.config.ollama.model);
                }
                self.mode = AppMode::Ask;
            }
            _ => {
                self.current_block = "/exit /feed /new /session /models /model <name>".to_string();
                self.mode = AppMode::Ask;
            }
        }
        Ok(())
    }

    // ── search ──

    fn enter_search_mode(&mut self) {
        self.search_results.clear();
        self.search_selected = 0;
        self.mode = AppMode::Search;
    }

    async fn execute_search(&mut self, query_text: &str) -> Result<()> {
        if query_text.is_empty() { return Ok(()); }
        if self.embedding_index.len() == 0 {
            self.set_status("No embeddings yet; fetch articles first");
            return Ok(());
        }
        self.loading_message = Some("Searching...".to_string());
        let feed_titles: std::collections::HashMap<i64, String> =
            self.feeds.iter().map(|f| (f.id, f.title.clone())).collect();
        if let Ok(results) = query::semantic_search(
            &self.ai, &self.config.ollama.embed_model, &self.embedding_index,
            query_text, 20, &feed_titles,
        ).await {
            self.search_results = results;
            self.search_selected = 0;
        }
        self.loading_message = None;
        self.set_status(&format!("{} results", self.search_results.len()));
        Ok(())
    }

    // ── tags ──

    async fn enter_tag_mode(&mut self) -> Result<()> {
        if let Some(e) = self.entries.get(self.selected_entry) {
            let repo = self.repo.lock().await;
            self.current_tags = repo.entry_tags(e.id)?.into_iter().map(|t| t.name).collect();
        } else { self.current_tags.clear(); }
        self.mode = AppMode::Tag;
        Ok(())
    }

    // ── highlights ──

    async fn show_highlights(&mut self) -> Result<()> {
        let repo = self.repo.lock().await;
        self.highlights = repo.list_highlights()?;
        self.mode = AppMode::Highlight;
        Ok(())
    }

    // ── digest ──

    async fn show_digest(&mut self) -> Result<()> {
        let repo = self.repo.lock().await;
        let entries = repo.list_all_entries()?;
        drop(repo);
        let recent: Vec<Entry> = entries.into_iter()
            .filter(|e| e.published_at.map(|d| (Utc::now() - d).num_hours() < 48).unwrap_or(false))
            .take(20).collect();
        if recent.is_empty() { self.set_status("No recent articles"); return Ok(()); }
        self.loading_message = Some("Digesting...".to_string());
        let r = digest::generate_digest(&self.ai, &self.config.ollama.model, &recent).await;
        self.loading_message = None;
        match r {
            Ok(t) => { self.digest_text = Some(t); self.mode = AppMode::Digest; }
            Err(e) => self.set_status(&format!("Error: {}", e)),
        }
        Ok(())
    }

    // ── star & export ──

    async fn toggle_star(&mut self) -> Result<()> {
        if let Some(e) = self.entries.get(self.selected_entry).cloned() {
            let repo = self.repo.lock().await;
            let new = repo.toggle_star(e.id)?;
            drop(repo);
            if let Some(me) = self.entries.get_mut(self.selected_entry) { me.is_starred = new; }
            self.set_status(if new { "Starred" } else { "Unstarred" });
        }
        Ok(())
    }

    async fn export_highlights(&mut self) -> Result<()> {
        let repo = self.repo.lock().await;
        let md = highlight::export_highlights(&repo)?;
        drop(repo);
        let name = format!("nuzzle-export-{}.md", Utc::now().format("%Y%m%d-%H%M%S"));
        std::fs::write(&name, &md)?;
        self.set_status(&format!("Exported → {}", name));
        Ok(())
    }
}
