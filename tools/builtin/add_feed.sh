#!/usr/bin/env bash
# @name add_feed
# @desc Add a new RSS/Atom feed URL to your collection. The feed will be fetched and its articles included in future searches.
# @arg url  The complete RSS feed URL to add (required)
# @session chat,search

set -euo pipefail

URL="${1:-}"
DB="${NUZZLE_DB:-$HOME/.local/share/nuzzle/feeds.db}"

if [ -z "$URL" ]; then
    echo "ERROR: url is required"
    exit 1
fi

# Basic URL validation
if [[ ! "$URL" =~ ^https?:// ]]; then
    echo "ERROR: Invalid URL. Must start with http:// or https://"
    exit 1
fi

# Insert into DB
sqlite3 "$DB" "
    INSERT OR IGNORE INTO feeds (title, url, feed_type) VALUES ('$URL', '$URL', 'rss');
" 2>/dev/null

if [ $? -eq 0 ]; then
    echo "Feed added: $URL"
    echo "NOTE: The feed will be fetched on next refresh. Use /feed to reload."
else
    echo "ERROR: Failed to add feed. It may already exist."
fi
