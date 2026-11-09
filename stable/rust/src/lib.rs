//! Plugins of the stable interface of W# (`ww.h`, version 1) in Rust of any toolchain: a plugin
//! of this crate loads into every program on W# that knows the interface, whatever compiler built
//! it, as one in C does (`docs/stable.md`).
//!
//! A plugin is a `cdylib` crate: a type that implements `View` (a frame of input in, a picture,
//! controls, the keys it takes, a cursor and a line of the status bar out; a control changed), a
//! `Plugin` (its name, its formats with their MIME types, the files it embeds, the functions that
//! open a file or some bytes and that find its types in the first bytes of a file), and
//! `plugin!`, which exports `ww_plugin_get`. Some bytes come as a `Source`, read in blocks through
//! the cache of the host; `open_file` reads any file the same way, `cache` tells its state. The
//! crate keeps what a frame gives (its texts, its controls) until the next call on the view,
//! reads the input and writes the output by the sizes the host gives, and turns a panic of the
//! plugin into a message of the view.
//!
//! ```
//! use ww_stable::{Control, Format, Frame, Input, Picture, Plugin, Source, View};
//!
//! struct Grey { level: f64, pixels: Vec<u8> }
//!
//! impl View for Grey {
//!     fn frame(&mut self, input: &Input) -> Frame<'_> {
//!         let (w, h) = (input.width.max(1), input.height.max(1));
//!         let v = (self.level * 25.5) as u8;
//!         self.pixels = [v, v, v, 255].repeat((w * h) as usize);
//!         Frame {
//!             picture: Some(Picture::new(w, h, &self.pixels)),
//!             changed: true,
//!             controls: vec![Control::slider(1, "Level", self.level, 0.0, 10.0, 1.0)],
//!             status: format!("level {}", self.level),
//!             ..Frame::default()
//!         }
//!     }
//!
//!     fn control(&mut self, id: u32, value: f64) {
//!         if id == 1 {
//!             self.level = value;
//!         }
//!     }
//! }
//!
//! /// The level of a grey file: its first byte.
//! fn level(first: Option<u8>) -> f64 {
//!     f64::from(first.unwrap_or(5).min(10))
//! }
//!
//! ww_stable::plugin!(Grey, Plugin {
//!     name: "Grey",
//!     formats: &[Format {
//!         name: "Grey files",
//!         extensions: &["grey"],
//!         mimes: &["application/x-grey"],
//!     }],
//!     files: &[],
//!     open: |path: &str| {
//!         let first = std::fs::read(path).ok().and_then(|b| b.first().copied());
//!         Ok(Grey { level: level(first), pixels: Vec::new() })
//!     },
//!     open_bytes: Some(|source: Source, _name: &str, _mime: &str| {
//!         let first = source.read(0, 1).first().copied();
//!         Ok(Grey { level: level(first), pixels: Vec::new() })
//!     }),
//!     sniff: Some(|head: &[u8]| head.starts_with(b"GREY").then_some("application/x-grey")),
//! });
//! ```

use std::ffi::{c_char, CStr, CString};
use std::sync::atomic::{AtomicPtr, Ordering};

pub mod ffi;

/// The version of `ww.h` the crate writes.
pub const API: u32 = ffi::WW_API_VERSION;

/// The most keys and controls of a frame the crate reads or gives.
const MOST_KEYS: u32 = 256;
const MOST_CHOICES: usize = 256;
/// The most bytes `sniff` of a plugin is given.
pub const SNIFFED: usize = 512;

/// A kind of file a plugin opens: its name, its extensions without the dot in lower case, and its
/// MIME types, which the host finds the plugin of some bytes by.
pub struct Format {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub mimes: &'static [&'static str],
}

/// A file the plugin embeds (`include_bytes!`): a page of its guide, a sample, a translation of
/// its texts (`lang/<code>/ui.json`).
pub struct File {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

/// The keys held with a key or with the mouse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Modifiers {
    fn of(bits: u32) -> Self {
        Modifiers {
            shift: bits & ffi::WW_SHIFT != 0,
            ctrl: bits & ffi::WW_CTRL != 0,
            alt: bits & ffi::WW_ALT != 0,
        }
    }
}

/// The buttons of the mouse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons {
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

impl Buttons {
    fn of(bits: u32) -> Self {
        Buttons {
            left: bits & ffi::WW_LEFT != 0,
            right: bits & ffi::WW_RIGHT != 0,
            middle: bits & ffi::WW_MIDDLE != 0,
        }
    }

    /// Whether any button is in.
    pub fn any(&self) -> bool {
        self.left || self.right || self.middle
    }
}

/// A key of the view: its name of W3C (`ArrowLeft`, `PageUp`, `Escape`, `a`, `+`), pressed or
/// released, with the modifiers held.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub name: String,
    pub pressed: bool,
    pub modifiers: Modifiers,
}

/// The input of a frame: the size of the view in pixels and their scale, the focus, the mouse
/// in the pixels of the view, the keys and the text since the last frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Input {
    pub width: u32,
    pub height: u32,
    /// Pixels a point of the window.
    pub scale: f32,
    pub focused: bool,
    pub hovered: bool,
    pub mouse: [f32; 2],
    pub held: Buttons,
    pub clicked: Buttons,
    pub double_clicked: Buttons,
    /// The pixels the pointer moved with a button down.
    pub drag: [f32; 2],
    /// Lines of the wheel, down positive.
    pub wheel: [f32; 2],
    pub modifiers: Modifiers,
    pub keys: Vec<Key>,
    pub text: String,
}

/// The groups of keys a view takes while it has the focus (`Frame::keys`); the others stay the
/// window's.
pub mod keys {
    use crate::ffi;

    pub const ARROWS: u32 = ffi::WW_KEYS_ARROWS;
    pub const PAGES: u32 = ffi::WW_KEYS_PAGES;
    pub const LETTERS: u32 = ffi::WW_KEYS_LETTERS;
    pub const DIGITS: u32 = ffi::WW_KEYS_DIGITS;
    pub const SIGNS: u32 = ffi::WW_KEYS_SIGNS;
    pub const SPACE: u32 = ffi::WW_KEYS_SPACE;
    pub const ESCAPE: u32 = ffi::WW_KEYS_ESCAPE;
    pub const ENTER: u32 = ffi::WW_KEYS_ENTER;
}

/// The cursor over the view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    #[default]
    Default,
    Cross,
    Hand,
    Grab,
    Grabbing,
    ZoomIn,
    ZoomOut,
    Move,
}

impl Cursor {
    fn code(self) -> u32 {
        match self {
            Cursor::Default => ffi::WW_CURSOR_DEFAULT,
            Cursor::Cross => ffi::WW_CURSOR_CROSS,
            Cursor::Hand => ffi::WW_CURSOR_HAND,
            Cursor::Grab => ffi::WW_CURSOR_GRAB,
            Cursor::Grabbing => ffi::WW_CURSOR_GRABBING,
            Cursor::ZoomIn => ffi::WW_CURSOR_ZOOM_IN,
            Cursor::ZoomOut => ffi::WW_CURSOR_ZOOM_OUT,
            Cursor::Move => ffi::WW_CURSOR_MOVE,
        }
    }
}

/// What a control of the bar is, with its value.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Button,
    Toggle(bool),
    Slider {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    /// The index of one of the choices.
    Choice {
        index: usize,
        choices: Vec<String>,
    },
    Label,
}

/// A control of the bar of the view; a change comes back to `View::control` by its id.
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub id: u32,
    pub label: String,
    pub kind: Kind,
}

impl Control {
    pub fn button(id: u32, label: &str) -> Self {
        Control {
            id,
            label: label.to_owned(),
            kind: Kind::Button,
        }
    }

    pub fn toggle(id: u32, label: &str, on: bool) -> Self {
        Control {
            id,
            label: label.to_owned(),
            kind: Kind::Toggle(on),
        }
    }

    pub fn slider(id: u32, label: &str, value: f64, min: f64, max: f64, step: f64) -> Self {
        Control {
            id,
            label: label.to_owned(),
            kind: Kind::Slider {
                value,
                min,
                max,
                step,
            },
        }
    }

    pub fn choice(id: u32, label: &str, index: usize, choices: &[&str]) -> Self {
        Control {
            id,
            label: label.to_owned(),
            kind: Kind::Choice {
                index,
                choices: choices.iter().map(|c| (*c).to_owned()).collect(),
            },
        }
    }

    pub fn label(id: u32, label: &str) -> Self {
        Control {
            id,
            label: label.to_owned(),
            kind: Kind::Label,
        }
    }
}

/// A picture of RGBA pixels, 8 bits a channel, `stride` bytes a row, kept by the view until its
/// next frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixels: &'a [u8],
}

impl<'a> Picture<'a> {
    /// A picture whose rows follow one another.
    pub fn new(width: u32, height: u32, pixels: &'a [u8]) -> Self {
        Picture {
            width,
            height,
            stride: width.saturating_mul(4),
            pixels,
        }
    }

    /// Whether the pixels hold every row the size and the stride say.
    fn whole(&self) -> bool {
        if self.width == 0 || self.height == 0 {
            return false;
        }
        let row = self.width as usize * 4;
        let stride = self.stride as usize;
        stride >= row && self.pixels.len() >= stride * (self.height as usize - 1) + row
    }
}

/// What a frame gives: the picture (none keeps the last one), whether it changed, the controls of
/// the bar, the keys the view takes (`keys::*`), the cursor, whether to draw another frame at once
/// (the plugin is at work) and a line of the status bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame<'a> {
    pub picture: Option<Picture<'a>>,
    pub changed: bool,
    pub controls: Vec<Control>,
    pub keys: u32,
    pub cursor: Cursor,
    pub again: bool,
    pub status: String,
}

/// A view of a file or of some bytes, made by a function of the `Plugin`.
pub trait View: 'static {
    /// A frame: the input since the last one in, the picture and the controls now out.
    fn frame(&mut self, input: &Input) -> Frame<'_>;
    /// A control of the bar changed: a button pressed (its value 0), a toggle (0 or 1), a slider
    /// or a choice (its index) set.
    fn control(&mut self, _id: u32, _value: f64) {}
}

/// A plugin of views `V`: its name, its formats, the files it embeds, the function that opens a
/// file (a path to the view or the reason), the one that opens some bytes under a name and a
/// MIME type (none for a plugin of files alone), and the one that finds a type of its formats in
/// the first bytes of a file (`SNIFFED` of them at most; none for a plugin of names alone).
pub struct Plugin<V> {
    pub name: &'static str,
    pub formats: &'static [Format],
    pub files: &'static [File],
    pub open: OpenFile<V>,
    pub open_bytes: Option<OpenBytes<V>>,
    pub sniff: Option<Sniff>,
}

/// The function of a plugin that opens a file: its path to the view, or the reason.
pub type OpenFile<V> = fn(&str) -> Result<V, String>;
/// The function of a plugin that opens some bytes: their source, their name and their MIME type
/// to the view, or the reason.
pub type OpenBytes<V> = fn(Source, &str, &str) -> Result<V, String>;
/// The function of a plugin that finds a type of its formats in the first bytes of a file.
pub type Sniff = fn(&[u8]) -> Option<&'static str>;

/// The table of the host, set by `ww_plugin_get`.
static HOST: AtomicPtr<ffi::WwHost> = AtomicPtr::new(std::ptr::null_mut());

/// The table of the host, the fields its size covers.
fn host() -> Option<ffi::WwHost> {
    let raw = HOST.load(Ordering::Acquire);
    if raw.is_null() {
        return None;
    }
    // SAFETY: the table of the host lives for the run; the bytes copied are the ones its size
    // covers, the others zero.
    unsafe {
        let mut own: ffi::WwHost = std::mem::zeroed();
        let n = ((*raw).size as usize).min(std::mem::size_of::<ffi::WwHost>());
        std::ptr::copy_nonoverlapping(
            raw.cast::<u8>(),
            (&mut own as *mut ffi::WwHost).cast::<u8>(),
            n,
        );
        Some(own)
    }
}

/// Some bytes the host reads for the plugin: a file, or a part of one, read in blocks through
/// the cache of the host, or bytes in memory. The source of `Plugin::open_bytes` lives as long as
/// its view; one of `open_file` or `slice` until it is dropped.
pub struct Source {
    raw: *const ffi::WwSource,
    /// Whether the plugin frees it (`open_file`, `slice`): the one of a view is the host's.
    owned: bool,
}

// SAFETY: the header lets any thread of the plugin read a source.
unsafe impl Send for Source {}

impl Source {
    /// The source of the host for the call of `open_bytes`, which the host frees.
    ///
    /// # Safety
    /// `raw` is a source the host gave, alive as long as this value.
    pub unsafe fn borrowed(raw: *const ffi::WwSource) -> Self {
        Source { raw, owned: false }
    }

    /// The fields of the source its size covers.
    fn raw(&self) -> ffi::WwSource {
        // SAFETY: by the contract of the constructors, the source lives; the bytes copied are the
        // ones its size covers, the others zero.
        unsafe {
            let mut own: ffi::WwSource = std::mem::zeroed();
            if self.raw.is_null() {
                return own;
            }
            let n = ((*self.raw).size as usize).min(std::mem::size_of::<ffi::WwSource>());
            std::ptr::copy_nonoverlapping(
                self.raw.cast::<u8>(),
                (&mut own as *mut ffi::WwSource).cast::<u8>(),
                n,
            );
            own
        }
    }

    /// The bytes of the source.
    pub fn len(&self) -> u64 {
        self.raw().length
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The file the bytes come from; none for bytes in memory.
    pub fn path(&self) -> Option<String> {
        let path = self.raw().path;
        // SAFETY: the path of a source ends with a zero and lives as long as it.
        (!path.is_null()).then(|| unsafe { text_of(path) })
    }

    /// Where the bytes start in their file.
    pub fn offset(&self) -> u64 {
        self.raw().offset
    }

    /// Copies the bytes from `offset` into `out`, as many as there are; gives the count.
    pub fn read_at(&self, offset: u64, out: &mut [u8]) -> usize {
        let raw = self.raw();
        let Some(read) = raw.read else {
            return 0;
        };
        let mut done = 0;
        while done < out.len() {
            let count = (out.len() - done).min(u32::MAX as usize) as u32;
            // SAFETY: the source lives; `out` holds `count` bytes from `done`.
            let n = unsafe {
                read(
                    raw.self_,
                    offset + done as u64,
                    out[done..].as_mut_ptr(),
                    count,
                )
            } as usize;
            if n == 0 {
                break;
            }
            done += n.min(count as usize);
        }
        done
    }

    /// `count` bytes from `offset`, or as many as there are.
    pub fn read(&self, offset: u64, count: usize) -> Vec<u8> {
        let count = (count as u64).min(self.len().saturating_sub(offset)) as usize;
        let mut out = vec![0u8; count];
        let n = self.read_at(offset, &mut out);
        out.truncate(n);
        out
    }

    /// A part of the source as a source of its own, reading the same blocks; none when the host
    /// has no `slice`.
    pub fn slice(&self, offset: u64, length: u64) -> Option<Source> {
        let slice = host()?.slice?;
        // SAFETY: the source lives; the host gives a new one or null.
        let raw = unsafe { slice(self.raw, offset, length) };
        (!raw.is_null()).then_some(Source { raw, owned: true })
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        if !self.owned || self.raw.is_null() {
            return;
        }
        if let Some(close) = host().and_then(|h| h.close_source) {
            // SAFETY: a source of `open_file` or `slice`, freed once.
            unsafe { close(self.raw) };
        }
    }
}

/// A file read in blocks through the cache of the host, or the reason.
pub fn open_file(path: &str) -> Result<Source, String> {
    let open = host()
        .and_then(|h| h.open_file)
        .ok_or_else(|| "the host reads no file for the plugin".to_owned())?;
    let path = c_text(path);
    let mut error = vec![0 as c_char; 1024];
    // SAFETY: a path that ends with a zero and a buffer of its size.
    let raw = unsafe { open(path.as_ptr(), error.as_mut_ptr(), error.len() as u32) };
    if raw.is_null() {
        error[1023] = 0;
        // SAFETY: the buffer ends with a zero.
        return Err(unsafe { text_of(error.as_ptr()) });
    }
    Ok(Source { raw, owned: true })
}

/// The state of the cache of the blocks of the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cache {
    pub block: u32,
    pub budget: u64,
    pub used: u64,
    pub blocks: u64,
    pub files: u64,
    pub reads: u64,
    pub hits: u64,
}

/// The state of the cache of the host; none when the host does not tell it.
pub fn cache() -> Option<Cache> {
    let get = host()?.cache?;
    let mut raw = ffi::WwCache {
        size: std::mem::size_of::<ffi::WwCache>() as u32,
        ..ffi::WwCache::default()
    };
    // SAFETY: a structure of its size for the call.
    unsafe { get(&mut raw) };
    Some(Cache {
        block: raw.block,
        budget: raw.budget,
        used: raw.used,
        blocks: raw.blocks,
        files: raw.files,
        reads: raw.reads,
        hits: raw.hits,
    })
}

/// A text for C: UTF-8 ending with a zero, the zeros inside it left out.
fn c_text(text: &str) -> CString {
    CString::new(text.replace('\0', "")).unwrap_or_default()
}

/// The text of a pointer of C, empty for none.
///
/// # Safety
/// `text` is null or a text that ends with a zero.
unsafe fn text_of(text: *const c_char) -> String {
    if text.is_null() {
        String::new()
    } else {
        // SAFETY: by the contract of the caller.
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    }
}

/// The message of a panic.
fn message(panic: Box<dyn std::any::Any + Send>) -> String {
    let why = panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_default();
    format!("the plugin stopped: {why}")
}

/// The table of a plugin and the texts and the arrays it points to (`plugin!` makes it once).
pub struct Table {
    plugin: ffi::WwPlugin,
    texts: Vec<CString>,
    _formats: Vec<ffi::WwFormat>,
    _lists: Vec<Vec<*const c_char>>,
    _files: Vec<ffi::WwFile>,
}

// SAFETY: the table and what it points to are written once and only read after; the pointers
// lead into the buffers of the table itself, which never move.
unsafe impl Send for Table {}
// SAFETY: as for Send.
unsafe impl Sync for Table {}

impl Table {
    /// The table of a plugin with the functions of the table (`plugin!` writes it): `open_bytes`
    /// and `sniff` are given to the host when the plugin has them.
    pub fn new<V: View>(
        plugin: &Plugin<V>,
        open: ffi::OpenFn,
        open_bytes: ffi::OpenBytesFn,
        sniff: ffi::SniffFn,
    ) -> Table {
        // Every text first: a CString keeps its buffer when the vector of them grows, so the
        // pointers taken after stay valid.
        let mut texts = vec![c_text(plugin.name)];
        for f in plugin.formats {
            texts.push(c_text(f.name));
            texts.extend(f.extensions.iter().map(|e| c_text(e)));
            texts.extend(f.mimes.iter().map(|m| c_text(m)));
        }
        for f in plugin.files {
            texts.push(c_text(f.path));
        }
        let mut at = 1;
        let mut lists: Vec<Vec<*const c_char>> = Vec::new();
        let mut names = Vec::new();
        for f in plugin.formats {
            names.push(texts[at].as_ptr());
            at += 1;
            let extensions: Vec<*const c_char> = texts[at..at + f.extensions.len()]
                .iter()
                .map(|t| t.as_ptr())
                .collect();
            at += f.extensions.len();
            let mimes: Vec<*const c_char> = texts[at..at + f.mimes.len()]
                .iter()
                .map(|t| t.as_ptr())
                .collect();
            at += f.mimes.len();
            lists.push(extensions);
            lists.push(mimes);
        }
        let formats: Vec<ffi::WwFormat> = plugin
            .formats
            .iter()
            .enumerate()
            .map(|(i, f)| ffi::WwFormat {
                name: names[i],
                extensions: lists[2 * i].as_ptr(),
                extension_count: f.extensions.len() as u32,
                mimes: lists[2 * i + 1].as_ptr(),
                mime_count: f.mimes.len() as u32,
            })
            .collect();
        let files: Vec<ffi::WwFile> = plugin
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| ffi::WwFile {
                path: texts[at + i].as_ptr(),
                bytes: f.bytes.as_ptr(),
                size: f.bytes.len() as u64,
            })
            .collect();
        Table {
            plugin: ffi::WwPlugin {
                api_version: API,
                size: std::mem::size_of::<ffi::WwPlugin>() as u32,
                name: texts[0].as_ptr(),
                formats: formats.as_ptr(),
                format_count: formats.len() as u32,
                files: if files.is_empty() {
                    std::ptr::null()
                } else {
                    files.as_ptr()
                },
                file_count: files.len() as u32,
                open: Some(open),
                open_bytes: plugin.open_bytes.map(|_| open_bytes),
                sniff: plugin.sniff.map(|_| sniff),
                frame: Some(glue::frame::<V>),
                control: Some(glue::control::<V>),
                close: Some(glue::close::<V>),
            },
            texts,
            _formats: formats,
            _lists: lists,
            _files: files,
        }
    }

    /// The table for `ww_plugin_get`.
    pub fn plugin(&self) -> *const ffi::WwPlugin {
        &self.plugin
    }

    /// The text of the table that spells a MIME type, which lives for the run.
    fn mime(&self, mime: &str) -> *const c_char {
        self.texts
            .iter()
            .find(|t| t.to_bytes().eq_ignore_ascii_case(mime.as_bytes()))
            .map_or(std::ptr::null(), |t| t.as_ptr())
    }
}

/// The functions of the table over a `View`, for `plugin!`.
#[doc(hidden)]
pub mod glue {
    use std::ffi::{c_char, c_void, CString};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::*;

    /// Keeps the table of the host for the functions of the crate.
    ///
    /// # Safety
    /// `host` is null or the table of the host, which lives for the run.
    pub unsafe fn set_host(host: *const ffi::WwHost) {
        if !host.is_null() {
            HOST.store(host.cast_mut(), Ordering::Release);
        }
    }

    /// A view with what its last frame gave, kept until the next call on it.
    struct Holder<V> {
        view: V,
        kept: Kept,
        /// Why the view stopped: a panic of the plugin, after which it is called no more.
        stopped: Option<CString>,
    }

    #[derive(Default)]
    struct Kept {
        status: CString,
        labels: Vec<CString>,
        choices: Vec<Vec<CString>>,
        pointers: Vec<Vec<*const c_char>>,
        controls: Vec<ffi::WwControl>,
    }

    /// The view the function of the plugin made, or null with the reason.
    ///
    /// # Safety
    /// `error` is null or a buffer of `size` bytes.
    unsafe fn made<V: View>(
        made: std::thread::Result<Result<V, String>>,
        error: *mut c_char,
        size: u32,
    ) -> *mut c_void {
        let why = match made {
            Ok(Ok(view)) => {
                let holder = Holder {
                    view,
                    kept: Kept::default(),
                    stopped: None,
                };
                return Box::into_raw(Box::new(holder)).cast();
            }
            Ok(Err(why)) => why,
            Err(panic) => message(panic),
        };
        // SAFETY: by the contract of the caller.
        unsafe { write_error(error, size, &why) };
        std::ptr::null_mut()
    }

    /// Opens a file with the function of the plugin: the view, or null with the reason.
    ///
    /// # Safety
    /// `path` is null or a text that ends with a zero; `error` is null or a buffer of `size`
    /// bytes.
    pub unsafe fn open<V: View>(
        path: *const c_char,
        error: *mut c_char,
        size: u32,
        make: OpenFile<V>,
    ) -> *mut c_void {
        // SAFETY: by the contract of the caller.
        unsafe {
            let path = text_of(path);
            made(catch_unwind(AssertUnwindSafe(|| make(&path))), error, size)
        }
    }

    /// Opens some bytes with the function of the plugin: the view, or null with the reason.
    ///
    /// # Safety
    /// `source` is a source of the host that lives as long as the view; `name` and `mime` are
    /// null or texts that end with a zero; `error` is null or a buffer of `size` bytes.
    pub unsafe fn open_bytes<V: View>(
        source: *const ffi::WwSource,
        name: *const c_char,
        mime: *const c_char,
        error: *mut c_char,
        size: u32,
        make: Option<OpenBytes<V>>,
    ) -> *mut c_void {
        // SAFETY: by the contract of the caller.
        unsafe {
            let Some(make) = make else {
                write_error(error, size, "the plugin opens files alone");
                return std::ptr::null_mut();
            };
            let (name, mime) = (text_of(name), text_of(mime));
            let source = Source::borrowed(source);
            let view = catch_unwind(AssertUnwindSafe(|| make(source, &name, &mime)));
            made(view, error, size)
        }
    }

    /// The type of the first bytes by the function of the plugin, as a text of the table.
    ///
    /// # Safety
    /// `bytes` is null or holds `count` bytes.
    pub unsafe fn sniff(
        bytes: *const u8,
        count: u32,
        find: Option<Sniff>,
        table: Option<&Table>,
    ) -> *const c_char {
        let (Some(find), Some(table)) = (find, table) else {
            return std::ptr::null();
        };
        if bytes.is_null() {
            return std::ptr::null();
        }
        // SAFETY: by the contract of the caller.
        let head = unsafe { std::slice::from_raw_parts(bytes, (count as usize).min(SNIFFED)) };
        match catch_unwind(AssertUnwindSafe(|| find(head))) {
            Ok(Some(mime)) => table.mime(mime),
            _ => std::ptr::null(),
        }
    }

    /// Writes a reason into the buffer of the host, cut at a character, with its zero.
    ///
    /// # Safety
    /// `error` is null or a buffer of `size` bytes.
    unsafe fn write_error(error: *mut c_char, size: u32, why: &str) {
        if error.is_null() || size == 0 {
            return;
        }
        let mut n = why.len().min(size as usize - 1);
        while !why.is_char_boundary(n) {
            n -= 1;
        }
        // SAFETY: n + 1 bytes fit the buffer.
        unsafe {
            std::ptr::copy_nonoverlapping(why.as_ptr().cast::<c_char>(), error, n);
            *error.add(n) = 0;
        }
    }

    /// The input of the host, the fields its size covers, the others zero.
    ///
    /// # Safety
    /// `input` is null or the input of the host for the call.
    unsafe fn read_input(input: *const ffi::WwInput) -> Input {
        if input.is_null() {
            return Input::default();
        }
        // SAFETY: the input of the host is valid for the call; a struct of integers, floats and
        // pointers is valid when zero, and the bytes copied are the ones the size covers.
        unsafe {
            let mut raw: ffi::WwInput = std::mem::zeroed();
            let n = ((*input).size as usize).min(std::mem::size_of::<ffi::WwInput>());
            std::ptr::copy_nonoverlapping(
                input.cast::<u8>(),
                (&mut raw as *mut ffi::WwInput).cast::<u8>(),
                n,
            );
            let mut keys = Vec::new();
            if !raw.keys.is_null() {
                for i in 0..raw.key_count.min(MOST_KEYS) as usize {
                    let k = &*raw.keys.add(i);
                    keys.push(Key {
                        name: text_of(k.name),
                        pressed: k.pressed != 0,
                        modifiers: Modifiers::of(k.modifiers),
                    });
                }
            }
            Input {
                width: raw.width,
                height: raw.height,
                scale: raw.scale,
                focused: raw.focused != 0,
                hovered: raw.hovered != 0,
                mouse: [raw.mouse_x, raw.mouse_y],
                held: Buttons::of(raw.buttons),
                clicked: Buttons::of(raw.clicks),
                double_clicked: Buttons::of(raw.double_clicks),
                drag: [raw.drag_x, raw.drag_y],
                wheel: [raw.wheel_x, raw.wheel_y],
                modifiers: Modifiers::of(raw.modifiers),
                keys,
                text: text_of(raw.text),
            }
        }
    }

    /// The output of a frame and what it points to.
    fn output(frame: &Frame<'_>) -> (ffi::WwOutput, Kept) {
        let mut kept = Kept {
            status: c_text(&frame.status),
            ..Kept::default()
        };
        for c in &frame.controls {
            kept.labels.push(c_text(&c.label));
            let texts: Vec<CString> = match &c.kind {
                Kind::Choice { choices, .. } => choices
                    .iter()
                    .take(MOST_CHOICES)
                    .map(|t| c_text(t))
                    .collect(),
                _ => Vec::new(),
            };
            kept.pointers
                .push(texts.iter().map(|t| t.as_ptr()).collect());
            kept.choices.push(texts);
        }
        for (i, c) in frame.controls.iter().enumerate() {
            let (kind, value, min, max, step) = match &c.kind {
                Kind::Button => (ffi::WW_BUTTON, 0.0, 0.0, 0.0, 0.0),
                Kind::Toggle(on) => (ffi::WW_TOGGLE, f64::from(u8::from(*on)), 0.0, 1.0, 1.0),
                Kind::Slider {
                    value,
                    min,
                    max,
                    step,
                } => (ffi::WW_SLIDER, *value, *min, *max, *step),
                Kind::Choice { index, choices } => {
                    let last = choices.len().saturating_sub(1) as f64;
                    (ffi::WW_CHOICE, *index as f64, 0.0, last, 1.0)
                }
                Kind::Label => (ffi::WW_LABEL, 0.0, 0.0, 0.0, 0.0),
            };
            let choices = &kept.pointers[i];
            kept.controls.push(ffi::WwControl {
                id: c.id,
                kind,
                label: kept.labels[i].as_ptr(),
                value,
                min,
                max,
                step,
                choices: if choices.is_empty() {
                    std::ptr::null()
                } else {
                    choices.as_ptr()
                },
                choice_count: choices.len() as u32,
            });
        }
        let picture = frame.picture.filter(Picture::whole);
        let out = ffi::WwOutput {
            size: 0,
            width: picture.map_or(0, |p| p.width),
            height: picture.map_or(0, |p| p.height),
            stride: picture.map_or(0, |p| p.stride),
            pixels: picture.map_or(std::ptr::null(), |p| p.pixels.as_ptr()),
            changed: u32::from(frame.changed && picture.is_some()),
            controls: if kept.controls.is_empty() {
                std::ptr::null()
            } else {
                kept.controls.as_ptr()
            },
            control_count: kept.controls.len() as u32,
            keys: frame.keys,
            cursor: frame.cursor.code(),
            again: u32::from(frame.again),
            status: kept.status.as_ptr(),
        };
        (out, kept)
    }

    /// Writes the fields of the output its size covers.
    ///
    /// # Safety
    /// `out` is null or the output of the host for the call.
    unsafe fn write_output(out: *mut ffi::WwOutput, mut written: ffi::WwOutput) {
        if out.is_null() {
            return;
        }
        // SAFETY: the output of the host is valid for the call, and the bytes written are the
        // ones its size covers.
        unsafe {
            written.size = (*out).size;
            let n = (written.size as usize).min(std::mem::size_of::<ffi::WwOutput>());
            std::ptr::copy_nonoverlapping(
                (&written as *const ffi::WwOutput).cast::<u8>(),
                out.cast::<u8>(),
                n,
            );
        }
    }

    /// A frame of the view.
    ///
    /// # Safety
    /// `view` is a view `open` gave for `V` and not closed; `input` and `out` are the ones of the
    /// host for the call.
    pub unsafe extern "C" fn frame<V: View>(
        view: *mut c_void,
        input: *const ffi::WwInput,
        out: *mut ffi::WwOutput,
    ) {
        if view.is_null() {
            return;
        }
        // SAFETY: by the contract of the caller.
        unsafe {
            let holder = &mut *view.cast::<Holder<V>>();
            let input = read_input(input);
            if holder.stopped.is_none() {
                let made = catch_unwind(AssertUnwindSafe(|| output(&holder.view.frame(&input))));
                match made {
                    Ok((written, kept)) => {
                        // The pointers of the output lead into the buffers of `kept`, which keep
                        // their place in the holder until the next frame.
                        holder.kept = kept;
                        write_output(out, written);
                        return;
                    }
                    Err(panic) => holder.stopped = Some(c_text(&message(panic))),
                }
            }
            let mut written: ffi::WwOutput = std::mem::zeroed();
            written.status = holder
                .stopped
                .as_ref()
                .map_or(std::ptr::null(), |s| s.as_ptr());
            write_output(out, written);
        }
    }

    /// A control of the bar changed.
    ///
    /// # Safety
    /// `view` is a view `open` gave for `V` and not closed.
    pub unsafe extern "C" fn control<V: View>(view: *mut c_void, id: u32, value: f64) {
        if view.is_null() {
            return;
        }
        // SAFETY: by the contract of the caller.
        let holder = unsafe { &mut *view.cast::<Holder<V>>() };
        if holder.stopped.is_none() {
            let changed = catch_unwind(AssertUnwindSafe(|| holder.view.control(id, value)));
            if let Err(panic) = changed {
                holder.stopped = Some(c_text(&message(panic)));
            }
        }
    }

    /// The view closes and is dropped.
    ///
    /// # Safety
    /// `view` is a view `open` gave for `V`, closed once.
    pub unsafe extern "C" fn close<V: View>(view: *mut c_void) {
        if view.is_null() {
            return;
        }
        // SAFETY: by the contract of the caller.
        let holder = unsafe { Box::from_raw(view.cast::<Holder<V>>()) };
        let _ = catch_unwind(AssertUnwindSafe(move || drop(holder)));
    }
}

/// Exports `ww_plugin_get`, the one function of `ww.h`, for a plugin of views of a type: the
/// type and its `Plugin`, a constant expression.
///
/// ```ignore
/// ww_stable::plugin!(Grey, Plugin {
///     name: "Grey",
///     formats: &[Format { name: "Grey files", extensions: &["grey"], mimes: &[] }],
///     files: &[],
///     open: |path: &str| Grey::read(path),
///     open_bytes: None,
///     sniff: None,
/// });
/// ```
#[macro_export]
macro_rules! plugin {
    ($view:ty, $plugin:expr $(,)?) => {
        /// The table of the plugin (`ww.h`): the host gives its own.
        // The host gives its table, which lives for the run (ww.h): the pointer is kept, never
        // read here.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[no_mangle]
        pub extern "C" fn ww_plugin_get(
            host: *const $crate::ffi::WwHost,
        ) -> *const $crate::ffi::WwPlugin {
            const PLUGIN: $crate::Plugin<$view> = $plugin;
            unsafe extern "C" fn open(
                path: *const ::std::ffi::c_char,
                error: *mut ::std::ffi::c_char,
                size: u32,
            ) -> *mut ::std::ffi::c_void {
                // SAFETY: the host gives a path that ends with a zero and a buffer of `size`
                // bytes, as ww.h says.
                unsafe { $crate::glue::open::<$view>(path, error, size, PLUGIN.open) }
            }
            unsafe extern "C" fn open_bytes(
                source: *const $crate::ffi::WwSource,
                name: *const ::std::ffi::c_char,
                mime: *const ::std::ffi::c_char,
                error: *mut ::std::ffi::c_char,
                size: u32,
            ) -> *mut ::std::ffi::c_void {
                // SAFETY: the host gives a source that lives as long as the view, the texts that
                // end with a zero and a buffer of `size` bytes, as ww.h says.
                unsafe {
                    $crate::glue::open_bytes::<$view>(
                        source,
                        name,
                        mime,
                        error,
                        size,
                        PLUGIN.open_bytes,
                    )
                }
            }
            unsafe extern "C" fn sniff(bytes: *const u8, count: u32) -> *const ::std::ffi::c_char {
                // SAFETY: the host gives `count` bytes for the call.
                unsafe { $crate::glue::sniff(bytes, count, PLUGIN.sniff, TABLE.get()) }
            }
            static TABLE: ::std::sync::OnceLock<$crate::Table> = ::std::sync::OnceLock::new();
            // SAFETY: the host gives its table, which lives for the run.
            unsafe { $crate::glue::set_host(host) };
            TABLE
                .get_or_init(|| $crate::Table::new::<$view>(&PLUGIN, open, open_bytes, sniff))
                .plugin()
        }
    };
}

#[cfg(test)]
mod tests;
