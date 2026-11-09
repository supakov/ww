//! The files of a program and of the libraries it takes, through one door. Natively they are the
//! files of the disk. In a browser a program sets a backend at its start (`ww::web` does): the
//! storage of the page for the files of the program, the files of the folder of the page read
//! over http, and a download for a file the user saves, whose path is under [`DOWNLOAD`]. So the
//! code that saves a document by its path works in both: a dialog of a browser gives a path under
//! `DOWNLOAD`, and the write is the download.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The root of the paths a write turns into a download, in a browser: `DOWNLOAD/<name>`.
pub const DOWNLOAD: &str = "download:";

/// Where the files are, when not on the disk.
pub trait Backend: Send + Sync {
    fn read(&self, path: &Path, limit: u64) -> io::Result<Vec<u8>>;
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn remove(&self, path: &Path);
    /// The files of a folder, not of its subfolders, sorted.
    fn list(&self, dir: &Path) -> Vec<PathBuf>;
}

static BACKEND: OnceLock<Box<dyn Backend>> = OnceLock::new();

/// Sets the backend of the files of the process, once (a program in a browser at its start).
pub fn set_backend(backend: Box<dyn Backend>) {
    let _ = BACKEND.set(backend);
}

/// Whether a path is one a write turns into a download.
pub fn is_download(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|c| c.as_os_str() == DOWNLOAD)
}

/// The path of a download of a file name.
pub fn download_path(name: &str) -> PathBuf {
    Path::new(DOWNLOAD).join(name)
}

/// The bytes of a file of at most `limit` bytes.
pub fn read(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    if let Some(b) = BACKEND.get() {
        return b.read(path, limit);
    }
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("larger than {limit} bytes"),
        ));
    }
    Ok(bytes)
}

/// Writes the bytes as the whole file: natively with its folder made first, through a temporary
/// neighbour that replaces the target, so a failed write leaves the old file whole.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(b) = BACKEND.get() {
        return b.write(path, bytes);
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

pub fn exists(path: &Path) -> bool {
    match BACKEND.get() {
        Some(b) => b.exists(path),
        None => path.is_file(),
    }
}

pub fn remove(path: &Path) {
    match BACKEND.get() {
        Some(b) => b.remove(path),
        None => {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The files of a folder, not of its subfolders, sorted.
pub fn list(dir: &Path) -> Vec<PathBuf> {
    if let Some(b) = BACKEND.get() {
        return b.list(dir);
    }
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_of_the_disk_go_through_the_door() {
        let dir = std::env::temp_dir().join(format!("ww-base-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("a").join("b.json");
        write(&file, b"{}").unwrap();
        assert!(exists(&file));
        assert_eq!(read(&file, 10).unwrap(), b"{}");
        assert!(read(&file, 1).is_err());
        assert_eq!(list(&dir.join("a")), vec![file.clone()]);
        remove(&file);
        assert!(!exists(&file));
        assert!(is_download(&download_path("x.reg")));
        assert!(!is_download(&file));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
