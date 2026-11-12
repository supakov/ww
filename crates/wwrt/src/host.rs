//! A program that shows files by plugins (docs/plugins.md, "A host of plugins"): the parts every
//! such program has, for its implementation of `ww::App` to call. A tab of a file or of some
//! bytes drawn by the view of a plugin (`FileDoc`) and the page of the plugins (`HostDoc`); a file
//! opened by the plugins of its name, of its first bytes and of any bytes in turn
//! (`open_path`); the bar of the view in front with its switches pressed and Open with for the
//! bytes it offers (`commands`, `menu_popup`); the items of the view in the menu of the window,
//! its context menu in a submenu, Open the file with and a submenu of each plugin that has items
//! (`view_menu`, `plugins_menu`); a file of a set of embedded files opened by the plugin of its
//! type (`open_embedded`); the section Plugins of the settings with the trust of the plugins, the
//! cache of the blocks and the card of each plugin (`settings_section`); the server of the tools
//! of the program and of its plugins with its tab, the tools of the window (open, close and save a
//! file, list the tabs) and a start without the window (`HostMcp`, `serve_mcp`); the acts of all
//! of them (`HostAct`, `apply`).
//!
//! A program keeps `HostDoc` in a variant of its documents, `HostAct` in a variant of its acts
//! and `PluginSettings` among the fields of its settings, and names them through `PluginApp`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ww::commands::{Item, cmd, menu, toggle};
use ww::embedded::Embedded;
use ww::i18n::{tr, trf};
use ww::shell::{menu_item, sub_menu};
use ww::{App, Document, Search, Shell, ShellAct, icons};

use crate::blocks::{self, BlockFile, Limits};
use crate::egui;
use crate::menus::{self, Choice, Picked};
use crate::plugin::{self, Host, Plugins, Request, Step, View};
use crate::trust::{Origin, Signature, Trust};
use crate::wwmcp::ui::{McpAct, McpPanel};
use crate::wwmcp::window::{ProgramTools, WindowBridge, WindowTools};
use crate::wwmcp::{self, Info, Server, Source};

/// The number of the menu Open with of the bar (`App::menu_popup`), apart from the ones of a
/// program.
pub const OPEN_WITH: usize = usize::MAX - 1;

/// The fields of a program with plugins in its settings: the settings of every plugin by the name
/// of its DLL, the plugins it trusts (read before the plugins load) and the limits of the cache
/// of the blocks. A program puts them into its own fields with `#[serde(flatten)]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginSettings {
    #[serde(default)]
    pub plugins: BTreeMap<String, Value>,
    #[serde(default)]
    pub trust: Trust,
    #[serde(default)]
    pub cache: Limits,
}

impl PluginSettings {
    /// The settings of a plugin by its id, `Null` before the first change.
    pub fn of(&self, id: &str) -> Value {
        self.plugins.get(id).cloned().unwrap_or(Value::Null)
    }
}

/// The plugins of a folder the settings trust, with the limits of the cache set first.
pub fn load(dir: &Path, settings: &PluginSettings) -> Plugins {
    blocks::cache().set_limits(settings.cache);
    Plugins::load_trusted(dir, &settings.trust)
}

/// A file in a tab, or some bytes in a tab of their own (no path): its name, its plugin and the
/// view the plugin made of it.
pub struct FileDoc {
    pub path: Option<PathBuf>,
    pub name: String,
    pub plugin: usize,
    pub view: Box<dyn View>,
}

/// A tab of the host: a file or some bytes by a plugin, the page of the plugins, or the tab of the
/// MCP server with the search of its journal.
pub enum HostDoc {
    File(FileDoc),
    Plugins,
    Mcp(Search),
}

/// The MCP server of a program with plugins: its tab (`McpPanel`), the bridge of the tools of its
/// window and the program's own sources of tools.
pub struct HostMcp {
    pub panel: McpPanel,
    pub bridge: Arc<WindowBridge>,
    pub own: Vec<Arc<dyn Source>>,
}

impl HostMcp {
    /// The server of a program: the tools of its window, the program's tools that act on its
    /// state (`program`, answered by `PluginApp::program_tool`), its other sources (`own`) and
    /// the ones of its plugins, with the configuration `mcp/default.wwmcp` next to the
    /// executable; the server is installed for the calls of the plugins
    /// (`crate::tools::install`).
    pub fn new(
        info: Info,
        plugins: &Plugins,
        program: Vec<wwmcp::ToolDef>,
        mut own: Vec<Arc<dyn Source>>,
    ) -> Self {
        let bridge = WindowBridge::new();
        if !program.is_empty() {
            let tools = ProgramTools {
                id: info.name.clone(),
                title: info.name.clone(),
                tools: program,
                bridge: bridge.clone(),
            };
            own.insert(0, Arc::new(tools));
        }
        let server = Arc::new(server_of(info.clone(), Some(&bridge), &own, plugins));
        crate::tools::install(server.clone());
        let path = wwmcp::config::Config::default_path(&ww::folders::exe_dir());
        let panel = McpPanel::new(path, &info.name, server);
        Self { panel, bridge, own }
    }
}

/// A server of the tools of a program: the tools of its window when it has one, its own sources,
/// then the ones of its plugins.
pub fn server_of(
    info: Info,
    window: Option<&Arc<WindowBridge>>,
    own: &[Arc<dyn Source>],
    plugins: &Plugins,
) -> Server {
    let mut server = Server::new(info, wwmcp::Config::default());
    if let Some(bridge) = window {
        server = server.with(WindowTools::new(bridge.clone()));
    }
    server
        .with_all(own.iter().cloned())
        .with_all(plugins.sources())
}

/// `PROGRAM --mcp ...` (the arguments after `--mcp`): the tools of the program and of its plugins
/// without the window, by the transport of the configuration, until the client stops.
pub fn serve_mcp(
    info: Info,
    plugins: &Plugins,
    own: Vec<Arc<dyn Source>>,
    args: &[String],
) -> Result<(), String> {
    let server = server_of(info, None, &own, plugins);
    let config = wwmcp::cli::config(args, &ww::folders::exe_dir(), server.sources())?;
    server.set_config(config);
    let server = Arc::new(server);
    crate::tools::install(server.clone());
    wwmcp::cli::serve(server)
}

impl Document for HostDoc {
    fn title(&self) -> String {
        match self {
            HostDoc::File(d) => d.name.clone(),
            HostDoc::Plugins => tr("Plugins").to_owned(),
            HostDoc::Mcp(_) => tr("MCP server").to_owned(),
        }
    }

    fn path(&self) -> Option<&Path> {
        match self {
            HostDoc::File(d) => d.path.as_deref(),
            HostDoc::Plugins | HostDoc::Mcp(_) => None,
        }
    }

    fn search(&mut self) -> Option<&mut Search> {
        match self {
            HostDoc::File(d) => d.view.search(),
            HostDoc::Plugins => None,
            HostDoc::Mcp(search) => Some(search),
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            HostDoc::File(d) if d.view.browses() => "\u{1F5BC}",
            HostDoc::File(_) => icons::PAGE,
            HostDoc::Plugins => icons::PACKAGE,
            HostDoc::Mcp(_) => icons::LINK,
        }
    }

    fn search_hint(&self) -> &'static str {
        match self {
            HostDoc::File(_) => "Search the text (Ctrl+F)",
            HostDoc::Plugins => "",
            HostDoc::Mcp(_) => "Search the journal",
        }
    }
}

/// What the host does after a frame.
pub enum HostAct {
    /// A command of the bar or an item of the menu of the view in front (`View::tools`,
    /// `View::menu`).
    Tool(&'static str),
    /// An item of the menu of a plugin (`Plugin::menu`), by the index of the plugin.
    PluginItem(usize, &'static str),
    /// A line a view or a plugin asks for in the status bar.
    Status(String),
    /// Some bytes in a tab of their own, by the plugin of their MIME type.
    OpenBytes {
        name: String,
        mime: String,
        bytes: BlockFile,
    },
    /// Some bytes in a tab of their own, by a plugin chosen as a type of its formats.
    OpenWith {
        name: String,
        plugin: usize,
        mime: String,
        bytes: BlockFile,
    },
    /// The bytes the view in front offers, by a choice of Open with.
    OpenOffered(Choice),
    /// A file in a tab of its own by a choice of Open the file with.
    OpenFileWith(PathBuf, Choice),
    /// Another file of the folder of the one in front, of a format its plugin opens.
    Browse(Step),
    /// The page of the plugins in a tab.
    Plugins,
    /// Every block leaves the cache.
    ClearCache,
    /// The tab of the MCP server in front.
    OpenMcp,
    /// An act of the tab of the MCP server.
    Mcp(McpAct),
}

/// The parts of a program the host needs: its plugins, where its tabs of the host and its acts
/// of the host are, and its settings of the plugins.
pub trait PluginApp: App {
    fn plugins(&self) -> &Arc<Plugins>;
    /// The tab of the host a document of the program is, if any.
    fn host_doc(doc: &Self::Doc) -> Option<&HostDoc>;
    fn host_doc_mut(doc: &mut Self::Doc) -> Option<&mut HostDoc>;
    /// A document of the program of a tab of the host.
    fn from_host_doc(doc: HostDoc) -> Self::Doc;
    /// An act of the program of an act of the host.
    fn host_act(act: HostAct) -> Self::Act;
    fn plugin_settings(extra: &Self::Extra) -> &PluginSettings;
    fn plugin_settings_mut(extra: &mut Self::Extra) -> &mut PluginSettings;
    /// The MCP server of the program, when it has one (`HostMcp::new` at its start).
    fn mcp(&self) -> Option<&HostMcp> {
        None
    }
    fn mcp_mut(&mut self) -> Option<&mut HostMcp> {
        None
    }
    /// Answers a tool of the program's own that acts on its state (`WindowAsk::Program` of its
    /// source), at a frame of the window.
    fn program_tool(
        &mut self,
        _shell: &mut Shell<Self>,
        tool: &str,
        _args: &Value,
    ) -> wwmcp::Outcome {
        Err(wwmcp::fail(
            wwmcp::Kind::NotFound,
            format!("no tool {tool}"),
        ))
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The view of the active tab when it is a file of a plugin.
fn active_file<A: PluginApp>(shell: &Shell<A>) -> Option<&FileDoc> {
    match shell.active_doc().and_then(A::host_doc) {
        Some(HostDoc::File(d)) => Some(d),
        _ => None,
    }
}

fn active_file_mut<A: PluginApp>(shell: &mut Shell<A>) -> Option<&mut FileDoc> {
    match shell.active_doc_mut().and_then(A::host_doc_mut) {
        Some(HostDoc::File(d)) => Some(d),
        _ => None,
    }
}

/// The acts of the requests of a view or a plugin.
pub fn requested<A: PluginApp>(requests: Vec<Request>) -> Vec<A::Act> {
    requests
        .into_iter()
        .map(|r| match r {
            Request::Browse(step) => A::host_act(HostAct::Browse(step)),
            Request::Status(line) => A::host_act(HostAct::Status(line)),
            Request::Open(path) => A::Act::from(ShellAct::OpenPath(path)),
            Request::OpenBytes { name, mime, bytes } => {
                A::host_act(HostAct::OpenBytes { name, mime, bytes })
            }
            Request::OpenWith {
                name,
                plugin,
                mime,
                bytes,
            } => A::host_act(HostAct::OpenWith {
                name,
                plugin,
                mime,
                bytes,
            }),
            Request::OpenEmbedded(files, link) => A::Act::from(ShellAct::OpenEmbedded(files, link)),
        })
        .collect()
}

/// The view of a file by a plugin.
fn open_view(
    plugins: &Plugins,
    settings: &Value,
    plugin: usize,
    path: &Path,
) -> Result<FileDoc, String> {
    let view = plugins.loaded[plugin]
        .plugin
        .open(path, settings)
        .map_err(|e| format!("{}: {e}", file_name(path)))?;
    Ok(FileDoc {
        path: Some(path.to_path_buf()),
        name: file_name(path),
        plugin,
        view,
    })
}

/// A file in a tab by the plugins of its extension, of its first bytes and of any bytes
/// (`Plugins::all_for_file`), the first that opens it; the reason of the first when none does.
pub fn open_path<A: PluginApp>(app: &A, shell: &mut Shell<A>, path: PathBuf) {
    let plugins = app.plugins().clone();
    let found = plugins.all_for_file(&path);
    if found.is_empty() {
        shell.status = trf("No plugin opens {name}", &[("name", &file_name(&path))]);
        return;
    }
    let every = A::plugin_settings(&shell.settings.extra).clone();
    shell.open_file(
        &path,
        |d| A::host_doc(d).and_then(Document::path) == Some(path.as_path()),
        |p| {
            let mut first = None;
            for plugin in &found {
                let settings = every.of(&plugins.loaded[*plugin].id);
                match open_view(&plugins, &settings, *plugin, p) {
                    Ok(doc) => return Ok(doc),
                    Err(why) => {
                        first.get_or_insert(why);
                    }
                }
            }
            Err(first.unwrap_or_default())
        },
        |doc, _| A::from_host_doc(HostDoc::File(doc)),
    );
}

/// The dialog Open with a filter of every format of the plugins, one of each format and one of
/// any file.
#[cfg(not(target_arch = "wasm32"))]
pub fn open_dialog(plugins: &Plugins) -> ww::rfd::FileDialog {
    let mut every: Vec<&str> = Vec::new();
    let mut dialog = ww::rfd::FileDialog::new().set_title(tr("Open"));
    for loaded in &plugins.loaded {
        for format in loaded.plugin.formats() {
            every.extend(format.extensions.iter().copied());
        }
    }
    if !every.is_empty() {
        dialog = dialog.add_filter(tr("Every format of the plugins"), every.as_slice());
    }
    for loaded in &plugins.loaded {
        for format in loaded.plugin.formats() {
            if !format.extensions.is_empty() {
                dialog = dialog.add_filter(tr(format.name), format.extensions);
            }
        }
    }
    dialog.add_filter(tr("All files"), &["*"])
}

/// A file of a set of embedded files: a page in a tab of the window, any other file by the
/// plugins of its name and of its first bytes, read without a copy.
pub fn open_embedded<A: PluginApp>(app: &A, shell: &mut Shell<A>, files: Embedded, link: &str) {
    let path = link.split('#').next().unwrap_or(link);
    if ww::shell::is_page(path) || files.get(path).is_none() {
        if let Err(why) = shell.open_page(files, link) {
            shell.status = why;
        }
        return;
    }
    let Some(file) = files.localized(path, &ww::i18n::current()) else {
        return;
    };
    let plugins = app.plugins().clone();
    let by_name: Vec<usize> = (0..plugins.loaded.len())
        .filter(|i| plugins.loaded[*i].opens(Path::new(file.path)))
        .collect();
    let mime = plugins.sniff(file.bytes);
    let mut found = by_name;
    for i in mime
        .as_deref()
        .map(|m| plugins.for_mime(m))
        .unwrap_or_default()
    {
        if !found.contains(&i) {
            found.push(i);
        }
    }
    let every = A::plugin_settings(&shell.settings.extra).clone();
    let mut first = None;
    for i in found {
        let loaded = &plugins.loaded[i];
        if !loaded.plugin.opens_bytes() {
            continue;
        }
        let as_mime = mime.clone().unwrap_or_else(|| {
            loaded
                .plugin
                .formats()
                .iter()
                .find(|f| {
                    f.extensions
                        .iter()
                        .any(|e| file.path.ends_with(&format!(".{e}")))
                })
                .and_then(|f| f.mimes.first())
                .map_or_else(String::new, |m| (*m).to_owned())
        });
        let bytes = BlockFile::from_static(file.bytes);
        match loaded
            .plugin
            .open_bytes(file.name(), &as_mime, bytes, &every.of(&loaded.id))
        {
            Ok(view) => {
                shell.open_tab(A::from_host_doc(HostDoc::File(FileDoc {
                    path: None,
                    name: file.name().to_owned(),
                    plugin: i,
                    view,
                })));
                return;
            }
            Err(why) => {
                first.get_or_insert(format!("{}: {why}", tr(loaded.plugin.name())));
            }
        }
    }
    shell.status =
        first.unwrap_or_else(|| trf("No plugin opens {name}", &[("name", &file.name())]));
}

/// The file a step leads to from the one in front, among the files of its folder its plugin
/// opens, by their names; it takes the place of that file in its tab.
fn browse<A: PluginApp>(app: &A, shell: &mut Shell<A>, step: Step) {
    let Some(FileDoc {
        path: Some(path),
        plugin,
        ..
    }) = active_file(shell)
    else {
        return;
    };
    let (path, plugin) = (path.clone(), *plugin);
    let Some(dir) = path.parent() else {
        return;
    };
    let plugins = app.plugins().clone();
    let loaded = &plugins.loaded[plugin];
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_file() && loaded.opens(p))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|p| file_name(p).to_lowercase());
    let Some(at) = files.iter().position(|p| *p == path) else {
        return;
    };
    let last = files.len() - 1;
    let next = match step {
        Step::First => 0,
        Step::Previous => at.saturating_sub(1),
        Step::Next => (at + 1).min(last),
        Step::Last => last,
    };
    let count = files.len();
    if next == at {
        shell.status = trf("{n} of {count}", &[("n", &(at + 1)), ("count", &count)]);
        return;
    }
    let settings = A::plugin_settings(&shell.settings.extra).of(&loaded.id);
    match open_view(&plugins, &settings, plugin, &files[next]) {
        Ok(doc) => {
            let active = shell.active;
            shell.docs[active] = A::from_host_doc(HostDoc::File(doc));
            shell.status = trf("{n} of {count}", &[("n", &(next + 1)), ("count", &count)]);
        }
        Err(e) => shell.status = e,
    }
}

/// The tab of the MCP server in front: its tab, or a new one.
pub fn mcp_tab<A: PluginApp>(shell: &mut Shell<A>) {
    if !shell.focus_tab(|d| matches!(A::host_doc(d), Some(HostDoc::Mcp(_)))) {
        shell.open_tab(A::from_host_doc(HostDoc::Mcp(Search::default())));
    }
}

/// Answers the calls of the tools of the window waiting, and saves the configuration of the MCP
/// server once it changed: at every frame, from `draw`.
fn mcp_frame<A: PluginApp>(app: &mut A, shell: &mut Shell<A>, ctx: &egui::Context) {
    let Some(bridge) = app.mcp().map(|m| m.bridge.clone()) else {
        return;
    };
    wwmcp::window::answer(app, shell, &bridge, ctx, &mut |app, shell, tool, args| {
        app.program_tool(shell, tool, args)
    });
    if let Some(m) = app.mcp_mut() {
        m.panel.persist();
    }
}

/// The page of the plugins in front: its tab, or a new one.
pub fn plugins_tab<A: PluginApp>(shell: &mut Shell<A>) {
    if !shell.focus_tab(|d| matches!(A::host_doc(d), Some(HostDoc::Plugins))) {
        shell.open_tab(A::from_host_doc(HostDoc::Plugins));
    }
}

/// The lines of where a DLL comes from: its SHA-256 and its signature.
fn origin_lines(ui: &mut egui::Ui, origin: &Origin) {
    ui.weak(trf("SHA-256: {sha}", &[("sha", &origin.sha256)]));
    let signature = match &origin.signature {
        Signature::None => tr("Not signed").to_owned(),
        Signature::Trusted(name) => trf("Signed by {name}", &[("name", name)]),
        Signature::Untrusted(why) => trf("Its signature is not trusted: {why}", &[("why", why)]),
    };
    ui.weak(signature);
}

/// The box of a DLL trusted, in the settings; gives whether it changed.
fn trusted_box(ui: &mut egui::Ui, trust: &mut Trust, origin: &Origin) -> bool {
    let mut on = trust.listed(&origin.sha256);
    let changed = ui.checkbox(&mut on, tr("Trusted")).changed();
    if changed {
        trust.set(&origin.sha256, on);
    }
    changed
}

/// A size in bytes for a person: bytes, KB or MB.
fn size_text(bytes: usize) -> String {
    if bytes >= 1 << 20 {
        trf(
            "{n} MB",
            &[("n", &format!("{:.1}", bytes as f64 / f64::from(1 << 20)))],
        )
    } else if bytes >= 1 << 10 {
        trf("{n} KB", &[("n", &(bytes >> 10))])
    } else {
        trf("{n} bytes", &[("n", &bytes)])
    }
}

/// The line of the state of the cache of the blocks.
pub fn cache_line() -> String {
    let s = blocks::cache().state();
    trf(
        "Cache of the blocks: {used} of {budget} in {blocks} blocks of {block}, {files} files; {reads} read from the disk, {hits} found in memory",
        &[
            ("used", &size_text(s.used)),
            ("budget", &size_text(s.limits.budget)),
            ("blocks", &s.blocks),
            ("block", &size_text(s.limits.block)),
            ("files", &s.files),
            ("reads", &s.reads),
            ("hits", &s.hits),
        ],
    )
}

/// The page of the plugins: what each opens, its files, where it comes from, and the DLLs that
/// did not load; a page of a plugin opens through `acts`.
pub fn plugins_page<A: PluginApp>(app: &A, ui: &mut egui::Ui, acts: &mut Vec<A::Act>) {
    let plugins = app.plugins();
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(A::name());
            ui.label(tr(
                "Open a file (Ctrl+O) or drop one on the window: the plugin of its format shows it.",
            ));
            ui.add_space(8.0);
            for loaded in &plugins.loaded {
                ui.strong(tr(loaded.plugin.name()));
                ui.weak(loaded.file.display().to_string());
                if let Some(origin) = &loaded.origin {
                    origin_lines(ui, origin);
                }
                for format in loaded.plugin.formats() {
                    let mut line =
                        format!("{}: {}", tr(format.name), format.extensions.join(", "));
                    if !format.mimes.is_empty() {
                        line.push_str(&format!(" ({})", format.mimes.join(", ")));
                    }
                    ui.label(line);
                }
                let files = loaded.plugin.files();
                let pages: Vec<&str> = files
                    .files()
                    .iter()
                    .filter(|f| !f.path.starts_with("lang/") && f.path.ends_with(".md"))
                    .map(|f| f.path)
                    .collect();
                if !pages.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for page in pages {
                            if ui.link(page).clicked() {
                                let act = ShellAct::OpenEmbedded(files, page.to_owned());
                                acts.push(A::Act::from(act));
                            }
                        }
                    });
                }
                ui.add_space(6.0);
            }
            for failed in &plugins.failed {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    trf(
                        "Not loaded: {file}: {why}",
                        &[("file", &failed.file.display()), ("why", &failed.why)],
                    ),
                );
                if let Some(origin) = &failed.origin {
                    origin_lines(ui, origin);
                }
            }
            ui.add_space(6.0);
            ui.weak(cache_line());
            ui.weak(plugin::runtime());
        });
    });
}

/// Draws the active tab when it is a tab of the host; false when it is not. It answers the tools
/// of the window first, so a program calls it at every frame.
pub fn draw<A: PluginApp>(
    app: &mut A,
    shell: &mut Shell<A>,
    ui: &mut egui::Ui,
    acts: &mut Vec<A::Act>,
) -> bool {
    mcp_frame(app, shell, ui.ctx());
    let (wrap_lines, page_width) = (shell.settings.wrap_lines, shell.settings.page_width);
    let every = A::plugin_settings(&shell.settings.extra).plugins.clone();
    let plugins = app.plugins().clone();
    match shell.active_doc_mut().and_then(A::host_doc_mut) {
        Some(HostDoc::File(doc)) => {
            let settings = every
                .get(&plugins.loaded[doc.plugin].id)
                .cloned()
                .unwrap_or(Value::Null);
            let mut host =
                Host::new(wrap_lines, page_width, &settings).with_plugins(&plugins, &every);
            host.plugin_id = &plugins.loaded[doc.plugin].id;
            doc.view.ui(ui, &mut host);
            acts.extend(requested::<A>(host.requests));
            true
        }
        Some(HostDoc::Plugins) => {
            plugins_page(app, ui, acts);
            true
        }
        Some(HostDoc::Mcp(search)) => {
            if let Some(m) = app.mcp_mut() {
                let mut own = Vec::new();
                wwmcp::ui::draw(
                    ui,
                    &mut m.panel,
                    search,
                    &mut own,
                    &mut |_| {},
                    &mut |_, _| {},
                );
                acts.extend(own.into_iter().map(|a| A::host_act(HostAct::Mcp(a))));
            }
            true
        }
        None => false,
    }
}

/// The bar of the view in front: its tools, the switches pressed, Open with for the bytes it
/// offers, Previous and Next when it steps through its folder; none for another tab.
pub fn commands<A: PluginApp>(shell: &Shell<A>) -> Option<Vec<Item<A::Act>>> {
    let doc = active_file(shell)?;
    let mut items = Vec::new();
    let tools = doc.view.tools();
    for tool in tools {
        let on = doc.view.pressed(tool.id);
        let act = A::host_act(HostAct::Tool(tool.id));
        items.push(toggle(tool.icon, tr(tool.label), on, act));
    }
    if doc.view.offer().is_some() {
        items.push(menu(icons::GO_TO, tr("Open with"), 260.0, OPEN_WITH));
    }
    if doc.view.browses() {
        items.push(Item::Separator);
        let previous = A::host_act(HostAct::Browse(Step::Previous));
        items.push(cmd(icons::BACK, tr("Previous"), previous));
        let next = A::host_act(HostAct::Browse(Step::Next));
        items.push(cmd(icons::FORWARD, tr("Next"), next));
    }
    Some(items)
}

/// The menu Open with of the bar: the plugins of the type of the bytes the view in front offers,
/// then the others.
pub fn menu_popup<A: PluginApp>(
    app: &A,
    shell: &Shell<A>,
    id: usize,
    ui: &mut egui::Ui,
    acts: &mut Vec<A::Act>,
) {
    if id != OPEN_WITH {
        return;
    }
    let Some(offer) = active_file(shell).and_then(|d| d.view.offer()) else {
        return;
    };
    ui.weak(&offer.name);
    let choices = app.plugins().choices_for_bytes(offer.mime.as_deref());
    if let Some(choice) = menus::open_with(ui, app.plugins(), &choices) {
        acts.push(A::host_act(HostAct::OpenOffered(choice)));
    }
}

/// The items of the view in front in the menu of the window: its own, its context menu in a
/// submenu and Open the file with, after a line.
pub fn view_menu<A: PluginApp>(
    app: &A,
    shell: &Shell<A>,
    ui: &mut egui::Ui,
    acts: &mut Vec<A::Act>,
) {
    let Some(doc) = active_file(shell) else {
        return;
    };
    let plugins = app.plugins();
    let items = doc.view.menu();
    let context = !doc.view.context().is_empty() || doc.view.offer().is_some();
    if !items.is_empty() || context || doc.path.is_some() {
        ui.separator();
    }
    for item in items {
        let act = A::host_act(HostAct::Tool(item.id));
        menu_item(ui, item.icon, tr(item.label), "", act, acts);
    }
    if context {
        let picked = sub_menu(ui, icons::CLIPBOARD, tr("Context menu"), |ui| {
            menus::items(ui, doc.view.as_ref(), Some(plugins.as_ref()))
        });
        match picked.flatten() {
            Some(Picked::Tool(id)) => acts.push(A::host_act(HostAct::Tool(id))),
            Some(Picked::OpenWith(choice)) => acts.push(A::host_act(HostAct::OpenOffered(choice))),
            None => {}
        }
    }
    if let Some(path) = &doc.path {
        let picked = sub_menu(ui, icons::OPEN, tr("Open the file with"), |ui| {
            let sniffed = plugins.sniff_file(path);
            let choices = plugins.choices_for_file(path, sniffed.as_deref());
            menus::open_with(ui, plugins, &choices)
        });
        if let Some(Some(choice)) = picked {
            acts.push(A::host_act(HostAct::OpenFileWith(path.clone(), choice)));
        }
    }
}

/// A submenu of each plugin that has items, and Plugins, the page of the plugins.
pub fn plugins_menu<A: PluginApp>(app: &A, ui: &mut egui::Ui, acts: &mut Vec<A::Act>) {
    for (i, loaded) in app.plugins().loaded.iter().enumerate() {
        let items = loaded.plugin.menu();
        if items.is_empty() {
            continue;
        }
        sub_menu(ui, icons::PACKAGE, tr(loaded.plugin.name()), |ui| {
            for item in items {
                let act = A::host_act(HostAct::PluginItem(i, item.id));
                menu_item(ui, item.icon, tr(item.label), "", act, acts);
            }
        });
    }
    let act = A::host_act(HostAct::Plugins);
    menu_item(ui, icons::PACKAGE, tr("Plugins"), "", act, acts);
    if app.mcp().is_some() {
        let act = A::host_act(HostAct::OpenMcp);
        menu_item(ui, icons::LINK, tr("MCP server"), "", act, acts);
    }
}

/// The plugin of the tab in front and what its view says, for the status bar.
pub fn status_left<A: PluginApp>(app: &A, shell: &Shell<A>) -> Vec<String> {
    let Some(doc) = active_file(shell) else {
        return Vec::new();
    };
    vec![
        tr(app.plugins().loaded[doc.plugin].plugin.name()).to_owned(),
        doc.view.status(),
    ]
}

/// The lines of About of a program with plugins: the build of the runtime and the count of the
/// plugins.
pub fn about_lines<A: PluginApp>(app: &A) -> Vec<String> {
    vec![
        plugin::runtime().to_owned(),
        trf("{n} plugins", &[("n", &app.plugins().loaded.len())]),
    ]
}

/// The size of a block and the budget of the cache in the settings; gives whether they changed.
fn cache_card(ui: &mut egui::Ui, limits: &mut Limits, acts_clear: &mut bool) -> bool {
    let mut changed = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.strong(tr("Cache of the blocks"));
        ui.weak(tr(
            "The views of large files read the blocks they show; the blocks used longest ago leave the memory when the blocks of every open file reach the budget.",
        ));
        let mut block_kb = limits.block / 1024;
        let mut budget_mb = limits.budget / (1 << 20);
        ui.horizontal(|ui| {
            ui.label(tr("A block, KB"));
            let min = blocks::MIN_BLOCK / 1024;
            let max = blocks::MAX_BLOCK / 1024;
            changed |= ui
                .add(egui::DragValue::new(&mut block_kb).range(min..=max))
                .changed();
            ui.label(tr("The budget, MB"));
            changed |= ui
                .add(egui::DragValue::new(&mut budget_mb).range(1..=65_536))
                .changed();
            if ui.button(tr("Empty the cache")).clicked() {
                *acts_clear = true;
            }
        });
        if changed {
            *limits = Limits {
                block: block_kb * 1024,
                budget: budget_mb << 20,
            }
            .checked();
        }
        ui.weak(cache_line());
    });
    changed
}

/// The section Plugins of the settings: which plugins load, the cache of the blocks, the card of
/// each plugin with its settings, then the DLLs that did not load.
pub fn settings_section<A: PluginApp>(app: &A, shell: &mut Shell<A>, ui: &mut egui::Ui) {
    let plugins = app.plugins().clone();
    let own = A::plugin_settings_mut(&mut shell.settings.extra);
    let trust = &mut own.trust;
    let mut changed = ui
        .checkbox(&mut trust.only_trusted, tr("Load only the trusted plugins"))
        .changed();
    changed |= ui
        .checkbox(
            &mut trust.signed,
            tr("Trust a plugin whose signature Windows trusts"),
        )
        .changed();
    ui.weak(tr(
        "A plugin is trusted by its SHA-256: a new build of it is a new plugin. A change takes effect at the next start.",
    ));
    ui.add_space(6.0);
    let mut clear = false;
    if cache_card(ui, &mut own.cache, &mut clear) {
        blocks::cache().set_limits(own.cache);
        changed = true;
    }
    if clear {
        blocks::cache().clear();
    }
    ui.add_space(6.0);
    for loaded in &plugins.loaded {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong(tr(loaded.plugin.name()));
            ui.weak(loaded.file.display().to_string());
            if let Some(origin) = &loaded.origin {
                origin_lines(ui, origin);
                changed |= trusted_box(ui, &mut own.trust, origin);
            }
            ui.add_space(4.0);
            if loaded.plugin.has_settings() {
                let mut settings = own.of(&loaded.id);
                if loaded.plugin.settings_ui(ui, &mut settings) {
                    own.plugins.insert(loaded.id.clone(), settings);
                    changed = true;
                }
            } else {
                ui.weak(tr("The plugin has no settings."));
            }
        });
        ui.add_space(6.0);
    }
    for failed in &plugins.failed {
        ui.colored_label(
            ui.visuals().error_fg_color,
            trf(
                "Not loaded: {file}: {why}",
                &[("file", &failed.file.display()), ("why", &failed.why)],
            ),
        );
        if let Some(origin) = &failed.origin {
            origin_lines(ui, origin);
            changed |= trusted_box(ui, &mut own.trust, origin);
        }
    }
    if changed {
        shell.settings_dirty = true;
    }
    ui.add_space(6.0);
    ui.weak(plugin::runtime());
}

/// Runs an act of the host.
pub fn apply<A: PluginApp>(app: &mut A, shell: &mut Shell<A>, act: HostAct) {
    let (wrap_lines, page_width) = (shell.settings.wrap_lines, shell.settings.page_width);
    let own = A::plugin_settings(&shell.settings.extra).clone();
    let every = &own.plugins;
    let plugins = app.plugins().clone();
    let settings_of = |plugin: usize| own.of(&plugins.loaded[plugin].id);
    match act {
        HostAct::Tool(id) => {
            let mut requests = Vec::new();
            if let Some(doc) = active_file_mut(shell) {
                let settings = settings_of(doc.plugin);
                let mut host =
                    Host::new(wrap_lines, page_width, &settings).with_plugins(&plugins, every);
                doc.view.act(id, &mut host);
                requests = host.requests;
            }
            shell.pending.extend(requested::<A>(requests));
        }
        HostAct::PluginItem(plugin, id) => {
            let settings = settings_of(plugin);
            let mut host =
                Host::new(wrap_lines, page_width, &settings).with_plugins(&plugins, every);
            plugins.loaded[plugin].plugin.act(id, &mut host);
            shell.pending.extend(requested::<A>(host.requests));
        }
        HostAct::Status(line) => shell.status = line,
        HostAct::OpenOffered(choice) => {
            let mut requests = Vec::new();
            if let Some(doc) = active_file_mut(shell) {
                let settings = settings_of(doc.plugin);
                let mut host =
                    Host::new(wrap_lines, page_width, &settings).with_plugins(&plugins, every);
                menus::open_offered(doc.view.as_mut(), &choice, &mut host);
                requests = host.requests;
            }
            shell.pending.extend(requested::<A>(requests));
        }
        HostAct::OpenWith {
            name,
            plugin,
            mime,
            bytes,
        } => {
            let Some(loaded) = plugins.loaded.get(plugin) else {
                return;
            };
            match loaded
                .plugin
                .open_bytes(&name, &mime, bytes, &settings_of(plugin))
            {
                Ok(view) => shell.open_tab(A::from_host_doc(HostDoc::File(FileDoc {
                    path: None,
                    name,
                    plugin,
                    view,
                }))),
                Err(why) => {
                    shell.status = trf("{name}: {why}", &[("name", &name), ("why", &why)]);
                }
            }
        }
        HostAct::OpenFileWith(path, choice) => {
            let Some(loaded) = plugins.loaded.get(choice.plugin) else {
                return;
            };
            let opened = loaded
                .plugin
                .open_as(&path, choice.mime, &settings_of(choice.plugin));
            match opened {
                Ok(view) => shell.open_tab(A::from_host_doc(HostDoc::File(FileDoc {
                    name: trf(
                        "{name} ({format})",
                        &[("name", &file_name(&path)), ("format", &tr(choice.format))],
                    ),
                    path: Some(path),
                    plugin: choice.plugin,
                    view,
                }))),
                Err(why) => {
                    let name = file_name(&path);
                    shell.status = trf("{name}: {why}", &[("name", &name), ("why", &why)]);
                }
            }
        }
        HostAct::OpenBytes { name, mime, bytes } => {
            let settings = |id: &str| own.of(id);
            match plugins.open_bytes(&name, &mime, bytes, &settings) {
                Ok((plugin, view)) => shell.open_tab(A::from_host_doc(HostDoc::File(FileDoc {
                    path: None,
                    name,
                    plugin,
                    view,
                }))),
                Err(why) => {
                    shell.status = trf("{name}: {why}", &[("name", &name), ("why", &why)]);
                }
            }
        }
        HostAct::Browse(step) => browse(app, shell, step),
        HostAct::Plugins => plugins_tab(shell),
        HostAct::ClearCache => blocks::cache().clear(),
        HostAct::OpenMcp => mcp_tab(shell),
        // The log of the server opens as any file: JSON Lines by a plugin of texts of a tree.
        HostAct::Mcp(McpAct::OpenLog(path)) => open_path(app, shell, path),
        HostAct::Mcp(act) => {
            if let Some(m) = app.mcp_mut()
                && let Some(line) = m.panel.apply(act)
            {
                shell.status = line;
            }
        }
    }
}

/// Attaches to the console of the parent process, so that a release build of a program without
/// a console prints in it; a process whose output goes to a file or a pipe (a script, CI) keeps it
/// there.
pub fn attach_console() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::{
            ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
        };
        // SAFETY: the calls only read the handle of the output and connect the process to an
        // existing console, if there is one.
        unsafe {
            let redirected =
                GetStdHandle(STD_OUTPUT_HANDLE).is_ok_and(|h| !h.is_invalid() && !h.0.is_null());
            if !redirected {
                let _ = AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
    }
}

/// The folder of the plugins: the one the environment variable names, else `plugins` next to
/// the executable. A debug build without that folder takes the folder of the executable itself
/// (a build of a workspace, where cargo writes every DLL); a release build never does, so that
/// a DLL put next to the executable of a program is not loaded as a plugin.
pub fn plugins_dir(variable: &str) -> PathBuf {
    if let Some(dir) = std::env::var_os(variable) {
        return PathBuf::from(dir);
    }
    plugins_dir_of(&ww::folders::exe_dir(), cfg!(debug_assertions))
}

/// The folder of the plugins of a program in the folder `here`; `workspace` allows the folder
/// itself when it has no folder `plugins`.
fn plugins_dir_of(here: &Path, workspace: bool) -> PathBuf {
    let folder = here.join("plugins");
    if workspace && !folder.is_dir() {
        here.to_path_buf()
    } else {
        folder
    }
}

/// The plugins with the formats they open, their files and where they come from, and the DLLs
/// that did not load: the text of `--plugins`.
pub fn list(plugins: &Plugins, dir: &Path, took: std::time::Duration) -> String {
    let origin = |origin: &Origin| {
        let signature = match &origin.signature {
            Signature::None => "not signed".to_owned(),
            Signature::Trusted(name) => format!("signed by {name}"),
            Signature::Untrusted(why) => format!("a signature not trusted: {why}"),
        };
        format!("  SHA-256 {}\n  {signature}\n", origin.sha256)
    };
    let mut out = format!(
        "{} plugins of {} loaded in {took:?}\nruntime: {}\n",
        plugins.loaded.len(),
        dir.display(),
        plugin::runtime()
    );
    for loaded in &plugins.loaded {
        out.push_str(&format!(
            "\n{} ({})\n",
            loaded.plugin.name(),
            loaded.file.display()
        ));
        if let Some(o) = &loaded.origin {
            out.push_str(&origin(o));
        }
        for format in loaded.plugin.formats() {
            out.push_str(&format!(
                "  {}: {}",
                format.name,
                format.extensions.join(", ")
            ));
            if !format.mimes.is_empty() {
                out.push_str(&format!(" ({})", format.mimes.join(", ")));
            }
            out.push('\n');
        }
        let files = loaded.plugin.files();
        if !files.is_empty() {
            let paths: Vec<&str> = files.files().iter().map(|f| f.path).collect();
            out.push_str(&format!("  files: {}\n", paths.join(", ")));
        }
    }
    for failed in &plugins.failed {
        out.push_str(&format!(
            "\nnot loaded: {}: {}\n",
            failed.file.display(),
            failed.why
        ));
        if let Some(o) = &failed.origin {
            out.push_str(&origin(o));
        }
    }
    out
}

/// Opens a file with the first plugin that takes it and draws three frames of its view without
/// a window: the text of `--probe`, with the times.
pub fn probe(plugins: &Plugins, path: &Path) -> Result<String, String> {
    let found = plugins.all_for_file(path);
    if found.is_empty() {
        return Err(format!("no plugin opens {}", path.display()));
    }
    let started = std::time::Instant::now();
    let mut opened = None;
    let mut first = None;
    for i in found {
        match plugins.loaded[i].plugin.open(path, &Value::Null) {
            Ok(view) => {
                opened = Some((i, view));
                break;
            }
            Err(why) => {
                first.get_or_insert(why);
            }
        }
    }
    let Some((i, mut view)) = opened else {
        return Err(first.unwrap_or_default());
    };
    let took = started.elapsed();
    let settings = Value::Null;
    let every = BTreeMap::new();
    let mut host = Host::new(true, ww::PageWidth::View, &settings).with_plugins(plugins, &every);
    let ctx = egui::Context::default();
    let started = std::time::Instant::now();
    let mut shapes = 0;
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| view.ui(ui, &mut host));
        shapes = output.shapes.len();
        // No renderer takes the textures here; egui wants them taken or cleared.
        output.textures_delta.clear();
    }
    Ok(format!(
        "{}: {}, opened in {took:?}, three frames in {:?} ({shapes} shapes in the last); {}",
        path.display(),
        plugins.loaded[i].plugin.name(),
        started.elapsed(),
        view.status()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_build_loads_no_dll_next_to_the_executable() {
        let here = std::env::temp_dir().join(format!("wwrt-plugins-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&here);
        std::fs::create_dir_all(&here).unwrap();
        // No folder `plugins`: a build of a workspace takes the folder of the executable, a
        // release build the folder that is not there, where nothing loads.
        assert_eq!(plugins_dir_of(&here, true), here);
        let folder = here.join("plugins");
        assert_eq!(plugins_dir_of(&here, false), folder);
        assert!(Plugins::load_dir(&folder).loaded.is_empty());
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(plugins_dir_of(&here, true), folder);
        assert_eq!(plugins_dir_of(&here, false), folder);
        let _ = std::fs::remove_dir_all(&here);
    }
}
