# Nuzzle

**Snuggle up to your feeds with AI.** — the AI-native terminal RSS reader and coding assistant.

Nuzzle is a single-binary TUI application that combines an RSS reader with an AI assistant powered by Ollama. Ask questions about your feeds, perform deep web research, or start a coding session — all from your terminal.

## Features

- **AI-powered reading** — summarize articles, ask questions, daily digests via Ollama
- **CLI Tool System** — tools are shell scripts in `~/.config/nuzzle/tools/`. The AI discovers and invokes them automatically. Extensible: drop in your own `.sh` scripts.
- **Tool activity feedback** — the header shows what the AI is doing in real time (searching, reading files, running commands)
- **`/search` command** — deep web research via DuckDuckGo. Produces `research.md` files the AI can reference later.
- **`/code` command** — coding sessions with shell access. The AI can execute commands, read/write files, and build projects. Sessions have isolated workspaces.
- **Session navigator** — `/session` opens a popup to browse and switch between sessions by type, name, and description
- **Cross-session context** — AI can search past sessions for relevant information. Research documents are linked to coding sessions automatically.
- **Session descriptions** — each session gets an AI-generated one-sentence summary
- **Streaming responses** — answers appear token-by-token as the AI generates them
- **Conversation memory** — each session saves chat history; the AI remembers context
- **Semantic search** — vector embeddings for finding articles by meaning
- **Braille animations** — smooth, minimal braille spinners
- **Flat, borderless UI** — zero chrome, just your content
- **Offline-friendly** — AI runs locally via Ollama; most features work without internet

## Install

```bash
curl -sSL https://raw.githubusercontent.com/askscience/nuzzle/master/install.sh | bash
```

**From a local clone** (preserves your changes):

```bash
./install.sh
```

**Upgrade** from upstream (clones latest from GitHub, replaces binary):

```bash
./install.sh --upgrade
```

Requirements: Rust toolchain, Ollama, `python3`, `curl`, `sqlite3` (for tool scripts).

## Quick start

```bash
nuzzle
```

On first run, Nuzzle fetches Hacker News and creates `~/.config/nuzzle/config.toml`.

Type a question in the `⟩` bar at the bottom and press Enter. The AI answers by searching your feeds.

### Workflow example

```
# 1. Research a topic
/search async Rust best practices 2025
# Header shows "web_search: searching..." → "fetch_page: loading..." → "done: web_search (3 lines), fetch_page (45 lines)"
# AI produces ~/.local/share/nuzzle/research/search-XXXXXX/research.md

# 2. Create a coding session
/code my-async-app
# AI detects the research.md and offers it as reference

# 3. Start coding
Let's build a CLI tool with tokio. Read the research file first, then scaffold the project.
# Header: "read_file research.md (2048 bytes)" → "exec cargo init..." → "write_file main.rs (512 bytes)"
```

## Slash commands

| Command | Action |
|---------|--------|
| `/exit` | Quit |
| `/feed` | Back to feeds view |
| `/new` | New chat session |
| `/search [query]` | Create search session, optionally start researching |
| `/code [name]` | Create coding session with shell + file tools |
| `/session [name]` | Open session popup or switch/create by name |
| `/models` | Open model selector popup |
| `/model <name>` | Switch AI model |

## Navigation

`j`/`k` or `↑`/`↓` — navigate. `Enter` — open/select. `Ctrl+C` — quit. `Esc`/`q` — close popup.

In Ask mode: `↑`/`↓` to browse previous conversation blocks.

In popups (models/sessions): `j`/`k` to navigate, `Enter` to select, `Esc` to cancel.

## Session types

### Chat (`/new` or default)
General Q&A. The AI can search your RSS feeds, read full articles, and perform deep research on your feed content.

### Search (`/search [query]`)
Deep web research. The AI uses `web_search` and `fetch_page` tools to find and read web content. Results are compiled into a `research.md` file saved in `~/.local/share/nuzzle/research/{session-name}/`. These files are automatically linked to subsequent coding sessions.

### Code (`/code [name]`)
Coding session with full shell access. The AI gets these tools:
- `exec` — run shell commands in the workspace
- `read_file` — read project files
- `write_file` — create or modify files
- `list_files` — explore directory structure
- `list_sessions` / `search_sessions` / `read_session` — find context from past sessions

Each code session has its own workspace at `~/.local/share/nuzzle/code/{session-name}/`.

## Tool scripts

Tools are shell scripts in `~/.config/nuzzle/tools/`. Each script declares its metadata in comment headers:

```bash
#!/usr/bin/env bash
# @name my_tool
# @desc Does something useful
# @arg query  Description of the argument
# @session chat,code
```

Nuzzle scans this directory on startup and makes all discovered tools available to the AI. The AI invokes them using a text-based protocol:

```
<tool>
my_tool --query "something"
</tool>
```

### Built-in tools

| Tool | Session | Description |
|------|---------|-------------|
| `search_news` | chat, search | Search RSS feed articles by keyword |
| `read_article` | chat, search | Read full text of an article |
| `deep_research` | chat, search | Search + read multiple articles in full |
| `add_feed` | chat, search | Add a new RSS feed URL |
| `web_search` | search, chat | Search the web via DuckDuckGo |
| `fetch_page` | search, code | Fetch and extract text from a URL |
| `exec` | code | Execute shell commands |
| `read_file` | code | Read file contents |
| `write_file` | code | Write/create a file |
| `list_files` | code | List directory contents |
| `list_sessions` | all | List past sessions with descriptions |
| `search_sessions` | all | Search across session messages |
| `read_session` | all | Read messages from a past session |

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

[search]
duckduckgo_enabled = true
brave_api_key = ""

[tools]
scripts_dir = "~/.config/nuzzle/tools"

[mcp]
enabled = false
servers = []
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
