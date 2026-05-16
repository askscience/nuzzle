#!/usr/bin/env bash
# @name fetch_page
# @desc Fetch and extract readable text content from a web URL. Use to read pages found via web_search.
# @arg url  The URL to fetch and extract text from (required)
# @session search,code

set -euo pipefail

URL="${1:-}"

if [ -z "$URL" ]; then
    echo "ERROR: url is required"
    exit 1
fi

echo "# Fetched: $URL"
echo ""

python3 -c "
import urllib.request, http.cookiejar, re, html, sys

url = '''$URL'''

cj = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))
opener.addheaders = [
    ('User-Agent', 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'),
    ('Accept', 'text/html,application/xhtml+xml'),
    ('Accept-Language', 'en-US,en;q=0.9'),
]

try:
    resp = opener.open(url, timeout=20)
    content = resp.read().decode('utf-8', errors='replace')
except Exception as e:
    print(f'ERROR: Failed to fetch URL: {e}')
    sys.exit(0)

# Strip scripts, styles, and HTML tags
content = re.sub(r'<(script|style|nav|header|footer)[^>]*>.*?</\1>', '', content, flags=re.DOTALL | re.IGNORECASE)
content = re.sub(r'<[^>]+>', ' ', content)
content = html.unescape(content)
content = re.sub(r'\n\s*\n', '\n\n', content)
content = re.sub(r'[ \t]+', ' ', content)

# Get meaningful lines
lines = [l.strip() for l in content.split('\n')]
lines = [l for l in lines if len(l) > 20]

# Limit output size
if len(lines) > 200:
    lines = lines[:200]
    lines.append('[Content truncated at 200 lines]')

print('\n'.join(lines))
" 2>/dev/null || echo "ERROR: Failed to extract text from the page."

echo ""
echo "*Page fetch complete.*"
