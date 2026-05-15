use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};

/// Render markdown text into a ratatui `Text` with full formatting.
pub fn render(text: &str) -> Text<'static> {
    let mut lines: Vec<Line> = vec![];
    for raw in text.lines() {
        // Blank lines become empty
        if raw.trim().is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        // Horizontal rule
        if raw.trim().chars().all(|c| c == '-' || c == '_' || c == '*' || c == '=') && raw.trim().len() >= 3 {
            let w = raw.len();
            lines.push(Line::from(Span::styled("─".repeat(w), Style::new().dim())));
            continue;
        }

        // Blockquote
        if raw.trim_start().starts_with("> ") || raw.trim_start().starts_with(">") {
            let content = raw.trim_start().trim_start_matches("> ").trim_start_matches('>');
            let quote_line = format!("│ {}", content);
            lines.push(Line::from(Span::styled(quote_line, Style::new().dim().fg(Color::Gray))));
            continue;
        }

        // Headings
        if let Some(heading) = parse_heading(raw) {
            lines.push(heading);
            continue;
        }

        // Unordered list
        if let Some(stripped) = raw.trim_start().strip_prefix("- ").or_else(|| raw.trim_start().strip_prefix("* ")) {
            let line = format!("  • {}", stripped);
            lines.push(Line::from(Span::styled(line, Style::new())));
            continue;
        }

        // Ordered list
        if let Some(stripped) = strip_ordered_prefix(raw.trim_start()) {
            let line = format!("  {}", stripped);
            lines.push(Line::from(Span::styled(line, Style::new())));
            continue;
        }

        // Regular paragraph — inline formatting
        lines.push(parse_inline(raw));
    }
    Text::from(lines)
}

fn parse_heading(line: &str) -> Option<Line<'static>> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("# ")     { return Some(heading(rest, 1)); }
    if let Some(rest) = trimmed.strip_prefix("## ")    { return Some(heading(rest, 2)); }
    if let Some(rest) = trimmed.strip_prefix("### ")   { return Some(heading(rest, 3)); }
    if let Some(rest) = trimmed.strip_prefix("#### ")  { return Some(heading(rest, 4)); }
    None
}

fn heading(text: &str, _level: u8) -> Line<'static> {
    Line::from(Span::styled(text.to_string(), Style::new().bold().fg(Color::Cyan)))
}

fn strip_ordered_prefix(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    if i > 0 && i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
        Some(format!("  {}. {}", &s[..i], &s[i + 2..]))
    } else {
        None
    }
}

/// Parse inline markdown: **bold**, *italic*, `code`, [link](url)
fn parse_inline(line: &str) -> Line<'static> {
    let mut spans: Vec<Span> = vec![];
    let mut pos = 0;
    let chars: Vec<char> = line.chars().collect();

        while pos < chars.len() {
            // Bold **text** or __text__
            if pos + 1 < chars.len() && ((chars[pos] == '*' && chars[pos + 1] == '*') || (chars[pos] == '_' && chars[pos + 1] == '_')) {
                let marker = if chars[pos] == '*' { "**" } else { "__" };
                let start = pos + 2;
                if let Some(end) = find_str(&chars, start, marker) {
                    let text: String = chars[start..end].iter().collect();
                    spans.push(Span::styled(text, Style::new().bold()));
                    pos = end + 2;
                    continue;
                }
            }
            // Italic *text* or _text_ (but not ** or __)
            if (chars[pos] == '*' || chars[pos] == '_') && (pos == 0 || (chars[pos - 1] != '*' && chars[pos - 1] != '_')) && pos + 1 < chars.len() {
                let start = pos + 1;
                if let Some(end) = find_char(&chars, start, chars[pos]) {
                    // Make sure not empty
                    if end > start {
                        let text: String = chars[start..end].iter().collect();
                        spans.push(Span::styled(text, Style::new().italic()));
                        pos = end + 1;
                        continue;
                    }
                }
            }
        // Inline code `text`
        if chars[pos] == '`' {
            let start = pos + 1;
            if let Some(end) = find_char(&chars, start, '`') {
                let text: String = chars[start..end].iter().collect();
                spans.push(Span::styled(text, Style::new().bg(Color::DarkGray).fg(Color::White)));
                pos = end + 1;
                continue;
            }
        }
        // Link [text](url)
        if chars[pos] == '[' {
            let title_start = pos + 1;
            if let Some(title_end) = find_char(&chars, title_start, ']') {
                if title_end + 1 < chars.len() && chars[title_end + 1] == '(' {
                    let url_start = title_end + 2;
                    if let Some(url_end) = find_char(&chars, url_start, ')') {
                        let title: String = chars[title_start..title_end].iter().collect();
                        let url: String = chars[url_start..url_end].iter().collect();
                        spans.push(Span::styled(
                            format!("{} ({})", title, url),
                            Style::new().add_modifier(Modifier::UNDERLINED).fg(Color::Cyan),
                        ));
                        pos = url_end + 1;
                        continue;
                    }
                }
            }
        }
        // Plain character
        spans.push(Span::raw(chars[pos].to_string()));
        pos += 1;
    }

    Line::from(spans)
}

fn find_char(chars: &[char], start: usize, target: char) -> Option<usize> {
    chars.iter().enumerate().skip(start).find(|(_, &c)| c == target).map(|(i, _)| i)
}

fn find_str(chars: &[char], start: usize, target: &str) -> Option<usize> {
    let target: Vec<char> = target.chars().collect();
    if target.is_empty() { return None; }
    for i in start..chars.len().saturating_sub(target.len() - 1) {
        if chars[i..].starts_with(&target) { return Some(i); }
    }
    None
}
