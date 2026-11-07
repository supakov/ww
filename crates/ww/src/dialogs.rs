//! The names of files and the files written safely.

use std::path::Path;

use ww_base::paths;

use crate::folders::exe_dir;

/// Writes a file through a temporary neighbour that replaces the target.
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A path as the files of the program keep it, in lower case: a name made of a file or a folder
/// inside the program folder stays when the folder moves.
pub fn kept_name(path: &Path) -> String {
    let kept = paths::stored(path, &exe_dir());
    kept.display().to_string().to_lowercase()
}

/// A file name stem made of a name: letters, digits, spaces, hyphens and underscores stay, the
/// rest becomes an underscore; `fallback` when nothing is left.
pub fn file_stem(name: &str, fallback: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = stem.trim().to_owned();
    if stem.is_empty() {
        fallback.to_owned()
    } else {
        stem
    }
}
