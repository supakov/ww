//! Files embedded in an executable or in a plugin (docs/embedded.md): the pages of a guide, the
//! translations, samples, pictures. `embed!` puts files of the crate into the binary at its
//! build; an `Embedded` set finds them by their path inside it (`docs/guide.md`), in the language
//! of the interface (`lang/<code>/docs/guide.md` first), and a set of a program may read the files
//! of a folder next to the executable over its own (a translation added after the build).
//!
//! A page of a set opens in a tab of the window (`ShellAct::OpenEmbedded`, `ShellDoc::Page`), and
//! its links lead to the other files of the set, relative to the page; a plugin gives its own set
//! (`Plugin::files` of wwrt), which the window opens the same way.
//!
//! ```ignore
//! static DOCS: ww::embedded::Embedded = ww::embed![
//!     "docs/guide.md",
//!     "docs/more.md",
//!     // a path of its own inside the set, read from another place of the crate
//!     "lang/uk/docs/guide.md" => "translations/uk/guide.md",
//! ];
//! ```

use std::borrow::Cow;
use std::path::Path;

/// The largest file of a folder that `read` takes in the place of an embedded one.
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;

/// A file embedded: its path inside the set (folders apart by `/`) and its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct File {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

impl File {
    /// The bytes as a text of UTF-8, without a mark of the byte order; `None` when they are not
    /// UTF-8.
    pub fn text(&self) -> Option<&'static str> {
        let text = std::str::from_utf8(self.bytes).ok()?;
        Some(text.strip_prefix('\u{FEFF}').unwrap_or(text))
    }

    /// The name of the file without its folders.
    pub fn name(&self) -> &'static str {
        self.path.rsplit('/').next().unwrap_or(self.path)
    }
}

/// A set of embedded files, and the folder next to the executable whose files may stand for
/// them (`with_folder`).
#[derive(Clone, Copy, Debug)]
pub struct Embedded {
    files: &'static [File],
    folder: Option<&'static str>,
}

/// Two sets are equal when they are one set (`same`) read with the same folder.
impl PartialEq for Embedded {
    fn eq(&self, other: &Self) -> bool {
        self.same(other) && self.folder == other.folder
    }
}

impl Embedded {
    /// A set without files: the answer of a plugin that embeds none.
    pub const EMPTY: Embedded = Embedded::new(&[]);

    pub const fn new(files: &'static [File]) -> Self {
        Embedded {
            files,
            folder: None,
        }
    }

    /// The same set whose files a folder next to the executable may stand for (`""` for the
    /// folder of the executable itself): `read` takes `<folder>/lang/<code>/<path>` before the
    /// embedded translation, so that a language added after the build shows its pages.
    pub const fn with_folder(self, folder: &'static str) -> Self {
        Embedded {
            files: self.files,
            folder: Some(folder),
        }
    }

    /// Every file of the set, in the order `embed!` names them.
    pub fn files(&self) -> &'static [File] {
        self.files
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Whether two sets are one (the same files in the binary).
    pub fn same(&self, other: &Embedded) -> bool {
        std::ptr::eq(self.files, other.files)
    }

    /// The file of a path, in any case, with `\` or `/` between its folders.
    pub fn get(&self, path: &str) -> Option<&'static File> {
        let wanted = normal(path)?;
        self.files
            .iter()
            .find(|f| normal(f.path).is_some_and(|p| p.eq_ignore_ascii_case(&wanted)))
    }

    /// The text of a file of UTF-8.
    pub fn text(&self, path: &str) -> Option<&'static str> {
        self.get(path).and_then(File::text)
    }

    /// The file of a path in a language: `lang/<code>/<path>`, else the file itself.
    pub fn localized(&self, path: &str, code: &str) -> Option<&'static File> {
        if !code.is_empty() && code != "en" {
            let wanted = normal(path)?;
            if let Some(f) = self.get(&format!("lang/{code}/{wanted}")) {
                return Some(f);
            }
        }
        self.get(path)
    }

    /// The bytes of a path in a language: a file of the folder of the set next to the executable
    /// (`with_folder`, `lang/<code>/<path>`), else the embedded translation, else the file of the
    /// set; `None` when the set has none of them.
    pub fn read(&self, path: &str, code: &str) -> Option<Cow<'static, [u8]>> {
        if let (Some(folder), Some(wanted)) = (self.folder, normal(path))
            && !code.is_empty()
            && code != "en"
        {
            let file = crate::folders::exe_dir()
                .join(folder)
                .join("lang")
                .join(code)
                .join(&wanted);
            if let Some(bytes) = read_limited(&file) {
                return Some(Cow::Owned(bytes));
            }
        }
        self.localized(path, code).map(|f| Cow::Borrowed(f.bytes))
    }

    /// The files of a folder of the set (`docs`, `""` for every file).
    pub fn under(&self, folder: &str) -> impl Iterator<Item = &'static File> + use<> {
        let prefix = normal(folder)
            .map(|f| format!("{f}/"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        let files = self.files;
        files.iter().filter(move |f| {
            normal(f.path).is_some_and(|p| p.to_ascii_lowercase().starts_with(&prefix))
        })
    }

    /// The translations of the set: the code and the text of every `lang/<code>/ui.json`, the
    /// form `ww::i18n::add` takes.
    pub fn translations(&self) -> Vec<(&'static str, &'static str)> {
        self.files
            .iter()
            .filter_map(|f| {
                let rest = f.path.strip_prefix("lang/")?;
                let (code, file) = rest.split_once('/')?;
                (file == "ui.json").then_some(())?;
                Some((code, f.text()?))
            })
            .collect()
    }
}

/// A file of a folder, at most `MAX_BYTES`.
fn read_limited(path: &Path) -> Option<Vec<u8>> {
    let size = std::fs::metadata(path).ok()?.len();
    if size > MAX_BYTES {
        return None;
    }
    std::fs::read(path).ok()
}

/// A path of a set with `/` between its folders, `.` and empty parts left out, `..` taking the
/// folder before it; `None` for a path that leaves the set or names nothing.
pub fn normal(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The path a link of a page names, relative to the folder of the page: `more.md` of
/// `docs/guide.md` is `docs/more.md`, `../README.md` is `README.md`. The part after `#` stays
/// with the link; `None` for a link to a heading of the page itself or out of the set.
pub fn join(page: &str, link: &str) -> Option<String> {
    let file = link.split('#').next().unwrap_or("");
    if file.is_empty() {
        return None;
    }
    let file = file.replace("%20", " ");
    if file.starts_with('/') {
        return normal(&file);
    }
    let folder = page.rsplit_once(['/', '\\']).map_or("", |(f, _)| f);
    normal(&format!("{folder}/{file}"))
}

/// Embeds files of the crate that names it: each by its path relative to the folder of its
/// `Cargo.toml`, which is also its path inside the set, or by a path of its own inside the set
/// and the place it is read from (`"lang/uk/docs/guide.md" => "translations/uk/guide.md"`).
#[macro_export]
macro_rules! embed {
    ($($path:literal $(=> $source:literal)?),* $(,)?) => {
        $crate::embedded::Embedded::new(&[$($crate::__embed_file!($path $(, $source)?)),*])
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __embed_file {
    ($path:literal) => {
        $crate::embedded::File {
            path: $path,
            bytes: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path)),
        }
    };
    ($path:literal, $source:literal) => {
        $crate::embedded::File {
            path: $path,
            bytes: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $source)),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    static SET: Embedded = Embedded::new(&[
        File {
            path: "docs/guide.md",
            bytes: b"\xEF\xBB\xBF# Guide\n[More](more.md)",
        },
        File {
            path: "docs/more.md",
            bytes: b"# More",
        },
        File {
            path: "lang/uk/docs/guide.md",
            bytes: b"# Guide (uk)",
        },
        File {
            path: "lang/uk/ui.json",
            bytes: br#"{"_language": "Ukrainian"}"#,
        },
        File {
            path: "sample.bin",
            bytes: &[0, 159, 146, 150],
        },
    ]);

    #[test]
    fn a_file_is_found_by_its_path_in_a_language() {
        assert_eq!(SET.text("docs/guide.md"), Some("# Guide\n[More](more.md)"));
        assert_eq!(
            SET.get("Docs\\Guide.MD").map(|f| f.name()),
            Some("guide.md")
        );
        assert_eq!(
            SET.get("./docs/../docs/more.md").map(|f| f.path),
            Some("docs/more.md")
        );
        assert!(SET.get("../docs/more.md").is_none(), "out of the set");
        assert_eq!(SET.text("sample.bin"), None, "no UTF-8");
        let uk = SET.localized("docs/guide.md", "uk").unwrap();
        assert_eq!(uk.path, "lang/uk/docs/guide.md");
        assert_eq!(
            SET.localized("docs/more.md", "uk").unwrap().path,
            "docs/more.md"
        );
        assert_eq!(
            SET.localized("docs/guide.md", "en").unwrap().path,
            "docs/guide.md"
        );
        assert_eq!(
            SET.read("docs/guide.md", "uk").as_deref(),
            Some(&b"# Guide (uk)"[..]),
            "a set without a folder reads its own files"
        );
        assert_eq!(SET.under("docs").count(), 2);
        assert_eq!(SET.under("").count(), 5);
        assert_eq!(
            SET.translations(),
            [("uk", r#"{"_language": "Ukrainian"}"#)]
        );
        assert!(SET.same(&SET) && !SET.same(&Embedded::EMPTY));
    }

    #[test]
    fn a_link_is_taken_from_the_folder_of_its_page() {
        assert_eq!(
            join("docs/guide.md", "more.md").as_deref(),
            Some("docs/more.md")
        );
        assert_eq!(
            join("docs/guide.md", "more.md#a-part").as_deref(),
            Some("docs/more.md")
        );
        assert_eq!(
            join("docs/guide.md", "../README.md").as_deref(),
            Some("README.md")
        );
        assert_eq!(
            join("guide.md", "docs/My%20page.md").as_deref(),
            Some("docs/My page.md")
        );
        assert_eq!(join("docs/guide.md", "/top.md").as_deref(), Some("top.md"));
        assert_eq!(join("docs/guide.md", "#a-heading"), None);
        assert_eq!(join("guide.md", "../../out.md"), None);
    }

    #[test]
    fn the_macro_embeds_the_files_of_the_crate() {
        static OWN: Embedded = crate::embed!["Cargo.toml", "lang/x/manifest" => "Cargo.toml"];
        assert!(OWN.text("Cargo.toml").unwrap().contains("name = \"ww\""));
        assert_eq!(
            OWN.get("lang/x/manifest").unwrap().bytes,
            OWN.files()[0].bytes
        );
        assert_eq!(
            OWN.localized("manifest", "x").unwrap().path,
            "lang/x/manifest"
        );
    }
}
