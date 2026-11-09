//! A page of a set of embedded files in a tab (`ShellDoc::Page`, `ShellAct::OpenEmbedded`): a
//! Markdown page or a text, in the language of the interface, whose links lead to the other files
//! of its set, relative to the page.

use eframe::egui;

use super::{App, Shell, ShellDoc, Window};
use crate::doc::Document;
use crate::embedded::{self, Embedded};
use crate::i18n::{self, trf};
use crate::mdview::{self, MdDoc};

/// A page of a set of embedded files.
pub struct PageDoc {
    pub files: Embedded,
    /// The path of the page in the set (the English one, whose folder the links start from).
    pub path: String,
    pub doc: MdDoc,
}

/// Whether a path of a set is shown as a page: Markdown, a text, or a file without an extension.
pub fn is_page(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        None => true,
        Some((_, ext)) => ["md", "markdown", "txt", "text", "log"]
            .iter()
            .any(|e| ext.eq_ignore_ascii_case(e)),
    }
}

/// The title of a page: its first heading, else the name of its file.
fn title_of(text: &str, path: &str) -> String {
    text.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("# "))
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_owned())
}

impl<A: App> Shell<A> {
    /// Opens a page of a set at the heading after `#` (`docs/guide.md#start`), in the language
    /// of the interface; its tab comes to the front when it is open. The reason when the set has
    /// no such file or the file is no page (`is_page`).
    pub fn open_page(&mut self, files: Embedded, link: &str) -> Result<(), String> {
        let (path, anchor) = match link.split_once('#') {
            Some((p, a)) => (p, Some(a.to_owned())),
            None => (link, None),
        };
        let Some(path) = embedded::normal(path).filter(|p| files.get(p).is_some()) else {
            return Err(trf(
                "No page {name} in the documentation",
                &[("name", &path)],
            ));
        };
        if !is_page(&path) {
            return Err(trf("{name} is not a page", &[("name", &path)]));
        }
        let code = i18n::current();
        let bytes = files.read(&path, &code).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text).to_owned();
        let found = |d: &A::Doc| {
            matches!(d.shell_doc(), Some(ShellDoc::Page(p))
                if p.files.same(&files) && p.path.eq_ignore_ascii_case(&path))
        };
        let i = self.show_or_open(found, || {
            let title = title_of(&text, &path);
            let doc = if path.to_ascii_lowercase().ends_with(".md")
                || path.to_ascii_lowercase().ends_with(".markdown")
            {
                MdDoc::new(title, None, text)
            } else {
                MdDoc::plain(title, None, text)
            };
            A::shell_doc(ShellDoc::Page(Box::new(PageDoc {
                files,
                path: path.clone(),
                doc,
            })))
        });
        if let Some(ShellDoc::Page(p)) = self.docs.get_mut(i).and_then(Document::shell_doc_mut) {
            p.doc.jump = anchor.is_some();
            p.doc.anchor = anchor;
        }
        Ok(())
    }

    /// The set and the path of the page of the active tab, when it is a page of a set.
    pub fn active_page(&self) -> Option<(Embedded, String)> {
        match self.active_doc().and_then(Document::shell_doc) {
            Some(ShellDoc::Page(p)) => Some((p.files, p.path.clone())),
            _ => None,
        }
    }
}

impl<A: App> Window<A> {
    /// The page of a set in the active tab.
    pub(super) fn page_view(&mut self, ui: &mut egui::Ui) {
        let (wrap, width) = (
            self.shell.settings.wrap_lines,
            self.shell.settings.page_width,
        );
        if let Some(ShellDoc::Page(p)) = self
            .shell
            .active_doc_mut()
            .and_then(Document::shell_doc_mut)
        {
            egui::CentralPanel::default()
                .show(ui, |ui| mdview::md_view(ui, &mut p.doc, wrap, width));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_a_text_or_markdown() {
        assert!(is_page("docs/guide.md") && is_page("README") && is_page("notes.TXT"));
        assert!(!is_page("sample.hello") && !is_page("picture.png"));
        assert_eq!(
            title_of("Intro\n# The guide\ntext", "docs/guide.md"),
            "The guide"
        );
        assert_eq!(title_of("no heading", "docs/notes.txt"), "notes.txt");
    }
}
