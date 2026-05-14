use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub general: General,
    pub ollama: Ollama,
    pub feeds: Feeds,
}

#[derive(Debug, Deserialize, Clone)]
pub struct General {
    pub poll_interval_secs: u64,
    pub db_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Ollama {
    pub endpoint: String,
    pub model: String,
    pub embed_model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Feeds {
    pub urls: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = dirs_data_dir();
        Self {
            general: General {
                poll_interval_secs: 600,
                db_path: data_dir.join("feeds.db"),
            },
            ollama: Ollama {
                endpoint: "http://localhost:11434".to_string(),
                model: "llama3.2".to_string(),
                embed_model: "nomic-embed-text".to_string(),
            },
            feeds: Feeds {
                urls: vec!["https://hnrss.org/frontpage".to_string()],
            },
        }
    }
}

fn dirs_data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".local/share/nuzzle")
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = dirs_config_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config at {:?}", config_path))?;
            let mut config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config at {:?}", config_path))?;

            let data_dir = dirs_data_dir();
            if config.general.db_path == PathBuf::from("default") {
                config.general.db_path = data_dir.join("feeds.db");
            }

            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = dirs_config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data_dir = dirs_data_dir();
        std::fs::create_dir_all(&data_dir)?;

        let mut s = String::new();
        s.push_str("[general]\n");
        s.push_str(&format!("poll_interval_secs = {}\n", self.general.poll_interval_secs));
        s.push_str(&format!("db_path = \"{}\"\n", self.general.db_path.display()));
        s.push_str("\n[ollama]\n");
        s.push_str(&format!("endpoint = \"{}\"\n", self.ollama.endpoint));
        s.push_str(&format!("model = \"{}\"\n", self.ollama.model));
        s.push_str(&format!("embed_model = \"{}\"\n", self.ollama.embed_model));
        s.push_str("\n[feeds]\n");
        s.push_str("urls = [\n");
        for url in &self.feeds.urls {
            s.push_str(&format!("  \"{}\",\n", url));
        }
        s.push_str("]\n");

        std::fs::write(&config_path, s)
            .with_context(|| format!("Failed to write config at {:?}", config_path))?;
        Ok(())
    }
}

fn dirs_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config/nuzzle/config.toml")
}
