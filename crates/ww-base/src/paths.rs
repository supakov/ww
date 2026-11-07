//! The paths the files of the program keep (`docs/spec/02-documents.md` section 2): a file inside
//! the folder of the program is kept relative to it, as `.\profiles\Office.wwprofile`, so the
//! folder can move or go to another PC with its settings, bookmarks, drafts and diffs; any other
//! file keeps its absolute path. In memory every path is absolute.

use std::path::{Component, Path, PathBuf};

/// The folder of the executable, or the current folder when it cannot be found.
pub fn program_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The form a file keeps a path in: relative to `base` and starting with `.` when the path lies
/// inside it, else the path as it is.
pub fn stored(path: &Path, base: &Path) -> PathBuf {
    match inside(path, base) {
        Some(rest) if rest.components().next().is_some() => Path::new(".").join(rest),
        _ => path.to_path_buf(),
    }
}

/// The path a stored form names: one that starts with `.` lies in `base`, any other is as it is.
pub fn resolved(path: &Path, base: &Path) -> PathBuf {
    let mut parts = path.components();
    if parts.next() == Some(Component::CurDir) {
        base.join(parts.as_path())
    } else {
        path.to_path_buf()
    }
}

/// [`stored`] for a file that keeps a path as text, which may also be a name (the title of a
/// preset) or a key of the registry: only an absolute path changes.
pub fn stored_text(text: &str, base: &Path) -> String {
    let path = Path::new(text);
    if text.is_empty() || !path.is_absolute() {
        return text.to_owned();
    }
    stored(path, base).display().to_string()
}

/// [`resolved`] for a path kept as text: only a text that starts with `.` and a separator changes.
pub fn resolved_text(text: &str, base: &Path) -> String {
    if !(text.starts_with(".\\") || text.starts_with("./")) {
        return text.to_owned();
    }
    resolved(Path::new(text), base).display().to_string()
}

/// The rest of `path` below `base`; the parts compare without case on Windows. `None` when the
/// path lies elsewhere or climbs out with `..`.
fn inside(path: &Path, base: &Path) -> Option<PathBuf> {
    if !path.is_absolute() || !base.is_absolute() {
        return None;
    }
    let mut parts = path.components();
    for b in base.components() {
        let a = parts.next()?;
        if !same(a, b) {
            return None;
        }
    }
    let rest = parts.as_path().to_path_buf();
    if rest
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    Some(rest)
}

fn same(a: Component, b: Component) -> bool {
    if cfg!(windows) {
        let lower = |c: Component| c.as_os_str().to_string_lossy().to_lowercase();
        lower(a) == lower(b)
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PathBuf {
        std::env::temp_dir().join("ww-paths").join("WinWeaver")
    }

    #[test]
    fn a_file_inside_the_program_folder_is_kept_relative() {
        let base = base();
        let file = base.join("profiles").join("Office.wwprofile");
        let kept = stored(&file, &base);
        let want = Path::new(".").join("profiles").join("Office.wwprofile");
        assert_eq!(kept, want);
        assert_eq!(resolved(&kept, &base), file);
        let moved = std::env::temp_dir().join("elsewhere").join("WinWeaver");
        let there = moved.join("profiles").join("Office.wwprofile");
        assert_eq!(resolved(&kept, &moved), there);
        let text = stored_text(&file.display().to_string(), &base);
        assert!(text.starts_with('.'), "{text}");
        assert_eq!(resolved_text(&text, &base), file.display().to_string());
    }

    #[test]
    fn other_paths_and_names_stay_as_they_are() {
        let base = base();
        let outside = base.with_file_name("other.wwprofile");
        assert_eq!(stored(&outside, &base), outside);
        assert_eq!(stored(&base, &base), base);
        let sibling = base.with_file_name("WinWeaver2").join("a");
        assert_eq!(stored(&sibling, &base), sibling);
        let name = Path::new("a.wwprofile");
        assert_eq!(stored(name, &base), name);
        assert_eq!(resolved(name, &base), name);
        for text in ["", "Office", "HKEY_CURRENT_USER\\Software", "a.wwprofile"] {
            assert_eq!(stored_text(text, &base), text);
            assert_eq!(resolved_text(text, &base), text);
        }
        let absolute = outside.display().to_string();
        assert_eq!(resolved_text(&absolute, &base), absolute);
    }

    #[cfg(windows)]
    #[test]
    fn the_case_of_the_folder_does_not_matter_on_windows() {
        let file = Path::new("C:\\WinWeaver\\Profiles\\A.wwprofile");
        let kept = stored(file, Path::new("c:\\winweaver"));
        assert_eq!(kept, Path::new(".\\Profiles\\A.wwprofile"));
    }
}
