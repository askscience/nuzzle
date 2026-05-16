#!/usr/bin/env bash
# @name search_news
# @desc Search through your RSS feed articles to find relevant stories. Returns matching articles with titles, summaries, and links.
# @arg query        Search keywords (required)
# @arg max_results  Maximum number of results (default 5)
# @session chat,search

set -euo pipefail

QUERY="${1:-}"
MAX="${2:-5}"
DB="${NUZZLE_DB:-$HOME/.local/share/nuzzle/feeds.db}"

if [ -z "$QUERY" ]; then
    echo "ERROR: query is required"
    exit 1
fi

# Search in title, summary, and content
sqlite3 -separator $'\t' "$DB" "
    SELECT e.title, e.summary, e.link, e.published_at, f.title
    FROM entries e
    JOIN feeds f ON e.feed_id = f.id
    WHERE e.title LIKE '%${QUERY//\'/\'\'}%'
       OR e.summary LIKE '%${QUERY//\'/\'\'}%'
       OR e.content LIKE '%${QUERY//\'/\'\'}%'
    ORDER BY e.published_at DESC
    LIMIT $MAX;
" 2>/dev/null | while IFS=$'\t' read -r title summary link date feed; do
    echo "### $title"
    echo "**Feed:** $feed | **Date:** ${date:-unknown}"
    echo "**Link:** ${link:-N/A}"
    echo ""
    echo "${summary:-No summary available}"
    echo ""
    echo "---"
    echo ""
done

if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo "No matching articles found. Try different keywords."
fi
