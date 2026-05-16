#!/usr/bin/env bash
# @name exec
# @desc Execute a shell command in the current session workspace. Use for running compilers, package managers, tests, and other dev tools. Commands run in the session's workspace directory.
# @arg command  The shell command to execute (required)
# @session code

set -euo pipefail

CMD="${1:-}"
WORKDIR="${NUZZLE_WORKSPACE:-$HOME/.local/share/nuzzle/code}"

if [ -z "$CMD" ]; then
    echo "ERROR: command is required"
    exit 1
fi

mkdir -p "$WORKDIR"

cd "$WORKDIR"
echo "\$ $CMD"
echo ""

eval "$CMD" 2>&1 || true

echo ""
echo "*Exit code: ${PIPESTATUS[0]:-0}*"
