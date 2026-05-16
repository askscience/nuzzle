#!/usr/bin/env bash
# @name deep_research
# @desc Perform deep research: search for articles AND read their full content. Use for in-depth analysis.
# @arg topic  The topic or question to research deeply (required)
# @arg depth  Number of articles to read in full (default 3, max 5)
# @session chat,search

set -euo pipefail

TOPIC="${1:-}"
DEPTH="${2:-3}"
DB="${NUZZLE_DB:-$HOME/.local/share/nuzzle/feeds.db}"

if [ -z "$TOPIC" ]; then
    echo "ERROR: topic is required"
    exit 1
fi

DEPTH=$(( DEPTH > 5 ? 5 : DEPTH ))
DEPTH=$(( DEPTH < 1 ? 1 : DEPTH ))

echo "# Deep Research: $TOPIC"
echo ""

# Find matching articles
RESULTS=$(sqlite3 -separator $'|||' "$DB" "
    SELECT e.id, e.title, e.content, e.summary, e.link, e.published_at, f.title
    FROM entries e
    JOIN feeds f ON e.feed_id = f.id
    WHERE e.title LIKE '%${TOPIC//\'/\'\'}%'
       OR e.summary LIKE '%${TOPIC//\'/\'\'}%'
       OR e.content LIKE '%${TOPIC//\'/\'\'}%'
    ORDER BY e.published_at DESC
    LIMIT $DEPTH;
" 2>/dev/null)

if [ -z "$RESULTS" ]; then
    echo "No articles found for this topic."
    echo "Consider adding more RSS feeds with /feed or add_feed tool."
    exit 0
fi

COUNT=0
while IFS='|||' read -r id title content summary link date feed; do
    COUNT=$((COUNT + 1))
    BODY="${content:-$summary}"
    echo "## Article $COUNT: ${title:-Untitled}"
    echo "**Source:** ${feed:-Unknown} | **Date:** ${date:-unknown}"
    echo "**Link:** ${link:-N/A}"
    echo ""
    echo "${BODY:-No content available}"
    echo ""
    echo "---"
    echo ""
done <<< "$RESULTS"

echo "*Research complete — $COUNT articles read in full.*"
