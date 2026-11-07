//! The files the window keeps for itself (the settings, the drafts, the themes, the log) through
//! one door. Natively they are files next to the executable, as they always were. In a browser
//! (wasm32) a file is an entry of the storage of the page (`localStorage`) under the key of its
//! path, the log goes to the console, and a file the user saves is a download. A file that
//! cannot be read or written gives `None` or a text, never a crash.

use std::path::{Path, PathBuf};

/// Whether there is a file at the path.
pub fn exists(path: &Path) -> bool {
    platform::exists(path)
}

/// The text of a file of at most `limit` bytes; `None` when there is none.
pub fn read(path: &Path, limit: u64) -> Result<Option<String>, String> {
    platform::read(path, limit)
}

/// Writes a text as the whole file. Natively the folder is made first and the text goes through
/// a temporary neighbour that replaces the target, so a failed write leaves the old file whole.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    platform::write(path, text)
}

/// Removes a file, if there is one.
pub fn remove(path: &Path) {
    platform::remove(path);
}

/// The files of a folder with one of the extensions (without the dot), sorted by name.
pub fn list(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    platform::list(dir, extensions)
}

/// A line of the log of a program: appended to `<dir>/<program>-<day>.log` natively, written to
/// the console of the browser.
pub fn log_line(dir: &Path, program: &str, level: &str, text: &str) {
    platform::log_line(dir, program, level, text);
}

/// Hands the browser a file to download: the bytes under that name, of that media type.
#[cfg(target_arch = "wasm32")]
pub fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    platform::download(name, bytes, mime)
}

/// Whether a path has one of the extensions.
fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .is_some_and(|e| extensions.iter().any(|x| e == *x))
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use ww_base::{clock, jsonfile};

    pub fn exists(path: &Path) -> bool {
        path.is_file()
    }

    pub fn read(path: &Path, limit: u64) -> Result<Option<String>, String> {
        if !path.is_file() {
            return Ok(None);
        }
        jsonfile::read_text(path, limit)
            .map(Some)
            .map_err(|e| e.to_string())
    }

    pub fn write(path: &Path, text: &str) -> Result<(), String> {
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn remove(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    pub fn list(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| super::has_extension(p, extensions))
            .collect();
        files.sort();
        files
    }

    /// A log that cannot be written is ignored.
    pub fn log_line(dir: &Path, program: &str, level: &str, text: &str) {
        let now = clock::now_utc_iso();
        let day = now.get(..10).unwrap_or("unknown");
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let file = dir.join(format!("{program}-{day}.log"));
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
        {
            let line = text.replace(['\r', '\n'], " ");
            let _ = writeln!(f, "[{now}] [{level}] {line}");
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use std::path::{Component, Path, PathBuf};

    use wasm_bindgen::JsCast;

    /// The first part of every key of the program in the storage of the page.
    const PREFIX: &str = "ww/";

    /// The key of a path: its parts after the prefix, joined by `/`; the current folder, which
    /// stands for the folder of the program, is left out.
    fn key(path: &Path) -> String {
        let parts: Vec<String> = path
            .components()
            .filter_map(|c| match c {
                Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();
        format!("{PREFIX}{}", parts.join("/"))
    }

    /// The storage of the page; an error when the page has none (a private window may refuse
    /// it).
    fn storage() -> Result<web_sys::Storage, String> {
        web_sys::window()
            .ok_or("the page has no window")?
            .local_storage()
            .ok()
            .flatten()
            .ok_or_else(|| "the page has no storage".to_owned())
    }

    pub fn exists(path: &Path) -> bool {
        storage()
            .ok()
            .and_then(|s| s.get_item(&key(path)).ok().flatten())
            .is_some()
    }

    pub fn read(path: &Path, limit: u64) -> Result<Option<String>, String> {
        let text = storage()?
            .get_item(&key(path))
            .map_err(|_| format!("{}: not read", path.display()))?;
        match text {
            Some(t) if t.len() as u64 > limit => {
                Err(format!("{}: larger than {limit} bytes", path.display()))
            }
            other => Ok(other),
        }
    }

    pub fn write(path: &Path, text: &str) -> Result<(), String> {
        storage()?
            .set_item(&key(path), text)
            .map_err(|_| format!("{}: the storage of the page refused it", path.display()))
    }

    pub fn remove(path: &Path) {
        if let Ok(s) = storage() {
            let _ = s.remove_item(&key(path));
        }
    }

    pub fn list(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
        let Ok(s) = storage() else {
            return Vec::new();
        };
        let prefix = format!("{}/", key(dir));
        let count = s.length().unwrap_or(0);
        let mut files: Vec<PathBuf> = (0..count)
            .filter_map(|i| s.key(i).ok().flatten())
            .filter_map(|k| {
                let name = k.strip_prefix(&prefix)?;
                let file = (!name.contains('/')).then(|| dir.join(name))?;
                super::has_extension(&file, extensions).then_some(file)
            })
            .collect();
        files.sort();
        files
    }

    pub fn log_line(_dir: &Path, _program: &str, level: &str, text: &str) {
        let line = format!("[{}] [{level}] {text}", ww_base::clock::now_utc_iso());
        web_sys::console::log_1(&line.into());
    }

    pub fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("the page has no document")?;
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(bytes));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(mime);
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
            .map_err(|_| "the file was not made")?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|_| "the address of the file was not made")?;
        let link: web_sys::HtmlAnchorElement = document
            .create_element("a")
            .map_err(|_| "the link was not made")?
            .dyn_into()
            .map_err(|_| "the link is not a link")?;
        link.set_href(&url);
        link.set_download(name);
        link.click();
        let _ = web_sys::Url::revoke_object_url(&url);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_round_trip_and_folders_list_by_extension() {
        let dir = std::env::temp_dir().join(format!("ww-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("notes").join("a.json");
        assert!(!exists(&file));
        assert_eq!(read(&file, 10).unwrap(), None);
        write(&file, "{}").unwrap();
        write(&dir.join("notes").join("b.txt"), "text").unwrap();
        assert!(exists(&file));
        assert_eq!(read(&file, 10).unwrap().as_deref(), Some("{}"));
        assert!(read(&file, 1).is_err(), "the limit holds");
        assert_eq!(list(&dir.join("notes"), &["json"]), vec![file.clone()]);
        assert_eq!(list(&dir.join("notes"), &["json", "txt"]).len(), 2);
        assert!(list(&dir.join("none"), &["json"]).is_empty());
        remove(&file);
        assert!(!exists(&file));
        log_line(&dir, "ww", "INFO", "a line");
        assert_eq!(list(&dir, &["log"]).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
