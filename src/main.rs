mod ai;
mod app;
mod config;
mod db;
mod event;
mod feed;
mod highlight;
mod markdown;
mod search;
mod tui;
mod tools;
mod types;

use std::sync::Arc;

use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::Mutex;

use crate::ai::client::OllamaClient;
use crate::app::App;
use crate::config::Config;
use crate::db::repository::Repository;
use crate::feed::manager::FeedManager;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    let config = Config::load()?;

    // Ensure data directory exists
    if let Some(parent) = config.general.db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let repo = Arc::new(Mutex::new(Repository::open(&config.general.db_path)?));

    // Load feeds from config if first run
    {
        let needs_load = {
            let r = repo.lock().await;
            r.list_feeds()?.is_empty()
        };
        if needs_load {
            let fm = FeedManager::new(repo.clone());
            let added = fm.load_feeds_from_config(&config).await?;
            log::info!("+{} feeds from config", added.len());
            // Fetch articles immediately
            match fm.refresh_all(&config).await {
                Ok((n, _)) => log::info!("+{} articles", n),
                Err(e) => log::warn!("fetch err: {}", e),
            }
        }
    }

    let ai = OllamaClient::new(&config.ollama.endpoint);

    // Check Ollama health
    if !ai.health_check().await? {
        log::warn!(
            "Cannot reach Ollama at {}. AI features will be unavailable.",
            config.ollama.endpoint
        );
    } else {
        log::info!(
            "Connected to Ollama at {} (model: {})",
            config.ollama.endpoint,
            config.ollama.model
        );
    }

    let feed_manager = FeedManager::new(repo.clone());

    // Setup terminal
    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    terminal.hide_cursor()?;

    // Build and run app
    let mut app = App::new(repo, ai, config, feed_manager);
    if let Err(e) = app.init().await {
        log::error!("App initialization error: {}", e);
    }

    let result = app.run(&mut terminal).await;

    // Cleanup
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}
