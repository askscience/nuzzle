# Nuzzle

**Snuggle up to your feeds with AI.** — the AI-native terminal RSS reader.

Type questions in the ask bar at the bottom. Nuzzle searches your feeds, streams answers from Ollama, and can even suggest and add new RSS feeds for you.

## Features

- **AI-powered reading** — summarize, ask questions, daily digests via Ollama
- **Tool-calling AI** — the model can search your articles and add new RSS feeds
- **Streaming responses** — answers appear token-by-token as the AI generates them
- **Slash commands** — `/models`, `/feed`, `/new`, `/exit`, `/session`, `/model`
- **Conversation memory** — each session saves your chat history; the AI remembers context
- **Semantic search** — vector embeddings for finding articles by meaning
- **Braille animations** — smooth, minimal braille spinners
- **Flat, borderless UI** — zero chrome, just your feeds
- **Offline-friendly** — AI runs locally via Ollama; your data never leaves your machine

## Install

```bash
curl -sSL https://raw.githubusercontent.com/askscience/nuzzle/main/install.sh | bash
```

Requirements: Rust toolchain, Ollama.

## Quick start

```bash
nuzzle
```

On first run, Nuzzle fetches Hacker News and creates `~/.config/nuzzle/config.toml`.

Type a question in the `⟩` bar at the bottom and press Enter.

## Slash commands

| Command | Action |
|---------|--------|
| `/exit` | Quit |
| `/feed` | Back to feeds view |
| `/new` | New conversation session |
| `/models` | List Ollama models |
| `/model <name>` | Switch model |
| `/session [name]` | List or switch sessions |

## Navigation

`j`/`k` or `↑`/`↓` — navigate. `Enter` — open. `Ctrl+C` — quit.

## Config

`~/.config/nuzzle/config.toml`:

```toml
[general]
poll_interval_secs = 600
db_path = "default"

[ollama]
endpoint = "http://localhost:11434"
model = "llama3.2"
embed_model = "nomic-embed-text"

[feeds]
urls = [
    "https://hnrss.org/frontpage",
]
```

## Build from source

```bash
git clone https://github.com/askscience/nuzzle
cd nuzzle
cargo build --release
./target/release/nuzzle
```

## License

GNU General Public License v3.0
