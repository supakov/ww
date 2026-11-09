# The plugins of W#

A program on W# shows files, or some bytes, by plugins: DLLs in the folder `plugins` next to its
executable. There are two kinds:

- **Native plugins** in Rust (this page): a `cdylib` on the runtime `wwrt.dll`, which holds W#,
  egui and the interface; a plugin is its own code alone and draws with the egui of the window,
  through the same traits and widgets as the program. Rust has no stable ABI, so a native plugin
  is built in the tree of the release of its host and again for every release (section 9).
- **Plugins of the stable interface** in any language with a C FFI (docs/stable.md): a DLL that
  exports `ww_plugin_get`, built once for every host that knows its version.

The demo, `examples/ww-demo`, is a host of both kinds; `examples/ww-hello` is the template of a
native plugin with every part once. Why the plugins are built this way, and what it costs, is in
docs/design.md.

## 1. A plugin in five parts

```rust
use std::path::Path;

use wwrt::plugin::{Format, Host, Plugin, View};
use wwrt::serde_json::Value;

static FORMATS: [Format; 1] = [Format {
    name: "Hello files",
    extensions: &["hello"],
    mimes: &["application/x-ww-hello"],
}];

pub struct HelloPlugin;

impl Plugin for HelloPlugin {
    fn name(&self) -> &'static str { "Hello" }
    fn formats(&self) -> &'static [Format] { &FORMATS }
    fn open(&self, path: &Path, settings: &Value) -> Result<Box<dyn View>, String> { /* ... */ }
}

struct HelloView { /* ... */ }

impl View for HelloView {
    fn ui(&mut self, ui: &mut wwrt::egui::Ui, host: &mut Host) { /* draw */ }
}

wwrt::plugin!(HelloPlugin);
```

1. **The crate** is a `cdylib` (and an `rlib`, for the tests of a program that take it as a
   library) that depends on `wwrt` alone and declares the feature `library = []`.
2. **The formats**: a name each, the extensions without the dot, the MIME types, which the host
   finds the plugin of some bytes by.
3. **`Plugin`**: opens a file or some bytes into a view, finds its types in the first bytes of a
   file, gives its menu items, its settings, its translations and the files it embeds.
4. **`View`**: draws itself in its tab, names the tools of the bar, the items of the menu and of
   its context menu, offers some of its bytes to the other plugins, has a search and a line of
   the status bar.
5. **`wwrt::plugin!`** exports `ww_plugin_abi` (the build of wwrt the plugin was made with) and
   `ww_plugin` (the plugin); the loader compares the first with its own build, then calls the
   second once and keeps the DLL for the whole run. With the feature `library` the crate exports
   neither, so the tests of a program take several plugins into one executable.

## 2. The interface (`wwrt::plugin`, version 1)

The version stays 1 (`wwrt::plugin::API`) until the first release of W#; the build of wwrt
(`ABI`) names it, so a plugin of another version is refused by its build.

- `Plugin`: `name`, `formats`, `open` (a path with the settings of the plugin), `open_as` (the
  file as a MIME type of its formats whatever its name, for Open the file with; `open` by
  default), `opens_bytes` and `open_bytes` (some bytes under a name and a MIME type with its
  parameters, `text/plain; charset=utf-16le`, as a `BlockFile`; a plugin of files alone keeps
  the answer `plugin::NO_BYTES`), `sniff` (section 4), `files` (section 6), `menu` and `act`
  (items in a submenu of the plugin in the menu of the window, answered with a `Host` for their
  requests), `translations` (the code and a file of each language, added to the window at the
  load), `has_settings` and `settings_ui` (its card in Settings, Plugins; the host keeps the JSON
  object of each plugin by the name of its DLL; `plugin::setting` and `set_setting` read and
  write a field).
- `View`: `ui` with a `Host` (Wrap lines, the width of pages, the settings of the plugin, the
  plugins of the window with the settings of each, the depth of the view, and the requests of the
  view), `tools` (the bar of the tab; `pressed` names the switches that are on, drawn pressed),
  `menu` (items in the menu of the window while its tab is in front), `context` (the commands of
  its selection or its cursor, in its context menu and in a submenu of the window), all answered
  by `act` with the host; `offer` and `offered` (the bytes the view offers to the other plugins:
  their name, their MIME type when it knows one, and a `BlockFile` of them); `search`; `browses`
  (Previous and Next in the bar); `status`.
- `Request`, what a view asks of the window after its frame: `Browse` (a step through the files of
  its folder), `Status` (a line of the status bar), `Open` (a file in a tab), `OpenBytes` (some
  bytes in a tab of their own by the plugin of their type), `OpenWith` (some bytes by a plugin
  chosen as a type of its formats), `OpenEmbedded` (a file of a set of embedded files).
- `Plugins`: the loader (`load_dir`, `load_trusted`, `add` for a plugin the program makes itself)
  and the lookups: `for_mime` (the plugins of a type, then of the types it falls back to),
  `all_for_file` (the plugins of a file in the order a host tries them: its extension, the type
  of its first bytes, any bytes), `sniff` and `sniff_file`, `open_bytes`, `choices_for_bytes`
  and `choices_for_file` (Open with).

## 3. Views inside views, menus, the keys and the mouse

- **A view inside a view** (`wwrt::inner`): `Host::opens` says whether a plugin of the window
  opens bytes of a type, and `Host::open_inside` gives an `Inner`, the view of the first plugin of
  the type that opens them, which the view keeps and draws in a part of its area (`Inner::ui`): a
  line with the tools of the inner view and the name, the type, the size and the plugin of the
  bytes, then the inner view under an id of its own. Its requests go to the host of the view
  outside, but a step through a folder. Views inside views stop at a depth of 4 (`inner::DEPTH`),
  so a file that holds itself cannot loop. A plugin draws with the ids of the place of its view
  (`ui.make_persistent_id`, `ui.id().with(..)`), never with global ones.
- **The menus of a view** (`wwrt::menus`): `context` draws the context menu of a view on the
  response of a part of its area and answers what is picked at once: an item of `View::context`
  by `act`, a choice of Open with by `open_offered`. `items` draws the same items for the submenu
  of the window. `ww::tree::show_with_menu` calls a closure for the context menu of a row of a
  tree, after a click of the secondary button selected it. Open with lists `Choice`s, a plugin
  and a format of it: the formats of the type of the bytes and of the types it falls back to
  first, marked found and drawn in the colour of the accent, then under Try as every other.
- **The keys and the mouse** (`wwrt::input`): `area` takes the rectangle of a view for its keys
  and its mouse and gives an `Input`; `cursor` sets the cursor. A click on a view gives it the
  focus of the keyboard; Tab and Escape give it back to the window. A view in focus gets the keys
  it names (the arrows, the page keys, letters); the others go to the window, so Ctrl+O, Ctrl+W,
  Ctrl+Tab and the menu keep working. The mouse: the position in the view, the buttons, the wheel
  with its modifiers, a drag (also outside the view until the button is up), the double click.
  A view still at work asks for another frame; an idle one costs nothing.

## 4. The type of some bytes

The host knows the least of the type of some bytes by itself (`mime::sniff`): a text of UTF-8 or
UTF-16, with or without a mark, is `text/plain`, with the `charset` of UTF-16 without its mark.
Every other type is the plugins': `Plugin::sniff` gives the type of a format of the plugin its
first bytes show (at most `mime::SNIFFED`, 512), and `Plugins::sniff` asks every plugin in turn,
adds the `charset` of a text of UTF-16 to a textual type, and falls back to the text of the host.
`mime::text` gives the first bytes as a text in any of those encodings, for a plugin that looks
for an object of JSON or a declaration of XML. `mime::canonical` makes the spellings of a type one
(`image/ico`, `image/x-icon`; `text/xml`, `application/xml`), and `mime::fallbacks` gives the types
a textual one falls back to when no plugin names it (a type of `+xml` to `application/xml`, then
every text to `text/plain`). A file of another extension or of none opens by what it holds, and
every file falls back to the plugins of any bytes (`mime::OCTET`, `application/octet-stream`).

## 5. The cache of the blocks

`wwrt::blocks`: a view of a file larger than the memory it may take reads the blocks it shows
through one cache of the program, and the block used longest ago leaves the memory when the
blocks of every open file together reach the budget. `BlockFile::open` reads a file through it,
`from_bytes` and `from_static` read bytes in memory and of the binary the same way; a
`BlockFile` is cheap to clone, and `slice` gives a part that reads the same blocks, so a view
offers a part of its file to another plugin without a copy (`View::offered`, `Request::OpenWith`).
`read_at`, `read`, `head` and `to_vec` (every byte, up to a limit) read it. `blocks::cache()` is
the cache of the program: `set_limits` (the size of a block, the budget), `state` (the bytes and
the blocks in memory, the files open, the blocks read from the disk and found in memory),
`clear`. A host keeps the limits in its settings (`host::PluginSettings::cache`) and draws them
in Settings, Plugins; a plugin of the stable interface reads through the same cache
(docs/stable.md, section 2).

## 6. Embedded files and translations

`Plugin::files` gives the files a plugin embeds (`ww::embed!`, docs/embedded.md): its pages, which
the page of the plugins links and `Request::OpenEmbedded` opens in a tab, samples its pages link,
and the translations of its texts as `lang/<code>/ui.json`, added to the window when the plugin
loads, as the ones of `Plugin::translations` are. The texts of a plugin are English, through `tr`
and `trf` of `wwrt::ww::i18n`; the host translates the name of the plugin, of its formats and the
labels of its tools. `wwrt::testing::plugin_texts` and `missing` make the test that every text has
its translation (the template has one).

## 7. Where a plugin comes from

`wwrt::trust`: the SHA-256 of every DLL of the folder and its Authenticode signature embedded in
the file, judged by `WinVerifyTrust` without the network (no revocation; a signature of a catalog
of Windows is not looked at), with the name of its signer. `Plugins::load_trusted` loads the DLLs
a `Trust` admits: every one, or with `only_trusted` the ones whose SHA-256 it lists and, with
`signed`, the ones whose signature Windows trusts; the others are not loaded, with their reason
and their origin. A plugin has the rights of the program: it is native code in its process.

## 8. A host of plugins (`wwrt::host`)

The parts every program with plugins has, for its `ww::App` to call:

- `HostDoc` (a file or some bytes by a plugin, `FileDoc`, and the page of the plugins) in a
  variant of the documents of the program, `HostAct` in a variant of its acts, `PluginSettings`
  (the settings of every plugin, the trust, the limits of the cache) flattened into its settings,
  all named through the trait `PluginApp`.
- `load` (the plugins of a folder the settings trust, the limits of the cache set first),
  `open_path` (a file by the plugins of its name, of its first bytes and of any bytes in turn),
  `open_dialog`, `open_embedded` (a file of a set by the plugin of its type), `draw`, `commands`
  (the bar of the view in front, its switches pressed, Open with for the bytes it offers, Previous
  and Next), `menu_popup`, `view_menu` (the items of the view, its context menu in a submenu, Open
  the file with), `plugins_menu` (a submenu of each plugin that has items, the page of the
  plugins), `status_left`, `about_lines`, `settings_section` (the trust, the cache, the card of
  each plugin), `apply`.
- For the command line: `attach_console`, `plugins_dir`, `list` (the text of `--plugins`) and
  `probe` (a file opened and drawn in three frames without a window, with the times).

The demo (`examples/ww-demo/src/app.rs`) is such a program, with two plugins it makes itself
(`Plugins::add`): a plugin of bundles of parts whose view shows the part chosen inside itself and
offers it to Open with, and a plugin of texts that names only its MIME types.

## 9. Building a native plugin

A native plugin works with the build of `wwrt.dll` it was made against alone: one compiler, one
`Cargo.lock`, one set of flags, and every crate of the build at the same place in one tree, since
cargo hashes a path dependency relative to the root of its workspace when it is inside it, else by
its absolute path (docs/design.md, finding 12). So:

- In this repository a plugin is a member of the workspace (`examples/ww-hello`): `cargo build
  --release` writes `target/release/<name>.dll` next to `ww-demo.exe` and `wwrt.dll`, and the demo
  loads it from the folder `plugins` or, in a build of the workspace, from its own folder.
- A program that takes W# as a git dependency publishes with each release the tree it was built
  in, W# and wwrt inside its root, with its `Cargo.lock`, its `.cargo/config.toml` and the
  version of its compiler in `rust-toolchain.toml`; a plugin of that program is a member of a copy
  of that tree, anywhere on a disk, and is built again for every release.

The rules of a native plugin:

- Every crate the plugin shares with the host comes through wwrt: `wwrt::egui`, `wwrt::ww`,
  `wwrt::serde` (a derive names it: `#[serde(crate = "wwrt::serde")]`), `wwrt::serde_json`,
  `wwrt::windows`, `wwrt::image`, `wwrt::sha2`, `wwrt::toml_edit`, `wwrt::quick_xml`. A
  dependency of the plugin on one of them, or another feature of one, makes another build of it,
  and the plugin does not load. A crate wwrt does not hold is the plugin's own.
- `cargo update` only for the crates of the plugin (`cargo update -p <crate>`), never for the ones
  of wwrt.
- The family links std dynamically (`-C prefer-dynamic` in `.cargo/config.toml`, with
  `+crt-static` for msvc): the program ships `std-<hash>.dll` of its toolchain next to
  `wwrt.dll` (`tools/imports.py --std` names it), and the toolchain gnu adds
  `libgcc_s_seh-1.dll` and `libwinpthread-1.dll`.
- A plugin is never unloaded; a panic of it reaches the window; it runs with the rights of the
  program.

The template: copy `examples/ww-hello` to a folder of the workspace, give the package its name,
write the plugin, then `cargo test -p <name>` and `cargo build --release -p <name>`.
