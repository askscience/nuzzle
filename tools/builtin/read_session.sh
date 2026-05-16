#!/usr/bin/env bash
# @name read_session
# @desc Read all messages from a specific past session. Use to review context from previous conversations.
# @arg session_name  Name of the session to read (required, partial match)
# @session chat,code,search

set -euo pipefail

NAME="${1:-}"
DB="${NUZZLE_DB:-$HOME/.local/share/nuzzle/feeds.db}"

if [ -z "$NAME" ]; then
    echo "ERROR: session name is required"
    exit 1
fi

# Find session ID
SID=$(sqlite3 "$DB" "
    SELECT id FROM sessions WHERE name LIKE '%${NAME//\'/\'\'}%' LIMIT 1;
" 2>/dev/null)

if [ -z "$SID" ]; then
    echo "Session not found: $NAME"
    exit 1
fi

sqlite3 "$DB" "
    SELECT '[' || role || ' @ ' || created_at || ']:' || char(10) || content || char(10)
    FROM messages
    WHERE session_id = $SID
    ORDER BY id ASC;
" 2>/dev/null

if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo "Session has no messages."
fi
