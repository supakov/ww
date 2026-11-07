//! Plugins of the stable interface of wview (`wview.h`, version 1) in Rust of any toolchain: a
//! plugin of this crate loads into every release of wview that knows the interface, whatever
//! compiler built it, as one in C does (`stable/README.md` of supakov/ww).
//!
//! A plugin is a `cdylib` crate: a type that implements `View` (a frame of input in, a picture,
//! controls, the keys it takes, a cursor and a line of the status bar out; a control changed),
//! and `plugin!`, which names the plugin, its formats, the type of its view and the function that
//! opens a file, and exports `wv_plugin_get`. The crate keeps what a frame gives (its texts, its
//! controls) until the next call on the view, reads the input and writes the output by the sizes
//! the host gives, and turns a panic of the plugin into a message of the view.
//!
//! ```
//! use wview_stable::{Control, Format, Frame, Input, Picture, View};
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
//! wview_stable::plugin! {
//!     name: "Grey",
//!     formats: &[Format { name: "Grey files", extensions: &["grey"] }],
//!     view: Grey,
//!     open: |_path: &str| Ok(Grey { level: 5.0, pixels: Vec::new() }),
//! }
//! ```

use std::ffi::{c_char, CStr, CString};

pub mod ffi;

/// The version of `wview.h` the crate writes.
pub const API: u32 = ffi::WV_API_VERSION;

/// The most keys and controls of a frame the crate reads or gives.
const MOST_KEYS: u32 = 256;
const MOST_CHOICES: usize = 256;

/// A kind of file a plugin opens: its name and its extensions without the dot, in lower case.
pub struct Format {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
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
            shift: bits & ffi::WV_SHIFT != 0,
            ctrl: bits & ffi::WV_CTRL != 0,
            alt: bits & ffi::WV_ALT != 0,
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
            left: bits & ffi::WV_LEFT != 0,
            right: bits & ffi::WV_RIGHT != 0,
            middle: bits & ffi::WV_MIDDLE != 0,
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

    pub const ARROWS: u32 = ffi::WV_KEYS_ARROWS;
    pub const PAGES: u32 = ffi::WV_KEYS_PAGES;
    pub const LETTERS: u32 = ffi::WV_KEYS_LETTERS;
    pub const DIGITS: u32 = ffi::WV_KEYS_DIGITS;
    pub const SIGNS: u32 = ffi::WV_KEYS_SIGNS;
    pub const SPACE: u32 = ffi::WV_KEYS_SPACE;
    pub const ESCAPE: u32 = ffi::WV_KEYS_ESCAPE;
    pub const ENTER: u32 = ffi::WV_KEYS_ENTER;
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
            Cursor::Default => ffi::WV_CURSOR_DEFAULT,
            Cursor::Cross => ffi::WV_CURSOR_CROSS,
            Cursor::Hand => ffi::WV_CURSOR_HAND,
            Cursor::Grab => ffi::WV_CURSOR_GRAB,
            Cursor::Grabbing => ffi::WV_CURSOR_GRABBING,
            Cursor::ZoomIn => ffi::WV_CURSOR_ZOOM_IN,
            Cursor::ZoomOut => ffi::WV_CURSOR_ZOOM_OUT,
            Cursor::Move => ffi::WV_CURSOR_MOVE,
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

/// A view of a file, made by the function `open` of `plugin!`.
pub trait View: 'static {
    /// A frame: the input since the last one in, the picture and the controls now out.
    fn frame(&mut self, input: &Input) -> Frame<'_>;
    /// A control of the bar changed: a button pressed (its value 0), a toggle (0 or 1), a slider
    /// or a choice (its index) set.
    fn control(&mut self, _id: u32, _value: f64) {}
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
    plugin: ffi::WvPlugin,
    _texts: Vec<CString>,
    _formats: Vec<ffi::WvFormat>,
    _extensions: Vec<Vec<*const c_char>>,
}

// SAFETY: the table and what it points to are written once and only read after; the pointers
// lead into the buffers of the table itself, which never move.
unsafe impl Send for Table {}
// SAFETY: as for Send.
unsafe impl Sync for Table {}

impl Table {
    /// The table of a plugin of the view `V`: its name, its formats and its `open` (`plugin!`
    /// writes it).
    pub fn new<V: View>(name: &str, formats: &[Format], open: ffi::OpenFn) -> Table {
        let mut texts = vec![c_text(name)];
        let mut extensions: Vec<Vec<*const c_char>> = Vec::new();
        let mut names = Vec::new();
        for f in formats {
            let start = texts.len();
            texts.push(c_text(f.name));
            names.push(start);
            texts.extend(f.extensions.iter().map(|e| c_text(e)));
            extensions.push(Vec::new());
        }
        // The pointers are taken once every text is in place: a CString keeps its buffer when
        // the vector of them grows.
        let mut at = 1;
        let mut list = Vec::new();
        for (i, f) in formats.iter().enumerate() {
            at += 1;
            for _ in f.extensions {
                extensions[i].push(texts[at].as_ptr());
                at += 1;
            }
            list.push(ffi::WvFormat {
                name: texts[names[i]].as_ptr(),
                extensions: extensions[i].as_ptr(),
                extension_count: f.extensions.len() as u32,
            });
        }
        Table {
            plugin: ffi::WvPlugin {
                api_version: API,
                name: texts[0].as_ptr(),
                formats: list.as_ptr(),
                format_count: list.len() as u32,
                open: Some(open),
                frame: Some(glue::frame::<V>),
                control: Some(glue::control::<V>),
                close: Some(glue::close::<V>),
            },
            _texts: texts,
            _formats: list,
            _extensions: extensions,
        }
    }

    /// The table for `wv_plugin_get`.
    pub fn plugin(&self) -> *const ffi::WvPlugin {
        &self.plugin
    }
}

/// The functions of the table over a `View`, for `plugin!`.
#[doc(hidden)]
pub mod glue {
    use std::ffi::{c_char, c_void, CString};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::*;

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
        controls: Vec<ffi::WvControl>,
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
        make: fn(&str) -> Result<V, String>,
    ) -> *mut c_void {
        // SAFETY: by the contract of the caller.
        let path = unsafe { text_of(path) };
        let why = match catch_unwind(AssertUnwindSafe(|| make(&path))) {
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
    unsafe fn read_input(input: *const ffi::WvInput) -> Input {
        if input.is_null() {
            return Input::default();
        }
        // SAFETY: the input of the host is valid for the call; a struct of integers, floats and
        // pointers is valid when zero, and the bytes copied are the ones the size covers.
        unsafe {
            let mut raw: ffi::WvInput = std::mem::zeroed();
            let n = ((*input).size as usize).min(std::mem::size_of::<ffi::WvInput>());
            std::ptr::copy_nonoverlapping(
                input.cast::<u8>(),
                (&mut raw as *mut ffi::WvInput).cast::<u8>(),
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
    fn output(frame: &Frame<'_>) -> (ffi::WvOutput, Kept) {
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
                Kind::Button => (ffi::WV_BUTTON, 0.0, 0.0, 0.0, 0.0),
                Kind::Toggle(on) => (ffi::WV_TOGGLE, f64::from(u8::from(*on)), 0.0, 1.0, 1.0),
                Kind::Slider {
                    value,
                    min,
                    max,
                    step,
                } => (ffi::WV_SLIDER, *value, *min, *max, *step),
                Kind::Choice { index, choices } => {
                    let last = choices.len().saturating_sub(1) as f64;
                    (ffi::WV_CHOICE, *index as f64, 0.0, last, 1.0)
                }
                Kind::Label => (ffi::WV_LABEL, 0.0, 0.0, 0.0, 0.0),
            };
            let choices = &kept.pointers[i];
            kept.controls.push(ffi::WvControl {
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
        let out = ffi::WvOutput {
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
    unsafe fn write_output(out: *mut ffi::WvOutput, mut written: ffi::WvOutput) {
        if out.is_null() {
            return;
        }
        // SAFETY: the output of the host is valid for the call, and the bytes written are the
        // ones its size covers.
        unsafe {
            written.size = (*out).size;
            let n = (written.size as usize).min(std::mem::size_of::<ffi::WvOutput>());
            std::ptr::copy_nonoverlapping(
                (&written as *const ffi::WvOutput).cast::<u8>(),
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
        input: *const ffi::WvInput,
        out: *mut ffi::WvOutput,
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
            let mut written: ffi::WvOutput = std::mem::zeroed();
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

/// Names the plugin of a crate and exports `wv_plugin_get`, the one function of `wview.h`: the
/// name of the plugin, its formats, the type of its view and the function that opens a file (a
/// path to the view or the reason it did not open).
///
/// ```ignore
/// wview_stable::plugin! {
///     name: "Grey",
///     formats: &[Format { name: "Grey files", extensions: &["grey"] }],
///     view: Grey,
///     open: |path: &str| Grey::read(path),
/// }
/// ```
#[macro_export]
macro_rules! plugin {
    (name: $name:expr, formats: $formats:expr, view: $view:ty, open: $open:expr $(,)?) => {
        /// The table of the plugin (`wview.h`).
        #[no_mangle]
        pub extern "C" fn wv_plugin_get() -> *const $crate::ffi::WvPlugin {
            unsafe extern "C" fn open(
                path: *const ::std::ffi::c_char,
                error: *mut ::std::ffi::c_char,
                size: u32,
            ) -> *mut ::std::ffi::c_void {
                // The function of the plugin is named outside the unsafe block: it is the
                // plugin's own, safe code.
                let make: fn(&str) -> ::std::result::Result<$view, ::std::string::String> = $open;
                // SAFETY: the host gives a path that ends with a zero and a buffer of `size`
                // bytes, as wview.h says.
                unsafe { $crate::glue::open(path, error, size, make) }
            }
            static TABLE: ::std::sync::OnceLock<$crate::Table> = ::std::sync::OnceLock::new();
            TABLE
                .get_or_init(|| $crate::Table::new::<$view>($name, $formats, open))
                .plugin()
        }
    };
}

#[cfg(test)]
mod tests;
