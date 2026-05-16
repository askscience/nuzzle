#!/usr/bin/env bash
# @name web_search
# @desc Search the web via DuckDuckGo. Returns result titles, URLs, and snippets. For in-depth research, follow up with fetch_page.
# @arg query        The search query (required)
# @arg max_results  Maximum number of results (default 5, max 10)
# @session search,chat

set -euo pipefail

QUERY="${1:-}"
MAX="${2:-5}"

if [ -z "$QUERY" ]; then
    echo "ERROR: query is required"
    exit 1
fi

MAX=$(( MAX > 10 ? 10 : MAX ))
MAX=$(( MAX < 1 ? 1 : MAX ))

echo "# Web Search: $QUERY"
echo ""

python3 -c "
import urllib.request, urllib.parse, http.cookiejar, re, html, sys

query = '''$QUERY'''
max_results = $MAX

cj = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))
opener.addheaders = [
    ('User-Agent', 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'),
    ('Accept', 'text/html,application/xhtml+xml'),
    ('Accept-Language', 'en-US,en;q=0.9'),
]

encoded = urllib.parse.quote(query)
url = f'https://html.duckduckgo.com/html/?q={encoded}'

try:
    resp = opener.open(url, timeout=15)
    content = resp.read().decode('utf-8', errors='replace')
except Exception as e:
    print(f'Search failed: {e}')
    sys.exit(0)

# Parse result links: <a class=\"result__a\" href=\"//duckduckgo.com/l/?uddg=ENCODED_URL&rut=...\">Title</a>
result_re = re.compile(
    r'<a[^>]*class=\"[^\"]*result__a[^\"]*\"[^>]*href=\"([^\"]+)\"[^>]*>(.*?)</a>',
    re.DOTALL
)

# Parse snippets: <a class=\"result__snippet\">Text with <b>tags</b></a>
snippet_re = re.compile(
    r'<a[^>]*class=\"[^\"]*result__snippet[^\"]*\"[^>]*>(.*?)</a>',
    re.DOTALL
)

all_results = result_re.findall(content)
all_snippets = snippet_re.findall(content)

count = 0
for href, title_block in all_results:
    if count >= max_results:
        break

    # Extract actual URL from DDG redirect
    # Format: //duckduckgo.com/l/?uddg=ENCODED_URL&rut=...
    real_url = href
    m = re.search(r'uddg=([^&]+)', href)
    if m:
        real_url = urllib.parse.unquote(m.group(1))

    # Clean title
    title = html.unescape(re.sub(r'<[^>]+>', '', title_block)).strip()
    if not title:
        continue

    count += 1

    # Get snippet
    snippet = ''
    for s in all_snippets[count-1:count+2]:
        s_text = html.unescape(re.sub(r'<[^>]+>', '', s)).strip()
        if s_text:
            snippet = s_text
            break

    print(f'### {count}. {title}')
    print(f'**URL:** {real_url}')
    print()
    if snippet:
        print(snippet)
    print()
    print('---')
    print()

if count == 0:
    print('No results found. Try different keywords or configure a Brave Search API key.')
" 2>/dev/null || {
    echo "Search error: Python script failed. Make sure python3 is installed."
}

echo ""
echo "*Search complete.*"
