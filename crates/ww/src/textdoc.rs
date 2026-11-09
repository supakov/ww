//! A text file in a tab: the text editor of egui with its undo, the matches of the search, the
//! position of the cursor, the draft of unsaved changes, the save with the line ends the file
//! came with. Page Up and Page Down move the cursor by the height of the view (with Shift they
//! select), and the editor has the focus again when its tab comes back to the front.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::clock::Instant;
use crate::i18n::{tr, trf};
use crate::{Draft, Search, dialogs, session, style};
use eframe::egui::{
    self, Galley, Key, Modifiers, text::CCursor, text::CCursorRange, text_edit::TextEditState,
};

/// A draft is written this long after the last change.
const DRAFT_DELAY: Duration = Duration::from_secs(2);

pub struct TextDoc {
    pub text: String,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    /// The file ends its lines with CR LF, kept at the save.
    pub crlf: bool,
    pub search: Search,
    /// The name of the draft file of the unsaved changes.
    pub draft_key: String,
    /// When the text last changed and has no draft yet.
    pub changed: Option<Instant>,
    /// The line and the column of the cursor, from 1.
    pub line: usize,
    pub col: usize,
    /// The characters to select at the next frame: a match of the search.
    select: Option<(usize, usize)>,
    /// The start of the last match, so that the next one follows it.
    last_match: Option<usize>,
    /// The text as laid out at the last frame, for the page keys.
    galley: Option<Arc<Galley>>,
    /// The height of the view at the last frame.
    view_height: f32,
    /// The last pass of egui that drew the tab: a gap means it came back to the front.
    last_pass: Option<u64>,
}

impl TextDoc {
    pub fn new(text: String, path: Option<PathBuf>) -> Self {
        let crlf = text.contains("\r\n");
        let text = if crlf {
            text.replace("\r\n", "\n")
        } else {
            text
        };
        let draft_key = match &path {
            Some(p) => session::key_for_path(p),
            None => session::new_key(),
        };
        Self {
            text,
            path,
            dirty: false,
            crlf,
            search: Search::default(),
            draft_key,
            changed: None,
            line: 1,
            col: 1,
            select: None,
            last_match: None,
            galley: None,
            view_height: 0.0,
            last_pass: None,
        }
    }

    /// The title of the tab: the name of the file, else Untitled.
    pub fn title(&self) -> String {
        match &self.path {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string()),
            None => format!("{}.txt", tr("Untitled")),
        }
    }

    /// The id of the editor widget, one a tab.
    pub fn id(&self) -> egui::Id {
        egui::Id::new(("ww-text", &self.draft_key))
    }

    /// The draft of a modified text: two seconds after the last change, or at once when the
    /// window closes.
    pub fn draft_due(&mut self, all: bool) -> Option<Draft> {
        if !self.dirty {
            return None;
        }
        let due = match self.changed {
            Some(t) => all || t.elapsed() >= DRAFT_DELAY,
            None => false,
        };
        if !due {
            return None;
        }
        self.changed = None;
        Some(Draft {
            key: self.draft_key.clone(),
            path: self.path.clone(),
            document: serde_json::Value::String(self.text.clone()),
        })
    }

    /// Writes the text to a file, with the line ends it came with.
    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        let text = if self.crlf {
            self.text.replace('\n', "\r\n")
        } else {
            self.text.clone()
        };
        dialogs::write_file(path, text.as_bytes())?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        self.changed = None;
        Ok(())
    }

    /// Selects the next (or the previous) match of a text after the last one, around the end,
    /// ignoring case; the start of the match, `None` when the text has none.
    pub fn find(&mut self, needle: &str, forward: bool) -> Option<usize> {
        let hay: Vec<char> = self.text.chars().collect();
        let pat: Vec<char> = needle.chars().collect();
        if pat.is_empty() || pat.len() > hay.len() {
            self.last_match = None;
            return None;
        }
        let same = |a: char, b: char| a.to_lowercase().eq(b.to_lowercase());
        let starts: Vec<usize> = (0..=hay.len() - pat.len())
            .filter(|&i| pat.iter().enumerate().all(|(k, c)| same(hay[i + k], *c)))
            .collect();
        let (first, last) = match (starts.first(), starts.last()) {
            (Some(f), Some(l)) => (*f, *l),
            _ => {
                self.last_match = None;
                return None;
            }
        };
        let next = match (self.last_match, forward) {
            (None, true) => first,
            (None, false) => last,
            (Some(at), true) => starts.iter().copied().find(|&s| s > at).unwrap_or(first),
            (Some(at), false) => starts
                .iter()
                .rev()
                .copied()
                .find(|&s| s < at)
                .unwrap_or(last),
        };
        self.last_match = Some(next);
        self.select = Some((next, next + pat.len()));
        Some(next)
    }

    /// Page Up and Page Down of the focused editor: the cursor goes up or down by the height of
    /// the view, keeping its place in the row; with Shift the text between is selected. The
    /// rows moved, in points, for the scroll of the view.
    fn page_keys(&self, ctx: &egui::Context, id: egui::Id) -> Option<f32> {
        if !ctx.memory(|m| m.has_focus(id)) {
            return None;
        }
        let galley = self.galley.as_ref()?;
        let step = (self.view_height - 2.0 * row_height(galley)).max(row_height(galley));
        // Shift first: a key without modifiers matches a key with Shift too.
        let (down, shift) = ctx.input_mut(|i| {
            if i.consume_key(Modifiers::SHIFT, Key::PageDown) {
                Some((true, true))
            } else if i.consume_key(Modifiers::SHIFT, Key::PageUp) {
                Some((false, true))
            } else if i.consume_key(Modifiers::NONE, Key::PageDown) {
                Some((true, false))
            } else if i.consume_key(Modifiers::NONE, Key::PageUp) {
                Some((false, false))
            } else {
                None
            }
        })?;
        let mut state = TextEditState::load(ctx, id).unwrap_or_default();
        let range = state
            .cursor
            .char_range()
            .unwrap_or_else(|| CCursorRange::one(CCursor::new(0)));
        let at = galley.pos_from_cursor(range.primary);
        let dy = if down { step } else { -step };
        let moved = galley.cursor_from_pos(egui::vec2(at.center().x, at.center().y + dy));
        let new = if shift {
            CCursorRange::two(range.secondary, moved)
        } else {
            CCursorRange::one(moved)
        };
        state.cursor.set_char_range(Some(new));
        state.store(ctx, id);
        Some(galley.pos_from_cursor(moved).center().y - at.center().y)
    }

    /// The editor: a monospace text field over the whole tab; `wrap` folds the lines to the
    /// width of the view, else the view scrolls sideways.
    pub fn view(&mut self, ui: &mut egui::Ui, wrap: bool) {
        let p = style::palette(ui);
        let id = self.id();
        // At the first frame and whenever the tab comes back to the front the editor takes the
        // focus, with the cursor and the selection it had.
        let pass = ui.ctx().cumulative_pass_nr();
        if self.last_pass.is_none_or(|last| last + 1 < pass) {
            ui.memory_mut(|m| m.request_focus(id));
        }
        // A text opened now starts with the cursor at its beginning, not at its end.
        if self.last_pass.is_none() {
            let mut state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
            if state.cursor.char_range().is_none() {
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::one(CCursor::new(0))));
                state.store(ui.ctx(), id);
            }
        }
        self.last_pass = Some(pass);
        let paged = self.page_keys(ui.ctx(), id);
        let font = egui::TextStyle::Monospace.resolve(ui.style());
        let color = ui.visuals().text_color();
        let mut no_wrap = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _width: f32| {
            let job = egui::text::LayoutJob::simple(
                text.as_str().to_owned(),
                font.clone(),
                color,
                f32::INFINITY,
            );
            ui.fonts_mut(|f| f.layout_job(job))
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.card))
            .show(ui, |ui| {
                if let Some((start, end)) = self.select.take() {
                    let mut state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
                    state.cursor.set_char_range(Some(CCursorRange::two(
                        CCursor::new(start),
                        CCursor::new(end),
                    )));
                    state.store(ui.ctx(), id);
                    ui.memory_mut(|m| m.request_focus(id));
                }
                // The field fills the view; the room of the scroll bars is kept, so a bar
                // appears only when the text is larger than the view.
                let bar = ui.spacing().scroll.allocated_width();
                let fill = ui.available_size() - egui::vec2(bar, bar);
                let scroll = if wrap {
                    egui::ScrollArea::vertical()
                } else {
                    egui::ScrollArea::both()
                };
                scroll
                    .id_salt(("ww-text-scroll", &self.draft_key))
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        let mut edit = egui::TextEdit::multiline(&mut self.text)
                            .id(id)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .min_size(fill)
                            .lock_focus(true);
                        if !wrap {
                            edit = edit.layouter(&mut no_wrap);
                        }
                        let output = edit.show(ui);
                        if output.response.changed() {
                            self.dirty = true;
                            self.changed = Some(Instant::now());
                            self.last_match = None;
                        }
                        if let Some(range) = output.cursor_range {
                            (self.line, self.col) =
                                position(&self.text, range.primary.index.into());
                        }
                        // The view goes with the cursor of a page key, so the cursor keeps its
                        // place on the screen.
                        if let Some(dy) = paged {
                            ui.scroll_with_delta(egui::vec2(0.0, -dy));
                        }
                        self.galley = Some(output.galley.clone());
                    });
                self.view_height = fill.y;
            });
    }
}

/// The text of a file of UTF-8, its mark of the byte order left out, when it holds at most `max`
/// bytes; else why not.
pub fn read_text(path: &Path, max: u64) -> Result<String, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    if size > max {
        return Err(trf(
            "{path} is larger than {mb} MB",
            &[("path", &path.display()), ("mb", &(max / 1024 / 1024))],
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| trf("{path} is not UTF-8 text", &[("path", &path.display())]))
}

/// The height of a row of the text.
fn row_height(galley: &Galley) -> f32 {
    galley
        .rows
        .first()
        .map(|r| r.height())
        .unwrap_or(14.0)
        .max(1.0)
}

/// The line and the column of a character index, from 1.
fn position(text: &str, index: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for c in text.chars().take(index) {
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matches_go_around_and_ignore_case() {
        let mut doc = TextDoc::new("Abc abc\r\nABC".to_owned(), None);
        assert!(doc.crlf);
        assert_eq!(doc.text, "Abc abc\nABC");
        assert_eq!(doc.find("abc", true), Some(0));
        assert_eq!(doc.find("abc", true), Some(4));
        assert_eq!(doc.find("abc", true), Some(8));
        assert_eq!(doc.find("abc", true), Some(0));
        assert_eq!(doc.find("abc", false), Some(8));
        assert_eq!(doc.find("zzz", true), None);
        assert_eq!(position(&doc.text, 9), (2, 2));
    }
}
