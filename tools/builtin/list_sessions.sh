#!/usr/bin/env bash
# @name list_sessions
# @desc List all past AI sessions with their names, types, and descriptions. Use to find relevant past conversations.
# @session chat,code,search

set -euo pipefail

DB="${NUZZLE_DB:-$HOME/.local/share/nuzzle/feeds.db}"

echo "# Sessions"
echo ""

sqlite3 -separator $' | ' "$DB" "
    SELECT '[' || session_type || ']',
           name,
           description,
           created_at
    FROM sessions
    ORDER BY created_at DESC
    LIMIT 50;
" 2>/dev/null | while IFS=' | ' read -r type name desc date; do
    echo "- $type **$name** — ${desc:-(no description)} — *$date*"
done

if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo "No sessions found."
fi
