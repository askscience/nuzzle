#!/bin/bash
set -e

# === Colors ===
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

DOT="  ${BLUE}•${NC}"

line() { printf "%b\n" "$1"; }

# === Cleanup on failure ===
TMPDIR=""
cleanup() {
    if [ -n "$TMPDIR" ] && [ -d "$TMPDIR" ]; then
        rm -rf "$TMPDIR"
    fi
}
trap cleanup EXIT

# === Banner ===
echo ""
line "${BOLD}${CYAN}  █▄░█ █░█ ▀█ ▀█ █░░ █▀▀"
line "  █░▀█ █▄█ █▄ █▄ █▄▄ ██▄${NC}"
line "  ${BOLD}Snuggle up to your feeds with AI.${NC}"
echo ""

# === Platform detection ===
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        OS_NAME="Linux"
        SHELL_RC=".bashrc"
        case "$SHELL" in
            */zsh) SHELL_RC=".zshrc" ;;
        esac
        ;;
    Darwin)
        OS_NAME="macOS"
        SHELL_RC=".zshrc"
        SHELL_PROFILE=".zprofile"
        ;;
    *)
        line "${RED}Unsupported OS: $OS${NC}"
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64)   ARCH_NAME="x86_64" ;;
    aarch64|arm64)  ARCH_NAME="arm64" ;;
    *)
        line "${YELLOW}Warning: untested architecture $ARCH — continuing anyway${NC}"
        ARCH_NAME="$ARCH"
        ;;
esac

line "$DOT Detected: ${BOLD}$OS_NAME${NC} on ${BOLD}$ARCH_NAME${NC}"
echo ""

# === Check prerequisites ===
# --- C compiler (needed for rusqlite bundled) ---
if ! command -v cc &>/dev/null; then
    line "$DOT C compiler not found"
    case "$OS" in
        Darwin)
            line "$DOT Installing Xcode Command Line Tools..."
            xcode-select --install 2>/dev/null || true
            line "   ${YELLOW}If prompted, complete the Xcode CLI install and re-run this script.${NC}"
            ;;
        Linux)
            line "$DOT Installing build-essential..."
            if command -v apt-get &>/dev/null; then
                sudo apt-get update -qq && sudo apt-get install -y -qq build-essential pkg-config libssl-dev
            elif command -v dnf &>/dev/null; then
                sudo dnf install -y gcc gcc-c++ make pkg-config openssl-devel
            elif command -v pacman &>/dev/null; then
                sudo pacman -S --noconfirm base-devel pkg-config openssl
            elif command -v apk &>/dev/null; then
                sudo apk add build-base pkgconfig openssl-dev
            else
                line "   ${YELLOW}Install a C compiler (gcc/clang) and re-run this script.${NC}"
            fi
            ;;
    esac
fi

# --- Git ---
if ! command -v git &>/dev/null; then
    line "$DOT Installing git..."
    case "$OS" in
        Darwin)
            if command -v brew &>/dev/null; then
                brew install git
            else
                line "${RED}Please install git: https://git-scm.com/downloads${NC}"
                exit 1
            fi
            ;;
        Linux)
            if command -v apt-get &>/dev/null; then
                sudo apt-get update -qq && sudo apt-get install -y -qq git
            elif command -v dnf &>/dev/null; then
                sudo dnf install -y git
            elif command -v pacman &>/dev/null; then
                sudo pacman -S --noconfirm git
            else
                line "${RED}Please install git manually and re-run.${NC}"
                exit 1
            fi
            ;;
    esac
fi

# --- Curl ---
if ! command -v curl &>/dev/null; then
    line "$DOT Installing curl..."
    case "$OS" in
        Darwin) brew install curl 2>/dev/null || true ;;
        Linux)
            if command -v apt-get &>/dev/null; then
                sudo apt-get update -qq && sudo apt-get install -y -qq curl
            elif command -v dnf &>/dev/null; then
                sudo dnf install -y curl
            fi
            ;;
    esac
fi

# === Install Rust ===
if ! command -v cargo &>/dev/null; then
    line "$DOT Installing Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    source "$HOME/.cargo/env"

    # Ensure ~/.cargo/bin is on PATH for the current session
    export PATH="$HOME/.cargo/bin:$PATH"
    # Persist across logins
    SHELL_RC_FILE=""
    case "$SHELL" in
        */zsh) SHELL_RC_FILE="$HOME/.zshrc" ;;
        */bash) SHELL_RC_FILE="$HOME/.bashrc" ;;
        */fish) SHELL_RC_FILE="$HOME/.config/fish/config.fish" ;;
        *) SHELL_RC_FILE="$HOME/.profile" ;;
    esac
    if [ -f "$SHELL_RC_FILE" ]; then
        if ! grep -q '.cargo/bin' "$SHELL_RC_FILE" 2>/dev/null; then
            echo "" >> "$SHELL_RC_FILE"
            echo "# Rust toolchain" >> "$SHELL_RC_FILE"
            echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$SHELL_RC_FILE"
        fi
    fi
else
    line "$DOT Rust: ${GREEN}$(cargo --version | head -1)${NC}"
fi

# === Check if already installed ===
if command -v nuzzle &>/dev/null; then
    line "${GREEN}Nuzzle already installed at $(which nuzzle)${NC}"
    line "Run 'nuzzle' to start."
    exit 0
fi

# === Clone & build ===
REPO="https://github.com/askscience/nuzzle"
TMPDIR=$(mktemp -d)

line "$DOT Cloning $REPO..."
git clone --depth 1 "$REPO" "$TMPDIR" 2>/dev/null || {
    line "${RED}Clone failed — is the repo public?${NC}"
    exit 1
}

line "$DOT Building release binary (this may take a few minutes)..."
(cd "$TMPDIR" && cargo build --release 2>&1) | while IFS= read -r l; do
    case "$l" in
        *Compiling*) printf "  %b\r" "${CYAN}${l}${NC}" ;;
        *error*)    line "\n${RED}$l${NC}" ;;
    esac
done
echo ""

# === Install binary ===
INSTALL_DIR=""
NO_SUDO=0

# Prefer ~/.local/bin (no sudo on most modern Linux, also common on macOS)
if [ -d "$HOME/.local/bin" ]; then
    INSTALL_DIR="$HOME/.local/bin"
    NO_SUDO=1
elif mkdir -p "$HOME/.local/bin" 2>/dev/null; then
    INSTALL_DIR="$HOME/.local/bin"
    NO_SUDO=1
# Fallback: ~/.cargo/bin (Rust's default, already on PATH if rustup was used)
elif [ -d "$HOME/.cargo/bin" ]; then
    INSTALL_DIR="$HOME/.cargo/bin"
    NO_SUDO=1
# Last resort: /usr/local/bin (needs sudo)
else
    INSTALL_DIR="/usr/local/bin"
    NO_SUDO=0
fi

if [ "$NO_SUDO" -eq 1 ]; then
    cp "$TMPDIR/target/release/nuzzle" "$INSTALL_DIR/nuzzle"
    line "${GREEN}Installed nuzzle → $INSTALL_DIR/nuzzle${NC}"
else
    sudo cp "$TMPDIR/target/release/nuzzle" "$INSTALL_DIR/nuzzle"
    line "${GREEN}Installed nuzzle → $INSTALL_DIR/nuzzle${NC}"
fi

# === PATH setup ===
case "$INSTALL_DIR" in
    "$HOME/.local/bin")
        case "$SHELL" in
            */zsh)
                RC_FILE="$HOME/.zshrc"
                PROFILE_FILE="$HOME/.zprofile"
                ;;
            */bash)
                RC_FILE="$HOME/.bashrc"
                PROFILE_FILE="$HOME/.bash_profile"
                ;;
            */fish)
                RC_FILE="$HOME/.config/fish/config.fish"
                PROFILE_FILE=""
                ;;
            *)
                RC_FILE="$HOME/.profile"
                PROFILE_FILE=""
                ;;
        esac
        ;;
    *)
        RC_FILE=""
        PROFILE_FILE=""
        ;;
esac

add_to_path() {
    local file="$1"
    if [ -f "$file" ] && grep -q "\.local/bin" "$file" 2>/dev/null; then
        return
    fi
    if [ -f "$file" ]; then
        echo "" >> "$file"
        echo "# Added by nuzzle installer" >> "$file"
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$file"
    fi
}

if [ -n "$RC_FILE" ]; then
    add_to_path "$RC_FILE"
fi
if [ -n "$PROFILE_FILE" ]; then
    add_to_path "$PROFILE_FILE"
fi

# =============================================================
# FIXUP: if we installed to ~/.local/bin but it is NOT in PATH
#        right now, prepend it so 'nuzzle' works immediately.
#        (The profile file above ensures it persists for new shells.)
# =============================================================
if [ "$INSTALL_DIR" = "$HOME/.local/bin" ]; then
    if ! echo "$PATH" | tr ':' '\n' | grep -Fxq "$HOME/.local/bin"; then
        export PATH="$HOME/.local/bin:$PATH"
    fi
fi

echo ""
if [ "$NO_SUDO" -eq 1 ]; then
    line "${GREEN}${BOLD}Done!${NC} Run 'nuzzle' to start."
else
    line "${YELLOW}${BOLD}Done!${NC} Run 'nuzzle' to start."
fi
echo ""
