//! The folders next to the executable, and a file shown in its folder.

use std::path::{Path, PathBuf};

use ww_base::paths;

/// The folder of the executable, or the current folder when it cannot be found.
pub fn exe_dir() -> PathBuf {
    paths::program_dir()
}

/// A folder next to the executable, by its name.
pub fn dir(name: &str) -> PathBuf {
    exe_dir().join(name)
}

/// Shows a file in its folder in Explorer, selected, or opens a folder; Explorer is started at
/// the request of the user and not waited for.
pub fn show_in_folder(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        let mut command = std::process::Command::new(root.join("explorer.exe"));
        if path.is_file() {
            command.raw_arg(format!("/select,\"{}\"", path.display()));
        } else if path.is_dir() {
            command.raw_arg(format!("\"{}\"", path.display()));
        } else {
            return Err(format!("{} does not exist", path.display()));
        }
        command.spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        Err(format!(
            "{} stays closed: folders open in Explorer on Windows only",
            path.display()
        ))
    }
}
