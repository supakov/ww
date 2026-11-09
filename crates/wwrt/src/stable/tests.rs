//! The tests of the host of the stable interface: a plugin of the header written in Rust as one in
//! C would be, and one written with the crate `ww-stable`, each wrapped and drawn.

use super::*;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

#[test]
fn the_structures_have_the_layout_of_the_header() {
    // The sizes of ww.h on a target of 64 bits, as a compiler of C lays them out.
    assert_eq!(std::mem::size_of::<WwFormat>(), 40);
    assert_eq!(std::mem::size_of::<WwFile>(), 24);
    assert_eq!(std::mem::size_of::<WwSource>(), 48);
    assert_eq!(std::mem::size_of::<WwCache>(), 56);
    assert_eq!(std::mem::size_of::<WwHost>(), 40);
    assert_eq!(std::mem::size_of::<WwKey>(), 16);
    assert_eq!(std::mem::size_of::<WwInput>(), 88);
    assert_eq!(std::mem::size_of::<WwControl>(), 64);
    assert_eq!(std::mem::size_of::<WwOutput>(), 64);
    assert_eq!(std::mem::size_of::<WwPlugin>(), 96);
    assert_eq!(std::mem::offset_of!(WwInput, keys), 64);
    assert_eq!(std::mem::offset_of!(WwOutput, controls), 32);
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
    controls: [WwControl; 2],
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
            std::ptr::copy_nonoverlapping(why.as_ptr().cast(), error, why.len().min(size as usize));
            return std::ptr::null_mut();
        }
    }
    Box::into_raw(Box::new(Picture {
        frames: 0,
        level: 5.0,
    }))
    .cast()
}

/// Some bytes as a picture: the level is their first byte.
unsafe extern "C" fn open_bytes(
    source: *const WwSource,
    _name: *const c_char,
    _mime: *const c_char,
    _error: *mut c_char,
    _size: u32,
) -> *mut c_void {
    // SAFETY: the source of the host lives as long as the view.
    unsafe {
        let s = &*source;
        let mut first = [0u8; 1];
        let n = (s.read.unwrap())(s.self_, 0, first.as_mut_ptr(), 1);
        Box::into_raw(Box::new(Picture {
            frames: 0,
            level: if n == 1 { f64::from(first[0]) } else { -1.0 },
        }))
        .cast()
    }
}

unsafe extern "C" fn sniff(bytes: *const u8, count: u32) -> *const c_char {
    // SAFETY: the host gives `count` bytes.
    let head = unsafe { std::slice::from_raw_parts(bytes, count as usize) };
    if head.starts_with(b"PIC") {
        c"image/x-test-picture".as_ptr()
    } else if head.starts_with(b"OTHER") {
        // A type of no format of the plugin: the host takes none.
        c"image/png".as_ptr()
    } else {
        std::ptr::null()
    }
}

unsafe extern "C" fn frame(view: *mut c_void, input: *const WwInput, out: *mut WwOutput) {
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
                WwControl {
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
                WwControl {
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

struct Table(WwPlugin);
// SAFETY: the table holds pointers to static data alone.
unsafe impl Sync for Table {}

#[repr(transparent)]
struct Pointer(*const c_char);
// SAFETY: a pointer to a static text.
unsafe impl Sync for Pointer {}
static EXTENSIONS: [Pointer; 1] = [Pointer(c"pic".as_ptr())];
static MIMES: [Pointer; 1] = [Pointer(c"image/x-test-picture".as_ptr())];
struct Formats([WwFormat; 1]);
// SAFETY: pointers to static data alone.
unsafe impl Sync for Formats {}
static FORMATS: Formats = Formats([WwFormat {
    name: c"Pictures of the test".as_ptr(),
    extensions: EXTENSIONS.as_ptr().cast(),
    extension_count: 1,
    mimes: MIMES.as_ptr().cast(),
    mime_count: 1,
}]);
struct Files([WwFile; 2]);
// SAFETY: pointers to static data alone.
unsafe impl Sync for Files {}
static FILES: Files = Files([
    WwFile {
        path: c"docs\\guide.md".as_ptr(),
        bytes: b"# Pictures".as_ptr(),
        size: 10,
    },
    WwFile {
        path: c"lang/uk/ui.json".as_ptr(),
        bytes: b"{}".as_ptr(),
        size: 2,
    },
]);
static TABLE: Table = Table(WwPlugin {
    api_version: 1,
    size: std::mem::size_of::<WwPlugin>() as u32,
    name: c"A stable plugin".as_ptr(),
    formats: FORMATS.0.as_ptr(),
    format_count: 1,
    files: FILES.0.as_ptr(),
    file_count: 2,
    open: Some(open),
    open_bytes: Some(open_bytes),
    sniff: Some(sniff),
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
    assert_eq!(plugin.formats()[0].mimes, ["image/x-test-picture"]);
    let files = plugin.files();
    assert_eq!(
        files.text("docs/guide.md"),
        Some("# Pictures"),
        "the path made one"
    );
    assert_eq!(files.translations(), [("uk", "{}")]);
    assert_eq!(plugin.sniff(b"PIC 1"), Some("image/x-test-picture"));
    assert_eq!(
        plugin.sniff(b"OTHER"),
        None,
        "a type of no format of the plugin"
    );
    assert_eq!(plugin.sniff(b"none"), None);
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
    let closed = CLOSED.load(Ordering::SeqCst);
    drop(view);
    assert_eq!(
        CLOSED.load(Ordering::SeqCst),
        closed + 1,
        "a closed tab closes its view"
    );
}

#[test]
fn some_bytes_reach_the_plugin_through_a_source() {
    // SAFETY: the table of the test is static.
    let plugin = unsafe { wrap(&TABLE.0) }.unwrap();
    assert!(plugin.opens_bytes());
    let file = BlockFile::from_bytes(vec![7, 8, 9]).slice(1, 2);
    let mut view = plugin
        .open_bytes(
            "part",
            "image/x-test-picture",
            file,
            &serde_json::Value::Null,
        )
        .unwrap();
    let ctx = egui::Context::default();
    run(&mut view, &ctx, Vec::new());
    assert!(view.status().ends_with("level 8"), "{}", view.status());
}

#[test]
fn the_table_of_the_host_reads_files_through_the_cache() {
    let path = std::env::temp_dir().join(format!("wwrt-stable-{}.bin", std::process::id()));
    std::fs::write(&path, b"0123456789").unwrap();
    // SAFETY: the table of the host is static; its sources are read and freed as ww.h says.
    unsafe {
        let host = &*host();
        assert_eq!((host.size, host.api_version), (40, 1));
        let name = CString::new(path.to_string_lossy().into_owned()).unwrap();
        let mut error = [0 as c_char; 128];
        let source = (host.open_file.unwrap())(name.as_ptr(), error.as_mut_ptr(), 128);
        assert!(!source.is_null());
        let s = &*source;
        assert_eq!((s.size, s.length, s.offset), (48, 10, 0));
        assert_eq!(text(s.path), path.to_string_lossy());
        let part = (host.slice.unwrap())(source, 4, 3);
        let p = &*part;
        assert_eq!((p.length, p.offset), (3, 4));
        let mut out = [0u8; 8];
        assert_eq!((p.read.unwrap())(p.self_, 1, out.as_mut_ptr(), 8), 2);
        assert_eq!(&out[..2], b"56");
        let mut state = WwCache {
            size: std::mem::size_of::<WwCache>() as u32,
            ..WwCache::default()
        };
        (host.cache.unwrap())(&raw mut state);
        assert_eq!(state.block as usize, blocks::cache().state().limits.block);
        assert!(state.files >= 1 && state.reads >= 1, "{}", state.files);
        (host.close_source.unwrap())(part);
        (host.close_source.unwrap())(source);
        let missing = c"no such file of the test";
        let none = (host.open_file.unwrap())(missing.as_ptr(), error.as_mut_ptr(), 128);
        assert!(none.is_null());
        assert!(text(error.as_ptr()).starts_with("no such file of the test: "));
    }
    let _ = std::fs::remove_file(&path);
}

/// A plugin of the stable interface written with the crate `ww-stable`, as one of Rust of
/// another toolchain would be.
mod with_the_crate {
    use ww_stable::{Control, Cursor, File, Format, Frame, Input, Picture, Plugin, Source, View};

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
                keys: ww_stable::keys::ARROWS,
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

    ww_stable::plugin!(
        Grey,
        Plugin {
            name: "Grey of the crate",
            formats: &[Format {
                name: "Grey files",
                extensions: &["grey"],
                mimes: &["application/x-grey"],
            }],
            files: &[File {
                path: "grey.md",
                bytes: b"# Grey",
            }],
            open: |path: &str| {
                if path.ends_with(".grey") {
                    Ok(Grey {
                        level: 5.0,
                        pixels: Vec::new(),
                    })
                } else {
                    Err(format!("{path} is no grey file"))
                }
            },
            // The level of some bytes is their length; the file they come from is read through
            // the host too.
            open_bytes: Some(|source: Source, _name: &str, _mime: &str| {
                let whole = source
                    .path()
                    .map(|p| ww_stable::open_file(&p).map(|f| f.len()));
                let level = match whole {
                    Some(Ok(n)) => (n - source.len()) as f64,
                    _ => source.len() as f64,
                };
                Ok(Grey {
                    level,
                    pixels: Vec::new(),
                })
            }),
            sniff: Some(|head: &[u8]| head.starts_with(b"GREY").then_some("application/x-grey")),
        }
    );
}

/// The two sides of `ww.h` checked against each other: the table the crate makes, wrapped by
/// this host and drawn.
#[test]
fn a_plugin_written_with_the_crate_is_a_plugin() {
    use ww_stable::ffi;
    assert_eq!(
        std::mem::size_of::<ffi::WwInput>(),
        std::mem::size_of::<WwInput>()
    );
    assert_eq!(
        std::mem::size_of::<ffi::WwOutput>(),
        std::mem::size_of::<WwOutput>()
    );
    assert_eq!(
        std::mem::size_of::<ffi::WwControl>(),
        std::mem::size_of::<WwControl>()
    );
    assert_eq!(
        std::mem::size_of::<ffi::WwPlugin>(),
        std::mem::size_of::<WwPlugin>()
    );
    assert_eq!(
        std::mem::size_of::<ffi::WwHost>(),
        std::mem::size_of::<WwHost>()
    );
    let table = with_the_crate::ww_plugin_get(host().cast());
    // SAFETY: the table of the crate has the layout of the header and lives for the run.
    let plugin = unsafe { wrap(&*table.cast::<WwPlugin>()) }.unwrap();
    assert_eq!(plugin.name(), "Grey of the crate");
    assert_eq!(plugin.formats()[0].extensions, ["grey"]);
    assert_eq!(plugin.files().text("grey.md"), Some("# Grey"));
    assert_eq!(plugin.sniff(b"GREY"), Some("application/x-grey"));
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
    // Some bytes of a file: a part of 3 of its 10 bytes, its file read by the plugin through the
    // host, so the level is the 7 bytes outside the part.
    let path = std::env::temp_dir().join(format!("wwrt-grey-{}.grey", std::process::id()));
    std::fs::write(&path, b"GREY012345").unwrap();
    let part = BlockFile::open(&path).unwrap().slice(2, 3);
    let mut view = plugin
        .open_bytes("part", "application/x-grey", part, &serde_json::Value::Null)
        .unwrap();
    run(&mut view, &ctx, Vec::new());
    assert!(view.status().contains("level 7"), "{}", view.status());
    drop(view);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_table_of_another_version_is_refused() {
    static OLD: Table = Table(WwPlugin {
        api_version: 2,
        size: std::mem::size_of::<WwPlugin>() as u32,
        name: c"Later".as_ptr(),
        formats: std::ptr::null(),
        format_count: 0,
        files: std::ptr::null(),
        file_count: 0,
        open: Some(open),
        open_bytes: None,
        sniff: None,
        frame: Some(frame),
        control: None,
        close: Some(close),
    });
    // SAFETY: the table of the test is static.
    let Err(why) = (unsafe { wrap(&OLD.0) }) else {
        panic!("a version this host does not know");
    };
    assert!(why.contains("stable interface 2"), "{why}");
    /// A table whose size covers the functions up to `open` alone: the others read as none.
    static SHORT: Table = Table(WwPlugin {
        api_version: 1,
        size: std::mem::offset_of!(WwPlugin, open_bytes) as u32,
        name: c"Short".as_ptr(),
        formats: std::ptr::null(),
        format_count: 0,
        files: std::ptr::null(),
        file_count: 0,
        open: Some(open),
        open_bytes: Some(open_bytes),
        sniff: Some(sniff),
        frame: Some(frame),
        control: None,
        close: Some(close),
    });
    // SAFETY: as above.
    let Err(why) = (unsafe { wrap(&SHORT.0) }) else {
        panic!("a table without its frame");
    };
    assert!(why.contains("without its functions"), "{why}");
}
