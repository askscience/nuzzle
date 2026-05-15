use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};

/// Render markdown into a styled ratatui Text.
pub fn render(text: &str) -> Text<'static> {
    let mut lines: Vec<Line> = vec![];
    for raw in text.lines() {
        let trimmed = raw.trim();

        // Blank line → visible space
        if trimmed.is_empty() {
            lines.push(Line::from(" "));
            continue;
        }

        // Horizontal rule
        if trimmed.chars().all(|c| c == '-' || c == '_' || c == '*') && trimmed.len() >= 3 {
            let dashes = "─".repeat(80);
            lines.push(Line::from(Span::styled(dashes, Style::new().dim())));
            continue;
        }

        // Blockquote
        if let Some(content) = trimmed.strip_prefix("> ") {
            lines.push(Line::from(Span::styled(
                format!("│ {}", content),
                Style::new().dim().fg(Color::Gray),
            )));
            continue;
        }

        // Headings
        if let Some(h) = try_heading(trimmed) {
            lines.push(h);
            continue;
        }

        // Bullet lists: -, *, or •
        if let Some(content) = trimmed.strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("• "))
        {
            lines.push(Line::from(Span::styled(format!("  • {}", content), Style::new())));
            continue;
        }

        // Ordered list
        if let Some(line) = try_ordered(trimmed) {
            lines.push(line);
            continue;
        }

        // Regular text with inline formatting
        lines.push(parse_inline(raw));
    }
    Text::from(lines)
}

// ── helpers ──

fn try_heading(line: &str) -> Option<Line<'static>> {
    for i in 1..=6 {
        let prefix = "#".repeat(i) + " ";
        if let Some(rest) = line.strip_prefix(&prefix) {
            return Some(Line::from(Span::styled(
                rest.to_string(),
                Style::new().bold().fg(Color::Cyan),
            )));
        }
    }
    None
}

fn try_ordered(s: &str) -> Option<Line<'static>> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    if i > 0 && i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
        Some(Line::from(Span::styled(format!("  {}. {}", &s[..i], &s[i + 2..]), Style::new())))
    } else {
        None
    }
}

// ── inline formatting ──

fn parse_inline(line: &str) -> Line<'static> {
    let mut spans: Vec<Span> = vec![];
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        // Bold **text** or __text__
        if pos + 1 < chars.len()
            && ((chars[pos] == '*' && chars[pos + 1] == '*')
                || (chars[pos] == '_' && chars[pos + 1] == '_'))
        {
            let marker: &[char] = if chars[pos] == '*' { &['*', '*'] } else { &['_', '_'] };
            let start = pos + 2;
            if let Some(end) = find_str(&chars, start, marker) {
                let text: String = chars[start..end].iter().collect();
                spans.push(Span::styled(text, Style::new().bold()));
                pos = end + 2;
                continue;
            }
        }

        // Italic *text* or _text_ (single char, not followed by same)
        if (chars[pos] == '*' || chars[pos] == '_')
            && pos + 1 < chars.len()
            && chars[pos + 1] != chars[pos]
        {
            let target = chars[pos];
            let start = pos + 1;
            if let Some(end) = find_char(&chars, start, target) {
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
            if let Some(title_end) = find_char(&chars, pos + 1, ']') {
                if title_end + 1 < chars.len() && chars[title_end + 1] == '(' {
                    if let Some(url_end) = find_char(&chars, title_end + 2, ')') {
                        let title: String = chars[pos + 1..title_end].iter().collect();
                        let url: String = chars[title_end + 2..url_end].iter().collect();
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

fn find_str(chars: &[char], start: usize, target: &[char]) -> Option<usize> {
    if target.is_empty() { return None; }
    for i in start..chars.len().saturating_sub(target.len() - 1) {
        if chars[i..].starts_with(target) { return Some(i); }
    }
    None
}
