//! The structures and the constants of `ww.h`, version 1, as a compiler of C lays them out.
//! A plugin written with the crate needs none of them: `crate::View`, `crate::Source` and
//! `crate::plugin!` write and read them.

use std::ffi::{c_char, c_void};

pub const WW_API_VERSION: u32 = 1;

pub const WW_SHIFT: u32 = 1;
pub const WW_CTRL: u32 = 2;
pub const WW_ALT: u32 = 4;

pub const WW_LEFT: u32 = 1;
pub const WW_RIGHT: u32 = 2;
pub const WW_MIDDLE: u32 = 4;

pub const WW_KEYS_ARROWS: u32 = 1;
pub const WW_KEYS_PAGES: u32 = 2;
pub const WW_KEYS_LETTERS: u32 = 4;
pub const WW_KEYS_DIGITS: u32 = 8;
pub const WW_KEYS_SIGNS: u32 = 16;
pub const WW_KEYS_SPACE: u32 = 32;
pub const WW_KEYS_ESCAPE: u32 = 64;
pub const WW_KEYS_ENTER: u32 = 128;

pub const WW_CURSOR_DEFAULT: u32 = 0;
pub const WW_CURSOR_CROSS: u32 = 1;
pub const WW_CURSOR_HAND: u32 = 2;
pub const WW_CURSOR_GRAB: u32 = 3;
pub const WW_CURSOR_GRABBING: u32 = 4;
pub const WW_CURSOR_ZOOM_IN: u32 = 5;
pub const WW_CURSOR_ZOOM_OUT: u32 = 6;
pub const WW_CURSOR_MOVE: u32 = 7;

pub const WW_BUTTON: u32 = 0;
pub const WW_TOGGLE: u32 = 1;
pub const WW_SLIDER: u32 = 2;
pub const WW_CHOICE: u32 = 3;
pub const WW_LABEL: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WwFormat {
    pub name: *const c_char,
    pub extensions: *const *const c_char,
    pub extension_count: u32,
    pub mimes: *const *const c_char,
    pub mime_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WwFile {
    pub path: *const c_char,
    pub bytes: *const u8,
    pub size: u64,
}

/// `read` of a source: its `self`, an offset, a buffer and its size, to the count read.
pub type ReadFn = unsafe extern "C" fn(*mut c_void, u64, *mut u8, u32) -> u32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WwSource {
    pub size: u32,
    pub length: u64,
    pub self_: *mut c_void,
    pub read: Option<ReadFn>,
    pub path: *const c_char,
    pub offset: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
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

pub type OpenFileFn = unsafe extern "C" fn(*const c_char, *mut c_char, u32) -> *const WwSource;
pub type SliceFn = unsafe extern "C" fn(*const WwSource, u64, u64) -> *const WwSource;
pub type CloseSourceFn = unsafe extern "C" fn(*const WwSource);
pub type CacheFn = unsafe extern "C" fn(*mut WwCache);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WwHost {
    pub size: u32,
    pub api_version: u32,
    pub open_file: Option<OpenFileFn>,
    pub slice: Option<SliceFn>,
    pub close_source: Option<CloseSourceFn>,
    pub cache: Option<CacheFn>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WwKey {
    pub name: *const c_char,
    pub pressed: u32,
    pub modifiers: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
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
#[derive(Clone, Copy)]
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
#[derive(Clone, Copy)]
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

/// `open` of the table: a path, a buffer for the reason and its size, to a view or null.
pub type OpenFn = unsafe extern "C" fn(*const c_char, *mut c_char, u32) -> *mut c_void;
/// `open_bytes` of the table: a source, a name, a MIME type, a buffer for the reason and its
/// size, to a view or null.
pub type OpenBytesFn = unsafe extern "C" fn(
    *const WwSource,
    *const c_char,
    *const c_char,
    *mut c_char,
    u32,
) -> *mut c_void;
/// `sniff` of the table: the first bytes and their count, to a type or null.
pub type SniffFn = unsafe extern "C" fn(*const u8, u32) -> *const c_char;
pub type FrameFn = unsafe extern "C" fn(*mut c_void, *const WwInput, *mut WwOutput);
pub type ControlFn = unsafe extern "C" fn(*mut c_void, u32, f64);
pub type CloseFn = unsafe extern "C" fn(*mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
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
    pub frame: Option<FrameFn>,
    pub control: Option<ControlFn>,
    pub close: Option<CloseFn>,
}
