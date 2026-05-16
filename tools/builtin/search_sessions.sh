#!/usr/bin/env bash
# @name search_sessions
# @desc Search across all session messages and descriptions for relevant past conversations. Use to find context from previous work.
# @arg query  Search keywords (required)
# @session chat,code,search

set -euo pipefail

QUERY="${1:-}"
DB="${NUZZLE_DB:-$HOME/.local/share/nuzzle/feeds.db}"

if [ -z "$QUERY" ]; then
    echo "ERROR: query is required"
    exit 1
fi

echo "# Sessions matching: $QUERY"
echo ""

sqlite3 -separator ' | ' "$DB" "
    SELECT s.session_type, s.name, s.description, SUBSTR(m.content, 1, 200), s.created_at
    FROM messages m
    JOIN sessions s ON m.session_id = s.id
    WHERE m.content LIKE '%${QUERY//\'/\'\'}%'
       OR s.description LIKE '%${QUERY//\'/\'\'}%'
       OR s.name LIKE '%${QUERY//\'/\'\'}%'
    GROUP BY s.id
    ORDER BY s.created_at DESC
    LIMIT 10;
" 2>/dev/null | while IFS=' | ' read -r type name desc preview date; do
    echo "## $type: $name"
    echo "**Description:** ${desc:-(none)}"
    echo "**Date:** $date"
    echo ""
    echo "${preview}..."
    echo ""
    echo "---"
    echo ""
done

if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo "No matching sessions found."
fi
