//! Markdown drawn by WW: a page is parsed (pulldown-cmark) into blocks, and every paragraph, heading
//! and cell is one text of egui (a `LayoutJob`), so its words, its code and its links share one
//! baseline, wrap at the width the view gives and scroll sideways without it. Lists, quotes,
//! blocks of code and tables are drawn here too, in the colours and fonts of the theme.

use std::ops::Range;
use std::sync::Arc;

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontId, Galley, Margin, Sense, Stroke, TextFormat,
    text::LayoutJob,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};

use crate::i18n::tr;
use crate::{icons, style};

/// A run of text in one style.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub strong: bool,
    pub emphasis: bool,
    pub code: bool,
    pub strike: bool,
    pub link: Option<String>,
}

/// The text of a paragraph, a heading or a cell: its runs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inline {
    pub spans: Vec<Span>,
}

impl Inline {
    /// The text without its styles.
    pub fn plain(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    fn is_blank(&self) -> bool {
        self.spans.iter().all(|s| s.text.trim().is_empty())
    }

    /// Adds text in a style, joined to the last run when the style is the same.
    fn push(&mut self, text: &str, style: &Span) {
        if let Some(last) = self.spans.last_mut()
            && last.strong == style.strong
            && last.emphasis == style.emphasis
            && last.code == style.code
            && last.strike == style.strike
            && last.link == style.link
        {
            last.text.push_str(text);
            return;
        }
        self.spans.push(Span {
            text: text.to_owned(),
            ..style.clone()
        });
    }
}

/// A block of a page.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(u8, Inline),
    Paragraph(Inline),
    Code(String),
    /// A list: the number of its first item when it is ordered, its items.
    List(Option<u64>, Vec<Vec<Block>>),
    Quote(Vec<Block>),
    /// A table: its rows, the first one the head.
    Table(Vec<Vec<Inline>>),
    Rule,
}

/// What a start of the parser opened, closed by its end.
enum Open {
    Paragraph,
    Heading(u8),
    Code,
    List,
    Item,
    Quote,
    Table,
    Row,
    Cell,
    Strong,
    Emphasis,
    Strike,
    Link,
    Other,
}

/// A container of blocks being filled.
enum Frame {
    Blocks(Vec<Block>),
    List(Option<u64>, Vec<Vec<Block>>),
    Table(Vec<Vec<Inline>>, Vec<Inline>),
}

struct Builder {
    frames: Vec<Frame>,
    open: Vec<Open>,
    inline: Option<Inline>,
    code: Option<String>,
    style: Span,
    links: Vec<String>,
}

impl Builder {
    fn push(&mut self, block: Block) {
        if let Some(Frame::Blocks(blocks)) = self.frames.last_mut() {
            blocks.push(block);
        }
    }

    /// The text collected outside a paragraph (the items of a tight list) as a paragraph.
    fn flush(&mut self) {
        if let Some(inline) = self.inline.take()
            && !inline.is_blank()
        {
            self.push(Block::Paragraph(inline));
        }
    }

    fn text(&mut self, text: &str) {
        if let Some(code) = &mut self.code {
            code.push_str(text);
            return;
        }
        let style = Span {
            link: self.links.last().cloned(),
            ..self.style.clone()
        };
        self.inline
            .get_or_insert_with(Inline::default)
            .push(text, &style);
    }

    fn start(&mut self, tag: Tag<'_>) {
        let open = match tag {
            Tag::Paragraph => {
                self.flush();
                self.inline = Some(Inline::default());
                Open::Paragraph
            }
            Tag::Heading { level, .. } => {
                self.flush();
                self.inline = Some(Inline::default());
                Open::Heading(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                })
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code = Some(String::new());
                Open::Code
            }
            Tag::List(first) => {
                self.flush();
                self.frames.push(Frame::List(first, Vec::new()));
                Open::List
            }
            Tag::Item => {
                self.frames.push(Frame::Blocks(Vec::new()));
                Open::Item
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.frames.push(Frame::Blocks(Vec::new()));
                Open::Quote
            }
            Tag::Table(_) => {
                self.flush();
                self.frames.push(Frame::Table(Vec::new(), Vec::new()));
                Open::Table
            }
            Tag::TableHead | Tag::TableRow => Open::Row,
            Tag::TableCell => {
                self.inline = Some(Inline::default());
                Open::Cell
            }
            Tag::Strong => {
                self.style.strong = true;
                Open::Strong
            }
            Tag::Emphasis => {
                self.style.emphasis = true;
                Open::Emphasis
            }
            Tag::Strikethrough => {
                self.style.strike = true;
                Open::Strike
            }
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                self.links.push(dest_url.to_string());
                Open::Link
            }
            _ => Open::Other,
        };
        self.open.push(open);
    }

    fn end(&mut self) {
        let Some(open) = self.open.pop() else {
            return;
        };
        match open {
            Open::Paragraph => self.flush(),
            Open::Heading(level) => {
                let inline = self.inline.take().unwrap_or_default();
                self.push(Block::Heading(level, inline));
            }
            Open::Code => {
                let mut code = self.code.take().unwrap_or_default();
                while code.ends_with('\n') {
                    code.pop();
                }
                self.push(Block::Code(code));
            }
            Open::List => {
                if let Some(Frame::List(first, items)) = self.frames.pop() {
                    self.push(Block::List(first, items));
                }
            }
            Open::Item => {
                self.flush();
                if let Some(Frame::Blocks(blocks)) = self.frames.pop()
                    && let Some(Frame::List(_, items)) = self.frames.last_mut()
                {
                    items.push(blocks);
                }
            }
            Open::Quote => {
                self.flush();
                if let Some(Frame::Blocks(blocks)) = self.frames.pop() {
                    self.push(Block::Quote(blocks));
                }
            }
            Open::Table => {
                if let Some(Frame::Table(rows, _)) = self.frames.pop() {
                    self.push(Block::Table(rows));
                }
            }
            Open::Row => {
                if let Some(Frame::Table(rows, row)) = self.frames.last_mut() {
                    rows.push(std::mem::take(row));
                }
            }
            Open::Cell => {
                let cell = self.inline.take().unwrap_or_default();
                if let Some(Frame::Table(_, row)) = self.frames.last_mut() {
                    row.push(cell);
                }
            }
            Open::Strong => self.style.strong = false,
            Open::Emphasis => self.style.emphasis = false,
            Open::Strike => self.style.strike = false,
            Open::Link => {
                self.links.pop();
            }
            Open::Other => {}
        }
    }
}

/// The blocks of a Markdown text.
pub fn parse(text: &str) -> Vec<Block> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut b = Builder {
        frames: vec![Frame::Blocks(Vec::new())],
        open: Vec::new(),
        inline: None,
        code: None,
        style: Span::default(),
        links: Vec::new(),
    };
    for event in Parser::new_ext(text, options) {
        match event {
            Event::Start(tag) => b.start(tag),
            Event::End(_) => b.end(),
            Event::Text(t) => b.text(&t),
            Event::Code(t) => {
                let style = Span {
                    code: true,
                    link: b.links.last().cloned(),
                    ..b.style.clone()
                };
                b.inline
                    .get_or_insert_with(Inline::default)
                    .push(&t, &style);
            }
            Event::SoftBreak => b.text(" "),
            Event::HardBreak => b.text("\n"),
            Event::Rule => {
                b.flush();
                b.push(Block::Rule);
            }
            Event::TaskListMarker(done) => b.text(if done { "[x] " } else { "[ ] " }),
            Event::FootnoteReference(name) => b.text(&format!("[{name}]")),
            _ => {}
        }
    }
    b.flush();
    match b.frames.into_iter().next() {
        Some(Frame::Blocks(blocks)) => blocks,
        _ => Vec::new(),
    }
}

/// How a page is drawn.
pub struct View {
    /// The width of a text: the page, or infinite when the lines do not wrap.
    pub width: f32,
    /// The width of the page (`Settings::page_width`): its tables fill it.
    pub page: f32,
    /// The heading to bring into view, by its anchor.
    pub anchor: Option<String>,
}

/// What a drawn page asks for: a link clicked, whether the heading of the anchor was found.
#[derive(Default)]
pub struct Outcome {
    pub link: Option<String>,
    pub revealed: bool,
}

/// The fonts and colours of a page.
struct Look {
    body: FontId,
    strong: FontId,
    code: FontId,
    text: Color32,
    link: Color32,
    code_bg: Color32,
    border: Color32,
    head_bg: Color32,
    stripe: Color32,
}

impl Look {
    fn of(ui: &egui::Ui) -> Self {
        let p = style::palette(ui);
        Self {
            body: style::font(ui, 13.0),
            strong: style::strong_font(ui, 13.0),
            code: style::mono(ui, 12.0),
            text: ui.visuals().text_color(),
            link: p.accent,
            code_bg: p.code_bg,
            border: p.border,
            head_bg: p.card2,
            stripe: p.hover,
        }
    }

    fn format(&self, span: &Span, base: &FontId, strong: &FontId) -> TextFormat {
        let font_id = if span.code {
            self.code.clone()
        } else if span.strong {
            strong.clone()
        } else {
            base.clone()
        };
        let color = if span.link.is_some() {
            self.link
        } else {
            self.text
        };
        TextFormat {
            font_id,
            color,
            background: if span.code {
                self.code_bg
            } else {
                Color32::TRANSPARENT
            },
            italics: span.emphasis,
            underline: if span.link.is_some() {
                Stroke::new(1.0, self.link)
            } else {
                Stroke::NONE
            },
            strikethrough: if span.strike {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            },
            valign: Align::BOTTOM,
            ..Default::default()
        }
    }
}

/// The text of an inline laid out at a width, with the ranges of characters of its links.
fn layout(
    ui: &egui::Ui,
    look: &Look,
    inline: &Inline,
    base: &FontId,
    strong: &FontId,
    width: f32,
) -> (Arc<Galley>, Vec<(Range<usize>, String)>) {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut links = Vec::new();
    let mut chars = 0;
    for span in &inline.spans {
        let n = span.text.chars().count();
        job.append(&span.text, 0.0, look.format(span, base, strong));
        if let Some(link) = &span.link {
            links.push((chars..chars + n, link.clone()));
        }
        chars += n;
    }
    (ui.fonts_mut(|f| f.layout_job(job)), links)
}

/// A text of the page: selectable, its links clickable.
fn text(
    ui: &mut egui::Ui,
    look: &Look,
    inline: &Inline,
    fonts: (&FontId, &FontId),
    width: f32,
    out: &mut Outcome,
) -> egui::Rect {
    let (galley, links) = layout(ui, look, inline, fonts.0, fonts.1, width);
    let response = ui.add(
        egui::Label::new(galley.clone())
            .selectable(true)
            .sense(Sense::click()),
    );
    if !links.is_empty()
        && let Some(pos) = response.hover_pos()
    {
        let index: usize = galley.cursor_from_pos(pos - response.rect.min).index.into();
        if let Some((_, url)) = links.iter().find(|(r, _)| r.contains(&index)) {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            if response.clicked() {
                out.link = Some(url.clone());
            }
            let rect = response.rect;
            response.on_hover_text_at_pointer(url.as_str());
            return rect;
        }
    }
    response.rect
}

/// The size of a heading by its level.
fn heading_size(level: u8) -> f32 {
    match level {
        1 => 20.0,
        2 => 17.0,
        3 => 15.0,
        _ => 13.0,
    }
}

/// The blocks of a page at a width.
pub fn show(ui: &mut egui::Ui, blocks: &[Block], view: &View, out: &mut Outcome) {
    let look = Look::of(ui);
    ui.spacing_mut().item_spacing.y = 0.0;
    draw_blocks(ui, &look, blocks, view.width, view, out);
}

fn draw_blocks(
    ui: &mut egui::Ui,
    look: &Look,
    blocks: &[Block],
    width: f32,
    view: &View,
    out: &mut Outcome,
) {
    for (i, block) in blocks.iter().enumerate() {
        match block {
            Block::Heading(level, inline) => {
                if i > 0 {
                    ui.add_space(style::size(ui, 12.0));
                }
                let size = heading_size(*level);
                let base = if *level >= 4 {
                    style::strong_font(ui, size)
                } else {
                    style::font(ui, size)
                };
                let strong = style::strong_font(ui, size);
                let rect = text(ui, look, inline, (&base, &strong), width, out);
                if view.anchor.as_deref() == Some(crate::mdview::slug(&inline.plain()).as_str())
                    && !out.revealed
                {
                    ui.scroll_to_rect(rect, Some(Align::Min));
                    out.revealed = true;
                }
                ui.add_space(style::size(ui, 6.0));
            }
            Block::Paragraph(inline) => {
                text(ui, look, inline, (&look.body, &look.strong), width, out);
                ui.add_space(style::size(ui, 8.0));
            }
            Block::Code(code) => {
                code_block(ui, look, code, width);
                ui.add_space(style::size(ui, 8.0));
            }
            Block::List(first, items) => {
                list(ui, look, *first, items, width, view, out);
                ui.add_space(style::size(ui, 4.0));
            }
            Block::Quote(inner) => {
                let indent = style::size(ui, 14.0);
                let shown = egui::Frame::new()
                    .inner_margin(Margin {
                        left: indent as i8,
                        ..Margin::ZERO
                    })
                    .show(ui, |ui| {
                        draw_blocks(ui, look, inner, width - indent, view, out);
                    });
                let rect = shown.response.rect;
                ui.painter().vline(
                    rect.left() + 3.0,
                    rect.top()..=rect.bottom() - style::size(ui, 8.0),
                    Stroke::new(3.0, look.border),
                );
            }
            Block::Table(rows) => {
                table(ui, look, rows, view, out, i);
                ui.add_space(style::size(ui, 8.0));
            }
            Block::Rule => {
                // As long as the page, not the view.
                let gap = style::size(ui, 6.0);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(width.min(view.page), 2.0 * gap + 1.0),
                    Sense::hover(),
                );
                ui.painter().hline(
                    rect.x_range(),
                    rect.center().y,
                    Stroke::new(1.0, look.border),
                );
            }
        }
    }
}

/// A list: the marker of every item in a column of its own, the blocks of the item beside it,
/// the first line of both on one baseline.
fn list(
    ui: &mut egui::Ui,
    look: &Look,
    first: Option<u64>,
    items: &[Vec<Block>],
    width: f32,
    view: &View,
    out: &mut Outcome,
) {
    let marker_width = style::size(ui, if first.is_some() { 26.0 } else { 18.0 });
    for (n, item) in items.iter().enumerate() {
        let marker = match first {
            Some(start) => format!("{}.", start + n as u64),
            None => "\u{2022}".to_owned(),
        };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let galley = ui.fonts_mut(|f| f.layout_no_wrap(marker, look.body.clone(), look.text));
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(marker_width, galley.size().y), Sense::hover());
            let x = rect.right() - galley.size().x - style::size(ui, 6.0);
            ui.painter()
                .galley(egui::pos2(x, rect.top()), galley, look.text);
            ui.vertical(|ui| {
                draw_blocks(ui, look, item, width - marker_width, view, out);
            });
        });
    }
}

/// A block of code: the background of the theme, the code wrapped or not as the page, a copy
/// button.
fn code_block(ui: &mut egui::Ui, look: &Look, code: &str, width: f32) {
    let margin = style::size(ui, 8.0);
    let button = style::size(ui, 26.0);
    let wraps = width.is_finite();
    egui::Frame::new()
        .fill(look.code_bg)
        .corner_radius(4)
        .inner_margin(Margin::symmetric(margin as i8, (margin * 0.75) as i8))
        .show(ui, |ui| {
            let inner = width - 2.0 * margin;
            if wraps {
                ui.set_width(inner);
            }
            ui.horizontal_top(|ui| {
                let mut job = LayoutJob::single_section(
                    code.to_owned(),
                    TextFormat::simple(look.code.clone(), look.text),
                );
                job.wrap.max_width = if wraps { inner - button } else { f32::INFINITY };
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                ui.add(egui::Label::new(galley).selectable(true));
                ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                    if ui
                        .small_button(icons::CLIPBOARD)
                        .on_hover_text(tr("Copy"))
                        .clicked()
                    {
                        ui.ctx().copy_text(code.to_owned());
                    }
                });
            });
        });
}

/// The space between the columns of a table, the padding of a cell.
const TABLE_GAP: f32 = 12.0;
const CELL_PAD: f32 = 4.0;

/// The widths of the columns of a table of `width`: shared by the length of the longest cell of
/// each column (`lengths`, 4 to 60 characters count), none narrower than its longest word
/// (`least`), which never breaks; the table is wider than `width` when the words do not fit.
pub fn column_widths(lengths: &[usize], least: &[f32], width: f32) -> Vec<f32> {
    let cols = lengths.len();
    let weights: Vec<f32> = lengths.iter().map(|n| (*n).clamp(4, 60) as f32).collect();
    let free = width - TABLE_GAP * (cols as f32 - 1.0);
    // Columns whose share is below their least width take it; the rest share what is left.
    let mut fixed = vec![false; cols];
    loop {
        let left = free
            - (0..cols)
                .filter(|c| fixed[*c])
                .map(|c| least[c])
                .sum::<f32>();
        let total: f32 = (0..cols).filter(|c| !fixed[*c]).map(|c| weights[c]).sum();
        let mut changed = false;
        for c in 0..cols {
            if !fixed[c] && (total <= 0.0 || left * weights[c] / total < least[c]) {
                fixed[c] = true;
                changed = true;
            }
        }
        if !changed {
            return (0..cols)
                .map(|c| {
                    if fixed[c] {
                        least[c].ceil()
                    } else {
                        // Whole points, so that the columns never add up to a hair more than
                        // the table and the page does not scroll sideways for it.
                        (left * weights[c] / total).floor()
                    }
                })
                .collect();
        }
    }
}

/// The width of the widest word of a cell, every piece of it in the font it is drawn with: a
/// word may run over runs of several styles (`(`, then code, then `,`).
fn widest_word(ui: &egui::Ui, look: &Look, cell: &Inline, head: bool) -> f32 {
    let mut widest = 0.0_f32;
    let mut current = 0.0_f32;
    for span in &cell.spans {
        let font = if span.code {
            &look.code
        } else if head || span.strong {
            &look.strong
        } else {
            &look.body
        };
        for (i, piece) in span.text.split(char::is_whitespace).enumerate() {
            if i > 0 {
                widest = widest.max(current);
                current = 0.0;
            }
            if !piece.is_empty() {
                current += ui.fonts_mut(|f| {
                    f.layout_no_wrap(piece.to_owned(), font.clone(), look.text)
                        .size()
                        .x
                });
            }
        }
    }
    widest.max(current)
}

/// A table: bands of cells of the widths of its columns, every row as high as its highest cell,
/// the head strong.
fn table(
    ui: &mut egui::Ui,
    look: &Look,
    rows: &[Vec<Inline>],
    view: &View,
    out: &mut Outcome,
    n: usize,
) {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    // The page less the frame of the table: its margin, the padding of a band, the stroke, both
    // sides.
    let width = view.page - 2.0 * (CELL_PAD + 6.0 + 1.0);
    let mut lengths = vec![0_usize; cols];
    let mut least = vec![20.0_f32; cols];
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            lengths[c] = lengths[c].max(cell.plain().chars().count());
            // A little room, so that a word as wide as its column is not wrapped.
            least[c] = least[c].max(widest_word(ui, look, cell, r == 0) + 2.0);
        }
    }
    let widths = column_widths(&lengths, &least, width);
    let stroke = Stroke::new(1.0, look.border);
    ui.push_id(("page-table", n), |ui| {
        egui::Frame::new()
            .stroke(stroke)
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                for (r, row) in rows.iter().enumerate() {
                    let fill = if r == 0 {
                        look.head_bg
                    } else if r % 2 == 0 {
                        look.stripe
                    } else {
                        Color32::TRANSPARENT
                    };
                    let band = egui::Frame::new()
                        .fill(fill)
                        .inner_margin(Margin::symmetric(CELL_PAD as i8, 3))
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = TABLE_GAP;
                                for (c, col) in widths.iter().copied().enumerate() {
                                    let empty = Inline::default();
                                    let cell = row.get(c).unwrap_or(&empty);
                                    let base = if r == 0 { &look.strong } else { &look.body };
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(col, 0.0),
                                        egui::Layout::top_down(Align::Min),
                                        |ui| {
                                            ui.set_width(col);
                                            text(ui, look, cell, (base, &look.strong), col, out);
                                        },
                                    );
                                }
                            });
                        });
                    let rect = band.response.rect;
                    if r + 1 < rows.len() {
                        ui.painter().hline(rect.x_range(), rect.bottom(), stroke);
                    }
                    let mut x = rect.left() + CELL_PAD;
                    for col in &widths[..cols - 1] {
                        x += col + TABLE_GAP;
                        ui.painter()
                            .vline(x - TABLE_GAP / 2.0, rect.y_range(), stroke);
                    }
                }
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str) -> Span {
        Span {
            text: text.to_owned(),
            ..Span::default()
        }
    }

    #[test]
    fn a_page_parses_into_its_blocks() {
        let text = "# Title\n\nSome **strong** and `code` with a [link](a.md#b).\n\n\
                    - one\n- two `x`\n\n1. first\n\n   more\n2. second\n\n> quoted\n\n\
                    ```\ncode\nlines\n```\n\n| A | B |\n|---|---|\n| 1 | two words |\n\n---\n";
        let blocks = parse(text);
        assert!(matches!(&blocks[0], Block::Heading(1, h) if h.plain() == "Title"));
        let Block::Paragraph(p) = &blocks[1] else {
            panic!("{:?}", blocks[1]);
        };
        assert_eq!(p.plain(), "Some strong and code with a link.");
        assert!(p.spans.iter().any(|s| s.strong && s.text == "strong"));
        assert!(p.spans.iter().any(|s| s.code && s.text == "code"));
        assert!(
            p.spans
                .iter()
                .any(|s| s.link.as_deref() == Some("a.md#b") && s.text == "link")
        );
        let Block::List(None, items) = &blocks[2] else {
            panic!("{:?}", blocks[2]);
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[1][0], Block::Paragraph(p) if p.plain() == "two x"));
        let Block::List(Some(1), items) = &blocks[3] else {
            panic!("{:?}", blocks[3]);
        };
        assert_eq!(items[0].len(), 2, "a loose item keeps its two paragraphs");
        assert!(matches!(&blocks[4], Block::Quote(q) if q.len() == 1));
        assert_eq!(blocks[5], Block::Code("code\nlines".to_owned()));
        let Block::Table(rows) = &blocks[6] else {
            panic!("{:?}", blocks[6]);
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][1].plain(), "two words");
        assert_eq!(blocks[7], Block::Rule);
        let mut joined = Inline::default();
        joined.push("a", &span(""));
        joined.push("b", &span(""));
        assert_eq!(joined.spans.len(), 1);
    }

    #[test]
    fn the_columns_share_the_width_and_keep_their_words() {
        let lengths = [13, 66, 10];
        let least = [104.0, 80.0, 80.0];
        let wide = column_widths(&lengths, &least, 1000.0);
        assert!(wide[1] > wide[0] * 3.0, "{wide:?}");
        let sum: f32 = wide.iter().sum();
        let free = 1000.0 - 2.0 * TABLE_GAP;
        assert!(sum <= free && free - sum < 3.0, "{wide:?}");
        // A narrow table: no column is narrower than its longest word.
        let narrow = column_widths(&lengths, &least, 300.0);
        assert!(narrow[0] >= 104.0 && narrow[2] >= 80.0, "{narrow:?}");
        assert!(
            narrow.iter().zip(least).all(|(w, l)| *w >= l - 0.01),
            "{narrow:?}"
        );
    }
}
