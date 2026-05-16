#!/usr/bin/env bash
# @name read_article
# @desc Read the full content of a specific article by title (partial match). Use after search_news to get full text.
# @arg title  Title of the article to read (partial match)
# @session chat,search

set -euo pipefail

TITLE="${1:-}"
DB="${NUZZLE_DB:-$HOME/.local/share/nuzzle/feeds.db}"

if [ -z "$TITLE" ]; then
    echo "ERROR: title argument is required"
    exit 1
fi

CONTENT=$(sqlite3 "$DB" "
    SELECT '### ' || COALESCE(e.title,'Untitled') || char(10) || char(10) ||
           COALESCE(e.content, e.summary, 'No content available')
    FROM entries e
    WHERE e.title LIKE '%${TITLE//\'/\'\'}%'
    LIMIT 1;
" 2>/dev/null)

if [ -z "$CONTENT" ]; then
    echo "Article not found. Check the title spelling."
else
    echo "$CONTENT"
fi
