//! The stable interface of the plugins (`stable/ww.h`, version 1, docs/stable.md): a DLL of any
//! language with a C FFI that exports `ww_plugin_get`. The structures here are the ones of the
//! header; `wrap` makes of such a table an ordinary `Plugin`, whose views draw the picture of the
//! plugin as a texture, its controls as widgets of egui over the view, and give it the keys and
//! the mouse of the view. The table of the host (`host_for`) lends every such plugin the cache of
//! the blocks of the program (a file read in blocks, a part of one, the state of the cache) and
//! the call of a tool of the program by its name. A DLL that exports `ww_tools_get` has tools
//! (`wrap_tools`): the plugin made of it serves them, with views or without.

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::Path;

use serde_json::Value;
use ww::embedded::{self, Embedded};
use wwmcp::{Call, Choice, Kind, Mode, Outcome, ToolDef, ToolError};

use crate::blocks::{self, BlockFile};
use crate::egui::{self, Key, Pos2, Rect, Vec2};
use crate::input::{self, Cursor};
use crate::plugin::{Format, Host, Plugin, View};

/// The version of the interface this host knows.
pub const API: u32 = 1;

pub const SHIFT: u32 = 1;
pub const CTRL: u32 = 2;
pub const ALT: u32 = 4;
pub const LEFT: u32 = 1;
pub const RIGHT: u32 = 2;
pub const MIDDLE: u32 = 4;
pub const KEYS_ARROWS: u32 = 1;
pub const KEYS_PAGES: u32 = 2;
pub const KEYS_LETTERS: u32 = 4;
pub const KEYS_DIGITS: u32 = 8;
pub const KEYS_SIGNS: u32 = 16;
pub const KEYS_SPACE: u32 = 32;
pub const KEYS_ESCAPE: u32 = 64;
pub const KEYS_ENTER: u32 = 128;
pub const BUTTON: u32 = 0;
pub const TOGGLE: u32 = 1;
pub const SLIDER: u32 = 2;
pub const CHOICE: u32 = 3;
pub const LABEL: u32 = 4;
pub const MODE_READ: u32 = 0;
pub const MODE_EDIT: u32 = 1;
pub const MODE_FILES: u32 = 2;

/// The most formats, extensions, types and files of a table the host reads.
const MOST: u32 = 64;
const MOST_FILES: u32 = 4096;

#[repr(C)]
pub struct WwFormat {
    pub name: *const c_char,
    pub extensions: *const *const c_char,
    pub extension_count: u32,
    pub mimes: *const *const c_char,
    pub mime_count: u32,
}

#[repr(C)]
pub struct WwFile {
    pub path: *const c_char,
    pub bytes: *const u8,
    pub size: u64,
}

#[repr(C)]
pub struct WwSource {
    pub size: u32,
    pub length: u64,
    pub self_: *mut c_void,
    pub read: Option<unsafe extern "C" fn(*mut c_void, u64, *mut u8, u32) -> u32>,
    pub path: *const c_char,
    pub offset: u64,
}

#[repr(C)]
#[derive(Default)]
pub struct WwCache {
    pub size: u32,
    pub block: u32,
    pub budget: u64,
    pub used: u64,
    pub blocks: u64,
    pub files: u64,
    pub reads: u64,
    pub hits: u64,
}

#[repr(C)]
pub struct WwHost {
    pub size: u32,
    pub api_version: u32,
    pub open_file: Option<unsafe extern "C" fn(*const c_char, *mut c_char, u32) -> *const WwSource>,
    pub slice: Option<unsafe extern "C" fn(*const WwSource, u64, u64) -> *const WwSource>,
    pub close_source: Option<unsafe extern "C" fn(*const WwSource)>,
    pub cache: Option<unsafe extern "C" fn(*mut WwCache)>,
    /// The id of the plugin the table was given to, a text of the host, for `call_tool`.
    pub this: *mut c_void,
    pub call_tool: Option<CallToolFn>,
    pub free_text: Option<unsafe extern "C" fn(*mut c_char)>,
}

/// The function of the host that calls a tool by its name for a plugin.
pub type CallToolFn =
    unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, *mut *mut c_char) -> i32;
/// The function of a plugin that runs one of its tools.
pub type ToolCallFn =
    unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, *mut *mut c_char) -> i32;

#[repr(C)]
pub struct WwTool {
    pub name: *const c_char,
    pub description: *const c_char,
    pub schema: *const c_char,
    pub group: *const c_char,
    pub mode: u32,
    pub needs: *const c_char,
}

#[repr(C)]
pub struct WwOption {
    pub key: *const c_char,
    pub title: *const c_char,
    pub values: *const *const c_char,
    pub labels: *const *const c_char,
    pub value_count: u32,
    pub initial: *const c_char,
    pub help: *const c_char,
}

#[repr(C)]
pub struct WwTools {
    pub api_version: u32,
    pub size: u32,
    pub name: *const c_char,
    pub tools: *const WwTool,
    pub tool_count: u32,
    pub options: *const WwOption,
    pub option_count: u32,
    pub call: Option<ToolCallFn>,
    pub free_answer: Option<unsafe extern "C" fn(*mut c_char)>,
}

#[repr(C)]
pub struct WwKey {
    pub name: *const c_char,
    pub pressed: u32,
    pub modifiers: u32,
}

#[repr(C)]
pub struct WwInput {
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub focused: u32,
    pub hovered: u32,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub buttons: u32,
    pub clicks: u32,
    pub double_clicks: u32,
    pub drag_x: f32,
    pub drag_y: f32,
    pub wheel_x: f32,
    pub wheel_y: f32,
    pub modifiers: u32,
    pub keys: *const WwKey,
    pub key_count: u32,
    pub text: *const c_char,
}

#[repr(C)]
pub struct WwControl {
    pub id: u32,
    pub kind: u32,
    pub label: *const c_char,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub choices: *const *const c_char,
    pub choice_count: u32,
}

#[repr(C)]
pub struct WwOutput {
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixels: *const u8,
    pub changed: u32,
    pub controls: *const WwControl,
    pub control_count: u32,
    pub keys: u32,
    pub cursor: u32,
    pub again: u32,
    pub status: *const c_char,
}

/// The function that opens a file.
pub type OpenFn = unsafe extern "C" fn(*const c_char, *mut c_char, u32) -> *mut c_void;
/// The function that opens some bytes of a source under a name and a type.
pub type OpenBytesFn = unsafe extern "C" fn(
    *const WwSource,
    *const c_char,
    *const c_char,
    *mut c_char,
    u32,
) -> *mut c_void;
/// The function that finds a type in the first bytes.
pub type SniffFn = unsafe extern "C" fn(*const u8, u32) -> *const c_char;

#[repr(C)]
pub struct WwPlugin {
    pub api_version: u32,
    pub size: u32,
    pub name: *const c_char,
    pub formats: *const WwFormat,
    pub format_count: u32,
    pub files: *const WwFile,
    pub file_count: u32,
    pub open: Option<OpenFn>,
    pub open_bytes: Option<OpenBytesFn>,
    pub sniff: Option<SniffFn>,
    pub frame: Option<unsafe extern "C" fn(*mut c_void, *const WwInput, *mut WwOutput)>,
    pub control: Option<unsafe extern "C" fn(*mut c_void, u32, f64)>,
    pub close: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// The keys of egui a view takes by the groups of `WW_KEYS_*`, with their names of W3C.
const KEYS: [(Key, &CStr, u32); 52] = [
    (Key::ArrowLeft, c"ArrowLeft", KEYS_ARROWS),
    (Key::ArrowRight, c"ArrowRight", KEYS_ARROWS),
    (Key::ArrowUp, c"ArrowUp", KEYS_ARROWS),
    (Key::ArrowDown, c"ArrowDown", KEYS_ARROWS),
    (Key::PageUp, c"PageUp", KEYS_PAGES),
    (Key::PageDown, c"PageDown", KEYS_PAGES),
    (Key::Home, c"Home", KEYS_PAGES),
    (Key::End, c"End", KEYS_PAGES),
    (Key::A, c"a", KEYS_LETTERS),
    (Key::B, c"b", KEYS_LETTERS),
    (Key::C, c"c", KEYS_LETTERS),
    (Key::D, c"d", KEYS_LETTERS),
    (Key::E, c"e", KEYS_LETTERS),
    (Key::F, c"f", KEYS_LETTERS),
    (Key::G, c"g", KEYS_LETTERS),
    (Key::H, c"h", KEYS_LETTERS),
    (Key::I, c"i", KEYS_LETTERS),
    (Key::J, c"j", KEYS_LETTERS),
    (Key::K, c"k", KEYS_LETTERS),
    (Key::L, c"l", KEYS_LETTERS),
    (Key::M, c"m", KEYS_LETTERS),
    (Key::N, c"n", KEYS_LETTERS),
    (Key::O, c"o", KEYS_LETTERS),
    (Key::P, c"p", KEYS_LETTERS),
    (Key::Q, c"q", KEYS_LETTERS),
    (Key::R, c"r", KEYS_LETTERS),
    (Key::S, c"s", KEYS_LETTERS),
    (Key::T, c"t", KEYS_LETTERS),
    (Key::U, c"u", KEYS_LETTERS),
    (Key::V, c"v", KEYS_LETTERS),
    (Key::W, c"w", KEYS_LETTERS),
    (Key::X, c"x", KEYS_LETTERS),
    (Key::Y, c"y", KEYS_LETTERS),
    (Key::Z, c"z", KEYS_LETTERS),
    (Key::Num0, c"0", KEYS_DIGITS),
    (Key::Num1, c"1", KEYS_DIGITS),
    (Key::Num2, c"2", KEYS_DIGITS),
    (Key::Num3, c"3", KEYS_DIGITS),
    (Key::Num4, c"4", KEYS_DIGITS),
    (Key::Num5, c"5", KEYS_DIGITS),
    (Key::Num6, c"6", KEYS_DIGITS),
    (Key::Num7, c"7", KEYS_DIGITS),
    (Key::Num8, c"8", KEYS_DIGITS),
    (Key::Num9, c"9", KEYS_DIGITS),
    (Key::Plus, c"+", KEYS_SIGNS),
    (Key::Minus, c"-", KEYS_SIGNS),
    (Key::Equals, c"=", KEYS_SIGNS),
    (Key::Space, c" ", KEYS_SPACE),
    (Key::Escape, c"Escape", KEYS_ESCAPE),
    (Key::Enter, c"Enter", KEYS_ENTER),
    (Key::Backspace, c"Backspace", 0),
    (Key::Delete, c"Delete", 0),
];

/// The egui keys of the groups a view names.
pub fn keys_of(groups: u32) -> Vec<Key> {
    KEYS.iter()
        .filter(|(_, _, g)| groups & g != 0)
        .map(|(k, _, _)| *k)
        .collect()
}

/// The name of W3C of a key.
fn name_of(key: Key) -> Option<&'static CStr> {
    KEYS.iter().find(|(k, _, _)| *k == key).map(|(_, n, _)| *n)
}

fn modifiers(m: egui::Modifiers) -> u32 {
    let mut out = 0;
    if m.shift {
        out |= SHIFT;
    }
    if m.ctrl || m.command {
        out |= CTRL;
    }
    if m.alt {
        out |= ALT;
    }
    out
}

/// A text of the plugin, empty for none.
///
/// # Safety
/// `text` is null or a text that ends with a zero and lives for the call.
unsafe fn text(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: by the contract of the caller.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// A text of the plugin kept for the run.
///
/// # Safety
/// As for `text`.
unsafe fn leaked(t: *const c_char) -> &'static str {
    // SAFETY: by the contract of the caller.
    Box::leak(unsafe { text(t) }.into_boxed_str())
}

/// The texts of an array of the plugin, at most `MOST`, each kept for the run.
///
/// # Safety
/// `list` is null or holds `count` texts that end with a zero.
unsafe fn leaked_list(list: *const *const c_char, count: u32) -> &'static [&'static str] {
    if list.is_null() {
        return &[];
    }
    // SAFETY: by the contract of the caller.
    let texts: Vec<&'static str> = (0..count.min(MOST) as usize)
        .map(|i| unsafe { leaked(*list.add(i)) })
        .collect();
    Box::leak(texts.into_boxed_slice())
}

/// The reason a plugin wrote into the buffer of the host, or `fallback`.
fn reason(error: &mut [c_char], fallback: &str) -> String {
    if let Some(last) = error.last_mut() {
        *last = 0;
    }
    // SAFETY: the buffer ends with a zero.
    let why = unsafe { text(error.as_ptr()) };
    if why.is_empty() {
        fallback.to_owned()
    } else {
        why
    }
}

/// Writes a reason into a buffer of the plugin, cut at a character, with its zero.
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

/// A source of the host: the structure of the header first, then the bytes it reads. A source
/// of `open_bytes` belongs to its view (`owned`), the plugin frees the others.
#[repr(C)]
struct SourceBox {
    raw: WwSource,
    file: BlockFile,
    path: Option<CString>,
    owned: bool,
}

impl SourceBox {
    /// A source of some bytes, its pointer for the plugin.
    fn make(file: BlockFile, owned: bool) -> *mut SourceBox {
        let path = file
            .path()
            .and_then(|p| CString::new(p.to_string_lossy().into_owned()).ok());
        let boxed = Box::into_raw(Box::new(SourceBox {
            raw: WwSource {
                size: std::mem::size_of::<WwSource>() as u32,
                length: file.len(),
                self_: std::ptr::null_mut(),
                read: Some(source_read),
                path: path.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
                offset: file.offset(),
            },
            file,
            path,
            owned,
        }));
        // SAFETY: the box was just made; its address is its `self`.
        unsafe { (*boxed).raw.self_ = boxed.cast() };
        boxed
    }
}

/// Runs the part of a function of the host a plugin called that is code of Rust which may fail
/// (a panic of a tool, of the cache, of a file): the failure gives `failed` and never leaves
/// the function, since a panic that crosses into the code of the plugin ends the program. The
/// panic hook has written the details.
fn guarded<T>(failed: T, run: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).unwrap_or(failed)
}

unsafe extern "C" fn source_read(this: *mut c_void, offset: u64, out: *mut u8, count: u32) -> u32 {
    if this.is_null() || out.is_null() {
        return 0;
    }
    // SAFETY: `self` of a source is its box, alive while the plugin may read it; `out` is a
    // buffer of `count` bytes of the plugin.
    guarded(0, || unsafe {
        let source = &*this.cast::<SourceBox>();
        let out = std::slice::from_raw_parts_mut(out, count as usize);
        source.file.read_at(offset, out).unwrap_or(0) as u32
    })
}

unsafe extern "C" fn host_open_file(
    path: *const c_char,
    error: *mut c_char,
    size: u32,
) -> *const WwSource {
    // SAFETY: the plugin gives a path that ends with a zero.
    let path = unsafe { text(path) };
    let opened = guarded(Err("the file was not opened".to_owned()), || {
        BlockFile::open(Path::new(&path)).map_err(|e| e.to_string())
    });
    match opened {
        Ok(file) => SourceBox::make(file, false).cast_const().cast(),
        Err(e) => {
            // SAFETY: the plugin gives a buffer of `size` bytes.
            unsafe { write_error(error, size, &format!("{path}: {e}")) };
            std::ptr::null()
        }
    }
}

unsafe extern "C" fn host_slice(
    source: *const WwSource,
    offset: u64,
    length: u64,
) -> *const WwSource {
    if source.is_null() {
        return std::ptr::null();
    }
    // SAFETY: a source of this host is the first field of its box.
    let source = unsafe { &*source.cast::<SourceBox>() };
    guarded(std::ptr::null(), || {
        SourceBox::make(source.file.slice(offset, length), false)
            .cast_const()
            .cast()
    })
}

unsafe extern "C" fn host_close_source(source: *const WwSource) {
    if source.is_null() {
        return;
    }
    let boxed = source.cast::<SourceBox>().cast_mut();
    // SAFETY: a source of this host is the first field of its box; the ones of open_file and
    // slice are freed once, by the plugin, and the ones of its views by the host alone.
    unsafe {
        if !(*boxed).owned {
            drop(Box::from_raw(boxed));
        }
    }
}

unsafe extern "C" fn host_cache(out: *mut WwCache) {
    if out.is_null() {
        return;
    }
    // A cache that fails leaves the structure of the plugin as it was.
    let Some(state) = guarded(None, || Some(blocks::cache().state())) else {
        return;
    };
    let written = WwCache {
        size: 0,
        block: state.limits.block as u32,
        budget: state.limits.budget as u64,
        used: state.used as u64,
        blocks: state.blocks as u64,
        files: state.files as u64,
        reads: state.reads,
        hits: state.hits,
    };
    // SAFETY: the plugin gives a structure of `size` bytes; the ones it covers are written.
    unsafe {
        let n = ((*out).size as usize).min(std::mem::size_of::<WwCache>());
        let size = (*out).size;
        std::ptr::copy_nonoverlapping((&raw const written).cast::<u8>(), out.cast::<u8>(), n);
        if n >= 4 {
            (*out).size = size;
        }
    }
}

/// The table of the host every plugin of the stable interface gets.
struct HostTable(WwHost);
// SAFETY: the table holds functions alone.
unsafe impl Sync for HostTable {}

static HOST: HostTable = HostTable(WwHost {
    size: std::mem::size_of::<WwHost>() as u32,
    api_version: API,
    open_file: Some(host_open_file),
    slice: Some(host_slice),
    close_source: Some(host_close_source),
    cache: Some(host_cache),
    this: std::ptr::null_mut(),
    call_tool: Some(host_call_tool),
    free_text: Some(host_free_text),
});

/// The table of the host without a plugin named: its calls of tools are the ones of a plugin
/// `stable`.
pub fn host() -> *const WwHost {
    &HOST.0
}

/// The table of the host for the plugin of an id (the name of its DLL), which its calls of tools
/// name; it lives for the run.
pub fn host_for(id: &str) -> *const WwHost {
    let id = CString::new(id.replace('\0', "")).unwrap_or_default();
    let table = WwHost {
        this: id.into_raw().cast(),
        ..HostTable::copy()
    };
    Box::leak(Box::new(table))
}

impl HostTable {
    fn copy() -> WwHost {
        WwHost {
            size: HOST.0.size,
            api_version: HOST.0.api_version,
            open_file: HOST.0.open_file,
            slice: HOST.0.slice,
            close_source: HOST.0.close_source,
            cache: HOST.0.cache,
            this: std::ptr::null_mut(),
            call_tool: HOST.0.call_tool,
            free_text: HOST.0.free_text,
        }
    }
}

/// A text for the plugin, which it frees with `free_text`.
fn given(text: &str) -> *mut c_char {
    CString::new(text.replace('\0', ""))
        .unwrap_or_default()
        .into_raw()
}

/// `call_tool` of the host: a tool of the program by its name, as the plugin of the table.
unsafe extern "C" fn host_call_tool(
    this: *mut c_void,
    name: *const c_char,
    args: *const c_char,
    answer: *mut *mut c_char,
) -> i32 {
    // SAFETY: the texts are the plugin's for the call, `this` a text of `host_for` or null.
    let (id, name, args) = unsafe {
        let id = if this.is_null() {
            "stable".to_owned()
        } else {
            text(this.cast_const().cast())
        };
        (id, text(name), text(args))
    };
    let args = if args.trim().is_empty() {
        Ok(Value::Object(Default::default()))
    } else {
        serde_json::from_str::<Value>(&args)
            .map_err(|e| ToolError::new(Kind::Invalid, format!("the arguments are not JSON: {e}")))
    };
    // A tool is code of the program or of another plugin: one that fails inside (a panic) is a
    // failed call for this plugin, not the end of the program.
    let failed = || {
        Err(ToolError::new(
            Kind::Failed,
            format!("the tool {name} failed inside the program"),
        ))
    };
    let outcome = args.and_then(|args| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::tools::call(&name, &args, &id)
        }))
        .unwrap_or_else(|_| failed())
    });
    let (code, text) = match outcome {
        Ok(value) => (0, value.to_string()),
        Err(e) => (e.kind.code(), e.message),
    };
    if !answer.is_null() {
        // SAFETY: the plugin gives where the answer goes.
        unsafe { *answer = given(&text) };
    }
    code
}

/// `free_text` of the host: frees a text the host gave.
unsafe extern "C" fn host_free_text(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: a text of `given`, freed once.
        drop(unsafe { CString::from_raw(text) });
    }
}

/// A plugin of the stable interface as a plugin of wwrt: its views (a table of `ww_plugin_get`)
/// and its tools (a table of `ww_tools_get`), either of them, or both.
pub struct Stable {
    table: &'static WwPlugin,
    name: &'static str,
    formats: &'static [Format],
    files: Embedded,
    tools: Option<StableTools>,
}

/// The tools of a plugin of the stable interface.
pub struct StableTools {
    table: &'static WwTools,
    tools: Vec<ToolDef>,
    options: Vec<Choice>,
}

/// The table of a plugin, the fields its size covers, the others zero.
///
/// # Safety
/// `table` is the table of a plugin as `wrap` takes it.
unsafe fn covered(table: &'static WwPlugin) -> &'static WwPlugin {
    // SAFETY: a table of integers, pointers and functions is valid when zero, and the bytes copied
    // are the ones its size covers.
    unsafe {
        let mut own: WwPlugin = std::mem::zeroed();
        let n = (table.size as usize).min(std::mem::size_of::<WwPlugin>());
        std::ptr::copy_nonoverlapping(
            (table as *const WwPlugin).cast::<u8>(),
            (&raw mut own).cast::<u8>(),
            n,
        );
        Box::leak(Box::new(own))
    }
}

/// The plugin of a table of the stable interface, after a check of its version and its functions.
///
/// # Safety
/// `table` is the table of a plugin that lives as long as the process (a DLL never unloaded), with
/// its texts, formats and files as the header describes them.
pub unsafe fn wrap(table: &'static WwPlugin) -> Result<Stable, String> {
    if table.api_version != API {
        return Err(format!(
            "of the stable interface {}; this host knows {API}",
            table.api_version
        ));
    }
    if (table.size as usize) < std::mem::offset_of!(WwPlugin, files) {
        return Err(format!(
            "a table of the stable interface of {} bytes",
            table.size
        ));
    }
    // SAFETY: by the contract of the caller.
    let table = unsafe { covered(table) };
    if table.open.is_none() || table.frame.is_none() || table.close.is_none() {
        return Err("a table of the stable interface without its functions".to_owned());
    }
    // SAFETY: by the contract of the caller, the texts and the arrays are as the header says.
    unsafe {
        let name = leaked(table.name);
        let mut formats = Vec::new();
        if !table.formats.is_null() {
            for i in 0..table.format_count.min(MOST) as usize {
                let f = &*table.formats.add(i);
                let extensions: Vec<&'static str> = leaked_list(f.extensions, f.extension_count)
                    .iter()
                    .map(|e| &*Box::leak(e.to_lowercase().into_boxed_str()))
                    .collect();
                formats.push(Format {
                    name: leaked(f.name),
                    extensions: Box::leak(extensions.into_boxed_slice()),
                    mimes: leaked_list(f.mimes, f.mime_count),
                });
            }
        }
        let mut files = Vec::new();
        if !table.files.is_null() {
            for i in 0..table.file_count.min(MOST_FILES) as usize {
                let f = &*table.files.add(i);
                if f.bytes.is_null() && f.size > 0 {
                    continue;
                }
                let bytes: &'static [u8] = if f.size == 0 {
                    &[]
                } else {
                    std::slice::from_raw_parts(f.bytes, f.size as usize)
                };
                let Some(path) = embedded::normal(&text(f.path)) else {
                    continue;
                };
                files.push(embedded::File {
                    path: Box::leak(path.into_boxed_str()),
                    bytes,
                });
            }
        }
        Ok(Stable {
            table,
            name,
            formats: Box::leak(formats.into_boxed_slice()),
            files: Embedded::new(Box::leak(files.into_boxed_slice())),
            tools: None,
        })
    }
}

/// The table of the tools of a plugin, the fields its size covers, the others zero.
///
/// # Safety
/// `table` is a table of tools as `wrap_tools` takes it.
unsafe fn covered_tools(table: &'static WwTools) -> &'static WwTools {
    // SAFETY: as in `covered`.
    unsafe {
        let mut own: WwTools = std::mem::zeroed();
        let n = (table.size as usize).min(std::mem::size_of::<WwTools>());
        std::ptr::copy_nonoverlapping(
            (table as *const WwTools).cast::<u8>(),
            (&raw mut own).cast::<u8>(),
            n,
        );
        Box::leak(Box::new(own))
    }
}

/// The tools of a table of `ww_tools_get`, after a check of its version and its function.
///
/// # Safety
/// `table` lives as long as the process (a DLL never unloaded), with its texts and arrays as the
/// header describes them.
pub unsafe fn wrap_tools(table: &'static WwTools) -> Result<StableTools, String> {
    if table.api_version != API {
        return Err(format!(
            "tools of the stable interface {}; this host knows {API}",
            table.api_version
        ));
    }
    // SAFETY: by the contract of the caller.
    let table = unsafe { covered_tools(table) };
    if table.call.is_none() {
        return Err("a table of tools without its function".to_owned());
    }
    let mut tools = Vec::new();
    let mut options = Vec::new();
    // SAFETY: by the contract of the caller, the texts and the arrays are as the header says.
    unsafe {
        if !table.tools.is_null() {
            for i in 0..table.tool_count.min(MOST * 4) as usize {
                let w = &*table.tools.add(i);
                let schema = serde_json::from_str::<Value>(&text(w.schema))
                    .ok()
                    .filter(Value::is_object)
                    .unwrap_or_else(wwmcp::tools::no_arguments);
                let mode = match w.mode {
                    MODE_EDIT => Mode::Edit,
                    MODE_FILES => Mode::Files,
                    _ => Mode::Read,
                };
                let mut tool =
                    ToolDef::new(&text(w.name), &text(w.group), &text(w.description), schema)
                        .mode(mode);
                for need in text(w.needs).split(',').filter(|n| !n.trim().is_empty()) {
                    let (key, value) = need.split_once('=').unwrap_or((need, ""));
                    tool = tool.needs(key.trim(), value.trim());
                }
                if !tool.name.is_empty() {
                    tools.push(tool);
                }
            }
        }
        if !table.options.is_null() {
            for i in 0..table.option_count.min(MOST) as usize {
                let o = &*table.options.add(i);
                let values = leaked_list(o.values, o.value_count);
                let labels = if o.labels.is_null() {
                    values
                } else {
                    leaked_list(o.labels, o.value_count)
                };
                let initial = text(o.initial);
                options.push(Choice {
                    key: text(o.key),
                    title: text(o.title),
                    values: values.iter().map(|v| (*v).to_owned()).collect(),
                    labels: labels.iter().map(|v| (*v).to_owned()).collect(),
                    default: if initial.is_empty() {
                        values.first().map(|v| (*v).to_owned()).unwrap_or_default()
                    } else {
                        initial
                    },
                    help: text(o.help),
                });
            }
        }
    }
    Ok(StableTools {
        table,
        tools,
        options,
    })
}

impl Stable {
    /// A plugin of tools alone, without views: a table of views with no function.
    pub fn tools_only(tools: StableTools) -> Stable {
        // SAFETY: a table of integers, pointers and functions is valid when zero.
        let table: &'static WwPlugin = Box::leak(Box::new(unsafe { std::mem::zeroed() }));
        // SAFETY: the name is a text of the table of the tools, as the header says.
        let name = unsafe { leaked(tools.table.name) };
        Stable {
            table,
            name,
            formats: &[],
            files: Embedded::new(&[]),
            tools: Some(tools),
        }
    }

    /// The plugin with its tools.
    pub fn with_tools(mut self, tools: StableTools) -> Stable {
        self.tools = Some(tools);
        self
    }
}

// SAFETY: the table and its texts are the static data of a DLL never unloaded, only read. The
// functions of its views are called from the thread of the window alone; the call of a tool
// comes from any thread of the host, several at once, which the header tells the plugin
// (ww.h: "the call of a tool, which any thread of the host makes, several at once").
unsafe impl Send for Stable {}
// SAFETY: as for Send.
unsafe impl Sync for Stable {}

impl Stable {
    /// A view of a pointer the plugin gave, or the reason in its buffer.
    fn view(
        &self,
        view: *mut c_void,
        error: &mut [c_char],
        source: Option<*mut SourceBox>,
    ) -> Result<Box<dyn View>, String> {
        if view.is_null() {
            if let Some(source) = source {
                // SAFETY: the box of a source of the host that no view took.
                drop(unsafe { Box::from_raw(source) });
            }
            return Err(reason(error, "the plugin did not open it"));
        }
        Ok(Box::new(StableView {
            table: self.table,
            view,
            source,
            texture: None,
            size: [0, 0],
            refused: false,
            controls: Vec::new(),
            keys: 0,
            cursor: 0,
            status: String::new(),
            changes: Vec::new(),
        }))
    }
}

impl Plugin for Stable {
    fn name(&self) -> &'static str {
        self.name
    }

    fn formats(&self) -> &'static [Format] {
        self.formats
    }

    fn files(&self) -> Embedded {
        self.files
    }

    fn open(&self, path: &Path, _settings: &serde_json::Value) -> Result<Box<dyn View>, String> {
        let path = CString::new(path.to_string_lossy().into_owned()).map_err(|e| e.to_string())?;
        let mut error = vec![0 as c_char; 1024];
        let open = self.table.open.ok_or("no open")?;
        // SAFETY: the plugin reads the path for the call and writes at most `error.len()` bytes.
        let view = unsafe { open(path.as_ptr(), error.as_mut_ptr(), error.len() as u32) };
        self.view(view, &mut error, None)
    }

    fn opens_bytes(&self) -> bool {
        self.table.open_bytes.is_some()
    }

    fn open_bytes(
        &self,
        name: &str,
        mime: &str,
        bytes: BlockFile,
        _settings: &serde_json::Value,
    ) -> Result<Box<dyn View>, String> {
        let Some(open) = self.table.open_bytes else {
            return Err(crate::plugin::NO_BYTES.to_owned());
        };
        let name = CString::new(name.replace('\0', "")).unwrap_or_default();
        let mime = CString::new(mime.replace('\0', "")).unwrap_or_default();
        let source = SourceBox::make(bytes, true);
        let mut error = vec![0 as c_char; 1024];
        // SAFETY: the source lives until the view closes (or is freed at once when there is none),
        // the texts for the call; the plugin writes at most `error.len()` bytes.
        let view = unsafe {
            open(
                source.cast_const().cast(),
                name.as_ptr(),
                mime.as_ptr(),
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        self.view(view, &mut error, Some(source))
    }

    fn tools(&self) -> Vec<ToolDef> {
        self.tools
            .as_ref()
            .map(|t| t.tools.clone())
            .unwrap_or_default()
    }

    fn tool_options(&self) -> Vec<Choice> {
        self.tools
            .as_ref()
            .map(|t| t.options.clone())
            .unwrap_or_default()
    }

    fn call_tool(&self, name: &str, args: &Value, call: &Call<'_>) -> Outcome {
        let Some(tools) = &self.tools else {
            return Err(ToolError::new(Kind::NotFound, format!("no tool {name}")));
        };
        let Some(run) = tools.table.call else {
            return Err(ToolError::new(Kind::NotFound, format!("no tool {name}")));
        };
        let c = |s: String| CString::new(s.replace('\0', "")).unwrap_or_default();
        let (tool, args) = (c(name.to_owned()), c(args.to_string()));
        let options = c(serde_json::to_string(call.options).unwrap_or_default());
        let mut answer: *mut c_char = std::ptr::null_mut();
        // SAFETY: the texts live for the call; the plugin writes its answer, freed below by its
        // own function.
        let (code, text) = unsafe {
            let code = run(
                tool.as_ptr(),
                args.as_ptr(),
                options.as_ptr(),
                &raw mut answer,
            );
            let text = text(answer);
            if !answer.is_null()
                && let Some(free) = tools.table.free_answer
            {
                free(answer);
            }
            (code, text)
        };
        if code == 0 {
            Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
        } else {
            Err(ToolError::new(Kind::of_code(code), text))
        }
    }

    fn sniff(&self, head: &[u8]) -> Option<&'static str> {
        let sniff = self.table.sniff?;
        let n = head.len().min(crate::mime::SNIFFED);
        // SAFETY: the bytes live for the call; the plugin gives a text of its own or null.
        let found = unsafe { text(sniff(head.as_ptr(), n as u32)) };
        if found.is_empty() {
            return None;
        }
        // The type is one of the formats of the plugin: the text kept for the run.
        self.formats
            .iter()
            .flat_map(|f| f.mimes.iter())
            .find(|m| m.eq_ignore_ascii_case(&found))
            .copied()
    }
}

/// A control of the plugin copied out of its memory, with the value the widget edits.
struct Control {
    id: u32,
    kind: u32,
    label: String,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    choices: Vec<String>,
}

/// The most bytes of the picture of a frame (RGBA): 8192 x 8192 pixels. A picture beyond it, or
/// with a side longer than the renderer takes for a texture, is refused: a plugin that names a
/// huge size makes the host allocate neither its copy nor a texture the graphics card refuses.
const PICTURE_BUDGET: u64 = 256 << 20;

/// Whether the host takes a picture of this size from a plugin: each side within the longest
/// side of a texture (`most_side`, of the renderer), the whole within `PICTURE_BUDGET`.
fn picture_fits(width: u32, height: u32, most_side: usize) -> bool {
    let side = u64::try_from(most_side).unwrap_or(u64::MAX);
    // The pixels of two sides of 32 bits fit 64 bits; their bytes may not, so the pixels are
    // counted against the budget of the pixels.
    u64::from(width) <= side
        && u64::from(height) <= side
        && u64::from(width) * u64::from(height) <= PICTURE_BUDGET / 4
}

pub struct StableView {
    table: &'static WwPlugin,
    view: *mut c_void,
    /// The source of the bytes of the view, freed after the view closes.
    source: Option<*mut SourceBox>,
    texture: Option<egui::TextureHandle>,
    /// The size of the picture of the texture, in pixels.
    size: [u32; 2],
    /// A picture of the plugin was refused for its size: the log says so once a view.
    refused: bool,
    controls: Vec<Control>,
    keys: u32,
    cursor: u32,
    status: String,
    /// The controls changed in the bar, given to the plugin before its next frame.
    changes: Vec<(u32, f64)>,
}

impl Drop for StableView {
    fn drop(&mut self) {
        if let Some(close) = self.table.close {
            // SAFETY: the view of this plugin, closed once.
            unsafe { close(self.view) };
        }
        if let Some(source) = self.source.take() {
            // SAFETY: the box of the source of the view, which closed; the plugin reads it no more.
            drop(unsafe { Box::from_raw(source) });
        }
    }
}

impl StableView {
    /// The controls of the plugin over its view.
    fn bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for c in &mut self.controls {
                let changed = match c.kind {
                    BUTTON => ui.button(&c.label).clicked(),
                    TOGGLE => {
                        let mut on = c.value != 0.0;
                        let r = ui.checkbox(&mut on, &c.label).changed();
                        c.value = f64::from(u8::from(on));
                        r
                    }
                    SLIDER => {
                        let mut slider = egui::Slider::new(&mut c.value, c.min..=c.max.max(c.min))
                            .text(&c.label);
                        if c.step > 0.0 {
                            slider = slider.step_by(c.step);
                        }
                        ui.add(slider).changed()
                    }
                    CHOICE => {
                        let mut index = c.value.max(0.0) as usize;
                        let shown = c.choices.get(index).cloned().unwrap_or_default();
                        ui.label(&c.label);
                        let mut changed = false;
                        egui::ComboBox::from_id_salt(("ww-stable", c.id))
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (i, choice) in c.choices.iter().enumerate() {
                                    changed |= ui.selectable_value(&mut index, i, choice).changed();
                                }
                            });
                        c.value = index as f64;
                        changed
                    }
                    _ => {
                        ui.label(&c.label);
                        false
                    }
                };
                if changed {
                    self.changes.push((c.id, c.value));
                }
            }
        });
    }

    /// Takes the output of a frame: the picture into the texture, the rest copied.
    ///
    /// # Safety
    /// `out` is the output the plugin wrote in its frame just now.
    unsafe fn take(&mut self, ctx: &egui::Context, out: &WwOutput) {
        // SAFETY: by the contract of the caller, every pointer of the output is the plugin's and
        // lives until its next call.
        unsafe {
            let (w, h) = (out.width, out.height);
            let new = out.changed != 0 && !out.pixels.is_null() && w > 0 && h > 0;
            let fits = picture_fits(w, h, ctx.input(|i| i.max_texture_side));
            if new && !fits && !self.refused {
                // The last picture stays; the rest of the output is taken as ever.
                self.refused = true;
                wwmcp::log::window_line(
                    "WARN",
                    &format!("a picture of {w} x {h} pixels of a plugin was refused: too large"),
                );
            }
            if new && fits {
                let stride = (out.stride as usize).max(w as usize * 4);
                let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
                for row in 0..h as usize {
                    let line =
                        std::slice::from_raw_parts(out.pixels.add(row * stride), w as usize * 4);
                    rgba.extend_from_slice(line);
                }
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                match &mut self.texture {
                    Some(t) => t.set(image, egui::TextureOptions::NEAREST),
                    None => {
                        self.texture = Some(ctx.load_texture(
                            "ww-stable",
                            image,
                            egui::TextureOptions::NEAREST,
                        ));
                    }
                }
                self.size = [w, h];
            }
            self.controls.clear();
            if !out.controls.is_null() {
                for i in 0..out.control_count.min(MOST) as usize {
                    let c = &*out.controls.add(i);
                    let mut choices = Vec::new();
                    if !c.choices.is_null() {
                        for j in 0..c.choice_count.min(256) as usize {
                            choices.push(text(*c.choices.add(j)));
                        }
                    }
                    self.controls.push(Control {
                        id: c.id,
                        kind: c.kind,
                        label: text(c.label),
                        value: c.value,
                        min: c.min,
                        max: c.max,
                        step: c.step,
                        choices,
                    });
                }
            }
            self.keys = out.keys;
            self.cursor = out.cursor;
            self.status = text(out.status);
            if out.again != 0 {
                ctx.request_repaint();
            }
        }
    }
}

impl View for StableView {
    fn ui(&mut self, ui: &mut egui::Ui, _host: &mut Host) {
        // The controls changed in the last frame go to the plugin first.
        if let Some(control) = self.table.control {
            for (id, value) in std::mem::take(&mut self.changes) {
                // SAFETY: the view of this plugin and an id it named.
                unsafe { control(self.view, id, value) };
            }
        }
        if !self.controls.is_empty() {
            egui::Panel::top(ui.id().with("ww-stable-controls")).show(ui, |ui| self.bar(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let keys = keys_of(self.keys);
                let (_, got) = input::area(ui, ui.id().with("stable"), rect, &keys);
                let scale = ui.ctx().pixels_per_point();
                let names: Vec<WwKey> = got
                    .keys
                    .iter()
                    .filter_map(|(k, m)| {
                        name_of(*k).map(|n| WwKey {
                            name: n.as_ptr(),
                            pressed: 1,
                            modifiers: modifiers(*m),
                        })
                    })
                    .collect();
                let (held, live) = ui.input(|i| {
                    let p = &i.pointer;
                    let mut held = 0;
                    if p.primary_down() {
                        held |= LEFT;
                    }
                    if p.secondary_down() {
                        held |= RIGHT;
                    }
                    if p.middle_down() {
                        held |= MIDDLE;
                    }
                    (held, got.hovered || got.dragging)
                });
                let mouse = got.pointer.unwrap_or(Pos2::ZERO);
                let mut clicks = 0;
                if got.clicked {
                    clicks |= LEFT;
                }
                if got.secondary_clicked {
                    clicks |= RIGHT;
                }
                if got.middle_clicked {
                    clicks |= MIDDLE;
                }
                let input = WwInput {
                    size: std::mem::size_of::<WwInput>() as u32,
                    width: (rect.width() * scale).round().max(1.0) as u32,
                    height: (rect.height() * scale).round().max(1.0) as u32,
                    scale,
                    focused: u32::from(got.focused),
                    hovered: u32::from(got.hovered),
                    mouse_x: mouse.x * scale,
                    mouse_y: mouse.y * scale,
                    buttons: if live { held } else { 0 },
                    clicks,
                    double_clicks: if got.double_clicked { LEFT } else { 0 },
                    drag_x: got.drag.x * scale,
                    drag_y: got.drag.y * scale,
                    wheel_x: got.wheel.x / 40.0,
                    wheel_y: got.wheel.y / 40.0,
                    modifiers: modifiers(got.modifiers),
                    keys: names.as_ptr(),
                    key_count: names.len() as u32,
                    text: c"".as_ptr(),
                };
                // SAFETY: an output of zeros is a valid one; the plugin fills what it knows.
                let mut out: WwOutput = unsafe { std::mem::zeroed() };
                out.size = std::mem::size_of::<WwOutput>() as u32;
                if let Some(frame) = self.table.frame {
                    // SAFETY: the input and the output live for the call; the view is this plugin's.
                    unsafe {
                        frame(self.view, &raw const input, &raw mut out);
                        self.take(ui.ctx(), &out);
                    }
                }
                input::cursor(ui, cursor(self.cursor));
                if let Some(texture) = &self.texture {
                    let size = Vec2::new(self.size[0] as f32, self.size[1] as f32) / scale;
                    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                    ui.painter_at(rect).image(
                        texture.id(),
                        Rect::from_min_size(rect.min, size),
                        uv,
                        egui::Color32::WHITE,
                    );
                }
            });
    }

    fn status(&self) -> String {
        self.status.clone()
    }
}

fn cursor(c: u32) -> Cursor {
    match c {
        1 => Cursor::Cross,
        2 => Cursor::Hand,
        3 => Cursor::Grab,
        4 => Cursor::Grabbing,
        5 => Cursor::ZoomIn,
        6 => Cursor::ZoomOut,
        7 => Cursor::Move,
        _ => Cursor::Default,
    }
}

#[cfg(test)]
mod tests;
