//! The structures and the constants of `wview.h`, version 1, as a compiler of C lays them out.
//! A plugin written with the crate needs none of them: `crate::View` and `crate::plugin!` write
//! and read them.

use std::ffi::{c_char, c_void};

pub const WV_API_VERSION: u32 = 1;

pub const WV_SHIFT: u32 = 1;
pub const WV_CTRL: u32 = 2;
pub const WV_ALT: u32 = 4;

pub const WV_LEFT: u32 = 1;
pub const WV_RIGHT: u32 = 2;
pub const WV_MIDDLE: u32 = 4;

pub const WV_KEYS_ARROWS: u32 = 1;
pub const WV_KEYS_PAGES: u32 = 2;
pub const WV_KEYS_LETTERS: u32 = 4;
pub const WV_KEYS_DIGITS: u32 = 8;
pub const WV_KEYS_SIGNS: u32 = 16;
pub const WV_KEYS_SPACE: u32 = 32;
pub const WV_KEYS_ESCAPE: u32 = 64;
pub const WV_KEYS_ENTER: u32 = 128;

pub const WV_CURSOR_DEFAULT: u32 = 0;
pub const WV_CURSOR_CROSS: u32 = 1;
pub const WV_CURSOR_HAND: u32 = 2;
pub const WV_CURSOR_GRAB: u32 = 3;
pub const WV_CURSOR_GRABBING: u32 = 4;
pub const WV_CURSOR_ZOOM_IN: u32 = 5;
pub const WV_CURSOR_ZOOM_OUT: u32 = 6;
pub const WV_CURSOR_MOVE: u32 = 7;

pub const WV_BUTTON: u32 = 0;
pub const WV_TOGGLE: u32 = 1;
pub const WV_SLIDER: u32 = 2;
pub const WV_CHOICE: u32 = 3;
pub const WV_LABEL: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WvFormat {
    pub name: *const c_char,
    pub extensions: *const *const c_char,
    pub extension_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WvKey {
    pub name: *const c_char,
    pub pressed: u32,
    pub modifiers: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WvInput {
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
    pub keys: *const WvKey,
    pub key_count: u32,
    pub text: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WvControl {
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
pub struct WvOutput {
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixels: *const u8,
    pub changed: u32,
    pub controls: *const WvControl,
    pub control_count: u32,
    pub keys: u32,
    pub cursor: u32,
    pub again: u32,
    pub status: *const c_char,
}

/// `open` of the table: a path, a buffer for the reason and its size, to a view or null.
pub type OpenFn = unsafe extern "C" fn(*const c_char, *mut c_char, u32) -> *mut c_void;
pub type FrameFn = unsafe extern "C" fn(*mut c_void, *const WvInput, *mut WvOutput);
pub type ControlFn = unsafe extern "C" fn(*mut c_void, u32, f64);
pub type CloseFn = unsafe extern "C" fn(*mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WvPlugin {
    pub api_version: u32,
    pub name: *const c_char,
    pub formats: *const WvFormat,
    pub format_count: u32,
    pub open: Option<OpenFn>,
    pub frame: Option<FrameFn>,
    pub control: Option<ControlFn>,
    pub close: Option<CloseFn>,
}
