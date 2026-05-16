#!/usr/bin/env bash
# @name write_file
# @desc Create or overwrite a file in the session workspace. The file content is read from stdin or provided as the second argument.
# @arg path     Path to the file relative to workspace (required)
# @arg content  Content to write (optional, reads from stdin if not provided)
# @session code

set -euo pipefail

FILE="${1:-}"
WORKDIR="${NUZZLE_WORKSPACE:-$HOME/.local/share/nuzzle/code}"

if [ -z "$FILE" ]; then
    echo "ERROR: file path is required"
    exit 1
fi

mkdir -p "$WORKDIR"
TARGET="$WORKDIR/$FILE"

# Create parent directories if needed
mkdir -p "$(dirname "$TARGET")"

if [ $# -gt 1 ]; then
    shift
    echo "$*" > "$TARGET"
else
    cat > "$TARGET"
fi

echo "Written: $FILE ($(wc -c < "$TARGET") bytes)"
