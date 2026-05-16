use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;
use crossterm::event::{self as cr_event, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::text::{Line, Span};
use ratatui::Terminal;
use tokio::sync::{mpsc, Mutex};
use tui_textarea::TextArea;

use crate::ai::client::{ChatMessage, OllamaClient};
use crate::ai::{digest, summarizer};
use crate::config::Config;
use crate::db::repository::Repository;
use crate::feed::manager::FeedManager;
use crate::highlight;
use crate::markdown;
use crate::search::index::EmbeddingIndex;
use crate::search::query;
use crate::tools::registry::ToolRegistry;
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
    session_type: SessionType,
    session_described: bool,
    session_list: Vec<AISession>,
    session_list_selected: usize,

    // tool system
    tool_registry: ToolRegistry,
    workspace_dir: PathBuf,

    // tool activity feedback
    tool_activity: Option<String>,
    tool_activity_rx: Option<mpsc::UnboundedReceiver<String>>,
    tool_activity_at: Instant,

    // conversation — blocks[0] = current/latest
    blocks: Vec<String>,
    block_idx: usize,        // 0 = latest, 1 = prev, ...
    app_ready: bool,
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

        let data_dir = PathBuf::from(
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
        ).join(".local/share/nuzzle");

        let mut tool_registry = ToolRegistry::new();
        let scripts_dir = config.tools.scripts_dir.clone();
        tool_registry.load(&scripts_dir);

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
            session_type: SessionType::Chat,
            session_described: false,
            session_list: vec![],
            session_list_selected: 0,
            tool_registry,
            workspace_dir: data_dir.join("code"),
            tool_activity: None,
            tool_activity_rx: None,
            tool_activity_at: Instant::now(),
            blocks: vec![],
            block_idx: 0,
            app_ready: false,
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
            self.session_id = repo.create_session_typed("default", &self.config.ollama.model, &SessionType::Chat)?;
            self.session_name = "default".to_string();
            self.session_type = SessionType::Chat;
        }
        self.install_builtin_tools();
        self.load_embedding_index().await;
        self.set_status(&format!("{} articles loaded", count));
        self.app_ready = true;
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

    fn install_builtin_tools(&mut self) {
        let scripts_dir = &self.config.tools.scripts_dir;
        let _ = std::fs::create_dir_all(scripts_dir);

        // Copy builtin scripts if they don't exist
        let builtin_base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/builtin");
        if !builtin_base.exists() { return; }

        if let Ok(entries) = std::fs::read_dir(&builtin_base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "sh") {
                    let dest = scripts_dir.join(path.file_name().unwrap());
                    if !dest.exists() {
                        let _ = std::fs::copy(&path, &dest);
                        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
                    }
                }
            }
        }
        // Reload registry with newly installed tools
        self.tool_registry.load(scripts_dir);
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

            // process tool status updates
            self.drain_tool_status();

            if self.save_answer_needed {
                self.save_answer_needed = false;
                // Save AI response to DB
                if let Some(block) = self.blocks.first() {
                    let ai_text = block.lines().filter(|l| !l.starts_with("⟩ ")).collect::<Vec<_>>().join("\n");
                    let sid = self.session_id;
                    let repo = self.repo.lock().await;
                    let _ = repo.add_message(sid, "assistant", &ai_text);

                    // Auto-generate session description after first exchange
                    if !self.session_described {
                        if let Ok(msgs) = repo.session_messages(sid, 6) {
                            drop(repo);
                            let chat_msgs: Vec<ChatMessage> = msgs.iter()
                                .map(|m| ChatMessage { role: m.role.clone(), content: m.content.clone(), tool_calls: None })
                                .collect();
                            if chat_msgs.len() >= 2 {
                                let ai_clone = self.ai.clone();
                                let model_clone = self.config.ollama.model.clone();
                                let repo2 = self.repo.clone();
                                tokio::spawn(async move {
                                    let desc = ai_clone.generate_description(&model_clone, &chat_msgs).await;
                                    if !desc.is_empty() {
                                        let r = repo2.lock().await;
                                        let _ = r.update_session_description(sid, &desc);
                                        drop(r);
                                    }
                                });
                                self.session_described = true;
                            }
                        } else {
                            drop(repo);
                        }
                    } else {
                        drop(repo);
                    }
                }
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
                if let Some(b) = self.blocks.first_mut() {
                    b.push_str(&tok);
                }
            }
        }
    }

    fn drain_tool_status(&mut self) {
        if let Some(rx) = &mut self.tool_activity_rx {
            while let Ok(msg) = rx.try_recv() {
                if msg == "done" || msg.starts_with("done:") {
                    if msg.starts_with("done:") {
                        self.tool_activity = Some(msg[5..].trim().to_string());
                        self.tool_activity_at = Instant::now();
                    }
                    self.tool_activity_rx = None;
                    break;
                }
                self.tool_activity = Some(msg);
                self.tool_activity_at = Instant::now();
            }
        }
        // Auto-clear after 8 seconds of no updates
        if self.tool_activity.is_some() && self.tool_activity_at.elapsed() > Duration::from_secs(8) {
            self.tool_activity = None;
        }
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            AppMode::Search => {
                if self.search_input.input(key) && key.code == KeyCode::Enter {
                    let t = self.search_input.lines().join(" ").trim().to_string();
                    self.search_input = TextArea::default();
                    if !t.is_empty() { self.execute_search(&t).await?; }
                }
                return Ok(());
            }
            AppMode::Ask => {
                if self.ask_input.input(key) {
                    if key.code == KeyCode::Enter {
                        let t = self.ask_input.lines().join(" ").trim().to_string();
                        self.ask_input = TextArea::default();
                        if !t.is_empty() {
                            self.handle_ask_submit(&t).await?;
                            return Ok(());
                        }
                    } else {
                        return Ok(());
                    }
                }
                match key.code {
                    KeyCode::Down if key.kind == KeyEventKind::Press => {
                        if self.block_idx + 1 < self.blocks.len() {
                            self.block_idx += 1;
                        }
                    }
                    KeyCode::Up if key.kind == KeyEventKind::Press => {
                        self.block_idx = self.block_idx.saturating_sub(1);
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
            AppMode::SessionSelect => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down if key.kind == KeyEventKind::Press => {
                        if self.session_list_selected + 1 < self.session_list.len() {
                            self.session_list_selected += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up if key.kind == KeyEventKind::Press => {
                        if self.session_list_selected > 0 {
                            self.session_list_selected -= 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(s) = self.session_list.get(self.session_list_selected).cloned() {
                            self.session_id = s.id;
                            self.session_name = s.name.clone();
                            self.session_type = s.session_type.clone();
                            self.session_described = true;
                            self.blocks.clear();
                            self.block_idx = 0;
                            self.set_status(&format!("Session: {} [{}]", s.name, s.session_type.as_str()));
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
                    let t = self.ask_input.lines().join(" ").trim().to_string();
                    self.ask_input = TextArea::default();
                    if !t.is_empty() {
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

        // Loading screen while fetching
        if let Some(msg) = &self.loading_message.clone() {
            f.render_widget(widgets::LoadingScreen { spinner: self.spinner.current(), message: msg }, area);
            return;
        }

        let (header_area, content_area, ask_area, nav_area) = layout::app_layout(area);
        let showing_answer = self.mode == AppMode::Ask || self.is_streaming || !self.blocks.is_empty();
        let narrow = area.width < 90;

        // ── Header ──
        let (hl, hr) = if showing_answer {
            let tool = self.tool_activity.as_deref().unwrap_or("");
            let left = format!(" Q&A  ·  {}/{}", self.block_idx + 1, self.blocks.len());
            if !tool.is_empty() {
                (left, tool.to_string())
            } else {
                (left, String::new())
            }
        } else if self.mode == AppMode::Search {
            (" Search  ·  Enter to run".to_string(), String::new())
        } else {
            let feed_name = self.feeds.get(self.selected_feed).map(|f| f.title.as_str()).unwrap_or("");
            let count = if self.mode == AppMode::Feeds || self.mode == AppMode::Articles {
                format!("{} articles", self.entries.len())
            } else { String::new() };
            (format!(" {}", feed_name), count)
        };
        f.render_widget(widgets::Header { left: &hl, right: &hr }, header_area);

        // ── Content ──
        if showing_answer {
            let visible = self.blocks.get(self.block_idx).map(|s| s.as_str()).unwrap_or("");
            let mut text = ratatui::text::Text::default();
            if let Some((question, answer)) = visible.split_once('\n') {
                text.lines.push(Line::from(Span::styled(question, ratatui::style::Style::new().fg(ratatui::style::Color::Rgb(180, 190, 218)))));
                text.extend(markdown::render(answer));
            } else {
                text = markdown::render(visible);
            }
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), content_area);
        } else if self.summary_text.is_some() {
            if let Some(s) = &self.summary_text {
                f.render_widget(Paragraph::new(s.as_str()).wrap(Wrap { trim: false }), content_area);
            }
        } else {
            self.render_content(f, content_area, narrow);
        }

        // ── Ask bar ──
        f.render_widget(widgets::AskBar { input: &self.ask_input, spinner: self.spinner.current(), is_streaming: self.is_streaming }, ask_area);

        // ── Nav ──
        let nav = if showing_answer { format!("  {}/{}  ↑↓ browse", self.block_idx + 1, self.blocks.len()) } else { String::new() };
        f.render_widget(widgets::NavBar { text: &nav }, nav_area);

        // ── Model popup ──
        if self.mode == AppMode::ModelSelect {
            let popup = layout::centered_rect(f.area(), 60, 70);
            f.render_widget(widgets::ModelList { models: &self.available_models, selected: self.model_list_selected, current: &self.config.ollama.model }, popup);
        }

        // ── Session popup ──
        if self.mode == AppMode::SessionSelect {
            let popup = layout::centered_rect(f.area(), 70, 75);
            f.render_widget(widgets::SessionList { sessions: &self.session_list, selected: self.session_list_selected, current_name: &self.session_name }, popup);
        }
    }

    fn render_content(&self, f: &mut ratatui::Frame, area: Rect, narrow: bool) {
        match self.mode {
            AppMode::Feeds | AppMode::Articles if narrow => {
                f.render_widget(widgets::ArticleList { entries: &self.entries, selected: self.selected_entry }, area);
            }
            AppMode::Feeds | AppMode::Articles => {
                let chunks = ratatui::layout::Layout::horizontal([ratatui::layout::Constraint::Percentage(25), ratatui::layout::Constraint::Percentage(75)]).split(area);
                f.render_widget(widgets::FeedList { feeds: &self.feeds, selected: self.selected_feed }, chunks[0]);
                f.render_widget(widgets::ArticleList { entries: &self.entries, selected: self.selected_entry }, chunks[1]);
            }
            AppMode::Reading => {
                let e = self.entries.get(self.selected_entry);
                f.render_widget(widgets::ArticleView { title: e.and_then(|e| e.title.as_deref()), content: e.and_then(|e| e.content.as_deref().or(e.summary.as_deref())), scroll: self.scroll_offset }, area);
            }
            AppMode::Search => {
                let t = if self.search_results.is_empty() { "  Type query, Enter to search".to_string() }
                else { self.search_results.iter().enumerate().map(|(i,r)| format!("  {}{}", if i==self.search_selected{"▸ "}else{"  "}, r.entry.title.as_deref().unwrap_or(""))).collect::<Vec<_>>().join("\n") };
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
            AppMode::Help | AppMode::ModelSelect | AppMode::SessionSelect => self.mode = self.prev_mode.clone(),
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

    // ── ask with text-based tool protocol + streaming ──

    async fn execute_ask(&mut self, question: &str) -> Result<()> {
        self.mode = AppMode::Ask;
        // Start new block at front
        self.blocks.insert(0, format!("⟩ {}\n\n", question));
        self.block_idx = 0;

        // Save user message
        {
            let repo = self.repo.lock().await;
            repo.add_message(self.session_id, "user", question)?;
        }

        let (tx, rx) = mpsc::unbounded_channel();
        self.streaming_rx = Some(rx);
        self.is_streaming = true;

        // Status channel for tool activity feedback
        let (status_tx, status_rx) = mpsc::unbounded_channel();
        self.tool_activity_rx = Some(status_rx);
        self.tool_activity = None;

        let client = self.ai.clone();
        let model = self.config.ollama.model.clone();
        let q = question.to_string();
        let sid = self.session_id;
        let stype = self.session_type.clone();
        let stype_str = stype.as_str().to_string();

        // Build tools description for this session type
        let tools_prompt = self.tool_registry.build_system_prompt(&stype_str);

        // Load session files for code/search sessions
        let session_files_prompt = self.build_session_files_prompt().await;

        let system_prompt = self.build_system_prompt(&tools_prompt, &session_files_prompt);

        // Build message history for the AI
        let hist = {
            let r = self.repo.clone();
            let r = r.lock().await;
            r.session_messages(sid, 20).unwrap_or_default()
        };
        let chat_history: Vec<ChatMessage> = hist.iter()
            .map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                tool_calls: None,
            })
            .collect();

        // Save whether this session needs description
        let needs_description = !self.session_described;

        let tool_reg = Arc::new(Mutex::new(self.tool_registry.clone()));

        // Build environment variables for tool scripts
        let mut env_vars = HashMap::new();
        env_vars.insert("NUZZLE_DB".to_string(), self.config.general.db_path.display().to_string());
        env_vars.insert("NUZZLE_WORKSPACE".to_string(), self.workspace_dir.display().to_string());

        // Spawn async task for tool-augmented chat
        tokio::spawn(async move {
            let resp = client.chat_with_text_tools(
                &model,
                &system_prompt,
                &q,
                &chat_history,
                tool_reg,
                &stype_str,
                env_vars,
                Some(status_tx),
            ).await;

            match resp {
                Ok(text) => {
                    // Stream the final response
                    let _ = client.generate_stream(&model, &system_prompt, &text, tx).await;
                }
                Err(e) => {
                    tx.send(format!("Error: {}\n", e)).ok();
                    tx.send("__DONE__".to_string()).ok();
                }
            }
        });

        // Store context for post-stream processing
        self.session_described = needs_description;

        Ok(())
    }

    fn build_system_prompt(&self, tools_prompt: &str, files_prompt: &str) -> String {
        let mut s = String::from("You are Nuzzle, a capable AI assistant in a terminal application.\n\
            You are helpful, direct, and professional. You speak concisely and clearly.\n\
            You support markdown formatting in your responses.\n\n");

        s.push_str("SESSION TYPE: ");
        s.push_str(self.session_type.as_str());
        s.push_str("\n\n");

        if !files_prompt.is_empty() {
            s.push_str("AVAILABLE REFERENCE FILES:\n");
            s.push_str(files_prompt);
            s.push_str("\n");
        }

        if !tools_prompt.is_empty() {
            s.push_str(tools_prompt);
        }

        s.push_str("\nINSTRUCTIONS:\n");
        match self.session_type {
            SessionType::Search => {
                s.push_str("- Use web_search to find information online.\n");
                s.push_str("- Use fetch_page to read full articles.\n");
                s.push_str("- After research, write results to research.md using write_file.\n");
                s.push_str("- Provide thorough, well-structured markdown output.\n");
            }
            SessionType::Code => {
                s.push_str("- Use exec to run commands (build, test, lint, etc).\n");
                s.push_str("- Use read_file to understand existing code.\n");
                s.push_str("- Use write_file to create or modify files.\n");
                s.push_str("- Use list_files to explore the project structure.\n");
                s.push_str("- Read reference files for project context.\n");
                s.push_str("- Focus on practical, working code solutions.\n");
            }
            SessionType::Chat => {
                s.push_str("- Default: 2-4 sentences, direct and informative.\n");
                s.push_str("- For analysis or after deep_research: thorough, multi-paragraph.\n");
                s.push_str("- Use **bold** for key points, # headings for structure.\n");
            }
        }
        s
    }

    async fn build_session_files_prompt(&self) -> String {
        let repo = self.repo.lock().await;
        let files = match repo.list_session_files(self.session_id) {
            Ok(f) => f,
            Err(_) => return String::new(),
        };
        drop(repo);

        if files.is_empty() { return String::new(); }

        let mut s = String::new();
        for f in &files {
            s.push_str(&format!("- {} ({}) at {}\n", f.filename, f.file_type, f.filepath));
        }
        s
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
                self.blocks.clear();
                self.block_idx = 0;
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
                self.session_id = repo.create_session_typed(&name, &self.config.ollama.model, &SessionType::Chat)?;
                drop(repo);
                self.session_name = name;
                self.session_type = SessionType::Chat;
                self.session_described = false;
                self.blocks.insert(0, "New chat session.".to_string());
                self.block_idx = 0;
                self.mode = AppMode::Ask;
            }
            "/search" => {
                self.start_search_session(parts.get(1).map(|s| s.trim()).unwrap_or("")).await?;
            }
            "/code" => {
                self.start_code_session(parts.get(1).map(|s| s.trim()).unwrap_or("")).await?;
            }
            "/session" => {
                if parts.len() > 1 {
                    let n = parts[1].trim();
                    let repo = self.repo.lock().await;
                    // Try to find existing session by name
                    let sessions = repo.list_sessions()?;
                    let existing = sessions.iter().find(|s| s.name == n).cloned();
                    let (sid, stype) = if let Some(ref s) = existing {
                        (s.id, s.session_type.clone())
                    } else {
                        let id = repo.create_session_typed(n, &self.config.ollama.model, &SessionType::Chat)?;
                        (id, SessionType::Chat)
                    };
                    drop(repo);
                    self.session_id = sid;
                    self.session_name = n.to_string();
                    self.session_type = stype;
                    self.session_described = true;
                    self.blocks.insert(0, format!("Switched to session \"{}\" [{}].", n, self.session_type.as_str()));
                    self.block_idx = 0;
                    self.mode = AppMode::Ask;
                } else {
                    let repo = self.repo.lock().await;
                    self.session_list = repo.list_sessions()?;
                    drop(repo);
                    self.session_list_selected = self.session_list.iter()
                        .position(|s| s.id == self.session_id)
                        .unwrap_or(0);
                    self.prev_mode = self.mode.clone();
                    self.mode = AppMode::SessionSelect;
                }
            }
            "/models" => {
                let m = self.ai.list_models().await.unwrap_or_default();
                if m.is_empty() {
                    self.blocks.insert(0, "No models.".to_string());
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
                    self.blocks.insert(0, format!("Model: {}", n));
                } else {
                    self.blocks.insert(0, format!("Current: {}", self.config.ollama.model));
                }
                self.mode = AppMode::Ask;
            }
            _ => {
                self.blocks.insert(0, "/exit /feed /new /session /search /code /models /model <name>".to_string());
                self.mode = AppMode::Ask;
            }
        }
        Ok(())
    }

    async fn start_search_session(&mut self, query: &str) -> Result<()> {
        let name = format!("search-{}", Utc::now().format("%H%M%S"));
        let repo = self.repo.lock().await;
        let sid = repo.create_session_typed(&name, &self.config.ollama.model, &SessionType::Search)?;
        drop(repo);

        self.session_id = sid;
        self.session_name = name;
        self.session_type = SessionType::Search;
        self.session_described = false;

        let research_dir = PathBuf::from(
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
        ).join(".local/share/nuzzle/research").join(&self.session_name);
        std::fs::create_dir_all(&research_dir).unwrap_or_default();
        self.workspace_dir = research_dir.clone();

        if query.is_empty() {
            self.blocks.insert(0, "Search session created. Type your research query.".to_string());
        } else {
            self.blocks.insert(0, format!("Researching: {}\n", query));
            self.execute_ask(&format!("Deep research on: {}. Search the web for authoritative information, \
                read relevant pages, and compile a comprehensive research document. \
                Write your findings to research.md using write_file.", query)).await?;
        }
        self.block_idx = 0;
        self.mode = AppMode::Ask;
        Ok(())
    }

    async fn start_code_session(&mut self, name: &str) -> Result<()> {
        let session_name = if name.is_empty() {
            format!("code-{}", Utc::now().format("%H%M%S"))
        } else {
            name.to_string()
        };
        let repo = self.repo.lock().await;
        let sid = repo.create_session_typed(&session_name, &self.config.ollama.model, &SessionType::Code)?;
        drop(repo);

        self.session_id = sid;
        self.session_name = session_name.clone();
        self.session_type = SessionType::Code;
        self.session_described = false;

        let code_dir = PathBuf::from(
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
        ).join(".local/share/nuzzle/code").join(&session_name);
        std::fs::create_dir_all(&code_dir).unwrap_or_default();
        self.workspace_dir = code_dir.clone();

        // Check for research sessions that might be relevant
        let research_files = self.find_research_files().await;

        let mut msg = format!("Code session \"{}\" created.\nWorkspace: {}\n", session_name, code_dir.display());
        if !research_files.is_empty() {
            msg.push_str("\nRelevant research documents found:\n");
            for f in &research_files {
                msg.push_str(&format!("- {}\n", f));
                // Link research files to this session
                let repo = self.repo.lock().await;
                let _ = repo.add_session_file(sid, &f, "research_md", &f);
                drop(repo);
            }
            msg.push_str("\nUse read_file to load these as reference documentation.");
        }

        self.blocks.insert(0, msg);
        self.block_idx = 0;
        self.mode = AppMode::Ask;
        Ok(())
    }

    async fn find_research_files(&self) -> Vec<String> {
        let research_base = PathBuf::from(
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
        ).join(".local/share/nuzzle/research");

        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&research_base) {
            for entry in entries.flatten() {
                let md_path = entry.path().join("research.md");
                if md_path.exists() {
                    files.push(md_path.display().to_string());
                }
            }
        }
        files
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
