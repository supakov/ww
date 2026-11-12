//! The native plugins of a program (docs/plugins.md): a DLL in the folder `plugins` next to the
//! executable that opens files of some formats and shows each in a tab. A format names its
//! extensions and its MIME types; a plugin and its views add items to the menu of the window; a
//! view can show some of its bytes with the plugin of their type inside itself
//! (`Host::open_inside`, `crate::inner`).
//!
//! A view offers some of its bytes to the other plugins (`View::offer`) and has a context of
//! commands (`View::context`), both in the menus `crate::menus` draws: the context menu of the
//! view, its copy in the menu of the window, and Open with, which opens the bytes or a file
//! with the plugin of any type (`Plugin::open_as`). Bytes travel as a `BlockFile`: a part of a
//! file read through the cache of the program, or bytes in memory.
//!
//! A plugin finds the types of its formats in the first bytes of a file (`Plugin::sniff`), so a
//! file of another name opens by what it holds, and it may embed files of its own
//! (`Plugin::files`: its guide, its translations, samples), which the window opens in tabs.
//!
//! A plugin is a `cdylib` crate that depends on wwrt alone, implements `Plugin` and `View` and
//! names its plugin with `wwrt::plugin!`. The host loads every DLL of the folder
//! (`Plugins::load_dir`), checks that it was built with this runtime (`ABI`), adds its
//! translations and keeps it for the whole run: a plugin is never unloaded, since its code and the
//! tables of its traits live as long as the views it made.

use std::collections::BTreeMap;
use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use ww::eframe::egui;
use ww::embedded::Embedded;
use ww::{PageWidth, Search};

use crate::blocks::BlockFile;
use crate::mime;
use crate::trust::{self, Origin, Trust};

/// The version of this interface: the traits and the types of this module (`ABI` names it too).
/// It stays 1 until the first release of W#; a release that changes the interface takes the next
/// number.
pub const API: u32 = 1;

/// The build of this runtime: its version, the interface, the compiler, the target and the
/// profile (`build.rs`), nul-terminated. A plugin carries the one it was built against (`plugin!`),
/// and `Plugins::load_dir` takes only the plugins of its own.
pub const ABI: &str = concat!(
    "wwrt ",
    env!("CARGO_PKG_VERSION"),
    "; interface 1; ",
    env!("WWRT_BUILD"),
    "\0"
);

/// A kind of file a plugin opens.
pub struct Format {
    /// The name of the format, for the filters of the dialog Open (the host translates it).
    pub name: &'static str,
    /// Its extensions without the dot, in lower case.
    pub extensions: &'static [&'static str],
    /// Its MIME types (`text/plain`, `image/png`), in any spelling `mime::canonical` knows: the
    /// host finds the plugin of some bytes by them (`Host::open_inside`).
    pub mimes: &'static [&'static str],
}

/// A command a view or a plugin offers: in the bar of the tab (`View::tools`) or in the menu of
/// the window (`View::menu`, `Plugin::menu`), answered by `View::act` or `Plugin::act`; the host
/// translates its label.
pub struct Tool {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: &'static str,
}

/// Where a step through the files of a folder goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    First,
    Previous,
    Next,
    Last,
}

/// What a view asks of the window, done after the frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// The file a step leads to in the folder of the view, among the files its plugin opens, in
    /// the place of the one shown.
    Browse(Step),
    /// A line of the status bar of the window.
    Status(String),
    /// A file in a tab of its own, by the plugin of its format.
    Open(PathBuf),
    /// Bytes in a tab of their own under a name, by the plugin of their MIME type.
    OpenBytes {
        name: String,
        mime: String,
        bytes: BlockFile,
    },
    /// Bytes in a tab of their own under a name, by a plugin chosen (its index among the plugins
    /// of the window) as a MIME type of its formats: a choice of Open with (`crate::menus`).
    OpenWith {
        name: String,
        plugin: usize,
        mime: String,
        bytes: BlockFile,
    },
    /// A file of a set of embedded files (the ones of a plugin, `Plugin::files`) by its path in
    /// the set, with the heading after `#`: a page in a tab of the window, any other file by the
    /// plugin of its type.
    OpenEmbedded(Embedded, String),
}

/// The bytes a view offers to the other plugins (`View::offer`): their name, for the tab they
/// open in, and their MIME type when the view knows it or found it in them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub name: String,
    pub mime: Option<String>,
}

/// What the host lends a view to draw with: the settings of the window that concern a view, the
/// settings of its plugin, the requests the view makes, and the plugins of the window for the
/// views inside a view.
pub struct Host<'a> {
    pub wrap_lines: bool,
    pub page_width: PageWidth,
    /// The settings of the plugin (`Plugin::settings_ui`), `Null` before the first change.
    pub settings: &'a Value,
    pub requests: Vec<Request>,
    /// The plugins of the window, for `open_inside`; `None` where a view stands alone (a test).
    pub plugins: Option<&'a Plugins>,
    /// The settings of every plugin by its id (`Loaded::id`).
    pub every_settings: Option<&'a BTreeMap<String, Value>>,
    /// How deep the view is inside other views: 0 for the view of a tab.
    pub depth: u32,
    /// The id of the plugin of the view, the caller of the tools it calls.
    pub plugin_id: &'a str,
}

impl<'a> Host<'a> {
    /// The host of the view of a tab, without the plugins of the window.
    pub fn new(wrap_lines: bool, page_width: PageWidth, settings: &'a Value) -> Self {
        Host {
            wrap_lines,
            page_width,
            settings,
            requests: Vec::new(),
            plugins: None,
            every_settings: None,
            depth: 0,
            plugin_id: "",
        }
    }

    /// Calls a tool of the program by its name (`crate::tools::call`), as the plugin of the
    /// view: the switches, the mode and the options of the server apply.
    pub fn call_tool(&self, name: &str, args: &Value) -> wwmcp::Outcome {
        crate::tools::call(name, args, self.plugin_id)
    }

    /// The same host with the plugins of the window and the settings of each.
    pub fn with_plugins(
        mut self,
        plugins: &'a Plugins,
        settings: &'a BTreeMap<String, Value>,
    ) -> Self {
        self.plugins = Some(plugins);
        self.every_settings = Some(settings);
        self
    }

    /// The settings of a plugin by its id, `Null` before the first change.
    pub fn settings_of(&self, id: &str) -> Value {
        self.every_settings
            .and_then(|s| s.get(id))
            .cloned()
            .unwrap_or(Value::Null)
    }
}

/// A plugin: the formats it opens and the view of a file of one of them.
pub trait Plugin: Send + Sync {
    /// The name of the plugin, for the list of the plugins and the messages (translated).
    fn name(&self) -> &'static str;
    fn formats(&self) -> &'static [Format];
    /// A view of the file with the settings of the plugin, or the text of why it cannot be opened.
    fn open(&self, path: &Path, settings: &Value) -> Result<Box<dyn View>, String>;
    /// A view of the file as a MIME type of the formats of the plugin, whatever its name (Open
    /// with: any file as its bytes, a text of any name as a text); empty for a format without a
    /// type. A plugin of one way to show a file keeps this answer, `open`.
    fn open_as(&self, path: &Path, _mime: &str, settings: &Value) -> Result<Box<dyn View>, String> {
        self.open(path, settings)
    }
    /// Whether the plugin opens bytes in memory (`open_bytes`): Open with offers its formats for
    /// the bytes of another view.
    fn opens_bytes(&self) -> bool {
        false
    }
    /// A view of some bytes under a name (a part of another file shown inside its view,
    /// `Host::open_inside`, or offered to Open with), of a MIME type of the formats of the plugin
    /// with its parameters (`text/plain; charset=utf-16le`); the view reads what it shows
    /// through the `BlockFile`, or all of it (`BlockFile::to_vec`). A plugin of files alone keeps
    /// this answer.
    fn open_bytes(
        &self,
        _name: &str,
        _mime: &str,
        _bytes: BlockFile,
        _settings: &Value,
    ) -> Result<Box<dyn View>, String> {
        Err(NO_BYTES.to_owned())
    }
    /// The MIME type of a format of the plugin the first bytes of a file show (`mime::SNIFFED`
    /// of them at most; `mime::text` gives them as a text), or none: the host asks every plugin
    /// in turn before it takes a text as `text/plain` (`Plugins::sniff`).
    fn sniff(&self, _head: &[u8]) -> Option<&'static str> {
        None
    }
    /// The files the plugin embeds (`ww::embed!`): its pages, which the page of the plugins
    /// links, samples, and its translations as `lang/<code>/ui.json`, added to the window at the
    /// load as the ones of `translations` are.
    fn files(&self) -> Embedded {
        Embedded::EMPTY
    }
    /// Items of the plugin in the menu of the window, in a submenu of its name.
    fn menu(&self) -> Vec<Tool> {
        Vec::new()
    }
    /// Answers an item of `menu`, with a host of the settings of the plugin for its requests.
    fn act(&self, _id: &str, _host: &mut Host) {}
    /// The texts of the plugin in other languages: the code and a file in the form of `ui.json`
    /// for each, added to the translations of the window at the load.
    fn translations(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
    /// Whether the plugin has settings for `settings_ui`.
    fn has_settings(&self) -> bool {
        false
    }
    /// The tools of the plugin for the server of the program (`wwmcp`, the tab MCP) and for a
    /// call by name: a plugin may have tools and no formats, a source of tools without a view.
    fn tools(&self) -> Vec<wwmcp::ToolDef> {
        Vec::new()
    }
    /// The options its tools need or read (its access settings), kept in the branch of the
    /// plugin of the configuration of the server and shown in its card of the tab MCP.
    fn tool_options(&self) -> Vec<wwmcp::Choice> {
        Vec::new()
    }
    /// Runs one of its tools after the server checked the switches, the mode and the options.
    /// Any thread calls it, several at once, and a tool may call another tool of the program by
    /// its name (`crate::tools::call`).
    fn call_tool(&self, name: &str, _args: &Value, _call: &wwmcp::Call<'_>) -> wwmcp::Outcome {
        Err(wwmcp::fail(
            wwmcp::Kind::NotFound,
            format!("no tool {name}"),
        ))
    }
    /// Draws the settings of the plugin in its card of Settings, Plugins; gives whether they
    /// changed, and the host saves them with the settings of the window.
    fn settings_ui(&self, _ui: &mut egui::Ui, _settings: &mut Value) -> bool {
        false
    }
}

/// A file open in a tab.
pub trait View {
    /// Draws the view in the panel of its tab; the keys and the mouse come through
    /// `wwrt::input::area`.
    fn ui(&mut self, ui: &mut egui::Ui, host: &mut Host);
    /// The commands of the bar of the tab.
    fn tools(&self) -> Vec<Tool> {
        Vec::new()
    }
    /// Whether a command of `tools` is a switch that is on: the bar draws it pressed.
    fn pressed(&self, _id: &str) -> bool {
        false
    }
    /// The commands of the context of the view (its selection, the node or the place of its
    /// cursor), answered by `act`: in its context menu (`crate::menus::context`) and in a
    /// submenu of the menu of the window.
    fn context(&self) -> Vec<Tool> {
        Vec::new()
    }
    /// The bytes the view offers to the other plugins (Open with), when it has some: what is
    /// selected, or the bytes from its cursor. `offered` gives them.
    fn offer(&self) -> Option<Offer> {
        None
    }
    /// The bytes of `offer` (a part of the file of the view, `BlockFile::slice`, or bytes in
    /// memory), or why they cannot be had.
    fn offered(&mut self) -> Result<BlockFile, String> {
        Err(NO_OFFER.to_owned())
    }
    /// Items of the view in the menu of the window while its tab is in front.
    fn menu(&self) -> Vec<Tool> {
        Vec::new()
    }
    /// Answers a command of `tools` or an item of `menu`, with the host for its requests.
    fn act(&mut self, _id: &str, _host: &mut Host) {}
    /// The search of the tab, when the view has one (the field at the end of the command bar).
    fn search(&mut self) -> Option<&mut Search> {
        None
    }
    /// Whether the view steps through the files of its folder: the bar then has Previous and Next.
    fn browses(&self) -> bool {
        false
    }
    /// A line of the status bar: the size of an image, the lines of a text.
    fn status(&self) -> String {
        String::new()
    }
}

/// Why a plugin of files alone does not open bytes (`Plugin::open_bytes`).
pub const NO_BYTES: &str = "the plugin opens files alone";
/// Why a view that offers nothing gives no bytes (`View::offered`).
pub const NO_OFFER: &str = "the view offers no bytes";

/// A setting of a plugin: the field `name` of its settings as `T`, or `default` when it is not
/// there or of another type.
pub fn setting<T: serde::de::DeserializeOwned>(settings: &Value, name: &str, default: T) -> T {
    settings
        .get(name)
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default)
}

/// Sets the field `name` of the settings of a plugin, making them an object when they are not.
pub fn set_setting(settings: &mut Value, name: &str, value: impl serde::Serialize) {
    if !settings.is_object() {
        *settings = Value::Object(serde_json::Map::new());
    }
    if let (Some(map), Ok(v)) = (settings.as_object_mut(), serde_json::to_value(value)) {
        map.insert(name.to_owned(), v);
    }
}

/// Names the plugin of a crate: `wwrt::plugin!(TextPlugin);` exports the build of wwrt it was made
/// with and the function that makes the plugin. With the feature `library` of the plugin (which it
/// declares, `library = []`) the crate exports neither: the tests of a program take several
/// plugins as libraries into one executable, where the names of the exports would clash.
#[macro_export]
macro_rules! plugin {
    ($plugin:expr) => {
        /// The build of wwrt this plugin was made with (`wwrt::plugin::ABI`).
        #[cfg(not(feature = "library"))]
        #[unsafe(no_mangle)]
        pub extern "C" fn ww_plugin_abi() -> *const u8 {
            $crate::plugin::ABI.as_ptr()
        }

        /// The plugin; the host calls it once, after `ww_plugin_abi` matched its own build.
        #[cfg(not(feature = "library"))]
        #[unsafe(no_mangle)]
        pub fn ww_plugin() -> Box<dyn $crate::plugin::Plugin> {
            Box::new($plugin)
        }
    };
}

/// A plugin loaded from its DLL.
pub struct Loaded {
    /// The name of its DLL without the extension: the key of its settings.
    pub id: String,
    pub file: PathBuf,
    /// Shared with the server of the tools, whose threads call its tools.
    pub plugin: Arc<dyn Plugin>,
    /// Where its DLL comes from; none for a plugin the program made itself.
    pub origin: Option<Origin>,
}

/// A DLL of the folder that did not load: why, and where it comes from when that could be read.
pub struct Failed {
    pub file: PathBuf,
    pub why: String,
    pub origin: Option<Origin>,
}

impl Loaded {
    /// Whether the plugin opens a file of the extension of this path.
    pub fn opens(&self, path: &Path) -> bool {
        let Some(ext) = path.extension().map(|e| e.to_string_lossy().to_lowercase()) else {
            return false;
        };
        self.plugin
            .formats()
            .iter()
            .any(|f| f.extensions.contains(&ext.as_str()))
    }

    /// Whether a format of the plugin names the MIME type, in any of its spellings.
    pub fn opens_mime(&self, mime: &str) -> bool {
        let wanted = mime::canonical(mime);
        self.plugin
            .formats()
            .iter()
            .any(|f| f.mimes.iter().any(|m| mime::canonical(m) == wanted))
    }
}

/// The plugins of a program, and the DLLs of its folder that did not load with the reason.
#[derive(Default)]
pub struct Plugins {
    pub loaded: Vec<Loaded>,
    pub failed: Vec<Failed>,
}

impl Plugins {
    /// Every DLL of the folder, by its name, but the one of wwrt itself (a build of the workspace
    /// writes the runtime and the plugins into one folder).
    pub fn load_dir(dir: &Path) -> Self {
        Self::load_trusted(dir, &Trust::default())
    }

    /// The DLLs of the folder the trust admits (`Trust::admits`), each with where it comes from;
    /// the others are not loaded, with the reason.
    pub fn load_trusted(dir: &Path, trust: &Trust) -> Self {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| {
                        let dll = p.extension().is_some_and(|e| {
                            e.eq_ignore_ascii_case(std::env::consts::DLL_EXTENSION)
                        });
                        let runtime = p
                            .file_stem()
                            .is_some_and(|s| s.eq_ignore_ascii_case("wwrt"));
                        dll && !runtime
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort_by_key(|p| p.to_string_lossy().to_lowercase());
        let mut out = Plugins::default();
        for file in files {
            let origin = match trust::origin_of(&file) {
                Ok(o) => o,
                Err(why) => {
                    out.failed.push(Failed {
                        file,
                        why,
                        origin: None,
                    });
                    continue;
                }
            };
            if let Err(why) = trust.admits(&origin) {
                out.failed.push(Failed {
                    file,
                    why,
                    origin: Some(origin),
                });
                continue;
            }
            match load(&file) {
                Ok(plugin) => {
                    out.add(file, plugin);
                    if let Some(last) = out.loaded.last_mut() {
                        last.origin = Some(origin);
                    }
                }
                Err(why) => out.failed.push(Failed {
                    file,
                    why,
                    origin: Some(origin),
                }),
            }
        }
        out
    }

    /// A plugin made by the program itself or a test, next to the ones of DLLs; its
    /// translations (`Plugin::translations` and the ones of its files) join the window.
    pub fn add(&mut self, file: PathBuf, plugin: Box<dyn Plugin>) {
        ww::i18n::add(plugin.translations());
        let own = plugin.files().translations();
        if !own.is_empty() {
            ww::i18n::add(Box::leak(own.into_boxed_slice()));
        }
        let id = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.loaded.push(Loaded {
            id,
            file,
            plugin: Arc::from(plugin),
            origin: None,
        });
    }

    /// The index of the first plugin that opens the extension of the file.
    pub fn for_path(&self, path: &Path) -> Option<usize> {
        self.loaded.iter().position(|l| l.opens(path))
    }

    /// The plugin of a file: the first of `all_for_file`.
    pub fn for_file(&self, path: &Path) -> Option<usize> {
        self.all_for_file(path).first().copied()
    }

    /// The MIME type of some bytes by their first ones: the first plugin that finds one of its
    /// types in them (`Plugin::sniff`, with the `charset` of a text of UTF-16 without its mark),
    /// else a text (`mime::sniff`), else none.
    pub fn sniff(&self, head: &[u8]) -> Option<String> {
        let head = &head[..head.len().min(mime::SNIFFED)];
        self.loaded
            .iter()
            .find_map(|l| l.plugin.sniff(head))
            .map(|m| mime::with_charset(m, head))
            .or_else(|| mime::sniff(head).map(str::to_owned))
    }

    /// The MIME type of a file by its first bytes (`sniff`); none when it does not read.
    pub fn sniff_file(&self, path: &Path) -> Option<String> {
        let file = std::fs::File::open(path).ok()?;
        let mut first = Vec::new();
        let mut part = std::io::Read::take(file, mime::SNIFFED as u64);
        std::io::Read::read_to_end(&mut part, &mut first).ok()?;
        self.sniff(&first)
    }

    /// The plugins of a file in the order a host tries them: the ones of its extension, then
    /// the ones of the MIME type of its first bytes (`sniff`), so that a file of another
    /// extension or of none opens by what it holds, then the ones of any bytes
    /// (`mime::OCTET`), which a file no other plugin opens falls back to.
    pub fn all_for_file(&self, path: &Path) -> Vec<usize> {
        let mut out: Vec<usize> = (0..self.loaded.len())
            .filter(|i| self.loaded[*i].opens(path))
            .collect();
        if !path.is_file() {
            return out;
        }
        let sniffed = self
            .sniff_file(path)
            .map(|m| self.for_mime(&m))
            .unwrap_or_default();
        for i in sniffed.into_iter().chain(self.for_mime(mime::OCTET)) {
            if !out.contains(&i) {
                out.push(i);
            }
        }
        out
    }

    /// The plugins that name a MIME type, in their order, then for a textual type the ones of
    /// the types it falls back to (`mime::fallbacks`).
    pub fn for_mime(&self, mime: &str) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        let wanted = std::iter::once(mime.to_owned())
            .chain(mime::fallbacks(mime).into_iter().map(str::to_owned));
        for m in wanted {
            for (i, l) in self.loaded.iter().enumerate() {
                if l.opens_mime(&m) && !out.contains(&i) {
                    out.push(i);
                }
            }
        }
        out
    }

    /// A view of some bytes by the first plugin of their MIME type that opens them, with its
    /// index, or why none did; `settings` gives the settings of a plugin by its id.
    pub fn open_bytes(
        &self,
        name: &str,
        mime: &str,
        bytes: BlockFile,
        settings: &dyn Fn(&str) -> Value,
    ) -> Result<(usize, Box<dyn View>), String> {
        let found = self.for_mime(mime);
        if found.is_empty() {
            return Err(format!("no plugin opens {}", mime::base(mime)));
        }
        let mut why = Vec::new();
        for &i in &found {
            let l = &self.loaded[i];
            match l
                .plugin
                .open_bytes(name, mime, bytes.clone(), &settings(&l.id))
            {
                Ok(view) => return Ok((i, view)),
                Err(e) => why.push(format!("{}: {e}", l.plugin.name())),
            }
        }
        Err(why.join("; "))
    }
}

/// The build of the DLL of wwrt itself, for About and the list of the plugins.
pub fn runtime() -> &'static str {
    ABI.trim_end_matches('\0')
}

/// A plugin of its DLL; this code runs in wwrt.dll, so `ABI` here is the build of that DLL.
fn load(path: &Path) -> Result<Box<dyn Plugin>, String> {
    let _quiet = Quiet::new();
    let library = open_library(path)?.map_err(|e| {
        let mut text = e.to_string();
        if let Some(source) = std::error::Error::source(&e) {
            text = format!("{text}: {source}");
        }
        if text.contains("os error 127") {
            format!("built against another build of wwrt: its imports are not in this one ({text})")
        } else {
            text
        }
    })?;
    // A plugin of the stable interface (ww.h) exports `ww_plugin_get`, `ww_tools_get` or both,
    // and no `ww_plugin`. Each gets a table of the host that names it, for its calls of tools.
    let id = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    // SAFETY: the two functions are declared by ww.h with these signatures; the host table they
    // take lives for the run, and the tables they give are the plugin's static data, which lives
    // while the DLL does, and the DLL is never unloaded.
    let (stable, tools) = unsafe {
        type Get =
            unsafe extern "C" fn(*const crate::stable::WwHost) -> *const crate::stable::WwPlugin;
        type GetTools =
            unsafe extern "C" fn(*const crate::stable::WwHost) -> *const crate::stable::WwTools;
        let views = library.get::<Get>(b"ww_plugin_get\0").ok();
        let tools = library.get::<GetTools>(b"ww_tools_get\0").ok();
        let host = if views.is_some() || tools.is_some() {
            crate::stable::host_for(&id)
        } else {
            std::ptr::null()
        };
        (views.map(|get| get(host)), tools.map(|get| get(host)))
    };
    if stable.is_some() || tools.is_some() {
        // SAFETY: see above; each table is checked by `wrap` or `wrap_tools` before it is used.
        let tools = match tools {
            Some(table) if table.is_null() => return Err("ww_tools_get gave no table".to_owned()),
            Some(table) => Some(unsafe { crate::stable::wrap_tools(&*table) }?),
            None => None,
        };
        let plugin = match (stable, tools) {
            (Some(table), _) if table.is_null() => {
                return Err("ww_plugin_get gave no table".to_owned());
            }
            (Some(table), tools) => {
                // SAFETY: see above.
                let plugin = unsafe { crate::stable::wrap(&*table) }?;
                match tools {
                    Some(t) => plugin.with_tools(t),
                    None => plugin,
                }
            }
            (None, Some(tools)) => crate::stable::Stable::tools_only(tools),
            (None, None) => unreachable!("one of the tables is there"),
        };
        std::mem::forget(library);
        return Ok(Box::new(plugin));
    }
    // SAFETY: `ww_plugin_abi` is the function `plugin!` exports, with this signature, and it gives
    // a nul-terminated text of the plugin's constant data.
    let built = unsafe {
        let abi = library
            .get::<extern "C" fn() -> *const u8>(b"ww_plugin_abi\0")
            .map_err(|e| format!("not a plugin of wwrt: {e}"))?;
        let text = abi();
        if text.is_null() {
            return Err("not a plugin of wwrt: ww_plugin_abi gave no text".to_owned());
        }
        CStr::from_ptr(text.cast::<c_char>())
            .to_string_lossy()
            .into_owned()
    };
    let own = runtime();
    if built != own {
        return Err(format!(
            "built with another runtime ({built}); this one is {own}"
        ));
    }
    // SAFETY: a plugin of this very build (checked above) exports `ww_plugin` as `plugin!` writes
    // it, so the signature and the layout of `Box<dyn Plugin>` are the ones of this runtime.
    let plugin = unsafe {
        let make = library
            .get::<fn() -> Box<dyn Plugin>>(b"ww_plugin\0")
            .map_err(|e| format!("not a plugin of wwrt: {e}"))?;
        make()
    };
    // Never unloaded: the code and the tables of the plugin's traits live as long as its views.
    std::mem::forget(library);
    Ok(plugin)
}

/// Opens the DLL of a plugin. On Windows the DLLs it imports are searched in its own folder, in
/// the folder of the program and in the system folder alone, never in the current folder or the
/// PATH: the trust of a plugin (`crate::trust`) covers its DLL, so what it brings in comes from
/// the places the user of the program fills. A DLL already in the process (wwrt, std) is found
/// by its name wherever it is. The outer error is why the path has no absolute form, which the
/// search in the folder of the DLL needs.
fn open_library(path: &Path) -> Result<Result<libloading::Library, libloading::Error>, String> {
    #[cfg(windows)]
    {
        use libloading::os::windows::{
            LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            LOAD_LIBRARY_SEARCH_SYSTEM32, Library,
        };
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR
            | LOAD_LIBRARY_SEARCH_APPLICATION_DIR
            | LOAD_LIBRARY_SEARCH_SYSTEM32;
        // SAFETY: loading a DLL runs its initialisation; the plugins come from the folder of the
        // program, which its user fills as he fills the folder of the program itself.
        Ok(unsafe { Library::load_with_flags(&path, flags) }.map(Into::into))
    }
    #[cfg(not(windows))]
    {
        // SAFETY: as above.
        Ok(unsafe { libloading::Library::new(path) })
    }
}

/// No dialog of Windows while a plugin loads: a DLL that misses an import (one built against
/// another wwrt.dll) fails with an error the list of the plugins shows.
struct Quiet(u32);

impl Quiet {
    fn new() -> Self {
        let mut old = 0u32;
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Diagnostics::Debug::{
                SEM_FAILCRITICALERRORS, SEM_NOOPENFILEERRORBOX, SetThreadErrorMode,
            };
            // SAFETY: the call changes the error mode of this thread alone and gives the old one.
            unsafe {
                SetThreadErrorMode(
                    SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX,
                    &raw mut old,
                );
            }
        }
        Quiet(old)
    }
}

impl Drop for Quiet {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Diagnostics::Debug::SetThreadErrorMode;
            // SAFETY: the mode of this thread comes back; the old one is not wanted.
            unsafe {
                SetThreadErrorMode(self.0, std::ptr::null_mut());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_names_the_interface() {
        assert!(
            runtime().contains(&format!("interface {API};")),
            "{}",
            runtime()
        );
        assert!(ABI.ends_with('\0') && !runtime().ends_with('\0'));
    }

    struct Named(&'static [Format]);

    impl Plugin for Named {
        fn name(&self) -> &'static str {
            "test"
        }
        fn formats(&self) -> &'static [Format] {
            self.0
        }
        fn open(&self, _: &Path, _: &Value) -> Result<Box<dyn View>, String> {
            Err(String::new())
        }
        /// The plugin of XML of the tests finds a declaration of XML.
        fn sniff(&self, head: &[u8]) -> Option<&'static str> {
            let text = mime::text(head)?;
            let xml = self.0[0].name == "XML" && text.trim_start().starts_with("<?xml");
            xml.then_some("text/xml")
        }
    }

    static TEXTS: [Format; 1] = [Format {
        name: "Texts",
        extensions: &["txt"],
        mimes: &["text/plain"],
    }];
    static XML: [Format; 1] = [Format {
        name: "XML",
        extensions: &["xml"],
        mimes: &["text/xml"],
    }];
    static ANY: [Format; 1] = [Format {
        name: "Bytes",
        extensions: &["bin"],
        mimes: &[mime::OCTET],
    }];

    /// A plugin of formats that opens bytes too.
    struct Bytes(&'static [Format]);

    impl Plugin for Bytes {
        fn name(&self) -> &'static str {
            "bytes"
        }
        fn formats(&self) -> &'static [Format] {
            self.0
        }
        fn open(&self, _: &Path, _: &Value) -> Result<Box<dyn View>, String> {
            Err(String::new())
        }
        fn opens_bytes(&self) -> bool {
            true
        }
    }

    #[test]
    fn a_plugin_is_found_by_a_mime_type_and_its_fallbacks() {
        let mut plugins = Plugins::default();
        plugins.add(PathBuf::from("text.dll"), Box::new(Named(&TEXTS)));
        plugins.add(PathBuf::from("xml.dll"), Box::new(Named(&XML)));
        assert_eq!(plugins.for_mime("text/plain; charset=utf-16le"), [0]);
        assert_eq!(plugins.for_mime("application/xml"), [1, 0]);
        assert_eq!(plugins.for_mime("image/svg+xml"), [1, 0]);
        assert_eq!(plugins.for_mime("text/html"), [0]);
        assert!(plugins.for_mime("image/png").is_empty());
        let bytes = BlockFile::from_bytes(b"<a/>".to_vec());
        let Err(why) = plugins.open_bytes("a.xml", "text/xml", bytes, &|_| Value::Null) else {
            panic!("neither plugin opens bytes");
        };
        assert_eq!(why, format!("test: {NO_BYTES}; test: {NO_BYTES}"));
    }

    #[test]
    fn a_file_of_no_extension_known_opens_by_its_first_bytes() {
        let mut plugins = Plugins::default();
        plugins.add(PathBuf::from("text.dll"), Box::new(Named(&TEXTS)));
        plugins.add(PathBuf::from("xml.dll"), Box::new(Named(&XML)));
        let dir = std::env::temp_dir().join(format!("wwrt-sniff-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = |name: &str, bytes: &[u8]| {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        assert_eq!(
            plugins.for_file(&file("a.xml", b"plain")),
            Some(1),
            "the extension first"
        );
        assert_eq!(plugins.for_file(&file("README", b"Read me.\n")), Some(0));
        assert_eq!(
            plugins.for_file(&file("data.out", b"<?xml version=\"1.0\"?><a/>")),
            Some(1)
        );
        assert_eq!(plugins.for_file(&file("blob", &[0, 1, 2, 0xFF])), None);
        assert_eq!(plugins.for_file(&dir.join("none")), None, "no file");
        // A plugin of any bytes takes what no other opens, and comes last for the others.
        plugins.add(PathBuf::from("any.dll"), Box::new(Named(&ANY)));
        assert_eq!(plugins.for_file(&file("blob", &[0, 1, 2, 0xFF])), Some(2));
        assert_eq!(plugins.all_for_file(&file("b.xml", b"<a/>")), [1, 0, 2]);
        assert_eq!(plugins.all_for_file(&file("notes", b"plain")), [0, 2]);
        // The type of the first bytes: the plugins first, then a text, with its charset.
        let wide: Vec<u8> = "<?xml version='1.0'?>"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            plugins.sniff(&wide).as_deref(),
            Some("text/xml; charset=utf-16le")
        );
        assert_eq!(plugins.sniff(b"plain").as_deref(), Some("text/plain"));
        assert_eq!(plugins.sniff(&[0, 1, 2, 0xFF]), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_choices_of_open_with_come_by_their_type() {
        let mut plugins = Plugins::default();
        plugins.add(PathBuf::from("text.dll"), Box::new(Bytes(&TEXTS)));
        plugins.add(PathBuf::from("xml.dll"), Box::new(Bytes(&XML)));
        plugins.add(PathBuf::from("files.dll"), Box::new(Named(&TEXTS)));
        plugins.add(PathBuf::from("any.dll"), Box::new(Bytes(&ANY)));
        let choices = plugins.choices_for_bytes(Some("application/xml"));
        let shown: Vec<(usize, bool)> = choices.iter().map(|c| (c.plugin, c.found)).collect();
        assert_eq!(
            shown,
            [(1, true), (0, true), (3, false)],
            "XML, then the text it falls back to, then the others; a plugin of files alone is none"
        );
        assert!(plugins.choices_for_bytes(None).iter().all(|c| !c.found));
        let path = Path::new("a.txt");
        let files = plugins.choices_for_file(path, Some(mime::OCTET));
        let found: Vec<usize> = files.iter().filter(|c| c.found).map(|c| c.plugin).collect();
        assert_eq!(found, [0, 2, 3], "by the name first, then by the bytes");
        assert_eq!(files.len(), 4);
    }

    #[test]
    fn a_dll_not_trusted_stays_out_with_its_origin() {
        let dir = std::env::temp_dir().join(format!("wwrt-trusted-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("other.dll"), b"no plugin").unwrap();
        let every = Plugins::load_dir(&dir);
        assert_eq!(every.failed.len(), 1);
        assert!(
            !every.failed[0].why.contains("not trusted"),
            "{}",
            every.failed[0].why
        );
        let sha = every.failed[0].origin.as_ref().unwrap().sha256.clone();
        let mut trust = Trust {
            only_trusted: true,
            ..Trust::default()
        };
        let none = Plugins::load_trusted(&dir, &trust);
        assert!(
            none.failed[0].why.starts_with("not trusted"),
            "{}",
            none.failed[0].why
        );
        trust.set(&sha, true);
        let listed = Plugins::load_trusted(&dir, &trust);
        assert!(
            !listed.failed[0].why.contains("not trusted"),
            "a listed DLL is tried"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_setting_falls_back_to_its_default() {
        let mut settings = Value::Null;
        assert_eq!(setting(&settings, "max", 64u32), 64);
        set_setting(&mut settings, "max", 128u32);
        assert_eq!(setting(&settings, "max", 64u32), 128);
        set_setting(&mut settings, "max", "many");
        assert_eq!(setting(&settings, "max", 64u32), 64);
    }
}
