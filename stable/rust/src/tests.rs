//! The tests of the crate: the layout of `wview.h`, and a plugin made with `plugin!` called through
//! its table as a host calls it.

use std::ffi::{c_char, CStr};
use std::mem::{offset_of, size_of};

use super::*;

#[test]
fn the_structures_have_the_layout_of_the_header() {
    // The sizes of wview.h on a target of 64 bits, as a compiler of C lays them out.
    assert_eq!(size_of::<ffi::WvFormat>(), 24);
    assert_eq!(size_of::<ffi::WvKey>(), 16);
    assert_eq!(size_of::<ffi::WvInput>(), 88);
    assert_eq!(size_of::<ffi::WvControl>(), 64);
    assert_eq!(size_of::<ffi::WvOutput>(), 64);
    assert_eq!(size_of::<ffi::WvPlugin>(), 64);
    assert_eq!(offset_of!(ffi::WvInput, keys), 64);
    assert_eq!(offset_of!(ffi::WvOutput, controls), 32);
}

/// A view of the tests: its frames, a level set by a slider, and the keys of its last frame.
struct Counter {
    frames: u32,
    level: f64,
    pixels: Vec<u8>,
}

impl View for Counter {
    fn frame(&mut self, input: &Input) -> Frame<'_> {
        self.frames += 1;
        if input.keys.iter().any(|k| k.name == "x") {
            panic!("x is no key of a counter");
        }
        let keys: Vec<&str> = input.keys.iter().map(|k| k.name.as_str()).collect();
        self.pixels = vec![self.frames as u8; 2 * 2 * 4];
        Frame {
            picture: Some(Picture::new(2, 2, &self.pixels)),
            changed: true,
            controls: vec![
                Control::slider(1, "Level", self.level, 0.0, 10.0, 1.0),
                Control::choice(2, "Palette", 1, &["grey", "fire"]),
                Control::toggle(3, "Smooth", true),
            ],
            keys: keys::ARROWS | keys::LETTERS,
            cursor: Cursor::Grab,
            again: self.frames < 3,
            status: format!(
                "frame {} of {} x {}, level {}, keys {}",
                self.frames,
                input.width,
                input.height,
                self.level,
                keys.join(" ")
            ),
        }
    }

    fn control(&mut self, id: u32, value: f64) {
        if id == 1 {
            self.level = value;
        }
    }
}

crate::plugin! {
    name: "Counter",
    formats: &[Format { name: "Counts", extensions: &["count", "cnt"] }],
    view: Counter,
    open: |path: &str| {
        if path.contains("bad") {
            Err("no such count".to_owned())
        } else {
            Ok(Counter { frames: 0, level: 5.0, pixels: Vec::new() })
        }
    },
}

fn table() -> &'static ffi::WvPlugin {
    // SAFETY: the table lives for the whole run.
    unsafe { &*wv_plugin_get() }
}

/// The input of a host of version 1 with some keys.
fn input(keys: &[ffi::WvKey]) -> ffi::WvInput {
    ffi::WvInput {
        size: size_of::<ffi::WvInput>() as u32,
        width: 640,
        height: 480,
        scale: 1.0,
        focused: 1,
        hovered: 1,
        mouse_x: 10.0,
        mouse_y: 20.0,
        buttons: 0,
        clicks: ffi::WV_LEFT,
        double_clicks: 0,
        drag_x: 0.0,
        drag_y: 0.0,
        wheel_x: 0.0,
        wheel_y: 0.0,
        modifiers: 0,
        keys: keys.as_ptr(),
        key_count: keys.len() as u32,
        text: c"".as_ptr(),
    }
}

/// The output a host of version 1 gives: its size, the rest zero.
fn output() -> ffi::WvOutput {
    // SAFETY: a struct of integers and pointers is valid when zero.
    let mut out: ffi::WvOutput = unsafe { std::mem::zeroed() };
    out.size = size_of::<ffi::WvOutput>() as u32;
    out
}

/// A text of the plugin.
fn text(t: *const c_char) -> String {
    // SAFETY: the plugin gives texts that end with a zero.
    unsafe { text_of(t) }
}

fn open(path: &CStr) -> *mut std::ffi::c_void {
    // SAFETY: a path that ends with a zero and no buffer for the reason.
    unsafe { (table().open.unwrap())(path.as_ptr(), std::ptr::null_mut(), 0) }
}

fn frame(view: *mut std::ffi::c_void, input: &ffi::WvInput, out: &mut ffi::WvOutput) {
    // SAFETY: a view of the plugin and the input and the output of the call.
    unsafe { (table().frame.unwrap())(view, input, out) }
}

fn close(view: *mut std::ffi::c_void) {
    // SAFETY: a view of the plugin, closed once.
    unsafe { (table().close.unwrap())(view) }
}

#[test]
fn the_table_names_the_plugin_and_its_formats() {
    let t = table();
    assert_eq!(t.api_version, 1);
    assert_eq!(text(t.name), "Counter");
    assert_eq!(t.format_count, 1);
    // SAFETY: the formats of the table, one of them with its extensions.
    let (name, extensions) = unsafe {
        let f = &*t.formats;
        let list: Vec<String> = (0..f.extension_count as usize)
            .map(|i| text(*f.extensions.add(i)))
            .collect();
        (text(f.name), list)
    };
    assert_eq!(name, "Counts");
    assert_eq!(extensions, ["count", "cnt"]);
    assert!(std::ptr::eq(wv_plugin_get(), wv_plugin_get()), "one table");
}

#[test]
fn a_view_opens_draws_takes_its_controls_and_closes() {
    let mut error = [0 as c_char; 8];
    // SAFETY: a path that ends with a zero and a buffer of 8 bytes.
    let bad = unsafe { (table().open.unwrap())(c"bad.count".as_ptr(), error.as_mut_ptr(), 8) };
    assert!(bad.is_null());
    assert_eq!(
        text(error.as_ptr()),
        "no such",
        "the reason cut to the buffer"
    );
    let view = open(c"a.count");
    assert!(!view.is_null());
    let keys = [ffi::WvKey {
        name: c"ArrowLeft".as_ptr(),
        pressed: 1,
        modifiers: ffi::WV_SHIFT,
    }];
    let mut out = output();
    frame(view, &input(&keys), &mut out);
    assert_eq!(
        (out.width, out.height, out.stride, out.changed),
        (2, 2, 8, 1)
    );
    // SAFETY: the output points to the pixels and the controls of the frame until the next call.
    unsafe {
        assert_eq!(*out.pixels, 1);
        assert_eq!(out.control_count, 3);
        let slider = &*out.controls;
        assert_eq!(
            (slider.id, slider.kind, slider.value),
            (1, ffi::WV_SLIDER, 5.0)
        );
        assert_eq!(text(slider.label), "Level");
        let choice = &*out.controls.add(1);
        assert_eq!(
            (choice.kind, choice.value, choice.choice_count),
            (ffi::WV_CHOICE, 1.0, 2)
        );
        assert_eq!(text(*choice.choices.add(1)), "fire");
        let toggle = &*out.controls.add(2);
        assert_eq!((toggle.kind, toggle.value), (ffi::WV_TOGGLE, 1.0));
    }
    assert_eq!(out.keys, ffi::WV_KEYS_ARROWS | ffi::WV_KEYS_LETTERS);
    assert_eq!((out.cursor, out.again), (ffi::WV_CURSOR_GRAB, 1));
    assert_eq!(
        text(out.status),
        "frame 1 of 640 x 480, level 5, keys ArrowLeft"
    );
    // SAFETY: a view of the plugin and an id it named.
    unsafe { (table().control.unwrap())(view, 1, 8.0) };
    frame(view, &input(&[]), &mut out);
    assert_eq!(text(out.status), "frame 2 of 640 x 480, level 8, keys ");
    close(view);
}

#[test]
fn the_sizes_of_the_host_bound_what_is_read_and_written() {
    let view = open(c"a.count");
    let keys = [ffi::WvKey {
        name: c"a".as_ptr(),
        pressed: 1,
        modifiers: 0,
    }];
    // A host that knows the input up to the scale: the focus, the mouse and the keys read as none.
    let mut short = input(&keys);
    short.size = offset_of!(ffi::WvInput, focused) as u32;
    // A host that knows the output up to the pixels: no more of it is written.
    let mut out = output();
    out.size = offset_of!(ffi::WvOutput, changed) as u32;
    frame(view, &short, &mut out);
    assert_eq!((out.width, out.changed), (2, 0));
    assert!(out.controls.is_null() && out.status.is_null());
    let mut out = output();
    frame(view, &short, &mut out);
    assert_eq!(text(out.status), "frame 2 of 640 x 480, level 5, keys ");
    close(view);
}

#[test]
fn a_panic_of_the_plugin_stops_its_view_and_says_why() {
    let view = open(c"a.count");
    let keys = [ffi::WvKey {
        name: c"x".as_ptr(),
        pressed: 1,
        modifiers: 0,
    }];
    let mut out = output();
    frame(view, &input(&keys), &mut out);
    assert!(out.pixels.is_null());
    let why = "the plugin stopped: x is no key of a counter";
    assert_eq!(text(out.status), why);
    let mut out = output();
    frame(view, &input(&[]), &mut out);
    assert_eq!(
        text(out.status),
        why,
        "a view that stopped is called no more"
    );
    close(view);
}

#[test]
fn a_picture_shorter_than_its_size_is_none() {
    let pixels = [0u8; 15];
    assert!(!Picture::new(2, 2, &pixels).whole());
    assert!(Picture::new(2, 2, &[0u8; 16]).whole());
    let rows = Picture {
        width: 1,
        height: 2,
        stride: 8,
        pixels: &[0u8; 12],
    };
    assert!(rows.whole(), "the last row needs no stride after it");
    assert!(!Picture::new(0, 2, &[]).whole());
}
