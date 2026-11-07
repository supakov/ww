//! The session: the drafts of unsaved documents next to the executable, and the log of the day;
//! both through `store`, so a browser keeps the drafts in the storage of the page and logs to
//! its console.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ww_base::{clock, jsonfile, paths};

use crate::store;

pub const DRAFT_FORMAT: &str = "ww-draft";
/// The format of the drafts of WinWeaver before WW; read, never written.
const EARLIER_DRAFT_FORMAT: &str = "winweaver-draft";
pub const DRAFT_VERSION: u32 = 1;
const DRAFT_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// The unsaved state of a document, written next to the executable until it is saved or
/// discarded: what the document gave as its draft, as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub format: String,
    pub version: u32,
    /// The file the document belongs to, empty for a document never saved; kept relative to the
    /// program folder when it lies inside it, absolute in memory.
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub saved: String,
    #[serde(alias = "profile")]
    pub document: Value,
}

/// FNV-1a, 64 bits: a stable name for the draft of a file.
pub fn fnv(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The draft key of a file: of its stored form, so it stays when the program folder moves.
pub fn key_for_path(path: &Path) -> String {
    let kept = paths::stored(path, &paths::program_dir());
    format!("p-{:016x}", fnv(&kept.to_string_lossy().to_lowercase()))
}

/// A draft key for a document that has no file yet, unique in this session.
pub fn new_key() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let start = ww_base::clock::SystemTime::now()
        .duration_since(ww_base::clock::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("n-{start}-{n}")
}

fn draft_file(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.json"))
}

/// Writes the draft of a document through a temporary file.
pub fn write_draft(
    dir: &Path,
    key: &str,
    path: Option<&Path>,
    document: &Value,
) -> Result<(), String> {
    let kept = path.map(|p| paths::stored(p, &paths::program_dir()));
    let draft = Draft {
        format: DRAFT_FORMAT.to_owned(),
        version: DRAFT_VERSION,
        path: kept.map(|p| p.display().to_string()).unwrap_or_default(),
        saved: clock::now_utc_iso(),
        document: document.clone(),
    };
    let mut text = serde_json::to_string_pretty(&draft).map_err(|e| e.to_string())?;
    text.push('\n');
    store::write(&draft_file(dir, key), &text)
}

/// Deletes the draft of a key, if there is one.
pub fn remove_draft(dir: &Path, key: &str) {
    store::remove(&draft_file(dir, key));
}

/// The drafts of a folder with their keys, oldest first, and the names of the files that could
/// not be read (they stay where they are).
pub fn read_drafts(dir: &Path) -> (Vec<(String, Draft)>, Vec<String>) {
    let mut drafts = Vec::new();
    let mut broken = Vec::new();
    for file in store::list(dir, &["json"]) {
        let name = jsonfile::display_name(&file);
        let read = store::read(&file, DRAFT_MAX_BYTES)
            .and_then(|text| text.ok_or_else(|| format!("{name}: not found")))
            .and_then(|text| jsonfile::parse::<Draft>(&text, &name).map_err(|e| e.to_string()))
            .and_then(|d| {
                if d.format == DRAFT_FORMAT || d.format == EARLIER_DRAFT_FORMAT {
                    Ok(d)
                } else {
                    Err(format!("{name}: not a draft"))
                }
            });
        match read {
            Ok(mut draft) => {
                draft.path = paths::resolved_text(&draft.path, &paths::program_dir());
                let key = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                drafts.push((key, draft));
            }
            Err(e) => broken.push(e),
        }
    }
    drafts.sort_by(|a, b| a.1.saved.cmp(&b.1.saved));
    (drafts, broken)
}

/// Appends a line to the log of the day in `dir`, named after the program (the console of a
/// browser); a log that cannot be written is ignored.
pub fn log(dir: &Path, program: &str, level: &str, text: &str) {
    store::log_line(dir, program, level, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_round_trip_and_broken_files_are_listed() {
        let dir = std::env::temp_dir().join(format!("ww-drafts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let document = serde_json::json!({ "name": "Draft", "text": "a line" });
        let key = key_for_path(Path::new("C:\\Notes\\Today.txt"));
        assert_eq!(key, key_for_path(Path::new("c:\\notes\\today.txt")));
        write_draft(
            &dir,
            &key,
            Some(Path::new("C:\\Notes\\Today.txt")),
            &document,
        )
        .unwrap();
        let other = new_key();
        assert_ne!(other, new_key());
        write_draft(&dir, &other, None, &document).unwrap();
        std::fs::write(dir.join("broken.json"), "{").unwrap();
        std::fs::write(
            dir.join("earlier.json"),
            "{\"format\": \"winweaver-draft\", \"version\": 1, \"profile\": {\"name\": \"Old\"}}",
        )
        .unwrap();
        let (drafts, broken) = read_drafts(&dir);
        assert_eq!(drafts.len(), 3);
        assert_eq!(broken.len(), 1);
        assert!(
            drafts
                .iter()
                .any(|(k, d)| k == &key && d.path.ends_with("Today.txt"))
        );
        assert!(
            drafts
                .iter()
                .any(|(k, d)| k == "earlier" && d.document["name"] == "Old")
        );
        remove_draft(&dir, &key);
        remove_draft(&dir, &other);
        remove_draft(&dir, "earlier");
        assert_eq!(read_drafts(&dir).0.len(), 0);
        log(&dir, "ww", "INFO", "a line\nwith a break");
        let logs: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("ww-"))
            .collect();
        assert_eq!(logs.len(), 1);
        let text = std::fs::read_to_string(logs[0].path()).unwrap();
        assert!(text.contains("[INFO] a line with a break"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
