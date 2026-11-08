# Plugins of W#: the study and the interface

Plugins written in Rust for the programs on W#, the study that chose their way, the interface of
the plugins (`wwrt`), the stable interface for other languages (`stable/`) and its examples
(`examples/`). The plugins of wview, its release, its SDK and its build are in `docs/plugins.md`
of supakov/wview; the sections keep the numbers they had in one document, so a reference to one
of them holds. The history of each version is in `releases/`.

## 1. The question

A program loads the DLLs of a folder, written in Rust as the program is. A plugin should hold its
own code alone: std, egui, eframe and W# belong to DLLs every program of this kind shares, which is
the exception such a program makes to the rule of one executable (AGENTS.md, rule 4). The way should
stay as close to ordinary Rust as it can: traits, `egui::Ui`, the types of W#, no C layer in
between.

wview is a direction of its own, and its plugins come in two kinds: native
ones with the SDK of a release, the main way of every example, and a stable interface of C for
other languages (the decisions of the customer of 04.11.2026, sections 6.5 and 6.7).

## 2. The ways Rust has

| Way | The plugin draws in the window of the host | Base code in each plugin | ABI | Verdict |
|---|---|---|---|---|
| A runtime as a Rust `dylib`, the plugins `cdylib` on it | yes: one egui, one `Ui`, the traits of W# | none | the compiler and the lock file of the build | chosen |
| `cdylib` with a C ABI | no: each plugin has its own egui, its own statics; a `Ui` of the host cannot cross | all of it, about 10 MB a plugin | stable | only for plugins of others that give data |
| `abi_stable`, `stabby` | no: only their stable types cross | all of it | stable, checked at the load | the same |
| WebAssembly (wasmtime, extism) | no: the plugin describes, the host draws | a runtime of 10 MB and more | stable, a sandbox | for plugins nobody vouches for |
| A process a plugin, IPC | no | a whole program | stable, isolated | for crashes and rights apart |
| Descriptions as data (JSON) | not code | none | the format of the file | inside a plugin: the binary formats of wv-binary (section 6.4) |

Rust has no stable ABI, so the first way needs one build of the host, the runtime and the plugins:
one compiler, one `Cargo.lock`, one profile. For a family of programs and plugins of one repository
that is no loss, and it is the only way where a plugin is its own code alone and draws with the egui
of the program.

## 3. What was built

The prototype of the study (now `wview/`):

- `wwrt`, a Rust `dylib` (`wwrt.dll`): W# with egui, eframe, glow, wgpu, winit, serde, serde_json,
  the crate `windows` and libloading, all re-exported (`wwrt::ww`, `wwrt::egui`, `wwrt::serde`,
  `wwrt::windows`). `wwrt::plugin` is the interface: `Plugin` (its name, its `Format`s, `open`),
  `View` (`ui`, `tools` and `act`, `search`, `browses`, `status`), `Host` (the settings a view
  draws with), the macro `plugin!` and `Plugins::load_dir`, the loader. `wwrt::blocks::BlockFile`
  reads a file in blocks with a cache of a fixed size.
- `wview`, the program: the window of W#, a tab a file, the command bar of the view in front, the
  page of the plugins without a file; `--plugins` lists them, `--probe FILE` opens a file and draws
  three frames without a window.
- `wv-text`, Markdown pages and text files, two formats in one plugin, drawn by `ww::mdview` with
  the search of W#, Wrap lines and Width of pages.
- `wv-image`, images through the Windows Imaging Component: the formats of the decoders of this
  Windows, read at the load (on this PC 14 decoders: BMP, GIF, ICO, CUR, JPEG, PNG, TIFF, DNG,
  JPEG XR, DDS, HEIF and AVIF, WebP, camera RAW of 36 extensions, JPEG XL); Fit, 100%, Zoom in,
  Zoom out and Ctrl with the wheel; an image above 8192 pixels a side is scaled down by WIC as it is
  decoded. The host steps through the images of the folder: Previous and Next in the bar, the
  arrows, Page Up and Page Down, Home and End.

A whole plugin is its formats, its view and one line:

```rust
use wwrt::plugin::{Format, Host, Plugin, View};

struct TextPlugin;

impl Plugin for TextPlugin {
    fn name(&self) -> &'static str { "Text and Markdown" }
    fn formats(&self) -> &'static [Format] { &FORMATS }
    fn open(&self, path: &Path) -> Result<Box<dyn View>, String> { /* read, make the view */ }
}

wwrt::plugin!(TextPlugin);
```

`plugin!` exports two functions: `ww_plugin_abi`, the build of wwrt the plugin was made with, and
`ww_plugin`, which makes the plugin. The loader compares the first with its own build, then calls
the second and keeps the DLL for the whole run.

## 4. Measurements

On 04.11.2026 on the computer of the developer: Rust 1.99.1, the target
`x86_64-pc-windows-gnu`, half of an i5-1035G1 (`cargo50.cmd`), the release profile of the study (no
LTO, one codegen unit, stripped).

| File | Size | It needs |
|---|---|---|
| `wview.exe` | 477 KB | `wwrt.dll`, `std-*.dll` |
| `wwrt.dll` | 20.9 MB | `std-*.dll` |
| `std-f903e3d16df36da5.dll` | 5.3 MB | the toolchain gnu also `libgcc_s_seh-1.dll` (142 KB) and `libwinpthread-1.dll` |
| `plugins\wv_text.dll` | 27 KB | `wwrt.dll`, `std-*.dll` |
| `plugins\wv_image.dll` | 52 KB | `wwrt.dll`, `std-*.dll` |

For comparison wedit, a static program with LTO: `wedit.exe` 12.9 MB. A plugin with its own copy of egui would weigh about what wedit does (an
estimate, not built).

| What | Time |
|---|---|
| The two plugins loaded (the folder read, the DLLs, the checks, the list of the decoders of WIC) | 0.9 ms |
| AGENTS.md (96 KB, 1007 lines) opened / three frames drawn | 12 ms / 78 ms |
| A text of 40 lines | 10 ms / 8 ms |
| A PNG of 640 x 480, a BMP of 320 x 240 | 13 ms / 2 ms |
| A PNG of 9000 x 2000, decoded at 8192 x 1820 | 144 ms / 46 ms |
| A whole release build of the study | 9 min 10 s |
| A whole debug build (the dependencies at opt-level 2) | 10 min 39 s |
| A plugin rebuilt after a change, debug (finding 11) | 50-55 s |
| wwrt, wview and both plugins after a change of wwrt, debug | 86 s |

The folder of the program ran with a PATH of `C:\Windows\System32` and `C:\Windows` alone. A DLL
that is no plugin (a copy of `std-*.dll` in `plugins`) and a plugin whose build differs (its text
of the build patched) were refused with their reasons, and Windows showed no dialog. The window
itself was not opened on the computer of the developer, which runs nothing visible: `--probe`
draws the same views with the same egui, without a screen.

## 5. What the study found

1. **One std for the family.** A `dylib` takes std into itself, and an executable that links it
   wants std of its own: rustc stops with `cannot satisfy dependencies so std only shows up once`.
   Every crate of the family is built with `-C prefer-dynamic` (`wview/.cargo/config.toml`),
   std becomes `std-<hash>.dll` of the toolchain, and it ships next to the executable.
2. **The 65535 exports of a DLL.** A `dylib` exports every public symbol of the crates it holds,
   and the export table of a PE file numbers its entries in 16 bits. A debug `wwrt.dll` wanted
   128973 exports: `ld: error: export ordinal too large: 128973` (debug builds share the instances
   of generic functions between crates and export them). The release one has 17390, 15616 with std
   apart. The largest shares: `windows` 4671, `read_fonts` 2556, `egui` 688, `wgpu_core` 615,
   `alloc` 529, `naga` 433, `glow` 416, `wgpu` 373, `winit` 285. A debug build links when the
   dependencies and wwrt are optimized (`opt-level = 2` for `"*"` and `wwrt` in the profile `dev`,
   since a crate at opt-level 2 shares no instances): 17600 exports, `wwrt.dll` 40.5 MB,
   `wview.exe` 2.2 MB, the plugins 150 and 320 KB, and only the program and the plugins stay
   unoptimized. When the runtime grows past the limit, it splits into two DLLs (wgpu and naga
   apart, say) or takes fewer features of `windows`.
3. **No LTO across the runtime.** rustc runs LTO for executables, cdylibs and static libraries,
   never for a `dylib`, and a `dylib` keeps every public function, since it exports them all:
   `wwrt.dll` with W# is larger than wedit with W# and LTO. With a release profile of
   `lto = "thin"` cargo builds `wwrt.dll` without LTO but every plugin and wview stop:
   `cannot prefer dynamic linking when performing LTO`, `only 'staticlib', 'bin', and 'cdylib'
   outputs are supported with LTO`. The family has a release profile of its own.
4. **Every shared crate through the runtime.** A plugin with a dependency of its own on a crate
   wwrt holds (serde, egui, windows) would bring a second copy, which rustc refuses; it takes
   `wwrt::serde` and so on. A derive of serde names its crate: `#[serde(crate = "wwrt::serde")]`.
   A crate wwrt does not hold (a parser of a format) is the plugin's own.
5. **One build, checked.** `plugin::ABI` is the build of wwrt: its version, the version of the
   interface, the compiler, the target and the profile (`wwrt/build.rs`). The study had the FNV of
   the lock file in it too, which phase 2 left out: rustc names every symbol by a hash of its
   crate, so other versions of the shared crates fail at the load anyway, and a plugin of the SDK
   has a lock file of its own. A plugin carries the build it was made with, and the loader, which
   runs inside `wwrt.dll`, compares it with its own. A plugin built against another `wwrt.dll`
   misses its imports and fails in `LoadLibrary`; the loader sets the error mode of its thread
   (`SetThreadErrorMode`), so Windows shows no dialog and the list of the plugins names the reason.
6. **A plugin is never unloaded.** Its code and the tables of its traits live as long as the views
   it made; `Plugins::load_dir` forgets the `Library`.
7. **COM belongs to the window.** winit takes the apartment of the main thread for drag and drop; a
   plugin that asks COM before the window (the list of the decoders) does it in a thread of its own.
8. **The toolchain gnu brings MinGW.** Its `dylib` and `std-*.dll` link the unwinder of MinGW
   dynamically: `libgcc_s_seh-1.dll` and `libwinpthread-1.dll` ship too. The toolchain msvc of CI
   needs none of them; a release of wview is built with it.
9. **A plugin has the rights of the program.** It is native code in the process, as trusted as the
   folder of the program; there is no sandbox. A later setting may list the SHA-256 of the plugins
   allowed.
10. **A panic of a plugin unwinds into the host**, since both have one std; the host can catch it
    around every call of a plugin and close the tab instead of the program.
11. **A link costs the import libraries of Windows.** A plugin changed rebuilds in about 50 s on
    this PC (measured in a debug build), and nearly none of it is the plugin: `link_binary` takes
    50.3 s, the linker 0.7 s, the codegen of the plugin 0.2 s (`-Z time-passes`). The crates in
    wwrt call 68 DLLs of Windows through `raw-dylib`, rustc writes an import library of each for
    every link that depends on them (`--print link-args` names them), and the toolchain gnu runs
    `dlltool` and `as` for each one. The toolchain msvc should write them through LLVM itself; CI
    measures what a link of a plugin costs there. The links of every program on W# on this PC likely pay the
    same.
12. **A plugin of the SDK loads when the release and the SDK are one tree.** Phase 2 packed the
    crates of the repository into an SDK at another place and built the template there: the
    release refused it with the error 127 of a missing import, though egui had the same hash in
    both builds. Cargo hashes a path dependency relative to the root of the workspace only when it
    is inside it, and W# is outside `wview/`. Built in the tree of `wview/sdk/pack.py`, where every
    crate is inside the root, the release (8 min on this PC) loaded the template built in a copy of
    the tree in another folder, and refused the plugin of the first SDK with its reason.

## 6. The plugins

### 6.1 The interface

Version 3 (`wwrt::plugin::API`; version 2 had no context of a view, no bytes offered and no
Open with, version 1 no MIME types, no menus and no bytes in memory; a plugin of an older
version is refused by its build, `ABI`):

- `Plugin`: `name`, `formats` (each its name, its extensions and its MIME types), `open` of a path
  with the settings of the plugin, `open_bytes` of some bytes in memory under a name and a MIME
  type with its parameters (`text/plain; charset=utf-16le`; a plugin of files alone keeps the
  answer `plugin::NO_BYTES`), `menu` and `act` (items in a submenu of the plugin in the menu of the
  window, answered with a `Host` for their requests), `translations` (the code and a file of each
  language, added to the window at the load by `ww::i18n::add`), `has_settings` and `settings_ui`
  (its card in Settings, Plugins; the host keeps the JSON object of each plugin in its settings by
  the name of the DLL; `plugin::setting` and `set_setting` read and write a field), `open_as`
  (the file as a MIME type of its formats whatever its name, for Open with: any file as its
  bytes, a text of any name as a text; `open` by default, and `open` again for an empty type)
  and `opens_bytes` (whether `open_bytes` opens anything, so that Open with offers its formats
  for bytes).
- `View`: `ui` with a `Host` (Wrap lines, the width of pages, the settings of the plugin, the
  plugins of the window with the settings of each, the depth of the view, and the requests of the
  view: `Browse` a step through the folder, `Status` a line of the status bar, `Open` a file in a
  tab, `OpenBytes` some bytes in a tab of their own by the plugin of their type, `OpenWith` some
  bytes by a plugin chosen as a type of its formats), `tools` (the bar of the tab; `pressed`
  names the ones that are a switch on, drawn pressed), `menu` (items in the menu of the window
  while its tab is in front) and `context` (the commands of its selection or its cursor, in its
  context menu and in a submenu of the window), all answered by `act` with the host, `offer`
  and `offered` (the bytes the view offers to the other plugins: their name, their MIME type
  when it knows one, and the bytes), `search`, `browses` (Previous and Next in the bar),
  `status`.
- **The menus of a view** (`wwrt::menus`): `context` draws the context menu of a view on the
  response of a part of its area (`input::area` gives one; `ww::tree::show_with_menu` calls a
  closure for a row of a tree, after a click of the secondary button selected it) and answers
  what is picked at once: an item of `View::context` by `act`, a choice of Open with by
  `open_offered`, whose `Request::OpenWith` the host opens in a tab. `items` draws the same
  items for the submenu of the window, which answers the `Picked` itself. Open with lists
  `Choice`s, a plugin and a format of it: `Plugins::choices_for_bytes` the formats of every
  plugin that opens bytes, the ones of the type of the bytes and of the types it falls back to
  first, marked found and drawn in the colour of the accent, then under Try as every other; a
  plugin of texts so opens the bytes of any part of a file as a text.
  `Plugins::choices_for_file` does the same for a file by its extension and its first bytes,
  opened by `Plugin::open_as`.
- **Views inside a view** (`wwrt::inner`): `Host::opens` says whether a plugin of the window opens
  bytes of a MIME type, and `Host::open_inside` gives an `Inner`, the view of the first plugin of
  the type that opens them, which the view keeps and draws in a part of its area (`Inner::ui`): a
  line on the colour of the cards with the tools of the inner view as buttons and the name, the
  type, the size and the plugin of the bytes, then the inner view under an id of its own (a serial
  number in `ui.push_id`), so two views of one kind keep apart. The inner view gets the keys and
  the mouse of its part by the rules of section 6.8, the search of the tab through the view
  outside (`Inner::search`), and its requests go to the host of the view outside, but a step
  through a folder. A view inside a view inside a view stops at a depth of 4 (`inner::DEPTH`), so a
  file that holds itself cannot loop. A plugin draws with the ids of the place of its view
  (`ui.make_persistent_id`; `ww::widgets::side_panel` takes one), never with global ones.
- **MIME types** (`wwrt::mime`): `sniff` names the type of the first 512 bytes (PNG, JPEG, GIF,
  BMP, ICO, TIFF, WebP, a portable executable, ZIP, PDF, and texts of UTF-8 or UTF-16 with or
  without a mark as XML, HTML, JSON (an object, or an array of a value of JSON; a `[section]`
  is a text) or plain text, UTF-16 without a mark in its `charset`); `canonical`
  makes the spellings of a type one (`image/ico`, `image/vnd.microsoft.icon` and `image/x-icon`,
  `text/xml` and `application/xml`); `fallbacks` gives the types a textual one falls back to when
  no plugin names it (a type of `+xml` to `application/xml`, then every text, JSON and script to
  `text/plain`). `Plugins::for_mime` lists the plugins of a type in that order and
  `Plugins::open_bytes` tries them in turn.
- **A file by what it holds** (`Plugins::all_for_file`, the first of them `for_file`): the
  plugins of its extension, then the ones of the MIME type of its first bytes, so that a file of
  another extension or of none opens (a text, an image, a program), then the ones of any bytes
  (`mime::OCTET`, `application/octet-stream`), which every file no other plugin opens falls
  back to; a host tries them in turn and keeps the reason of the first. The window opens every
  file so, the command line `--probe` too.
- **Where a plugin comes from** (`wwrt::trust`): the SHA-256 of every DLL of the folder and its
  Authenticode signature embedded in the file, judged by `WinVerifyTrust` without the network (no
  revocation; a signature of a catalog of Windows is not looked at), with the name of its signer.
  `Plugins::load_trusted` loads the DLLs a `Trust` admits: every one, or with `only_trusted` the
  ones whose SHA-256 it lists and, with `signed`, the ones whose signature Windows trusts; the
  others are not loaded, with their reason and their origin. wview keeps its `Trust` in its
  settings, reads them before any plugin loads, and shows the SHA-256 and the signature on the
  page of the plugins, in Settings, Plugins (with a box Trusted on every card, a DLL not loaded
  too) and in `--plugins`.
- `wwrt::blocks::BlockFile::from_bytes`: bytes in memory read like a file.
- `wwrt::input`: `area` takes the rectangle of a view for its keys and its mouse by the rules of
  section 6.8 and gives an `Input`; `cursor` sets the cursor.
- `wwrt::testing`: the texts of the sources of a plugin and the ones its translations lack.

The window keeps a tab of bytes without a path under their name, lists the MIME types of every
format on the page of the plugins, draws the switches of the bar pressed, puts Open with in the
bar of a view that offers bytes, and holds in its menu the items of the view in front after
Close tab, the context of the view in a submenu Context menu, Open the file with (a new tab of
the file by the plugin and the format chosen) and a submenu of each plugin that has items
before Plugins. The template `wv-hello` has every part once: a MIME type, bytes in memory, an
item of the plugin (Say hello, a line of the status bar), two of its view (Bigger, Smaller), a
switch of the bar (Frame), the context menu of its view and its text offered as
`text/plain`.

### 6.7 Plugins of others: the hybrid

The decision of the customer of 04.11.2026: two kinds of plugins, one of them the main.

1. **Native plugins with the SDK of a release, the main way.** Every example is one: text and
   Markdown, images, binary structures. Every release of wview publishes what its build was:
   `wview-sdk-<version>.zip` (`wview/sdk/pack.py` of supakov/wview, its README
   `wview/sdk/README.md`) is the tree the release itself is built in: W# and wwrt in `framework/` (out of the
   checkout of supakov/ww, with the fields of its workspace written out), wview in `crates/`, the
   plugins and the template `wv-hello` in `plugins/`, `Cargo.lock`, the flags (`.cargo/config.toml`)
   and the compiler (`rust-toolchain.toml` with the version of the release). rustc names the symbols
   of `wwrt.dll` by the versions, the features and the flags of its crates and by their sources, and
   cargo hashes a path dependency relative to the root of the workspace only when it is inside it,
   else by its absolute path. In the repository W# is outside `wview/`, so a plugin built from a
   copy of the repository layout in another folder missed the imports of `wwrt.dll` (the hash of
   `ww` was `Csl8FSR6xLGnH` in one build and `CsluDd2lLouDL` in the other, the ones of egui alike);
   in the tree every crate is inside its root, and a plugin built in a copy of it anywhere loads. A
   plugin made with it draws with egui as the plugins of the repository do and is built again for
   every release, since the loader refuses it otherwise (finding 5); CI builds the template of the
   SDK in another folder and loads it into wview.
2. **A stable interface for data, for other languages.** A C ABI of versioned structures (section
   6.9): a plugin in C, C++, Zig, Go, Delphi or Rust of any version gives the host a picture and the
   controls around it, and the host draws them and hands the plugin the keys and the mouse. It
   outlives the releases. One example shows it: the Mandelbrot set (section 6.10).

Besides them a `.wwformat` of wv-binary (section 6.4) adds a binary format without code: data,
read with the limits of every untrusted input (rule 8). A plugin nobody vouches for could later run
as WebAssembly in a sandbox, behind the same stable interface and hosted by a native plugin
(`wv-wasm`), so that wview carries a runtime of WebAssembly only where that plugin is installed.

A plugin of others, of either kind, has the rights of the program (finding 9), and one in C can
crash it. The page of the plugins names where each one comes from, and a setting may allow only
the plugins whose SHA-256 or signature (Authenticode, checked with `WinVerifyTrust`) it lists.

### 6.8 The keys and the mouse of a view

Both kinds of plugins follow the same rules:

- **Focus.** A click on a view gives it the focus of the keyboard; Tab and Escape give it back to
  the window. A view in focus gets the keys it declares first (the arrows, the page keys, Home and
  End, letters, everything); the others go to the window, so Ctrl+O, Ctrl+W, Ctrl+Tab and the menu
  keep working.
- **The mouse.** The position in the pixels of the view, the buttons, the wheel (with Ctrl, Shift
  and Alt), the movement while a button is down, also outside the view until the button is up, the
  double click, and the pointer leaving the view.
- **The cursor.** A view names its cursor (the default, a cross, a hand, a closed hand while
  dragging, a loupe), the window sets it.
- **A frame on demand.** A view that is still at work (a picture drawn in steps) asks for another
  frame; an idle one costs nothing.

### 6.9 The stable interface

`wview.h`, a header of C and its version, and a crate of Rust over it for authors who write Rust
of another toolchain. A plugin exports one function, `wv_plugin_get`, which gives a table of
functions and data; a native plugin exports `ww_plugin` instead, so the loader tells them apart. In
short:

```c
#define WV_API_VERSION 1

typedef struct wv_format {
    const char *name;                 /* UTF-8, for the filters of Open */
    const char *const *extensions;    /* without the dot, lower case */
    uint32_t extension_count;
} wv_format;

typedef struct wv_key {
    const char *name;                 /* the W3C name of the key: "ArrowLeft", "PageUp", "+" */
    uint32_t pressed;                 /* 1 pressed, 0 released */
    uint32_t modifiers;               /* WV_CTRL | WV_SHIFT | WV_ALT */
} wv_key;

typedef struct wv_input {
    uint32_t size;                    /* sizeof(wv_input): newer hosts add fields at the end */
    uint32_t width, height;           /* the area of the view, pixels */
    float scale;                      /* pixels a point of the window */
    uint32_t focused, hovered;
    float mouse_x, mouse_y;           /* pixels of the view */
    uint32_t buttons;                 /* WV_LEFT | WV_RIGHT | WV_MIDDLE, held now */
    uint32_t clicks, double_clicks;   /* buttons clicked since the last frame */
    float drag_x, drag_y;             /* movement while a button was down */
    float wheel_x, wheel_y;           /* lines since the last frame */
    uint32_t modifiers;
    const wv_key *keys;               /* the keys of the view since the last frame */
    uint32_t key_count;
    const char *text;                 /* text typed since the last frame, UTF-8 */
} wv_input;

typedef struct wv_control {
    uint32_t id;
    uint32_t kind;                    /* WV_BUTTON, WV_TOGGLE, WV_SLIDER, WV_CHOICE, WV_LABEL */
    const char *label;
    double value, min, max, step;     /* a slider; a toggle 0 or 1; a choice its index */
    const char *const *choices;
    uint32_t choice_count;
} wv_control;

typedef struct wv_output {
    uint32_t size;
    uint32_t width, height, stride;   /* the picture, RGBA with 8 bits a channel */
    const uint8_t *pixels;            /* the plugin's memory, valid until its next call */
    uint32_t changed;                 /* the picture differs from the last one */
    const wv_control *controls;       /* the controls of the bar of the tab */
    uint32_t control_count;
    uint32_t keys;                    /* WV_KEYS_ARROWS | WV_KEYS_PAGES | WV_KEYS_LETTERS | ... */
    uint32_t cursor;                  /* WV_CURSOR_DEFAULT, _CROSS, _HAND, _GRAB, _ZOOM */
    uint32_t again;                   /* draw another frame: the plugin is still at work */
    const char *status;               /* a line of the status bar, UTF-8 */
} wv_output;

typedef struct wv_plugin {
    uint32_t api_version;             /* WV_API_VERSION the plugin was written for */
    const char *name;
    const wv_format *formats;
    uint32_t format_count;
    void *(*open)(const char *path, char *error, uint32_t error_size);
    void (*frame)(void *view, const wv_input *input, wv_output *output);
    void (*control)(void *view, uint32_t id, double value);
    void (*close)(void *view);
} wv_plugin;

__declspec(dllexport) const wv_plugin *wv_plugin_get(void);
```

The rules: every call comes from the thread of the window and every text is UTF-8; what the host
gives lives for the call, what the plugin gives lives until its next call on that view; a plugin
may run threads of its own; a host refuses an `api_version` it does not know and reads only the
fields a `size` covers, so both sides grow at the end. In the host the loader of wwrt wraps such a
DLL into an ordinary `Plugin` and `View` (`wwrt::stable`): the picture becomes a texture, the
controls egui widgets in the bar, the input of egui becomes `wv_input`.

### 6.10 The example of the stable interface: the Mandelbrot set

One plugin in C, `wv-mandelbrot`, built by CI with the compiler of Visual Studio. It opens a
`.mandel` file, a few lines of text with the centre, the zoom, the iterations and the palette of a
view, and saves the view back. Its controls in the bar: Iterations (a slider), Palette (a choice:
classic, fire, ice, grey), Smooth colours (a toggle), Zoom in, Zoom out, Reset. Its mouse: the wheel
zooms at the pointer, a drag moves the picture with the closed hand, Shift and a drag draws a
rectangle that a release zooms into, a double click centres and zooms in, the right button zooms
out. Its keys: the arrows move, + and - zoom, Page Up and Page Down double or halve the iterations,
P takes the next palette, Home resets, Escape drops a rectangle. It draws in steps: a coarse picture
at once, then finer rows while it asks for frames, so the window never waits; the status bar names
the centre, the zoom, the iterations and the time of the last picture.

As it is built:

- **`stable/wview.h`**, version 1, as sketched in 6.9 with its constants: the modifiers,
  the buttons, the groups of keys (`WV_KEYS_ARROWS`, `_PAGES`, `_LETTERS`, `_DIGITS`, `_SIGNS`,
  `_SPACE`, `_ESCAPE`, `_ENTER`), the cursors and the kinds of controls. A view takes Escape only
  when it names the group, so Escape gives the focus back otherwise (the Mandelbrot set takes it
  while a rectangle is drawn).
- **`wwrt::stable`**: the structures of the header as `#[repr(C)]` (their sizes checked by a
  test: 24, 16, 88, 64, 64 and 64 bytes), `wrap` (a table of version 1 with its functions into a
  `Plugin`), and its view: the controls copied out of the plugin's memory and drawn as egui widgets
  in a strip over the view (a slider, a toggle, a choice, a button, a label), a change given back
  to `control` before the next frame; the input of the view (`wwrt::input::area` with the keys of
  the groups the plugin named in its last frame) as `wv_input` in pixels; the picture into a
  texture when it changed; the cursor, `again` (another frame at once) and the status. The loader
  takes a DLL that exports `wv_plugin_get` as such a plugin, else a native one; the version of an
  unknown interface is its reason.
- **`stable/rust`**, the crate `wview-stable` for Rust of any toolchain (edition 2021, Rust
  1.77, no dependency, its own fields so that it copies out whole; a member of the workspace and of
  the tree of the SDK): `ffi` (the structures and the constants of `wview.h`), the trait `View`
  (`frame` from an `Input` to a `Frame`, `control`), `Picture`, `Control` with its `Kind`,
  `Cursor`, `keys`, and `plugin!`, which writes `wv_plugin_get` over a `Table` made once (its texts
  and arrays in buffers of its own). The functions of the table (`glue`) keep the texts and the
  controls of the last frame in the box of the view, copy the input and the output by the sizes the
  host gives, and catch a panic: the view stops with the reason in its status and is called no
  more. Its tests call the table as a host does (the layout, a frame, a control, the sizes, a
  panic), the example of its documentation is a doctest, and a test of wwrt wraps a plugin of the
  crate with `wrap` and draws it, so the two sides of the header are checked against each other.
- **`examples/wv-mandelbrot/mandelbrot.c`**: the plugin of 6.10 in C11, the CRT linked in
  (`/MT`), drawn in passes of blocks of 8, 4, 2 and 1 pixels within 12 ms a frame, the cardioid
  and the bulb of period 2 skipped, smooth colours by the normalized count, four palettes; Save
  writes the `.mandel` back. `seahorse.mandel` is its example, in the archive as
  `examples\seahorse.mandel`.
- **CI** builds it with the `cl` of the runner (vswhere, vcvars64), prints its imports, draws it in
  the screenshot `v13-` (`WVIEW_STABLE`), puts it in the archive and checks that the release opens
  the example and draws it without a window (`wview --probe`). `build-all.cmd` builds it where
  vswhere finds Visual Studio and leaves it out with a word elsewhere. The SDK carries `stable/`.

