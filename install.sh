#!/bin/bash
set -e

echo "Nuzzle — AI-native terminal RSS reader"
echo ""

if ! command -v cargo &>/dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

if command -v nuzzle &>/dev/null; then
    echo "Nuzzle already installed at $(which nuzzle)"
    echo "Run 'nuzzle' to start."
    exit 0
fi

REPO="https://github.com/askscience/nuzzle"
TMPDIR=$(mktemp -d)

echo "Cloning $REPO..."
git clone "$REPO" "$TMPDIR" 2>/dev/null || {
    echo "Clone failed — is the repo public? Try:"
    echo "  git clone $REPO && cd nuzzle && cargo build --release"
    exit 1
}

cd "$TMPDIR"
echo "Building..."
cargo build --release
sudo cp target/release/nuzzle /usr/local/bin/nuzzle
rm -rf "$TMPDIR"

echo ""
echo "Done. Run 'nuzzle' to start."
