use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};

const HEADING: Color = Color::Rgb(189, 166, 232);
const LINK: Color = Color::Rgb(138, 173, 224);
const QUOTE: Color = Color::Rgb(147, 147, 155);
const CODE_BG: Color = Color::Rgb(36, 39, 52);
const CODE_FG: Color = Color::Rgb(231, 221, 208);
const DIM: Color = Color::Rgb(147, 147, 155);

pub fn render(text: &str) -> Text<'static> {
    let mut lines: Vec<Line> = vec![];
    let mut in_code_block = false;

    for raw in text.lines() {
        let trimmed = raw.trim();

        if in_code_block {
            if trimmed == "```" || trimmed.starts_with("```") {
                in_code_block = false;
                continue;
            }
            let style = Style::new().bg(CODE_BG).fg(CODE_FG);
            lines.push(Line::from(Span::styled(raw.to_string(), style)));
            continue;
        }

        if trimmed.strip_prefix("```").is_some() {
            in_code_block = true;
            continue;
        }

        if trimmed.is_empty() {
            lines.push(Line::from(" "));
            continue;
        }

        if trimmed.chars().all(|c| c == '-' || c == '_' || c == '*') && trimmed.len() >= 3 {
            let dashes = "\u{2500}".repeat(80);
            lines.push(Line::from(Span::styled(dashes, Style::new().fg(DIM))));
            continue;
        }

        if let Some(content) = trimmed
            .strip_prefix("> ")
            .or_else(|| trimmed.strip_prefix(">"))
        {
            let content = content.trim_start();
            let mut spans = vec![Span::styled("\u{2502} ", Style::new().fg(QUOTE))];
            let quote_style = Style::new().fg(QUOTE);
            for span in parse_inline(content).spans {
                spans.push(span.patch_style(quote_style));
            }
            lines.push(Line::from(spans));
            continue;
        }

        if let Some(h) = try_heading(trimmed) {
            lines.push(h);
            continue;
        }

        if let Some(content) = trimmed.strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("\u{2022} "))
        {
            let mut spans = vec![Span::raw("  \u{2022} ")];
            spans.extend(parse_inline(content).spans);
            lines.push(Line::from(spans));
            continue;
        }

        if let Some(line) = try_ordered(trimmed) {
            lines.push(line);
            continue;
        }

        if raw.len() >= 4 && raw.chars().take(4).all(|c| c == ' ') {
            let style = Style::new().bg(CODE_BG).fg(CODE_FG);
            lines.push(Line::from(Span::styled(raw.to_string(), style)));
            continue;
        }

        lines.push(parse_inline(raw));
    }
    Text::from(lines)
}

fn try_heading(line: &str) -> Option<Line<'static>> {
    let bytes = line.as_bytes();
    let mut hash_count = 0;
    while hash_count < bytes.len() && bytes[hash_count] == b'#' {
        hash_count += 1;
    }
    if hash_count == 0 || hash_count > 6 {
        return None;
    }
    let rest = &line[hash_count..];
    let rest = rest.strip_prefix(' ').unwrap_or(rest);
    let heading_style = Style::new().bold().fg(HEADING);
    let spans: Vec<Span> = parse_inline(rest)
        .spans
        .into_iter()
        .map(|s| s.patch_style(heading_style))
        .collect();
    Some(Line::from(spans))
}

fn try_ordered(s: &str) -> Option<Line<'static>> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    if i > 0 && i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
        let mut spans = vec![Span::raw(format!("  {}. ", &s[..i]))];
        spans.extend(parse_inline(&s[i + 2..]).spans);
        Some(Line::from(spans))
    } else {
        None
    }
}

fn parse_inline(line: &str) -> Line<'static> {
    let mut spans: Vec<Span> = vec![];
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        let c = chars[pos];

        if c == '\\' && pos + 1 < chars.len() {
            let next = chars[pos + 1];
            if matches!(next, '*' | '_' | '`' | '[' | ']' | '(' | ')' | '#' | '\\') {
                spans.push(Span::raw(next.to_string()));
                pos += 2;
                continue;
            }
        }

        let double = pos + 1 < chars.len() && chars[pos + 1] == c;
        let triple = double && pos + 2 < chars.len() && chars[pos + 2] == c;

        if triple && (c == '*' || c == '_') {
            let marker: &[char] = &[c, c, c];
            if let Some(end) = find_str(&chars, pos + 3, marker) {
                let inner: String = chars[pos + 3..end].iter().collect();
                spans.push(Span::styled(inner, Style::new().bold().italic()));
                pos = end + 3;
                continue;
            }
        }

        if double && (c == '*' || c == '_') {
            let marker: &[char] = &[c, c];
            if let Some(end) = find_str(&chars, pos + 2, marker) {
                if end > pos + 2 {
                    let inner: String = chars[pos + 2..end].iter().collect();
                    spans.push(Span::styled(inner, Style::new().bold()));
                    pos = end + 2;
                    continue;
                }
            }
        }

        if (c == '*' || c == '_') && !double && pos + 1 < chars.len() {
            if let Some(end) = find_char(&chars, pos + 1, c) {
                if end > pos + 1 {
                    let inner: String = chars[pos + 1..end].iter().collect();
                    spans.push(Span::styled(inner, Style::new().italic()));
                    pos = end + 1;
                    continue;
                }
            }
        }

        if c == '`' {
            if double {
                if let Some(end) = find_str(&chars, pos + 2, &['`', '`']) {
                    let inner: String = chars[pos + 2..end].iter().collect();
                    spans.push(Span::styled(inner, Style::new().bg(CODE_BG).fg(CODE_FG)));
                    pos = end + 2;
                    continue;
                }
            } else {
                if let Some(end) = find_char(&chars, pos + 1, '`') {
                    let inner: String = chars[pos + 1..end].iter().collect();
                    spans.push(Span::styled(inner, Style::new().bg(CODE_BG).fg(CODE_FG)));
                    pos = end + 1;
                    continue;
                }
            }
        }

        if c == '[' {
            if let Some(title_end) = find_char(&chars, pos + 1, ']') {
                if title_end + 1 < chars.len() && chars[title_end + 1] == '(' {
                    if let Some(url_end) = find_char(&chars, title_end + 2, ')') {
                        let title: String = chars[pos + 1..title_end].iter().collect();
                        let url: String = chars[title_end + 2..url_end].iter().collect();
                        spans.push(Span::styled(
                            format!("{} ({})", title, url),
                            Style::new().add_modifier(Modifier::UNDERLINED).fg(LINK),
                        ));
                        pos = url_end + 1;
                        continue;
                    }
                }
            }
        }

        let plain_start = pos;
        while pos < chars.len() && !is_marker_start(&chars, pos) {
            pos += 1;
        }
        if pos == plain_start {
            spans.push(Span::raw(c.to_string()));
            pos += 1;
        } else {
            let text: String = chars[plain_start..pos].iter().collect();
            spans.push(Span::raw(text));
        }
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

fn is_marker_start(chars: &[char], pos: usize) -> bool {
    if pos >= chars.len() { return false; }
    let c = chars[pos];
    if c == '\\' { return true; }
    if c == '*' || c == '_' { return true; }
    if c == '`' { return true; }
    if c == '[' { return true; }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bullet_with_bold_and_link() {
        let input = "  \u{2022}   **Waymo** is recalling robotaxis. [Read more](https://example.com)";
        let text = render(input);
        assert_eq!(text.lines.len(), 1, "should be one line");
        let spans = &text.lines[0].spans;
        assert!(spans.len() >= 5, "expected at least 5 spans, got {}", spans.len());
        assert!(spans[0].content.contains('\u{2022}'), "first span should be bullet, got: {:?}", spans[0].content);
        assert_eq!(spans[2].content, "Waymo");
        assert!(spans[2].style.add_modifier == ratatui::style::Modifier::BOLD);
        let last = &spans[spans.len() - 1];
        assert!(last.content.contains("Read more"), "last span should be link, got: {:?}", last.content);
        assert_eq!(last.style.fg, Some(LINK));
    }

    #[test]
    fn test_plain_bold() {
        let input = "Hello **world**!";
        let text = render(input);
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content == "world" && s.style.add_modifier == ratatui::style::Modifier::BOLD));
    }

    #[test]
    fn test_plain_link() {
        let input = "Click [here](https://example.com) now";
        let text = render(input);
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content.contains("here") && s.style.fg == Some(LINK)));
    }

    #[test]
    fn test_fenced_code_block() {
        let input = "Some text\n```\nlet x = 1;\nprintln!(\"{}\", x);\n```\nMore text";
        let text = render(input);
        assert_eq!(text.lines.len(), 5);
        assert!(text.lines[0].spans[0].content.contains("Some text"));
        assert!(text.lines[1].spans[0].content.contains("let x = 1"));
        assert_eq!(text.lines[1].spans[0].style.fg, Some(CODE_FG));
        assert!(text.lines[4].spans[0].content.contains("More text"));
    }

    #[test]
    fn test_fenced_code_with_lang() {
        let input = "```rust\nfn main() {}\n```";
        let text = render(input);
        assert_eq!(text.lines.len(), 1);
        assert!(text.lines[0].spans[0].content.contains("fn main()"));
        assert_eq!(text.lines[0].spans[0].style.fg, Some(CODE_FG));
    }

    #[test]
    fn test_blockquote_no_space() {
        let input = ">quoted text";
        let text = render(input);
        assert!(text.lines[0].spans.iter().any(|s| s.content.contains("quoted")));
    }

    #[test]
    fn test_triple_asterisk_bold_italic() {
        let input = "***bold italic***";
        let text = render(input);
        assert_eq!(text.lines.len(), 1);
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content == "bold italic"
            && s.style.add_modifier.contains(ratatui::style::Modifier::BOLD)
            && s.style.add_modifier.contains(ratatui::style::Modifier::ITALIC)));
    }

    #[test]
    fn test_italic() {
        let input = "some *italic* text";
        let text = render(input);
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content == "italic" && s.style.add_modifier == ratatui::style::Modifier::ITALIC));
    }

    #[test]
    fn test_escaped_asterisk() {
        let input = r"literal \*star\* here";
        let text = render(input);
        let content: String = text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("*star*"), "expected literal asterisks, got: {:?}", content);
    }

    #[test]
    fn test_double_backtick_code() {
        let input = "use ``std::fmt`` here";
        let text = render(input);
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content == "std::fmt" && s.style.fg == Some(CODE_FG)));
    }

    #[test]
    fn test_heading_without_space() {
        let input = "#Title";
        let text = render(input);
        let spans = &text.lines[0].spans;
        assert!(spans[0].style.add_modifier.contains(ratatui::style::Modifier::BOLD));
        assert_eq!(spans[0].style.fg, Some(HEADING));
    }

    #[test]
    fn test_indented_code_block() {
        let input = "    indented code";
        let text = render(input);
        assert_eq!(text.lines[0].spans[0].style.fg, Some(CODE_FG));
    }

    #[test]
    fn test_lone_asterisk_passes_through() {
        let input = "text * more text";
        let text = render(input);
        let content: String = text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains('*'), "lone * should pass through");
    }

    #[test]
    fn test_unclosed_bold_outputs_literal() {
        let input = "text **unclosed";
        let text = render(input);
        let content: String = text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("**"), "unclosed ** should render as literal");
    }
}
