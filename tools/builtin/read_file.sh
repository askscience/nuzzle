#!/usr/bin/env bash
# @name read_file
# @desc Read the contents of a file in the session workspace.
# @arg path  Path to the file relative to workspace (required)
# @session code

set -euo pipefail

FILE="${1:-}"
WORKDIR="${NUZZLE_WORKSPACE:-$HOME/.local/share/nuzzle/code}"

if [ -z "$FILE" ]; then
    echo "ERROR: file path is required"
    exit 1
fi

TARGET="$WORKDIR/$FILE"

if [ ! -f "$TARGET" ]; then
    echo "ERROR: File not found: $FILE"
    exit 1
fi

echo "# $FILE"
echo "\`\`\`"
cat "$TARGET"
echo "\`\`\`"
