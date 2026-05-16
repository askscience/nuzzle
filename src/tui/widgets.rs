use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget, Wrap};
use tui_textarea::TextArea;

// Pastel palette
const ACCENT: Color = Color::Rgb(152, 208, 238);
const DIM: Color = Color::Rgb(147, 147, 155);
const QUESTION: Color = Color::Rgb(180, 190, 218);
const CURSOR: Color = Color::Rgb(238, 238, 238);

fn accent_style() -> Style { Style::new().fg(ACCENT) }
fn accent_bold() -> Style { Style::new().bold().fg(ACCENT) }
fn dim_style() -> Style { Style::new().fg(DIM) }

// ── Header ──

pub struct Header<'a> {
    pub left: &'a str,
    pub right: &'a str,
}

impl Widget for Header<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = dim_style();
        let sep = "─".repeat(area.width as usize);
        let lw = area.width as usize;
        let left = if self.left.len() > lw { &self.left[..lw] } else { self.left };
        buf.set_string(area.x, area.y, left, style);
        if !self.right.is_empty() {
            let rw = self.right.len().min(lw.saturating_sub(left.len() + 1));
            if rw > 0 {
                let text = &self.right[self.right.len().saturating_sub(rw)..];
                let x = area.x + lw.saturating_sub(rw) as u16;
                buf.set_string(x, area.y, text, style);
            }
        }
        if area.height > 1 {
            buf.set_string(area.x, area.y + 1, &sep, dim_style());
        }
    }
}

// ── Feed list ──

pub struct FeedList<'a> {
    pub feeds: &'a [crate::types::Feed],
    pub selected: usize,
}

impl Widget for FeedList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width.saturating_sub(2) as usize;
        for (i, feed) in self.feeds.iter().enumerate() {
            if i as u16 >= area.height { break; }
            let sel = i == self.selected;
            let style = if sel { accent_bold() } else { dim_style() };
            let prefix = if sel { "▸ " } else { "  " };
            let line = format!("{}{}", prefix, feed.title);
            let trunc: String = line.chars().take(width).collect();
            buf.set_string(area.x + 1, area.y + i as u16, &trunc, style);
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
        let width = area.width.saturating_sub(3) as usize;
        for (i, e) in self.entries.iter().enumerate() {
            if i as u16 >= area.height { break; }
            let sel = i == self.selected;
            let num = format!("{:2}", i + 1);
            let style = if sel { accent_bold() } else if e.is_read { dim_style() } else { Style::new() };
            let title = e.title.as_deref().unwrap_or("(untitled)");
            let line = format!(" {} │ {}", num, title);
            let trunc: String = line.chars().take(width).collect();
            buf.set_string(area.x + 1, area.y + i as u16, &trunc, style);
        }
    }
}

// ── Article view ──

pub struct ArticleView<'a> {
    pub title: Option<&'a str>,
    pub content: Option<&'a str>,
    pub scroll: u16,
}

impl Widget for ArticleView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = self.title.unwrap_or("");
        let content = self.content.unwrap_or("");
        let text = format!("\n  {}\n\n{}", title, content);
        Paragraph::new(text).wrap(Wrap { trim: false }).scroll((self.scroll, 0)).render(area, buf);
    }
}

// ── Ask bar ──

pub struct AskBar<'a> {
    pub input: &'a TextArea<'static>,
    pub spinner: &'a str,
    pub is_streaming: bool,
}

impl Widget for AskBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let prefix = if self.is_streaming { format!("{} ", self.spinner) } else { "⟩ ".to_string() };
        let pstyle = if self.is_streaming { accent_bold() } else { Style::new().fg(QUESTION) };
        buf.set_string(area.x + 1, area.y, &prefix, pstyle);
        let text = self.input.lines().join("");
        let pl = prefix.len() as u16 + 1;
        let max = area.width.saturating_sub(pl + 2) as usize;
        let (display, is_empty) = if text.is_empty() {
            (String::new(), true)
        } else if text.len() > max {
            (format!("..{}", &text[text.len().saturating_sub(max.saturating_sub(2))..]), false)
        } else {
            (text, false)
        };
        let style = if is_empty { dim_style() } else { Style::new() };
        buf.set_string(area.x + pl, area.y, &display, style);
        if !is_empty {
            let cp = self.input.cursor().1;
            let cx = area.x + pl + cp.min(display.len()) as u16;
            buf.set_string(cx, area.y, "▌", Style::new().bold().fg(CURSOR));
        }
    }
}

// ── Nav bar ──

pub struct NavBar<'a> {
    pub text: &'a str,
}

impl Widget for NavBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_string(area.x + 1, area.y, self.text, dim_style());
    }
}

// ── Search ──

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
            let style = if sel { accent_bold() } else { Style::new() };
            let line = format!("{}{}", prefix, title);
            let w = area.width as usize;
            let trunc: String = line.chars().take(w).collect();
            buf.set_string(area.x, area.y + i as u16, &trunc, style);
        }
    }
}

// ── Help ──

pub struct HelpOverlay;

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let help = " Nuzzle · commands\n\n /exit  quit    /feed  feeds    /new  session\n /models  pick model    /model <name>  switch\n\n ↑/↓  navigate    Enter  open\n type in the ask bar below to chat with AI";
        buf.set_string(area.x + 1, area.y, help, dim_style());
    }
}

// ── Loading screen ──

pub struct LoadingScreen<'a> {
    pub spinner: &'a str,
    pub message: &'a str,
}

impl Widget for LoadingScreen<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            "".to_string(), "".to_string(), "".to_string(),
            format!("      {}  N U Z Z L E", self.spinner),
            "".to_string(),
            format!("         {}", self.message),
            "".to_string(), "".to_string(),
        ];
        let h = lines.len() as u16;
        let start_y = area.y + area.height.saturating_sub(h) / 2;
        for (i, line) in lines.iter().enumerate() {
            let x = area.x + area.width.saturating_sub(line.len() as u16) / 2;
            let style = if i == 3 { accent_bold() } else { Style::new() };
            buf.set_string(x, start_y + i as u16, line, style);
        }
    }
}

// ── Model selector ──

pub struct ModelList<'a> {
    pub models: &'a [String],
    pub selected: usize,
    pub current: &'a str,
}

impl Widget for ModelList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let block = Block::default().borders(Borders::ALL).border_type(BorderType::Plain)
            .border_style(accent_style()).title_top(" Models ");
        let inner = block.inner(area);
        block.render(area, buf);
        if self.models.is_empty() {
            buf.set_string(inner.x + 1, inner.y + 1, "No models.", dim_style());
            return;
        }
        for (i, m) in self.models.iter().enumerate() {
            if i as u16 + 2 >= inner.height { break; }
            let sel = i == self.selected;
            let pfx = if sel { "▸ " } else { "  " };
            let mark = if m == self.current { " ←" } else { "" };
            let style = if sel { accent_bold() } else { Style::new() };
            let line = format!("{}{}{}", pfx, m, mark);
            let w = inner.width.saturating_sub(2) as usize;
            let trunc: String = line.chars().take(w).collect();
            buf.set_string(inner.x + 1, inner.y + 2 + i as u16, &trunc, style);
        }
    }
}
