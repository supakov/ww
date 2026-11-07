//! A document in a tab: what the window asks of it, and the search of a tab.

use std::path::{Path, PathBuf};

use crate::shell::ShellDoc;

/// The searches a tab remembers.
pub const SEARCH_HISTORY: usize = 20;

/// The search of a tab, at the right end of its navigation row or its command bar.
#[derive(Default)]
pub struct Search {
    pub open: bool,
    pub text: String,
    /// The field takes the focus at the next frame.
    pub focus: bool,
}

impl Search {
    /// The lower case words of the search text.
    pub fn words(&self) -> Vec<String> {
        self.text
            .split_whitespace()
            .map(str::to_lowercase)
            .collect()
    }
}

/// Whether a text holds every word of a search, ignoring case.
pub fn matches_words(hay: &str, words: &[String]) -> bool {
    let hay = hay.to_lowercase();
    words.iter().all(|w| hay.contains(w.as_str()))
}

/// Puts a search first in the history of the searches; `true` when the history changed.
pub fn remember_search(history: &mut Vec<String>, text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    if history.first().is_some_and(|h| h == text) {
        return false;
    }
    history.retain(|h| h != text);
    history.insert(0, text.to_owned());
    history.truncate(SEARCH_HISTORY);
    true
}

/// How the selection of a tab came into view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scrolled {
    /// The row was scrolled to (or there was none): the tab stops asking.
    Revealed,
    /// The selection moved with the keys: the row comes into view at the next frame.
    Follow,
}

/// What a draft of a document holds: its key (the name of the file of the draft), the file the
/// document belongs to and the document as JSON.
pub struct Draft {
    pub key: String,
    pub path: Option<PathBuf>,
    pub document: serde_json::Value,
}

/// The draft of a document that changes in many places: offered while the document has unsaved
/// changes, at most every two seconds (at once when the window closes), and written only when it
/// differs from the last draft.
#[derive(Debug, Clone)]
pub struct DraftState {
    pub key: String,
    written: Option<u64>,
    checked: Option<ww_base::clock::Instant>,
}

impl DraftState {
    /// The draft of a file, or of a document without one.
    pub fn new(path: Option<&Path>) -> Self {
        Self::with_key(path.map_or_else(crate::session::new_key, crate::session::key_for_path))
    }

    pub fn with_key(key: String) -> Self {
        Self {
            key,
            written: None,
            checked: None,
        }
    }

    /// Whether to make the draft now: the document has changes and the last look is two seconds
    /// old, or the window closes.
    pub fn ready(&mut self, dirty: bool, all: bool) -> bool {
        if !dirty {
            self.written = None;
            return false;
        }
        let delay = std::time::Duration::from_secs(2);
        if !all && self.checked.is_some_and(|t| t.elapsed() < delay) {
            return false;
        }
        self.checked = Some(ww_base::clock::Instant::now());
        true
    }

    /// The draft of a document, unless the last one written holds the same.
    pub fn offer(&mut self, path: Option<PathBuf>, document: serde_json::Value) -> Option<Draft> {
        let hash = crate::session::fnv(&document.to_string());
        if self.written == Some(hash) {
            return None;
        }
        self.written = Some(hash);
        Some(Draft {
            key: self.key.clone(),
            path,
            document,
        })
    }
}

/// What every tab answers, so the window draws it without knowing the document.
pub trait Document {
    /// The title of the tab: the name of its file, else `name`.
    fn title(&self) -> String {
        match self.path() {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => self.name(),
        }
    }

    /// The title of a document that has no file.
    fn name(&self) -> String {
        String::new()
    }

    /// Whether the document has changes its file does not.
    fn dirty(&self) -> bool {
        false
    }

    /// The file of the document: the hint of the tab, and opened again at the next start.
    fn path(&self) -> Option<&Path> {
        None
    }

    /// The search of the tab, for the tabs that have one.
    fn search(&mut self) -> Option<&mut Search> {
        None
    }

    /// The icon of the tab.
    fn icon(&self) -> &'static str {
        ""
    }

    /// The hint of the search field of the tab, empty for a tab without a search.
    fn search_hint(&self) -> &'static str {
        ""
    }

    /// Whether the tab has commands in the command bar.
    fn has_tools(&self) -> bool {
        true
    }

    /// Whether the tab has a navigation row: a history, a breadcrumb and the star.
    fn has_nav(&self) -> bool {
        false
    }

    /// The selection of the tab came into view, or should at the next frame.
    fn scrolled(&mut self, _how: Scrolled) {}

    /// The key of the draft of the document, removed when the tab closes.
    fn draft_key(&self) -> Option<String> {
        None
    }

    /// The draft to write now: a document with unsaved changes gives it a while after the last
    /// change, or at once when `all` (the window closes), and then not again until a change.
    fn draft_due(&mut self, _all: bool) -> Option<Draft> {
        None
    }

    /// The document of the window itself a tab holds (the settings, the editor of a theme).
    fn shell_doc(&self) -> Option<&ShellDoc> {
        None
    }

    fn shell_doc_mut(&mut self) -> Option<&mut ShellDoc> {
        None
    }
}
