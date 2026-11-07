//! Markdown tabs: a page of the program or any `.md` file, drawn by `markdown` (every paragraph
//! one text of egui), with the search of the tab and the anchors of the headings; a `.txt` file
//! or a file without an extension as plain text. Each tab keeps its place on the page and the
//! selected text when another tab comes in front, and scrolls with the keys.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::doc::{Document, Search, matches_words};
use crate::markdown::{self, Block, Outcome, View};
use crate::settings::PageWidth;
use eframe::egui::{self, Key, Modifiers, text_selection::LabelSelectionState};

/// The keys of the scroll areas of the tabs, one a document.
static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

/// A step of the arrow keys, in points.
const LINE_STEP: f32 = 40.0;

pub struct MdDoc {
    pub title: String,
    pub path: Option<PathBuf>,
    pub text: String,
    /// The blocks of the text, parsed again when the text changes (its hash).
    parsed: Option<(u64, Vec<Block>)>,
    pub search: Search,
    /// The heading to show, by its anchor, and whether to scroll to it at the next frame.
    pub anchor: Option<String>,
    pub jump: bool,
    /// The page of the program the document shows, by its id; a file or a text has none.
    pub page: Option<&'static str>,
    /// Plain text: shown as it is in a monospace font, not as Markdown.
    pub plain: bool,
    /// The key of the scroll area of the tab.
    key: u64,
    /// The last pass of egui that drew the tab: a gap means it came back to the front.
    last_pass: u64,
    /// The scroll offset, the height of the view and of the page at the last frame.
    offset: f32,
    view_height: f32,
    content_height: f32,
    /// The selected text of the page as it was at the start of its last frame.
    selection: Option<LabelSelectionState>,
}

impl MdDoc {
    pub fn new(title: impl Into<String>, path: Option<PathBuf>, text: String) -> Self {
        Self {
            title: title.into(),
            path,
            text,
            parsed: None,
            search: Search::default(),
            anchor: None,
            jump: false,
            page: None,
            plain: false,
            key: NEXT_KEY.fetch_add(1, Ordering::Relaxed),
            last_pass: 0,
            offset: 0.0,
            view_height: 0.0,
            content_height: 0.0,
            selection: None,
        }
    }

    /// The scroll offset of the page at the last frame.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// A text file shown as it is, in a monospace font.
    pub fn plain(title: impl Into<String>, path: Option<PathBuf>, text: String) -> Self {
        let mut doc = Self::new(title, path, text);
        doc.plain = true;
        doc
    }

    /// A page of the program, opened once by its id.
    pub fn page(id: &'static str, title: impl Into<String>, text: String) -> Self {
        let mut doc = Self::new(title, None, text);
        doc.page = Some(id);
        doc
    }
}

impl Document for MdDoc {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn search(&mut self) -> Option<&mut Search> {
        Some(&mut self.search)
    }
}

/// The anchor of a heading as GitHub writes it: lower case, spaces as hyphens, other punctuation
/// left out.
pub fn slug(heading: &str) -> String {
    let mut out = String::new();
    for c in heading.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' {
            out.push('-');
        } else if c == '_' {
            out.push('_');
        }
    }
    out
}

/// The blocks of a page that hold every word of a search, each with the heading above it: the
/// paragraphs, lists and code blocks between blank lines.
pub fn matching_blocks(text: &str, words: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut heading = String::new();
    let mut in_code = false;
    let mut block = String::new();
    let flush = |block: &mut String, heading: &str, out: &mut Vec<String>| {
        if !block.trim().is_empty() && matches_words(block, words) {
            if heading.is_empty() || block.trim_start().starts_with('#') {
                out.push(block.clone());
            } else {
                out.push(format!("{heading}\n\n{block}"));
            }
        }
        block.clear();
    };
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
        }
        if !in_code && line.trim().is_empty() {
            flush(&mut block, &heading, &mut out);
            continue;
        }
        if !in_code && line.starts_with('#') {
            flush(&mut block, &heading, &mut out);
            heading = format!("#### {}", line.trim_start_matches('#').trim());
        }
        block.push_str(line);
        block.push('\n');
    }
    flush(&mut block, &heading, &mut out);
    out
}

/// The new scroll offset of a key of the page, when one was pressed: the arrows by a step, the
/// page keys by the view less a step, Home and End to the ends.
fn key_offset(ctx: &egui::Context, offset: f32, view: f32, content: f32) -> Option<f32> {
    if ctx.egui_wants_keyboard_input() {
        return None;
    }
    let page = (view - LINE_STEP).max(LINE_STEP);
    let end = (content - view).max(0.0);
    let wanted = ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            Some(offset + LINE_STEP)
        } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            Some(offset - LINE_STEP)
        } else if i.consume_key(Modifiers::NONE, Key::PageDown) {
            Some(offset + page)
        } else if i.consume_key(Modifiers::NONE, Key::PageUp) {
            Some(offset - page)
        } else if i.consume_key(Modifiers::NONE, Key::Home)
            || i.consume_key(Modifiers::CTRL, Key::Home)
        {
            Some(0.0)
        } else if i.consume_key(Modifiers::NONE, Key::End)
            || i.consume_key(Modifiers::CTRL, Key::End)
        {
            Some(end)
        } else {
            None
        }
    })?;
    Some(wanted.clamp(0.0, end))
}

/// Plain text in a monospace font, folded to the width of the page when the lines wrap.
fn plain_text(ui: &mut egui::Ui, text: &str, view: &View) {
    let label = egui::Label::new(egui::RichText::new(text).monospace()).selectable(true);
    let label = if view.width.is_finite() {
        label.wrap()
    } else {
        label.extend()
    };
    ui.scope(|ui| {
        ui.set_max_width(view.page);
        ui.add(label);
    });
}

/// The page, as wide as `page` says; `wrap` folds its lines to that width, else the view scrolls
/// sideways. Its tables fill the width of the page; a table or a block of code wider than the
/// view scrolls sideways either way. A link to a heading of the page brings it into view; any
/// other link is handed to egui, and the shell takes it from there (a page or a text file opens
/// in the program, another link is asked first). The arrows, the page keys, Home and End scroll
/// the page while no text field has the focus.
pub fn md_view(ui: &mut egui::Ui, doc: &mut MdDoc, wrap: bool, page: PageWidth) {
    let words = doc.search.words();
    let hash = crate::session::fnv(&doc.text);
    if !doc.plain && doc.parsed.as_ref().is_none_or(|(h, _)| *h != hash) {
        doc.parsed = Some((hash, markdown::parse(&doc.text)));
    }
    // The tab came back to the front: the text selected in it before is selected again.
    let ctx = ui.ctx().clone();
    let pass = ctx.cumulative_pass_nr();
    let back = doc.last_pass != 0 && doc.last_pass + 1 < pass;
    doc.last_pass = pass;
    if back && let Some(saved) = doc.selection.take() {
        ctx.with_plugin::<LabelSelectionState, _>(|s| *s = saved);
    }
    doc.selection = ctx.with_plugin::<LabelSelectionState, _>(|s| s.clone());
    let mut out = Outcome::default();
    let mut area = egui::ScrollArea::both()
        .id_salt(("md-page", doc.key))
        .auto_shrink(false);
    if let Some(offset) = key_offset(&ctx, doc.offset, doc.view_height, doc.content_height) {
        area = area.vertical_scroll_offset(offset);
    }
    let shown = area.show(ui, |ui| {
        // A limit: so many pixels or characters at most, the view when it is narrower.
        let view_width = ui.available_width();
        let page = match page {
            PageWidth::View => view_width,
            PageWidth::Pixels(n) => (n as f32).min(view_width),
            PageWidth::Characters(n) => {
                let font = crate::style::font(ui, 13.0);
                (n as f32 * ui.fonts_mut(|f| f.glyph_width(&font, 'n'))).min(view_width)
            }
        };
        let view = View {
            width: if wrap { page } else { f32::INFINITY },
            page,
            anchor: if doc.jump { doc.anchor.clone() } else { None },
        };
        if words.is_empty() {
            if doc.plain {
                plain_text(ui, &doc.text, &view);
            } else if let Some((_, blocks)) = &doc.parsed {
                markdown::show(ui, blocks, &view, &mut out);
            }
            return;
        }
        let places = matching_blocks(&doc.text, &words);
        ui.weak(crate::i18n::trf(
            "{n} places hold the search",
            &[("n", &places.len())],
        ));
        for place in &places {
            ui.separator();
            if doc.plain {
                plain_text(ui, place, &view);
            } else {
                markdown::show(ui, &markdown::parse(place), &view, &mut out);
            }
        }
    });
    doc.offset = shown.state.offset.y;
    doc.view_height = shown.inner_rect.height();
    doc.content_height = shown.content_size.y;
    if doc.jump {
        doc.jump = false;
    }
    match out.link {
        Some(link) if link.starts_with('#') => {
            doc.anchor = Some(link.trim_start_matches('#').to_owned());
            doc.jump = true;
            ui.ctx().request_repaint();
        }
        Some(link) => ui.ctx().open_url(egui::OpenUrl::same_tab(link)),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_finds_the_blocks_with_their_headings() {
        let text = "# Guide\n\nFirst steps here.\n\n## Build\n\nCopy autounattend.xml to the stick.\n\n```\nkeep code\n\nwhole\n```\n";
        let words = vec!["autounattend".to_owned()];
        let blocks = matching_blocks(text, &words);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].starts_with("#### Build"), "{blocks:?}");
        let code = matching_blocks(text, &["whole".to_owned()]);
        assert_eq!(code.len(), 1);
        assert!(code[0].contains("keep code"), "a code block stays whole");
    }

    #[test]
    fn anchors_are_the_slugs_of_the_headings() {
        assert_eq!(slug(" Microsoft Edge"), "microsoft-edge");
        assert_eq!(slug("Table of removed apps"), "table-of-removed-apps");
        assert_eq!(slug("7. Work status"), "7-work-status");
    }
}
