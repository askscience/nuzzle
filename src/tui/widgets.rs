use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget, Wrap};
use tui_textarea::TextArea;

// ── Feed list sidebar ──

pub struct FeedList<'a> {
    pub feeds: &'a [crate::types::Feed],
    pub selected: usize,
}

impl Widget for FeedList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (i, feed) in self.feeds.iter().enumerate() {
            if i as u16 >= area.height { break; }
            let sel = i == self.selected;
            let prefix = if sel { "▸ " } else { "  " };
            let style = if sel { ratatui::style::Style::new().bold().cyan() } else { ratatui::style::Style::new() };
            let line = format!("{}{}", prefix, feed.title);
            let trunc = if line.len() > area.width as usize { &line[..area.width as usize] } else { &line };
            buf.set_string(area.x, area.y + i as u16, trunc, style);
        }
    }
}

// ── Article list ──

pub struct ArticleList<'a> {
    pub entries: &'a [crate::types::Entry],
    pub selected: usize,
}

impl Widget for ArticleList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (i, entry) in self.entries.iter().enumerate() {
            if i as u16 >= area.height { break; }
            let sel = i == self.selected;
            let prefix = if sel { "▸ " } else { "  " };
            let style = if sel { ratatui::style::Style::new().bold().cyan() } else if entry.is_read { ratatui::style::Style::new().dim() } else { ratatui::style::Style::new() };
            let title = entry.title.as_deref().unwrap_or("(untitled)");
            let line = format!("{}{}", prefix, title);
            let trunc = if line.len() > area.width as usize { &line[..area.width as usize] } else { &line };
            buf.set_string(area.x, area.y + i as u16, trunc, style);
        }
    }
}

// ── Article reader view ──

pub struct ArticleView<'a> {
    pub title: Option<&'a str>,
    pub content: Option<&'a str>,
    pub scroll: u16,
}

impl Widget for ArticleView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = self.title.unwrap_or("");
        let content = self.content.unwrap_or("");
        let text = format!("\n{}\n\n{}", title, content);
        Paragraph::new(text).wrap(Wrap { trim: false }).scroll((self.scroll, 0)).render(area, buf);
    }
}

// ── Status bar ──

pub struct StatusBar<'a> {
    pub items: &'a [(&'a str, &'a str)],
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text: Vec<String> = self.items.iter().map(|(k, d)| format!("{} {}  ", k, d)).collect();
        buf.set_string(area.x, area.y, &text.join(""), ratatui::style::Style::new().dim());
    }
}

pub struct MiniBar<'a> {
    pub text: &'a str,
}

impl Widget for MiniBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_string(area.x, area.y, self.text, ratatui::style::Style::new().dim());
    }
}

// ── Ask bar (always visible, includes spinner when streaming) ──

pub struct AskBar<'a> {
    pub input: &'a TextArea<'static>,
    pub spinner: &'a str,
    pub is_streaming: bool,
}

impl Widget for AskBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let prefix = if self.is_streaming {
            format!("{} │ ", self.spinner)
        } else {
            "⟩ ".to_string()
        };
        let pstyle = if self.is_streaming {
            ratatui::style::Style::new().bold().cyan()
        } else {
            ratatui::style::Style::new().dim().blue()
        };
        buf.set_string(area.x, area.y, &prefix, pstyle);

        let text = self.input.lines().join("");
        let text_len = text.len();
        let pl = prefix.len() as u16;
        let (display, is_empty) = if text.is_empty() {
            let ph = self.input.placeholder_text();
            (if ph.is_empty() { String::new() } else { ph.to_string() }, true)
        } else {
            let max = area.width.saturating_sub(pl + 1) as usize;
            if text_len > max {
                (format!("..{}", &text[text_len.saturating_sub(max.saturating_sub(2))..]), false)
            } else {
                (text, false)
            }
        };
        let style = if is_empty { ratatui::style::Style::new().dim() } else { ratatui::style::Style::new() };
        buf.set_string(area.x + pl, area.y, &display, style);
        if !is_empty {
            let cp = self.input.cursor().1;
            let cx = area.x + pl + cp.min(text_len) as u16;
            buf.set_string(cx, area.y, "▌", ratatui::style::Style::new().bold().white());
        }
    }
}

// ── Search results ──

pub struct SearchResults<'a> {
    pub results: &'a [(String, String)],
    pub selected: usize,
}

impl Widget for SearchResults<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (i, (title, _)) in self.results.iter().enumerate() {
            if i as u16 >= area.height { break; }
            let sel = i == self.selected;
            let prefix = if sel { "▸ " } else { "  " };
            let style = if sel { ratatui::style::Style::new().bold().cyan() } else { ratatui::style::Style::new() };
            let line = format!("{}{}", prefix, title);
            let trunc = if line.len() > area.width as usize { &line[..area.width as usize] } else { &line };
            buf.set_string(area.x, area.y + i as u16, trunc, style);
        }
    }
}

// ── Help panel ──

pub struct HelpOverlay;

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let help = vec![
            " Nuzzle — keyboard reference",
            "",
            " j/k      Navigate              Enter    Open / select",
            " r        Refresh feeds         s        Summarize article",
            " /        Semantic search       *        Toggle star",
            " t        Show tags             d        Daily AI digest",
            " h        Show highlights       e        Export → .md",
            " Tab      Show help             Esc      Go back",
            " q        Quit",
            "",
            " Ask bar at bottom: type any question, press Enter.",
            " The AI searches your feeds and streams a response.",
        ].join("\n");
        buf.set_string(area.x, area.y, &help, ratatui::style::Style::new().dim());
    }
}

pub struct ModelList<'a> {
    pub models: &'a [String],
    pub selected: usize,
    pub current: &'a str,
}

impl Widget for ModelList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(ratatui::style::Style::new().cyan())
            .title_top(" Models — j/k select, Enter choose, Esc cancel ");
        let inner = block.inner(area);
        block.render(area, buf);

        let max_items = inner.height as usize;
        let header = format!("  Current: {}  ", self.current);
        buf.set_string(inner.x + 1, inner.y, &header, ratatui::style::Style::new().dim());

        if self.models.is_empty() {
            buf.set_string(inner.x + 1, inner.y + 2, "  No models found.", ratatui::style::Style::new().dim());
            return;
        }

        for (i, model) in self.models.iter().enumerate() {
            let row = i + 2;
            if row >= max_items { break; }
            let sel = i == self.selected;
            let prefix = if sel { "▸ " } else { "  " };
            let mark = if model == self.current { " ←" } else { "" };
            let style = if sel {
                ratatui::style::Style::new().bold().cyan()
            } else {
                ratatui::style::Style::new()
            };
            let line = format!("{}{}{}", prefix, model, mark);
            let width = inner.width.saturating_sub(2) as usize;
            let trunc = if line.len() > width { &line[..width] } else { &line };
            buf.set_string(inner.x + 1, inner.y + row as u16, trunc, style);
        }
    }
}
