//! The files of a program and of the libraries it takes, through one door. Natively they are the
//! files of the disk. In a browser a program sets a backend at its start (`ww::web` does): the
//! storage of the page for the files of the program, the files of the folder of the page read
//! over http, and a download for a file the user saves, whose path is under [`DOWNLOAD`]. So the
//! code that saves a document by its path works in both: a dialog of a browser gives a path under
//! `DOWNLOAD`, and the write is the download.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// Writes the bytes as the whole file: through the backend when one is set, else on the disk
/// (`write_disk`).
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(b) = BACKEND.get() {
        return b.write(path, bytes);
    }
    write_disk(path, bytes)
}

/// The number of the next temporary neighbour this process makes.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
/// How many names of a temporary neighbour are tried before the write gives up.
const TEMPORARY_TRIES: u32 = 64;
/// The most characters of the name of a file its temporary neighbour repeats.
const TEMPORARY_STEM: usize = 100;

/// The number of the process in the name of a temporary neighbour; a browser has none.
fn process_number() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::process::id()
    }
}

/// A new file next to `path` for its bytes on their way: the whole name of the file (its first
/// characters when it is very long) with the process, a number and `.tmp` after it, so that it
/// is no file of the user (`report.json` never takes `report.tmp`). It is made anew: a name
/// that exists is left alone and the next number is tried.
fn temporary(path: &Path) -> io::Result<(PathBuf, std::fs::File)> {
    let name: String = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .take(TEMPORARY_STEM)
        .collect();
    let process = process_number();
    let mut last = io::Error::from(io::ErrorKind::AlreadyExists);
    for _ in 0..TEMPORARY_TRIES {
        let number = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_file_name(format!("{name}.{process}-{number}.tmp"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = e,
            Err(e) => return Err(e),
        }
    }
    Err(last)
}

/// Writes the bytes as the whole file of the disk, whatever the backend: its folder is made
/// first, the bytes go into a temporary neighbour of a name of its own, reach the disk
/// (`sync_all`), and the neighbour then replaces the target, so a failed write leaves the old
/// file whole. The neighbour is removed when the write or the replacement fails.
pub fn write_disk(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    // The file closes at the end of its block, before it is renamed or removed.
    let (tmp, written) = {
        let (tmp, mut file) = temporary(path)?;
        let written = file.write_all(bytes).and_then(|()| file.sync_all());
        (tmp, written)
    };
    let replaced = written.and_then(|()| std::fs::rename(&tmp, path));
    if replaced.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    replaced
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

    #[test]
    fn a_write_takes_no_file_of_the_user_and_leaves_no_neighbour() {
        let dir = std::env::temp_dir().join(format!("ww-base-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // The neighbour `x.tmp` of the user survives the saving of `x.json`.
        let users = dir.join("x.tmp");
        std::fs::write(&users, b"the notes of the user").unwrap();
        let file = dir.join("x.json");
        write(&file, b"[1]").unwrap();
        write(&file, b"[2]").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"[2]");
        assert_eq!(std::fs::read(&users).unwrap(), b"the notes of the user");
        assert_eq!(list(&dir), vec![file.clone(), users.clone()]);
        // A name that is taken is left alone: the next number is tried.
        let taken = NEXT_TEMPORARY.load(Ordering::Relaxed);
        let name = format!("x.json.{}-{taken}.tmp", std::process::id());
        std::fs::write(dir.join(&name), b"of another writer").unwrap();
        write_disk(&file, b"[3]").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"[3]");
        assert_eq!(
            std::fs::read(dir.join(&name)).unwrap(),
            b"of another writer"
        );
        std::fs::remove_file(dir.join(&name)).unwrap();
        // A write that fails (the target is a folder) removes its temporary neighbour, and a
        // very long name still has one.
        let folder = dir.join("folder.json");
        std::fs::create_dir_all(&folder).unwrap();
        assert!(write(&folder, b"[4]").is_err());
        assert_eq!(list(&dir), vec![file.clone(), users]);
        let long = dir.join(format!("{}.json", "n".repeat(200)));
        write(&long, b"[5]").unwrap();
        assert_eq!(std::fs::read(&long).unwrap(), b"[5]");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
