//! The tests of the crate: the layout of `ww.h`, and a plugin made with `plugin!` called through
//! its table as a host calls it, with a host of the tests that reads bytes in memory.

use std::ffi::{c_char, c_void, CStr};
use std::mem::{offset_of, size_of};
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

#[test]
fn the_structures_have_the_layout_of_the_header() {
    // The sizes of ww.h on a target of 64 bits, as a compiler of C lays them out.
    assert_eq!(size_of::<ffi::WwFormat>(), 40);
    assert_eq!(size_of::<ffi::WwFile>(), 24);
    assert_eq!(size_of::<ffi::WwSource>(), 48);
    assert_eq!(size_of::<ffi::WwCache>(), 56);
    assert_eq!(size_of::<ffi::WwHost>(), 64);
    assert_eq!(size_of::<ffi::WwTool>(), 48);
    assert_eq!(size_of::<ffi::WwOption>(), 56);
    assert_eq!(size_of::<ffi::WwTools>(), 64);
    assert_eq!(size_of::<ffi::WwKey>(), 16);
    assert_eq!(size_of::<ffi::WwInput>(), 88);
    assert_eq!(size_of::<ffi::WwControl>(), 64);
    assert_eq!(size_of::<ffi::WwOutput>(), 64);
    assert_eq!(size_of::<ffi::WwPlugin>(), 96);
    assert_eq!(offset_of!(ffi::WwInput, keys), 64);
    assert_eq!(offset_of!(ffi::WwOutput, controls), 32);
    assert_eq!(offset_of!(ffi::WwSource, read), 24);
    assert_eq!(offset_of!(ffi::WwPlugin, open), 48);
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

crate::plugin!(
    Counter,
    Plugin {
        name: "Counter",
        formats: &[Format {
            name: "Counts",
            extensions: &["count", "cnt"],
            mimes: &["application/x-count"],
        }],
        files: &[File {
            path: "guide.md",
            bytes: b"# Counts",
        }],
        open: |path: &str| {
            if path.contains("bad") {
                Err("no such count".to_owned())
            } else {
                Ok(Counter {
                    frames: 0,
                    level: 5.0,
                    pixels: Vec::new(),
                })
            }
        },
        // The level of some bytes: the digit of their last byte.
        open_bytes: Some(|source: Source, _name: &str, mime: &str| {
            if mime != "application/x-count" {
                return Err(format!("no count of {mime}"));
            }
            let last = source.read(source.len().saturating_sub(1), 1);
            let level = last
                .first()
                .map_or(0.0, |b| f64::from(b.wrapping_sub(b'0')));
            Ok(Counter {
                frames: 0,
                level,
                pixels: Vec::new(),
            })
        }),
        sniff: Some(|head: &[u8]| head.starts_with(b"COUNT").then_some("APPLICATION/X-COUNT")),
    }
);

/// The bytes the host of the tests reads, a file of it named `memory`.
static BYTES: &[u8] = b"COUNT 0123456789";
/// The sources of `open_file` and `slice` the host of the tests gave and that are not freed.
static OPEN: AtomicU32 = AtomicU32::new(0);

/// A source of the host of the tests: its structure first, then the part of the bytes it reads.
#[repr(C)]
struct Part {
    raw: ffi::WwSource,
    start: u64,
}

unsafe extern "C" fn part_read(this: *mut c_void, offset: u64, out: *mut u8, count: u32) -> u32 {
    // SAFETY: `self` of a source of the tests is its part, and `out` holds `count` bytes.
    unsafe {
        let part = &*this.cast::<Part>();
        if offset >= part.raw.length {
            return 0;
        }
        let n = (u64::from(count)).min(part.raw.length - offset) as usize;
        let at = (part.start + offset) as usize;
        std::ptr::copy_nonoverlapping(BYTES[at..].as_ptr(), out, n);
        n as u32
    }
}

fn part(start: u64, length: u64) -> *const ffi::WwSource {
    let boxed = Box::into_raw(Box::new(Part {
        raw: ffi::WwSource {
            size: size_of::<ffi::WwSource>() as u32,
            length,
            self_: std::ptr::null_mut(),
            read: Some(part_read),
            path: c"memory".as_ptr(),
            offset: start,
        },
        start,
    }));
    // SAFETY: the box was just made.
    unsafe { (*boxed).raw.self_ = boxed.cast() };
    boxed.cast_const().cast()
}

unsafe extern "C" fn fake_open_file(
    path: *const c_char,
    error: *mut c_char,
    size: u32,
) -> *const ffi::WwSource {
    // SAFETY: a path that ends with a zero.
    if unsafe { CStr::from_ptr(path) } == c"memory" {
        OPEN.fetch_add(1, Ordering::SeqCst);
        return part(0, BYTES.len() as u64);
    }
    let why = c"no such file".to_bytes_with_nul();
    // SAFETY: the buffer of the plugin holds `size` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(why.as_ptr().cast(), error, why.len().min(size as usize))
    };
    std::ptr::null()
}

unsafe extern "C" fn fake_slice(
    source: *const ffi::WwSource,
    offset: u64,
    length: u64,
) -> *const ffi::WwSource {
    // SAFETY: a source of the tests is a part.
    let p = unsafe { &*source.cast::<Part>() };
    let offset = offset.min(p.raw.length);
    OPEN.fetch_add(1, Ordering::SeqCst);
    part(p.start + offset, length.min(p.raw.length - offset))
}

unsafe extern "C" fn fake_close_source(source: *const ffi::WwSource) {
    OPEN.fetch_sub(1, Ordering::SeqCst);
    // SAFETY: a part the host of the tests made, freed once.
    drop(unsafe { Box::from_raw(source.cast::<Part>().cast_mut()) });
}

unsafe extern "C" fn cache_state(out: *mut ffi::WwCache) {
    // SAFETY: the plugin gives a structure of its size.
    unsafe {
        (*out).block = 4096;
        (*out).reads = 7;
    }
}

struct HostTable(ffi::WwHost);
// SAFETY: functions alone.
unsafe impl Sync for HostTable {}

static HOST_OF_TESTS: HostTable = HostTable(ffi::WwHost {
    size: size_of::<ffi::WwHost>() as u32,
    api_version: 1,
    open_file: Some(fake_open_file),
    slice: Some(fake_slice),
    close_source: Some(fake_close_source),
    cache: Some(cache_state),
    this: std::ptr::null_mut(),
    call_tool: Some(fake_call_tool),
    free_text: Some(fake_free_text),
});

/// A host whose one tool, `twice`, gives its argument `n` twice; any other is not found.
unsafe extern "C" fn fake_call_tool(
    _this: *mut c_void,
    name: *const c_char,
    args: *const c_char,
    answer: *mut *mut c_char,
) -> i32 {
    // SAFETY: the crate gives texts that end with a zero and where the answer goes.
    let (name, args) = unsafe { (text_of(name), text_of(args)) };
    let (code, text) = if name == "twice" {
        let n: i64 = args
            .trim_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .unwrap_or(0);
        (0, (2 * n).to_string())
    } else {
        (TOOL_NOT_FOUND, format!("no tool {name}"))
    };
    // SAFETY: as above.
    unsafe { *answer = c_text(&text).into_raw() };
    code
}

unsafe extern "C" fn fake_free_text(text: *mut c_char) {
    // SAFETY: a text of `fake_call_tool`, freed once.
    drop(unsafe { CString::from_raw(text) });
}

fn table() -> &'static ffi::WwPlugin {
    // SAFETY: the table lives for the whole run, the host of the tests too.
    unsafe { &*ww_plugin_get(&HOST_OF_TESTS.0) }
}

/// The input of a host of version 1 with some keys.
fn input(keys: &[ffi::WwKey]) -> ffi::WwInput {
    ffi::WwInput {
        size: size_of::<ffi::WwInput>() as u32,
        width: 640,
        height: 480,
        scale: 1.0,
        focused: 1,
        hovered: 1,
        mouse_x: 10.0,
        mouse_y: 20.0,
        buttons: 0,
        clicks: ffi::WW_LEFT,
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
fn output() -> ffi::WwOutput {
    // SAFETY: a struct of integers and pointers is valid when zero.
    let mut out: ffi::WwOutput = unsafe { std::mem::zeroed() };
    out.size = size_of::<ffi::WwOutput>() as u32;
    out
}

/// A text of the plugin.
fn text(t: *const c_char) -> String {
    // SAFETY: the plugin gives texts that end with a zero.
    unsafe { text_of(t) }
}

fn open(path: &CStr) -> *mut c_void {
    // SAFETY: a path that ends with a zero and no buffer for the reason.
    unsafe { (table().open.unwrap())(path.as_ptr(), std::ptr::null_mut(), 0) }
}

fn frame(view: *mut c_void, input: &ffi::WwInput, out: &mut ffi::WwOutput) {
    // SAFETY: a view of the plugin and the input and the output of the call.
    unsafe { (table().frame.unwrap())(view, input, out) }
}

fn close(view: *mut c_void) {
    // SAFETY: a view of the plugin, closed once.
    unsafe { (table().close.unwrap())(view) }
}

#[test]
fn the_table_names_the_plugin_its_formats_and_its_files() {
    let t = table();
    assert_eq!((t.api_version, t.size), (1, 96));
    assert_eq!(text(t.name), "Counter");
    assert_eq!(t.format_count, 1);
    // SAFETY: the formats of the table, one of them with its extensions and types.
    let (name, extensions, mimes) = unsafe {
        let f = &*t.formats;
        let list = |l: *const *const c_char, n: u32| -> Vec<String> {
            (0..n as usize).map(|i| text(*l.add(i))).collect()
        };
        (
            text(f.name),
            list(f.extensions, f.extension_count),
            list(f.mimes, f.mime_count),
        )
    };
    assert_eq!(name, "Counts");
    assert_eq!(extensions, ["count", "cnt"]);
    assert_eq!(mimes, ["application/x-count"]);
    assert_eq!(t.file_count, 1);
    // SAFETY: the file of the table.
    let (path, bytes) = unsafe {
        let f = &*t.files;
        (
            text(f.path),
            std::slice::from_raw_parts(f.bytes, f.size as usize),
        )
    };
    assert_eq!((path.as_str(), bytes), ("guide.md", &b"# Counts"[..]));
    assert!(t.open_bytes.is_some() && t.sniff.is_some());
    assert!(std::ptr::eq(table(), table()), "one table");
}

#[test]
fn the_first_bytes_name_a_type_of_the_formats() {
    let sniff = table().sniff.unwrap();
    // SAFETY: the bytes live for the call.
    let found = unsafe { sniff(b"COUNT 3".as_ptr(), 7) };
    assert_eq!(text(found), "application/x-count", "the text of the table");
    // SAFETY: as above.
    assert!(unsafe { sniff(b"other".as_ptr(), 5) }.is_null());
}

#[test]
fn a_view_of_some_bytes_reads_its_source() {
    let source = part(6, 4);
    let mut error = [0 as c_char; 64];
    let open_bytes = table().open_bytes.unwrap();
    // SAFETY: a source of the host, texts that end with a zero and a buffer of 64 bytes.
    let view = unsafe {
        open_bytes(
            source,
            c"part".as_ptr(),
            c"application/x-count".as_ptr(),
            error.as_mut_ptr(),
            64,
        )
    };
    assert!(!view.is_null(), "{}", text(error.as_ptr()));
    let mut out = output();
    frame(view, &input(&[]), &mut out);
    assert!(text(out.status).contains("level 3"), "{}", text(out.status));
    close(view);
    // SAFETY: as above.
    let other = unsafe {
        open_bytes(
            source,
            c"part".as_ptr(),
            c"text/plain".as_ptr(),
            error.as_mut_ptr(),
            64,
        )
    };
    assert!(other.is_null());
    assert_eq!(text(error.as_ptr()), "no count of text/plain");
    // SAFETY: the part of the test, freed once.
    drop(unsafe { Box::from_raw(source.cast::<Part>().cast_mut()) });
}

#[test]
fn the_host_reads_a_file_and_a_part_of_it_for_the_plugin() {
    // The table of the host is kept by ww_plugin_get.
    let _ = table();
    let before = OPEN.load(Ordering::SeqCst);
    {
        let file = open_file("memory").unwrap();
        assert_eq!((file.len(), file.path().as_deref()), (16, Some("memory")));
        assert_eq!(file.read(6, 4), b"0123");
        let part = file.slice(10, 100).unwrap();
        assert_eq!((part.len(), part.offset()), (6, 10));
        assert_eq!(part.read(0, 100), b"456789");
        let mut out = [0u8; 3];
        assert_eq!(part.read_at(4, &mut out), 2);
        assert_eq!(&out[..2], b"89");
        assert_eq!(OPEN.load(Ordering::SeqCst), before + 2);
    }
    assert_eq!(
        OPEN.load(Ordering::SeqCst),
        before,
        "a source dropped is freed"
    );
    assert_eq!(open_file("other").err().as_deref(), Some("no such file"));
    let state = cache().unwrap();
    assert_eq!((state.block, state.reads), (4096, 7));
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
    let keys = [ffi::WwKey {
        name: c"ArrowLeft".as_ptr(),
        pressed: 1,
        modifiers: ffi::WW_SHIFT,
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
            (1, ffi::WW_SLIDER, 5.0)
        );
        assert_eq!(text(slider.label), "Level");
        let choice = &*out.controls.add(1);
        assert_eq!(
            (choice.kind, choice.value, choice.choice_count),
            (ffi::WW_CHOICE, 1.0, 2)
        );
        assert_eq!(text(*choice.choices.add(1)), "fire");
        let toggle = &*out.controls.add(2);
        assert_eq!((toggle.kind, toggle.value), (ffi::WW_TOGGLE, 1.0));
    }
    assert_eq!(out.keys, ffi::WW_KEYS_ARROWS | ffi::WW_KEYS_LETTERS);
    assert_eq!((out.cursor, out.again), (ffi::WW_CURSOR_GRAB, 1));
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
    let keys = [ffi::WwKey {
        name: c"a".as_ptr(),
        pressed: 1,
        modifiers: 0,
    }];
    // A host that knows the input up to the scale: the focus, the mouse and the keys read as none.
    let mut short = input(&keys);
    short.size = offset_of!(ffi::WwInput, focused) as u32;
    // A host that knows the output up to the pixels: no more of it is written.
    let mut out = output();
    out.size = offset_of!(ffi::WwOutput, changed) as u32;
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
    let keys = [ffi::WwKey {
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

/// The table of the tools the crate writes: the texts, the options with their values, the call
/// with its answer and its error, and a call of a tool of the host by its name.
#[test]
fn the_table_of_the_tools_is_the_one_of_the_header() {
    fn run(tool: &str, args: &str, _options: &str) -> Result<String, (i32, String)> {
        match tool {
            "echo" => Ok(args.to_owned()),
            "host" => call_tool("twice", r#"{"n": 21}"#),
            "boom" => panic!("broken"),
            _ => Err((TOOL_NOT_FOUND, format!("no tool {tool}"))),
        }
    }
    static TOOLS: Tools = Tools {
        name: "Tests",
        tools: &[Tool {
            name: "echo",
            description: "Echoes.",
            schema: r#"{"type": "object"}"#,
            group: "words",
            mode: MODE_EDIT,
            needs: "access=on",
        }],
        options: &[ToolOption {
            key: "access",
            title: "Access",
            values: &["off", "on"],
            labels: &["Off", "On"],
            initial: "off",
            help: "",
        }],
        call: run,
    };
    unsafe extern "C" fn call(
        tool: *const c_char,
        args: *const c_char,
        options: *const c_char,
        answer: *mut *mut c_char,
    ) -> i32 {
        // SAFETY: the test gives texts that end with a zero and where the answer goes.
        unsafe { glue::call_tool(tool, args, options, answer, TOOLS.call) }
    }
    // SAFETY: the host of the tests lives for the run.
    unsafe { glue::set_host(&HOST_OF_TESTS.0) };
    let table = ToolsTable::new(&TOOLS, call);
    // SAFETY: the table and its texts live as long as `table`.
    unsafe {
        let t = &*table.tools();
        assert_eq!(
            (t.api_version, t.size as usize),
            (1, size_of::<ffi::WwTools>())
        );
        assert_eq!(text_of(t.name), "Tests");
        assert_eq!(t.tool_count, 1);
        let tool = &*t.tools;
        assert_eq!(text_of(tool.needs), "access=on");
        assert_eq!(tool.mode, MODE_EDIT);
        let option = &*t.options;
        assert_eq!(option.value_count, 2);
        assert_eq!(text_of(*option.labels.add(1)), "On");
        let run = |name: &str, args: &str| {
            let (name, args, options) = (c_text(name), c_text(args), c_text("{}"));
            let mut answer: *mut c_char = std::ptr::null_mut();
            let code =
                (t.call.unwrap())(name.as_ptr(), args.as_ptr(), options.as_ptr(), &mut answer);
            let text = text_of(answer);
            (t.free_answer.unwrap())(answer);
            (code, text)
        };
        assert_eq!(run("echo", r#"{"a": 1}"#), (0, r#"{"a": 1}"#.to_owned()));
        assert_eq!(run("host", ""), (0, "42".to_owned()));
        assert_eq!(run("none", "").0, TOOL_NOT_FOUND);
        let (code, why) = run("boom", "");
        assert_eq!(code, TOOL_FAILED);
        assert!(why.contains("broken"), "{why}");
    }
    assert_eq!(call_tool("other", "{}").unwrap_err().0, TOOL_NOT_FOUND);
}
