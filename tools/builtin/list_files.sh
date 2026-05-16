#!/usr/bin/env bash
# @name list_files
# @desc List files and directories in the session workspace.
# @arg path  Directory to list relative to workspace (default: root)
# @session code

set -euo pipefail

PATH_ARG="${1:-.}"
WORKDIR="${NUZZLE_WORKSPACE:-$HOME/.local/share/nuzzle/code}"

mkdir -p "$WORKDIR"
TARGET="$WORKDIR/$PATH_ARG"

if [ ! -d "$TARGET" ] && [ ! -f "$TARGET" ]; then
    echo "ERROR: Path not found: $PATH_ARG"
    exit 1
fi

echo "# $PATH_ARG"
echo ""
ls -lhA "$TARGET" 2>&1 || echo "Error listing directory"
