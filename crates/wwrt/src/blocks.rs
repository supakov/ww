//! Files read in blocks through one cache of the program (docs/plugins.md, "The cache of the
//! blocks"): a view of a file larger than the memory it may take (a binary file, a long log)
//! reads the blocks it shows, and the block used longest ago leaves the cache when the blocks of
//! every open file together reach its budget. The program sets the size of a block and the
//! budget (`Cache::set_limits`, Settings of a program with plugins) and shows the state
//! (`Cache::state`); a plugin of the stable interface reads through the same cache
//! (`ww_host` of `ww.h`).
//!
//! A `BlockFile` is cheap to clone, and a part of it (`slice`) reads the blocks of the same file:
//! a view offers some bytes of its file to another plugin (Open with) without a copy. The same
//! type serves bytes already in memory, read whole.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use serde::{Deserialize, Serialize};

/// The size of a block and the budget of the cache at first.
pub const BLOCK: usize = 64 * 1024;
pub const BUDGET: usize = 64 * 1024 * 1024;
/// The sizes of a block the cache takes.
pub const MIN_BLOCK: usize = 4 * 1024;
pub const MAX_BLOCK: usize = 4 * 1024 * 1024;

/// The size of a block and the most bytes of blocks in memory: the settings of the cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub block: usize,
    pub budget: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            block: BLOCK,
            budget: BUDGET,
        }
    }
}

impl Limits {
    /// The limits within what the cache takes: a block of `MIN_BLOCK` to `MAX_BLOCK` bytes, a
    /// budget of one block at least.
    pub fn checked(self) -> Self {
        let block = self.block.clamp(MIN_BLOCK, MAX_BLOCK);
        Limits {
            block,
            budget: self.budget.max(block),
        }
    }
}

/// What the cache holds now and what it did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub limits: Limits,
    /// The bytes and the blocks in memory.
    pub used: usize,
    pub blocks: usize,
    /// The files read through the cache that are open.
    pub files: usize,
    /// The blocks read from the disk, and the ones found in memory.
    pub reads: u64,
    pub hits: u64,
}

/// The blocks of the files of a program in memory, within a budget.
pub struct Cache {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    limits: Limits,
    /// The blocks by their file, their size and their index.
    blocks: HashMap<(u64, usize, u64), Entry>,
    used: usize,
    tick: u64,
    files: usize,
    reads: u64,
    hits: u64,
}

struct Entry {
    bytes: Arc<[u8]>,
    used: u64,
}

/// The cache of the program, one for every plugin (wwrt.dll holds it).
static CACHE: LazyLock<Arc<Cache>> = LazyLock::new(|| Cache::new(Limits::default()));

/// The cache of the program.
pub fn cache() -> &'static Arc<Cache> {
    &CACHE
}

/// The ids of the files read through a cache, one each.
static FILES: AtomicU64 = AtomicU64::new(1);

/// A key of a block: its file, the size of a block it was read with, its index.
type Key = (u64, usize, u64);

impl Cache {
    /// A cache of its own (a test, a tool), apart from the one of the program.
    pub fn new(limits: Limits) -> Arc<Self> {
        Arc::new(Cache {
            inner: Mutex::new(Inner {
                limits: limits.checked(),
                ..Inner::default()
            }),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn state(&self) -> State {
        let inner = self.lock();
        State {
            limits: inner.limits,
            used: inner.used,
            blocks: inner.blocks.len(),
            files: inner.files,
            reads: inner.reads,
            hits: inner.hits,
        }
    }

    /// New limits: a new size of a block empties the cache, a smaller budget lets the blocks
    /// used longest ago go.
    pub fn set_limits(&self, limits: Limits) {
        let limits = limits.checked();
        let mut inner = self.lock();
        if inner.limits.block != limits.block {
            inner.blocks.clear();
            inner.used = 0;
        }
        inner.limits = limits;
        inner.shrink(0);
    }

    /// Every block leaves the memory; the files read them again when they need them.
    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.blocks.clear();
        inner.used = 0;
    }

    fn block(&self) -> usize {
        self.lock().limits.block
    }

    fn get(&self, key: Key) -> Option<Arc<[u8]>> {
        let mut inner = self.lock();
        inner.tick += 1;
        let tick = inner.tick;
        let found = inner.blocks.get_mut(&key).map(|e| {
            e.used = tick;
            e.bytes.clone()
        });
        if found.is_some() {
            inner.hits += 1;
        }
        found
    }

    fn put(&self, key: Key, bytes: Arc<[u8]>) {
        let mut inner = self.lock();
        inner.reads += 1;
        if key.1 != inner.limits.block || bytes.len() > inner.limits.budget {
            return;
        }
        inner.shrink(bytes.len());
        inner.tick += 1;
        let used = inner.tick;
        inner.used += bytes.len();
        if let Some(old) = inner.blocks.insert(key, Entry { bytes, used }) {
            inner.used -= old.bytes.len();
        }
    }

    fn opened(&self) {
        self.lock().files += 1;
    }

    /// The blocks of a file that closed leave the memory.
    fn forget(&self, file: u64) {
        let mut inner = self.lock();
        inner.files = inner.files.saturating_sub(1);
        let before = inner.blocks.len();
        inner.blocks.retain(|k, _| k.0 != file);
        if inner.blocks.len() != before {
            inner.used = inner.blocks.values().map(|e| e.bytes.len()).sum();
        }
    }
}

impl Inner {
    /// The blocks used longest ago go until `more` bytes fit the budget.
    fn shrink(&mut self, more: usize) {
        while self.used + more > self.limits.budget {
            let Some(oldest) = self
                .blocks
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| *k)
            else {
                break;
            };
            if let Some(e) = self.blocks.remove(&oldest) {
                self.used -= e.bytes.len();
            }
        }
    }
}

/// A file read in blocks through a cache, a part of one, or bytes in memory.
#[derive(Clone)]
pub struct BlockFile {
    shared: Arc<Shared>,
    /// Where the part starts in the file and how long it is.
    start: u64,
    len: u64,
}

struct Shared {
    id: u64,
    path: Option<PathBuf>,
    backing: Backing,
    cache: Option<Arc<Cache>>,
    /// The blocks of the file read from the disk.
    reads: AtomicU64,
}

enum Backing {
    /// A file and its length when it was opened.
    File(Mutex<File>, u64),
    Memory(Vec<u8>),
    Static(&'static [u8]),
}

impl Drop for Shared {
    fn drop(&mut self) {
        if let Some(cache) = &self.cache {
            cache.forget(self.id);
        }
    }
}

impl BlockFile {
    /// A file, read in blocks through the cache of the program.
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::open_in(path, cache().clone())
    }

    /// A file, read in blocks through a cache of its own.
    pub fn open_in(path: &Path, cache: Arc<Cache>) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        cache.opened();
        Ok(BlockFile {
            shared: Arc::new(Shared {
                id: FILES.fetch_add(1, Ordering::Relaxed),
                path: Some(path.to_path_buf()),
                backing: Backing::File(Mutex::new(file), len),
                cache: Some(cache),
                reads: AtomicU64::new(0),
            }),
            start: 0,
            len,
        })
    }

    /// Bytes in memory, read like a file of them.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let len = bytes.len() as u64;
        Self::of(Backing::Memory(bytes), len)
    }

    /// Bytes of the binary (an embedded file), read like a file without a copy.
    pub fn from_static(bytes: &'static [u8]) -> Self {
        Self::of(Backing::Static(bytes), bytes.len() as u64)
    }

    fn of(backing: Backing, len: u64) -> Self {
        BlockFile {
            shared: Arc::new(Shared {
                id: FILES.fetch_add(1, Ordering::Relaxed),
                path: None,
                backing,
                cache: None,
                reads: AtomicU64::new(0),
            }),
            start: 0,
            len,
        }
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The file the bytes are read from; none for bytes in memory.
    pub fn path(&self) -> Option<&Path> {
        self.shared.path.as_deref()
    }

    /// Where the bytes start in their file: 0 but for a part (`slice`).
    pub fn offset(&self) -> u64 {
        self.start
    }

    /// The blocks of the file read from the disk so far.
    pub fn reads(&self) -> u64 {
        self.shared.reads.load(Ordering::Relaxed)
    }

    /// A part of the bytes, from `offset` and at most `len` long, read through the same blocks.
    pub fn slice(&self, offset: u64, len: u64) -> Self {
        let offset = offset.min(self.len);
        BlockFile {
            shared: self.shared.clone(),
            start: self.start + offset,
            len: len.min(self.len - offset),
        }
    }

    /// Copies the bytes from `offset` into `out`, as many as there are; gives the count.
    pub fn read_at(&self, offset: u64, out: &mut [u8]) -> io::Result<usize> {
        if offset >= self.len {
            return Ok(0);
        }
        let count = (out.len() as u64).min(self.len - offset) as usize;
        let out = &mut out[..count];
        let at = self.start + offset;
        match &self.shared.backing {
            Backing::Memory(bytes) => {
                out.copy_from_slice(&bytes[at as usize..at as usize + count]);
            }
            Backing::Static(bytes) => {
                out.copy_from_slice(&bytes[at as usize..at as usize + count]);
            }
            Backing::File(file, file_len) => self.read_blocks(file, *file_len, at, out)?,
        }
        Ok(count)
    }

    /// The bytes of `out` from `at` of the file, block by block through the cache.
    fn read_blocks(
        &self,
        file: &Mutex<File>,
        file_len: u64,
        at: u64,
        out: &mut [u8],
    ) -> io::Result<()> {
        let cache = self.shared.cache.as_ref().unwrap_or_else(|| cache());
        let block = cache.block();
        let mut done = 0;
        while done < out.len() {
            let pos = at + done as u64;
            let index = pos / block as u64;
            let key = (self.shared.id, block, index);
            let bytes = match cache.get(key) {
                Some(b) => b,
                None => {
                    let first = index * block as u64;
                    let size = file_len.saturating_sub(first).min(block as u64) as usize;
                    let mut bytes = vec![0u8; size];
                    {
                        let mut f = file.lock().unwrap_or_else(|e| e.into_inner());
                        f.seek(SeekFrom::Start(first))?;
                        f.read_exact(&mut bytes)?;
                    }
                    self.shared.reads.fetch_add(1, Ordering::Relaxed);
                    let bytes: Arc<[u8]> = bytes.into();
                    cache.put(key, bytes.clone());
                    bytes
                }
            };
            let from = (pos - index * block as u64) as usize;
            if from >= bytes.len() {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
            }
            let n = (bytes.len() - from).min(out.len() - done);
            out[done..done + n].copy_from_slice(&bytes[from..from + n]);
            done += n;
        }
        Ok(())
    }

    /// `count` bytes from `offset`, or as many as there are.
    pub fn read(&self, offset: u64, count: usize) -> io::Result<Vec<u8>> {
        let count = (count as u64).min(self.len.saturating_sub(offset)) as usize;
        let mut out = vec![0u8; count];
        let n = self.read_at(offset, &mut out)?;
        out.truncate(n);
        Ok(out)
    }

    /// The first bytes, at most `count`, for the type of the bytes (`mime::SNIFFED`); none when
    /// they do not read.
    pub fn head(&self, count: usize) -> Vec<u8> {
        self.read(0, count).unwrap_or_default()
    }

    /// Every byte, when there are at most `limit`; else the reason.
    pub fn to_vec(&self, limit: u64) -> Result<Vec<u8>, String> {
        if self.len > limit {
            return Err(format!(
                "{} bytes, more than the {limit} the view reads whole",
                self.len
            ));
        }
        self.read(0, self.len as usize).map_err(|e| e.to_string())
    }
}

impl std::fmt::Debug for BlockFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockFile")
            .field("path", &self.shared.path)
            .field("start", &self.start)
            .field("len", &self.len)
            .finish()
    }
}

/// Two `BlockFile`s are equal when they read the same bytes of the same file or memory.
impl PartialEq for BlockFile {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
            && self.start == other.start
            && self.len == other.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str, len: u32) -> (PathBuf, Vec<u8>) {
        let name = format!("wwrt-blocks-{name}-{}.bin", std::process::id());
        let path = std::env::temp_dir().join(name);
        let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &data).unwrap();
        (path, data)
    }

    #[test]
    fn a_file_is_read_by_blocks_within_the_budget() {
        let (path, data) = sample("budget", 300_000);
        let cache = Cache::new(Limits {
            block: 65_536,
            budget: 2 * 65_536,
        });
        let file = BlockFile::open_in(&path, cache.clone()).unwrap();
        assert_eq!(file.len(), 300_000);
        let mut out = vec![0u8; 100];
        // Across the end of block 0 and the start of block 1.
        assert_eq!(file.read_at(65_500, &mut out).unwrap(), 100);
        assert_eq!(out, data[65_500..65_600]);
        assert_eq!((file.reads(), cache.state().blocks), (2, 2));
        // Block 1 again from the cache, then block 4 pushes out block 0, the oldest.
        file.read_at(70_000, &mut out).unwrap();
        assert_eq!(file.reads(), 2);
        let mut tail = vec![0u8; 100];
        assert_eq!(file.read_at(299_950, &mut tail).unwrap(), 50);
        assert_eq!(tail[..50], data[299_950..]);
        assert_eq!((file.reads(), cache.state().blocks), (3, 2));
        file.read_at(70_000, &mut out).unwrap();
        assert_eq!(file.reads(), 3, "block 1 stayed, it was used after block 0");
        file.read_at(0, &mut out).unwrap();
        assert_eq!(file.reads(), 4, "block 0 left the cache");
        assert_eq!(file.read_at(400_000, &mut out).unwrap(), 0);
        let state = cache.state();
        assert_eq!((state.files, state.reads, state.hits), (1, 4, 2));
        assert!(state.used <= state.limits.budget);
        drop(file);
        let state = cache.state();
        assert_eq!(
            (state.blocks, state.files),
            (0, 0),
            "a file closed takes its blocks along"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_part_reads_the_blocks_of_its_file() {
        let (path, data) = sample("part", 200_000);
        let cache = Cache::new(Limits {
            block: 16_384,
            budget: 1 << 20,
        });
        let file = BlockFile::open_in(&path, cache.clone()).unwrap();
        let part = file.slice(100_000, 50_000);
        assert_eq!((part.len(), part.offset()), (50_000, 100_000));
        assert_eq!(part.read(0, 10).unwrap(), data[100_000..100_010]);
        assert_eq!(part.read(49_995, 100).unwrap(), data[149_995..150_000]);
        let reads = file.reads();
        assert_eq!(file.read(100_000, 10).unwrap(), data[100_000..100_010]);
        assert_eq!(
            file.reads(),
            reads,
            "the part read the blocks for the whole file"
        );
        let inner = part.slice(10, 5);
        assert_eq!(inner.read(0, 100).unwrap(), data[100_010..100_015]);
        assert_eq!(file.slice(250_000, 10).len(), 0);
        assert_eq!(
            part.to_vec(10).unwrap_err(),
            "50000 bytes, more than the 10 the view reads whole"
        );
        assert_eq!(part.head(3), data[100_000..100_003]);
        assert_eq!(part.path(), Some(path.as_path()));
        cache.set_limits(Limits {
            block: 8_192,
            budget: 1 << 20,
        });
        assert_eq!(
            cache.state().blocks,
            0,
            "another size of a block empties it"
        );
        assert_eq!(part.read(0, 10).unwrap(), data[100_000..100_010]);
        drop((file, part, inner));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn bytes_in_memory_read_like_a_file() {
        let bytes = BlockFile::from_bytes((0..10u8).collect());
        assert_eq!(bytes.len(), 10);
        let mut out = [0u8; 4];
        assert_eq!(bytes.read_at(8, &mut out).unwrap(), 2);
        assert_eq!(out[..2], [8, 9]);
        assert_eq!(bytes.read_at(20, &mut out).unwrap(), 0);
        assert_eq!(bytes.reads(), 0);
        assert_eq!(bytes.slice(2, 3).to_vec(10).unwrap(), [2, 3, 4]);
        static EMBEDDED: [u8; 3] = [7, 8, 9];
        let fixed = BlockFile::from_static(&EMBEDDED);
        assert_eq!(fixed.slice(1, 9).read(0, 9).unwrap(), [8, 9]);
        assert!(fixed.path().is_none());
        let least = Limits {
            block: 1,
            budget: 0,
        };
        assert_eq!(least.checked().block, MIN_BLOCK);
        assert_eq!(least.checked().budget, MIN_BLOCK);
    }
}
