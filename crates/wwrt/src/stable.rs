//! The stable interface of the plugins (`stable/wview.h`, version 1, docs/plugins.md 6.9):
//! a DLL of any language with a C FFI that exports `wv_plugin_get`. The structures here are the
//! ones of the header; `wrap` makes of such a table an ordinary `Plugin`, whose views draw the
//! picture of the plugin as a texture, its controls as widgets of egui over the view, and give it
//! the keys and the mouse of the view.

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::Path;

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

#[repr(C)]
pub struct WvFormat {
    pub name: *const c_char,
    pub extensions: *const *const c_char,
    pub extension_count: u32,
}

#[repr(C)]
pub struct WvKey {
    pub name: *const c_char,
    pub pressed: u32,
    pub modifiers: u32,
}

#[repr(C)]
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

#[repr(C)]
pub struct WvPlugin {
    pub api_version: u32,
    pub name: *const c_char,
    pub formats: *const WvFormat,
    pub format_count: u32,
    pub open: Option<unsafe extern "C" fn(*const c_char, *mut c_char, u32) -> *mut c_void>,
    pub frame: Option<unsafe extern "C" fn(*mut c_void, *const WvInput, *mut WvOutput)>,
    pub control: Option<unsafe extern "C" fn(*mut c_void, u32, f64)>,
    pub close: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// The keys of egui a view takes by the groups of `WV_KEYS_*`, with their names of W3C.
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

/// A plugin of the stable interface as a plugin of wwrt.
pub struct Stable {
    table: &'static WvPlugin,
    name: &'static str,
    formats: &'static [Format],
}

/// The plugin of a table of the stable interface, after a check of its version and its functions.
///
/// # Safety
/// `table` is the table of a plugin that lives as long as the process (a DLL never unloaded), with
/// its texts and formats as the header describes them.
pub unsafe fn wrap(table: &'static WvPlugin) -> Result<Stable, String> {
    if table.api_version != API {
        return Err(format!(
            "of the stable interface {}; this wview knows {API}",
            table.api_version
        ));
    }
    if table.open.is_none() || table.frame.is_none() || table.close.is_none() {
        return Err("a table of the stable interface without its functions".to_owned());
    }
    // SAFETY: by the contract of the caller, the texts and the arrays are as the header says.
    unsafe {
        let name: &'static str = Box::leak(text(table.name).into_boxed_str());
        let mut formats = Vec::new();
        for i in 0..table.format_count.min(64) as usize {
            let f = &*table.formats.add(i);
            let mut extensions = Vec::new();
            for j in 0..f.extension_count.min(64) as usize {
                let e = text(*f.extensions.add(j)).to_lowercase();
                extensions.push(&*Box::leak(e.into_boxed_str()));
            }
            formats.push(Format {
                name: Box::leak(text(f.name).into_boxed_str()),
                extensions: Box::leak(extensions.into_boxed_slice()),
                mimes: &[],
            });
        }
        Ok(Stable {
            table,
            name,
            formats: Box::leak(formats.into_boxed_slice()),
        })
    }
}

// SAFETY: the table and its texts are the static data of a DLL never unloaded, only read; the
// functions of the plugin are called from the thread of the window alone, as the header says.
unsafe impl Send for Stable {}
// SAFETY: as for Send.
unsafe impl Sync for Stable {}

impl Plugin for Stable {
    fn name(&self) -> &'static str {
        self.name
    }

    fn formats(&self) -> &'static [Format] {
        self.formats
    }

    fn open(&self, path: &Path, _settings: &serde_json::Value) -> Result<Box<dyn View>, String> {
        let path = CString::new(path.to_string_lossy().into_owned()).map_err(|e| e.to_string())?;
        let mut error = vec![0 as c_char; 1024];
        let open = self.table.open.ok_or("no open")?;
        // SAFETY: the plugin reads the path for the call and writes at most `error.len()` bytes.
        let view = unsafe { open(path.as_ptr(), error.as_mut_ptr(), error.len() as u32) };
        if view.is_null() {
            error[1023] = 0;
            // SAFETY: the buffer ends with a zero.
            let why = unsafe { text(error.as_ptr()) };
            return Err(if why.is_empty() {
                "the plugin did not open the file".to_owned()
            } else {
                why
            });
        }
        Ok(Box::new(StableView {
            table: self.table,
            view,
            texture: None,
            size: [0, 0],
            controls: Vec::new(),
            keys: 0,
            cursor: 0,
            status: String::new(),
            changes: Vec::new(),
        }))
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

pub struct StableView {
    table: &'static WvPlugin,
    view: *mut c_void,
    texture: Option<egui::TextureHandle>,
    /// The size of the picture of the texture, in pixels.
    size: [u32; 2],
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
                        egui::ComboBox::from_id_salt(("wv-stable", c.id))
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
    unsafe fn take(&mut self, ctx: &egui::Context, out: &WvOutput) {
        // SAFETY: by the contract of the caller, every pointer of the output is the plugin's and
        // lives until its next call.
        unsafe {
            let (w, h) = (out.width, out.height);
            if out.changed != 0 && !out.pixels.is_null() && w > 0 && h > 0 {
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
                            "wv-stable",
                            image,
                            egui::TextureOptions::NEAREST,
                        ));
                    }
                }
                self.size = [w, h];
            }
            self.controls.clear();
            for i in 0..out.control_count.min(64) as usize {
                let c = &*out.controls.add(i);
                let mut choices = Vec::new();
                for j in 0..c.choice_count.min(256) as usize {
                    choices.push(text(*c.choices.add(j)));
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
            egui::Panel::top("wv-stable-controls").show(ui, |ui| self.bar(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let keys = keys_of(self.keys);
                let (_, got) = input::area(ui, ui.id().with("stable"), rect, &keys);
                let scale = ui.ctx().pixels_per_point();
                let names: Vec<WvKey> = got
                    .keys
                    .iter()
                    .filter_map(|(k, m)| {
                        name_of(*k).map(|n| WvKey {
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
                let input = WvInput {
                    size: std::mem::size_of::<WvInput>() as u32,
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
                let mut out: WvOutput = unsafe { std::mem::zeroed() };
                out.size = std::mem::size_of::<WvOutput>() as u32;
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
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn the_structures_have_the_layout_of_the_header() {
        // The sizes of wview.h on a target of 64 bits, as a compiler of C lays them out.
        assert_eq!(std::mem::size_of::<WvFormat>(), 24);
        assert_eq!(std::mem::size_of::<WvKey>(), 16);
        assert_eq!(std::mem::size_of::<WvInput>(), 88);
        assert_eq!(std::mem::size_of::<WvControl>(), 64);
        assert_eq!(std::mem::size_of::<WvOutput>(), 64);
        assert_eq!(std::mem::size_of::<WvPlugin>(), 64);
        assert_eq!(std::mem::offset_of!(WvInput, keys), 64);
        assert_eq!(std::mem::offset_of!(WvOutput, controls), 32);
    }

    /// A plugin of the stable interface written in Rust for the test, as one in C would be.
    struct Picture {
        frames: u32,
        level: f64,
    }

    static CLOSED: AtomicU32 = AtomicU32::new(0);
    static LAST: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static PIXELS: [u8; 16] = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 9, 9, 9, 255];

    struct Frame {
        controls: [WvControl; 2],
        status: CString,
    }

    thread_local! {
        // Kept between the frames of the test: what a plugin keeps until its next call.
        static FRAME: std::cell::RefCell<Option<Box<Frame>>> = const { std::cell::RefCell::new(None) };
    }

    unsafe extern "C" fn open(path: *const c_char, error: *mut c_char, size: u32) -> *mut c_void {
        // SAFETY: the host gives a path that ends with a zero and a buffer of `size` bytes.
        unsafe {
            let path = CStr::from_ptr(path).to_string_lossy();
            if path.contains("bad") {
                let why = c"no such picture".to_bytes_with_nul();
                std::ptr::copy_nonoverlapping(
                    why.as_ptr().cast(),
                    error,
                    why.len().min(size as usize),
                );
                return std::ptr::null_mut();
            }
        }
        Box::into_raw(Box::new(Picture {
            frames: 0,
            level: 5.0,
        }))
        .cast()
    }

    unsafe extern "C" fn frame(view: *mut c_void, input: *const WvInput, out: *mut WvOutput) {
        // SAFETY: the view this plugin opened; the input and the output of the host for the call.
        unsafe {
            let p = &mut *view.cast::<Picture>();
            let input = &*input;
            p.frames += 1;
            let keys: Vec<String> = (0..input.key_count as usize)
                .map(|i| {
                    CStr::from_ptr((*input.keys.add(i)).name)
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            *LAST.lock().unwrap() = keys;
            let kept = Box::new(Frame {
                controls: [
                    WvControl {
                        id: 1,
                        kind: SLIDER,
                        label: c"Level".as_ptr(),
                        value: p.level,
                        min: 0.0,
                        max: 10.0,
                        step: 1.0,
                        choices: std::ptr::null(),
                        choice_count: 0,
                    },
                    WvControl {
                        id: 2,
                        kind: BUTTON,
                        label: c"Reset".as_ptr(),
                        value: 0.0,
                        min: 0.0,
                        max: 0.0,
                        step: 0.0,
                        choices: std::ptr::null(),
                        choice_count: 0,
                    },
                ],
                status: CString::new(format!(
                    "frame {} of {} x {}, level {}",
                    p.frames, input.width, input.height, p.level
                ))
                .unwrap(),
            });
            // The box stays where it is while the thread keeps it, so its pointers stay valid.
            let f: *const Frame = &*kept;
            FRAME.with(|frame| *frame.borrow_mut() = Some(kept));
            let f = &*f;
            let out = &mut *out;
            out.width = 2;
            out.height = 2;
            out.stride = 8;
            out.pixels = PIXELS.as_ptr();
            out.changed = u32::from(p.frames == 1);
            out.controls = f.controls.as_ptr();
            out.control_count = 2;
            out.keys = KEYS_ARROWS | KEYS_LETTERS;
            out.cursor = 3;
            out.status = f.status.as_ptr();
        }
    }

    unsafe extern "C" fn control(view: *mut c_void, id: u32, value: f64) {
        // SAFETY: the view this plugin opened.
        let p = unsafe { &mut *view.cast::<Picture>() };
        match id {
            1 => p.level = value,
            _ => p.level = 5.0,
        }
    }

    unsafe extern "C" fn close(view: *mut c_void) {
        // SAFETY: the view this plugin opened, closed once.
        drop(unsafe { Box::from_raw(view.cast::<Picture>()) });
        CLOSED.fetch_add(1, Ordering::SeqCst);
    }

    struct Table(WvPlugin);
    // SAFETY: the table holds pointers to static data alone.
    unsafe impl Sync for Table {}

    static EXTENSIONS: [Pointer; 1] = [Pointer(c"pic".as_ptr())];
    #[repr(transparent)]
    struct Pointer(*const c_char);
    // SAFETY: a pointer to a static text.
    unsafe impl Sync for Pointer {}
    struct Formats([WvFormat; 1]);
    // SAFETY: pointers to static data alone.
    unsafe impl Sync for Formats {}
    static FORMATS: Formats = Formats([WvFormat {
        name: c"Pictures of the test".as_ptr(),
        extensions: EXTENSIONS.as_ptr().cast(),
        extension_count: 1,
    }]);
    static TABLE: Table = Table(WvPlugin {
        api_version: 1,
        name: c"A stable plugin".as_ptr(),
        formats: FORMATS.0.as_ptr(),
        format_count: 1,
        open: Some(open),
        frame: Some(frame),
        control: Some(control),
        close: Some(close),
    });

    fn run(view: &mut Box<dyn View>, ctx: &egui::Context, events: Vec<egui::Event>) {
        let settings = serde_json::Value::Null;
        let mut host = Host::new(true, ww::PageWidth::View, &settings);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| view.ui(ui, &mut host));
        output.textures_delta.clear();
    }

    #[test]
    fn a_plugin_of_the_stable_interface_is_a_plugin() {
        // SAFETY: the table of the test is static.
        let plugin = unsafe { wrap(&TABLE.0) }.unwrap();
        assert_eq!(plugin.name(), "A stable plugin");
        assert_eq!(plugin.formats()[0].extensions, ["pic"]);
        let Err(why) = plugin.open(Path::new("bad.pic"), &serde_json::Value::Null) else {
            panic!("the plugin refuses a bad file");
        };
        assert_eq!(why, "no such picture");
        let mut view = plugin
            .open(Path::new("good.pic"), &serde_json::Value::Null)
            .unwrap();
        let ctx = egui::Context::default();
        run(&mut view, &ctx, Vec::new());
        assert!(
            view.status().starts_with("frame 1 of 400 x "),
            "{}",
            view.status()
        );
        // A click gives the view the keys it names; ArrowLeft and P are among them, F5 not.
        let at = Pos2::new(200.0, 200.0);
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        run(
            &mut view,
            &ctx,
            vec![egui::Event::PointerMoved(at), click(true), click(false)],
        );
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        run(
            &mut view,
            &ctx,
            vec![key(Key::ArrowLeft), key(Key::P), key(Key::F5)],
        );
        assert_eq!(
            *LAST.lock().unwrap(),
            vec!["ArrowLeft".to_owned(), "p".to_owned()]
        );
        assert!(view.status().contains("level 5"));
        drop(view);
        assert_eq!(
            CLOSED.load(Ordering::SeqCst),
            1,
            "a closed tab closes its view"
        );
    }

    /// A plugin of the stable interface written with the crate `wview-stable`, as one of Rust of
    /// another toolchain would be.
    mod with_the_crate {
        use wview_stable::{Control, Cursor, Format, Frame, Input, Picture, View, keys};

        pub struct Grey {
            level: f64,
            pixels: Vec<u8>,
        }

        impl View for Grey {
            fn frame(&mut self, input: &Input) -> Frame<'_> {
                let v = (self.level * 25.0) as u8;
                self.pixels = [v, v, v, 255].repeat(4);
                let pressed: Vec<&str> = input.keys.iter().map(|k| k.name.as_str()).collect();
                Frame {
                    picture: Some(Picture::new(2, 2, &self.pixels)),
                    changed: true,
                    controls: vec![
                        Control::slider(1, "Level", self.level, 0.0, 10.0, 1.0),
                        Control::button(2, "Reset"),
                    ],
                    keys: keys::ARROWS,
                    cursor: Cursor::Cross,
                    again: false,
                    status: format!(
                        "{} x {}, level {}, keys {}",
                        input.width,
                        input.height,
                        self.level,
                        pressed.join(" ")
                    ),
                }
            }

            fn control(&mut self, id: u32, value: f64) {
                self.level = if id == 1 { value } else { 5.0 };
            }
        }

        wview_stable::plugin! {
            name: "Grey of the crate",
            formats: &[Format { name: "Grey files", extensions: &["grey"] }],
            view: Grey,
            open: |path: &str| {
                if path.ends_with(".grey") {
                    Ok(Grey { level: 5.0, pixels: Vec::new() })
                } else {
                    Err(format!("{path} is no grey file"))
                }
            },
        }
    }

    /// The two sides of `wview.h` checked against each other: the table the crate makes, wrapped
    /// by this host and drawn.
    #[test]
    fn a_plugin_written_with_the_crate_is_a_plugin() {
        use wview_stable::ffi;
        assert_eq!(
            std::mem::size_of::<ffi::WvInput>(),
            std::mem::size_of::<WvInput>()
        );
        assert_eq!(
            std::mem::size_of::<ffi::WvOutput>(),
            std::mem::size_of::<WvOutput>()
        );
        assert_eq!(
            std::mem::size_of::<ffi::WvControl>(),
            std::mem::size_of::<WvControl>()
        );
        assert_eq!(
            std::mem::size_of::<ffi::WvPlugin>(),
            std::mem::size_of::<WvPlugin>()
        );
        let table = with_the_crate::wv_plugin_get();
        // SAFETY: the table of the crate has the layout of the header and lives for the run.
        let plugin = unsafe { wrap(&*table.cast::<WvPlugin>()) }.unwrap();
        assert_eq!(plugin.name(), "Grey of the crate");
        assert_eq!(plugin.formats()[0].extensions, ["grey"]);
        let Err(why) = plugin.open(Path::new("a.pic"), &serde_json::Value::Null) else {
            panic!("the plugin refuses a file of another format");
        };
        assert_eq!(why, "a.pic is no grey file");
        let mut view = plugin
            .open(Path::new("a.grey"), &serde_json::Value::Null)
            .unwrap();
        let ctx = egui::Context::default();
        run(&mut view, &ctx, Vec::new());
        assert!(view.status().starts_with("400 x "), "{}", view.status());
        let at = Pos2::new(200.0, 200.0);
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        run(
            &mut view,
            &ctx,
            vec![egui::Event::PointerMoved(at), click(true), click(false)],
        );
        let key = egui::Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        run(&mut view, &ctx, vec![key]);
        assert!(
            view.status().ends_with("level 5, keys ArrowRight"),
            "{}",
            view.status()
        );
    }

    #[test]
    fn a_table_of_another_version_is_refused() {
        static OLD: Table = Table(WvPlugin {
            api_version: 2,
            name: c"Later".as_ptr(),
            formats: std::ptr::null(),
            format_count: 0,
            open: Some(open),
            frame: Some(frame),
            control: None,
            close: Some(close),
        });
        // SAFETY: the table of the test is static.
        let Err(why) = (unsafe { wrap(&OLD.0) }) else {
            panic!("a version this host does not know");
        };
        assert!(why.contains("stable interface 2"), "{why}");
    }
}
