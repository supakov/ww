//! The demo of W#: a program on the window of W# (`ww::App`) and a host of plugins
//! (`wwrt::host::PluginApp`), with every part once. Its tabs are the files of the plugins and the
//! page of the plugins (`HostDoc`), its own notes (`ww::textdoc::TextDoc`), Markdown files, the
//! tab of the widgets and the documents of the window (the settings, the editor of a theme, the
//! pages of its embedded guide). It embeds its guide, the translations of its texts and its samples
//! (`FILES`), adds a colour to the themes, two plugins of its own and a section of its settings,
//! runs a count in the background, and serves the tools of its window, of its own and of its
//! plugins over MCP (`HostMcp`, the tab MCP server).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use wwrt::egui::{self, Color32};
use wwrt::host::{self, HostAct, HostDoc, HostMcp, PluginApp, PluginSettings};
use wwrt::plugin::Plugins;
use wwrt::serde::{Deserialize, Serialize};
use wwrt::ww::clock::Instant;
use wwrt::ww::commands::{Item, cmd, keyed, menu, toggle, when};
use wwrt::ww::embedded::Embedded;
use wwrt::ww::i18n::{tr, trf};
use wwrt::ww::mdview::{self, MdDoc};
use wwrt::ww::shell::{menu_entry, menu_item};
use wwrt::ww::style::{self, ExtraColor};
use wwrt::ww::textdoc::{self, TextDoc};
use wwrt::ww::{
    App, Document, Draft, Job, Messages, Nav, Question, Search, Shell, ShellAct, ShellDoc, icons,
    keys, session,
};
use wwrt::wwmcp::{self, Args, Info, Kind, Mode, Outcome, ToolDef, fail};

use crate::bundle::BundlePlugin;
use crate::texts::TextsPlugin;
use crate::widgets::{self, WidgetAct, Widgets};

/// The version of the program, for About and the log.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The files the program embeds: its guide (two of its pages translated), its samples and the
/// translations of its texts. A file `lang/<code>/docs/guide.md` next to the executable stands
/// for the embedded one.
pub static FILES: Embedded = wwrt::ww::embed![
    "docs/guide.md",
    "docs/window.md",
    "docs/plugins.md",
    "docs/files.md",
    "lang/uk/docs/guide.md",
    "lang/ru/docs/guide.md",
    "samples/demo.wwbundle",
    "samples/notes.txt",
    "lang/uk/ui.json",
    "lang/ru/ui.json",
]
.with_folder("");

/// The first page of the guide.
pub const GUIDE: &str = "docs/guide.md";

/// The translations of the window: the `ui.json` files of the embedded set.
static LANGUAGES: LazyLock<Vec<(&'static str, &'static str)>> =
    LazyLock::new(|| FILES.translations());

/// The colour the program adds to the themes.
static COLORS: [ExtraColor; 1] = [ExtraColor {
    name: widgets::STAR,
    label: "Star of the demo",
    light: Color32::from_rgb(196, 120, 0),
    dark: Color32::from_rgb(255, 200, 70),
}];

/// The largest note the program opens.
const MAX_NOTE: u64 = 8 * 1024 * 1024;
/// The number of the menu of the folders in the bar of the widgets.
const FOLDERS: usize = 1;

/// The fields of the program in the settings file: the ones of the host of the plugins, and its
/// own (the section Demo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(crate = "wwrt::serde", default)]
pub struct Extra {
    #[serde(flatten)]
    pub host: PluginSettings,
    /// The line Greet puts into a note.
    pub greeting: String,
    /// How far the count of the background goes.
    pub count_to: u32,
}

impl Default for Extra {
    fn default() -> Self {
        Extra {
            host: PluginSettings::default(),
            greeting: "Hello from the settings".to_owned(),
            count_to: 50,
        }
    }
}

impl wwrt::ww::Extra for Extra {
    const FORMAT: &'static str = "ww-demo-settings";
}

/// A tab of the demo.
pub enum Doc {
    /// A file or some bytes by a plugin, or the page of the plugins.
    Host(HostDoc),
    /// A note: a text of the program itself, changed and saved.
    Note(Box<TextDoc>),
    /// A Markdown file of the disk.
    Page(MdDoc),
    /// The widgets of W#.
    Widgets(Box<Widgets>),
    /// The settings, the editor of a theme, a page of a set of embedded files.
    Shell(ShellDoc),
}

impl Document for Doc {
    fn title(&self) -> String {
        match self {
            Doc::Host(d) => d.title(),
            Doc::Note(d) => d.title(),
            Doc::Page(d) => d.title(),
            Doc::Widgets(d) => d.title(),
            Doc::Shell(d) => d.title(),
        }
    }

    fn dirty(&self) -> bool {
        match self {
            Doc::Note(d) => d.dirty,
            Doc::Shell(d) => d.dirty(),
            _ => false,
        }
    }

    fn path(&self) -> Option<&Path> {
        match self {
            Doc::Host(d) => d.path(),
            Doc::Note(d) => d.path.as_deref(),
            Doc::Page(d) => d.path(),
            Doc::Widgets(_) => None,
            Doc::Shell(d) => d.path(),
        }
    }

    fn search(&mut self) -> Option<&mut Search> {
        match self {
            Doc::Host(d) => d.search(),
            Doc::Note(d) => Some(&mut d.search),
            Doc::Page(d) => d.search(),
            Doc::Widgets(d) => d.search(),
            Doc::Shell(d) => d.search(),
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Doc::Host(d) => d.icon(),
            Doc::Note(_) => icons::EDIT,
            Doc::Page(_) => icons::BOOK,
            Doc::Widgets(d) => d.icon(),
            Doc::Shell(d) => d.icon(),
        }
    }

    fn search_hint(&self) -> &'static str {
        match self {
            Doc::Host(d) => d.search_hint(),
            Doc::Note(_) => "Search the note (Ctrl+F, F3 the next)",
            Doc::Page(_) => "Search the page (Ctrl+F)",
            Doc::Widgets(d) => d.search_hint(),
            Doc::Shell(d) => d.search_hint(),
        }
    }

    fn has_tools(&self) -> bool {
        match self {
            Doc::Shell(d) => d.has_tools(),
            _ => true,
        }
    }

    fn has_nav(&self) -> bool {
        matches!(self, Doc::Widgets(_))
    }

    fn draft_key(&self) -> Option<String> {
        match self {
            Doc::Note(d) => Some(d.draft_key.clone()),
            Doc::Shell(d) => d.draft_key(),
            _ => None,
        }
    }

    fn draft_due(&mut self, all: bool) -> Option<Draft> {
        match self {
            Doc::Note(d) => d.draft_due(all),
            Doc::Shell(d) => d.draft_due(all),
            _ => None,
        }
    }

    fn shell_doc(&self) -> Option<&ShellDoc> {
        match self {
            Doc::Shell(d) => Some(d),
            _ => None,
        }
    }

    fn shell_doc_mut(&mut self) -> Option<&mut ShellDoc> {
        match self {
            Doc::Shell(d) => Some(d),
            _ => None,
        }
    }
}

/// What a frame asks of the program, done after it.
pub enum Act {
    /// An act of the window itself, run by the shell.
    Shell(ShellAct<Demo>),
    /// An act of the host of the plugins.
    Host(HostAct),
    /// An act of the tab of the widgets.
    Widget(WidgetAct),
    NewNote,
    Guide,
    Widgets,
    StartJob,
    StopJob,
    Ask,
    /// The answer of the question, by the label of its button.
    Answered(&'static str),
    /// The next match of the search of the note in front.
    FindNext,
    /// The greeting of the settings at the end of the note in front.
    Greet,
}

impl From<ShellAct<Demo>> for Act {
    fn from(act: ShellAct<Demo>) -> Self {
        Act::Shell(act)
    }
}

/// The count in the background: how far it is, whether it is asked to stop, whether it ended.
struct Count {
    done: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    ended: Arc<AtomicBool>,
    to: u32,
    since: Instant,
}

/// The demo program.
pub struct Demo {
    plugins: Arc<Plugins>,
    /// The files named on the command line, opened after the session is restored.
    files: Vec<PathBuf>,
    count: Option<Count>,
    /// The MCP server: the tools of the window, `new_note` and the tools of the plugins.
    mcp: HostMcp,
}

/// What the server says of the program at `initialize`.
pub fn info() -> Info {
    Info {
        name: String::from("ww-demo"),
        version: VERSION.to_owned(),
        instructions: "The demo of W#. list_files names the tabs of its window; open_file, close_file and save_file open, close and save them; new_note opens a note with a text; the plugins add tools of their own.".to_owned(),
    }
}

/// The tools of the program itself that act on its window, answered by `program_tool`.
pub fn demo_tools() -> Vec<ToolDef> {
    vec![
        ToolDef::new(
            "new_note",
            "notes",
            "Opens a new note in a tab of the window with a text.",
            wwmcp::schema(
                wwrt::serde_json::json!({ "text": wwmcp::text("The text of the note") }),
                &[],
            ),
        )
        .mode(Mode::Edit),
    ]
}

/// The plugins of a folder the settings trust, and the two the program makes itself.
pub fn plugins_of(dir: &Path, settings: &PluginSettings) -> Plugins {
    let mut plugins = host::load(dir, settings);
    own_plugins(&mut plugins);
    plugins
}

/// The plugins the program makes itself, after the ones of the folder.
pub fn own_plugins(plugins: &mut Plugins) {
    plugins.add(PathBuf::from("bundles"), Box::new(BundlePlugin));
    plugins.add(PathBuf::from("texts"), Box::new(TextsPlugin));
}

impl Demo {
    pub fn new(plugins: Arc<Plugins>, files: Vec<PathBuf>) -> Self {
        style::install_colors(&COLORS);
        let mcp = HostMcp::new(info(), &plugins, demo_tools(), Vec::new());
        Self {
            plugins,
            files,
            count: None,
            mcp,
        }
    }

    /// A note of a file in a tab, once.
    fn open_note(&mut self, shell: &mut Shell<Self>, path: PathBuf) {
        shell.open_file(
            &path,
            |d| matches!(d, Doc::Note(n) if n.path.as_ref() == Some(&path)),
            |p| textdoc::read_text(p, MAX_NOTE),
            |text, path| Doc::Note(Box::new(TextDoc::new(text, Some(path)))),
        );
    }

    /// A Markdown file as a page, once.
    fn open_md(&mut self, shell: &mut Shell<Self>, path: PathBuf) {
        shell.open_file(
            &path,
            |d| matches!(d, Doc::Page(p) if p.path.as_deref() == Some(path.as_path())),
            |p| textdoc::read_text(p, MAX_NOTE),
            |text, path| {
                let name = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                Doc::Page(MdDoc::new(name, Some(path), text))
            },
        );
    }

    fn widgets_tab(&self, shell: &mut Shell<Self>) {
        if !shell.focus_tab(|d| matches!(d, Doc::Widgets(_))) {
            shell.open_tab(Doc::Widgets(Box::default()));
        }
    }

    /// Starts the count of the background, up to the number of the settings.
    fn start_count(&mut self, shell: &mut Shell<Self>) {
        if self.count.is_some() {
            shell.status = tr("The count runs already").to_owned();
            return;
        }
        let to = shell.settings.extra.count_to.max(1);
        let count = Count {
            done: Arc::new(AtomicU32::new(0)),
            stop: Arc::new(AtomicBool::new(false)),
            ended: Arc::new(AtomicBool::new(false)),
            to,
            since: Instant::now(),
        };
        let (done, stop, ended) = (count.done.clone(), count.stop.clone(), count.ended.clone());
        std::thread::spawn(move || {
            for _ in 0..to {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
                done.fetch_add(1, Ordering::Relaxed);
            }
            ended.store(true, Ordering::Relaxed);
        });
        self.count = Some(count);
    }

    /// Saves the note in front: to its file, or to the one a dialog names.
    fn save_note(&mut self, shell: &mut Shell<Self>, ask: bool) {
        let Some(Doc::Note(doc)) = shell.active_doc() else {
            shell.status = tr("This tab saves nothing; a note does").to_owned();
            return;
        };
        let path = match (&doc.path, ask) {
            (Some(p), false) => p.clone(),
            _ => {
                let dialog = wwrt::ww::rfd::FileDialog::new()
                    .set_title(tr("Save as"))
                    .add_filter(tr("Notes"), &["txt", "note"])
                    .add_filter(tr("All files"), &["*"])
                    .set_file_name(doc.title());
                let Some(p) = dialog.save_file() else {
                    return;
                };
                p
            }
        };
        let Some(Doc::Note(doc)) = shell.active_doc_mut() else {
            return;
        };
        let old = doc.draft_key.clone();
        match doc.save_to(&path) {
            Ok(()) => {
                doc.draft_key = session::key_for_path(&path);
                let new = doc.draft_key.clone();
                session::remove_draft(&shell.drafts_dir, &old);
                session::remove_draft(&shell.drafts_dir, &new);
                shell.saved(&path);
                shell.remember_recent(&path);
            }
            Err(e) => shell.save_failed(e),
        }
    }
}

impl PluginApp for Demo {
    fn plugins(&self) -> &Arc<Plugins> {
        &self.plugins
    }

    fn host_doc(doc: &Doc) -> Option<&HostDoc> {
        match doc {
            Doc::Host(d) => Some(d),
            _ => None,
        }
    }

    fn host_doc_mut(doc: &mut Doc) -> Option<&mut HostDoc> {
        match doc {
            Doc::Host(d) => Some(d),
            _ => None,
        }
    }

    fn from_host_doc(doc: HostDoc) -> Doc {
        Doc::Host(doc)
    }

    fn host_act(act: HostAct) -> Act {
        Act::Host(act)
    }

    fn plugin_settings(extra: &Extra) -> &PluginSettings {
        &extra.host
    }

    fn plugin_settings_mut(extra: &mut Extra) -> &mut PluginSettings {
        &mut extra.host
    }

    fn mcp(&self) -> Option<&HostMcp> {
        Some(&self.mcp)
    }

    fn mcp_mut(&mut self) -> Option<&mut HostMcp> {
        Some(&mut self.mcp)
    }

    /// `new_note`: a note of the text in a new tab.
    fn program_tool(
        &mut self,
        shell: &mut Shell<Self>,
        tool: &str,
        args: &wwrt::serde_json::Value,
    ) -> Outcome {
        if tool != "new_note" {
            return Err(fail(Kind::NotFound, format!("no tool {tool}")));
        }
        let text = Args(args).optional_text("text");
        shell.open_tab(Doc::Note(Box::new(TextDoc::new(text, None))));
        Ok(wwrt::serde_json::json!({
            "tab": shell.active + 1,
            "title": shell.docs[shell.active].title(),
        }))
    }
}

impl App for Demo {
    type Doc = Doc;
    type Act = Act;
    type Extra = Extra;

    const NAME: &'static str = "ww-demo";
    const NEW_HINT: &'static str = "New note (Ctrl+N)";

    fn languages() -> &'static [(&'static str, &'static str)] {
        LANGUAGES.as_slice()
    }

    fn shell_doc(doc: ShellDoc) -> Doc {
        Doc::Shell(doc)
    }

    fn shell_act(act: Act) -> Result<ShellAct<Self>, Act> {
        match act {
            Act::Shell(a) => Ok(a),
            other => Err(other),
        }
    }

    /// The files of the command line in tabs; a window without a tab shows the guide.
    fn restored(&mut self, shell: &mut Shell<Self>) {
        for path in std::mem::take(&mut self.files) {
            self.open_path(shell, path);
        }
        if shell.docs.is_empty() {
            self.open_embedded(shell, FILES, GUIDE);
        }
    }

    fn new_document(&mut self, shell: &mut Shell<Self>) {
        shell.open_tab(Doc::Note(Box::new(TextDoc::new(String::new(), None))));
    }

    /// A note of a `.txt` or `.note` file, a page of a `.md` file, any other file by the plugins.
    fn open_path(&mut self, shell: &mut Shell<Self>, path: PathBuf) {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "txt" | "note" => self.open_note(shell, path),
            "md" => self.open_md(shell, path),
            _ => host::open_path(self, shell, path),
        }
    }

    fn open_dialog(&self, _shell: &Shell<Self>) -> wwrt::ww::rfd::FileDialog {
        host::open_dialog(&self.plugins)
            .add_filter(tr("Notes"), &["txt", "note"])
            .add_filter(tr("Markdown pages"), &["md"])
    }

    fn save(&mut self, shell: &mut Shell<Self>, ask: bool) {
        self.save_note(shell, ask);
    }

    /// A draft of a note of the last session, in the tab of its file or in a new one.
    fn restore_draft(
        &mut self,
        shell: &mut Shell<Self>,
        key: String,
        path: Option<PathBuf>,
        document: wwrt::serde_json::Value,
    ) -> bool {
        let wwrt::serde_json::Value::String(text) = document else {
            return false;
        };
        let mut doc = TextDoc::new(String::new(), path.filter(|p| p.exists()));
        doc.text = text;
        doc.dirty = true;
        doc.draft_key = key;
        shell.open_tab(Doc::Note(Box::new(doc)));
        true
    }

    /// A link of a page of the disk: a Markdown file as a page, a text as a note.
    fn open_link(&mut self, shell: &mut Shell<Self>, link: &str) {
        match shell.linked_file(link) {
            Some((path, _)) if path.is_file() => self.open_path(shell, path),
            Some((path, _)) => {
                shell.status = trf("{path} is not there", &[("path", &path.display())]);
            }
            None => {}
        }
    }

    /// A page of a set of embedded files in a tab of the window, any other file of it by the
    /// plugin of its type.
    fn open_embedded(&mut self, shell: &mut Shell<Self>, files: Embedded, link: &str) {
        host::open_embedded(self, shell, files, link);
    }

    fn about(&self, shell: &Shell<Self>) -> Vec<String> {
        let mut lines = shell.about_lines(VERSION);
        lines.extend(host::about_lines(self));
        lines
    }

    fn draw(&mut self, shell: &mut Shell<Self>, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        if host::draw(self, shell, ui, acts) {
            return;
        }
        let (wrap, width) = (shell.settings.wrap_lines, shell.settings.page_width);
        match shell.active_doc_mut() {
            Some(Doc::Note(doc)) => doc.view(ui, wrap),
            Some(Doc::Page(doc)) => {
                egui::CentralPanel::default().show(ui, |ui| mdview::md_view(ui, doc, wrap, width));
            }
            Some(Doc::Widgets(w)) => {
                let mut asked = Vec::new();
                w.view(ui, &mut asked);
                acts.extend(asked.into_iter().map(Act::Widget));
            }
            _ => {}
        }
    }

    fn commands(&self, shell: &Shell<Self>) -> Vec<Item<Act>> {
        let open = keyed(icons::OPEN, tr("Open"), &keys::OPEN, ShellAct::Open);
        if let Some(mut items) = host::commands(shell) {
            items.insert(0, open);
            if items.len() > 1 {
                items.insert(1, Item::Separator);
            }
            return items;
        }
        match shell.active_doc() {
            Some(Doc::Note(doc)) => vec![
                keyed(icons::SAVE, tr("Save"), &keys::SAVE, ShellAct::Save),
                Item::Separator,
                cmd(icons::ADD, tr("Greet"), Act::Greet),
                when(
                    icons::FIND,
                    tr("Find the next"),
                    !doc.search.text.trim().is_empty(),
                    Act::FindNext,
                ),
            ],
            Some(Doc::Widgets(w)) => vec![
                cmd(icons::CHECK, tr("Check"), Act::Widget(WidgetAct::Check)),
                toggle(
                    icons::ONLY_ON,
                    tr("Counts"),
                    w.counts,
                    Act::Widget(WidgetAct::Counts),
                ),
                menu(icons::FOLDER, tr("Folders"), 200.0, FOLDERS),
            ],
            Some(Doc::Page(_)) => vec![open],
            Some(Doc::Shell(_)) => Vec::new(),
            _ => vec![open],
        }
    }

    fn menu_popup(&self, shell: &Shell<Self>, id: usize, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        if id == FOLDERS {
            if let Some(Doc::Widgets(w)) = shell.active_doc() {
                for (i, f) in w.folders.iter().enumerate() {
                    if menu_entry(ui, icons::FOLDER, &f.name, "", i != w.chosen) {
                        acts.push(Act::Widget(WidgetAct::Choose(i)));
                    }
                }
            }
            return;
        }
        host::menu_popup(self, shell, id, ui, acts);
    }

    fn menu(&self, shell: &Shell<Self>, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        menu_item(
            ui,
            icons::NEW,
            tr("New note"),
            keys::NEW.text,
            Act::NewNote,
            acts,
        );
        let open = Act::Shell(ShellAct::Open);
        menu_item(ui, icons::OPEN, tr("Open..."), keys::OPEN.text, open, acts);
        if matches!(shell.active_doc(), Some(Doc::Note(_))) {
            let save = Act::Shell(ShellAct::Save);
            menu_item(ui, icons::SAVE, tr("Save"), keys::SAVE.text, save, acts);
            let save_as = Act::Shell(ShellAct::SaveAs);
            menu_item(
                ui,
                icons::SAVE,
                tr("Save as..."),
                keys::SAVE_AS.text,
                save_as,
                acts,
            );
        }
        let close = Act::Shell(ShellAct::CloseTab(shell.active, false));
        menu_item(
            ui,
            icons::CLOSE,
            tr("Close tab"),
            keys::CLOSE.text,
            close,
            acts,
        );
        host::view_menu(self, shell, ui, acts);
        ui.separator();
        menu_item(ui, icons::SELECT_ALL, tr("Widgets"), "", Act::Widgets, acts);
        menu_item(ui, icons::BOOK, tr("Guide"), "F1", Act::Guide, acts);
        if self.count.is_some() {
            menu_item(ui, icons::STOP, tr("Stop the job"), "", Act::StopJob, acts);
        } else {
            menu_item(ui, icons::START, tr("Start a job"), "", Act::StartJob, acts);
        }
        menu_item(ui, icons::INFO, tr("Ask a question"), "", Act::Ask, acts);
        ui.separator();
        host::plugins_menu(self, ui, acts);
        let settings = Act::Shell(ShellAct::OpenSettings);
        menu_item(
            ui,
            icons::GEAR,
            tr("Settings"),
            keys::SETTINGS.text,
            settings,
            acts,
        );
        menu_item(
            ui,
            icons::INFO,
            tr("About"),
            "",
            Act::Shell(ShellAct::About),
            acts,
        );
        menu_item(
            ui,
            icons::EXIT,
            tr("Exit"),
            "",
            Act::Shell(ShellAct::Exit),
            acts,
        );
    }

    /// The navigation row of the widgets: Back and Forward, the breadcrumb and the star.
    fn nav(&self, shell: &Shell<Self>) -> Option<Nav<Act>> {
        let Some(Doc::Widgets(w)) = shell.active_doc() else {
            return None;
        };
        Some(Nav {
            history: Some((
                !w.back.is_empty(),
                !w.forward.is_empty(),
                Act::Widget(WidgetAct::Back),
                Act::Widget(WidgetAct::Forward),
            )),
            crumbs: w
                .crumbs()
                .into_iter()
                .map(|(text, act)| (text, act.map(Act::Widget)))
                .collect(),
            star: Some((w.folders[w.chosen].starred, Act::Widget(WidgetAct::Star))),
        })
    }

    fn messages(&self, shell: &Shell<Self>) -> Option<Messages<Act>> {
        let Some(Doc::Widgets(w)) = shell.active_doc() else {
            return None;
        };
        let (error, summary) = w.summary()?;
        Some(Messages {
            error,
            summary,
            clear: Act::Widget(WidgetAct::ClearMessages),
        })
    }

    fn messages_body(&self, shell: &Shell<Self>, ui: &mut egui::Ui, _acts: &mut Vec<Act>) {
        if let Some(Doc::Widgets(w)) = shell.active_doc() {
            w.messages_body(ui);
        }
    }

    fn status_left(&self, shell: &Shell<Self>) -> Vec<String> {
        match shell.active_doc() {
            Some(Doc::Note(d)) => vec![trf(
                "Line {line}, column {col}",
                &[("line", &d.line), ("col", &d.col)],
            )],
            _ => host::status_left(self, shell),
        }
    }

    /// The blocks in the cache, at the right of the status bar.
    fn status_right(&self, _shell: &Shell<Self>, ui: &mut egui::Ui) {
        let state = wwrt::blocks::cache().state();
        ui.weak(trf("{n} blocks in the cache", &[("n", &state.blocks)]))
            .on_hover_text(host::cache_line());
    }

    fn jobs(&self, _shell: &Shell<Self>) -> Vec<Job<Act>> {
        let Some(count) = &self.count else {
            return Vec::new();
        };
        let done = count.done.load(Ordering::Relaxed);
        vec![
            Job::new(tr("A count in the background"))
                .detail(trf("{n} of {to}", &[("n", &done), ("to", &count.to)]))
                .since(count.since)
                .stop(Act::StopJob),
        ]
    }

    fn settings_sections(&self) -> Vec<&'static str> {
        vec!["Demo", "Plugins"]
    }

    fn settings_general(&mut self, _shell: &mut Shell<Self>, ui: &mut egui::Ui) {
        ui.weak(tr("A program adds lines of its own to General."));
    }

    fn settings_section(
        &mut self,
        shell: &mut Shell<Self>,
        section: usize,
        ui: &mut egui::Ui,
        _acts: &mut Vec<Act>,
    ) {
        if section == 1 {
            host::settings_section(self, shell, ui);
            return;
        }
        let extra = &mut shell.settings.extra;
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(tr("The greeting of a note"));
            changed |= ui.text_edit_singleline(&mut extra.greeting).changed();
        });
        ui.horizontal(|ui| {
            ui.label(tr("The count goes to"));
            changed |= ui
                .add(egui::DragValue::new(&mut extra.count_to).range(1..=10_000))
                .changed();
        });
        ui.weak(tr(
            "The settings of the program sit next to the ones of the window in one file.",
        ));
        if changed {
            shell.settings_dirty = true;
        }
    }

    /// F1, the guide; F3, the next match of a note.
    fn shortcuts(&self, _shell: &Shell<Self>, input: &mut egui::InputState) -> Vec<Act> {
        let mut acts = Vec::new();
        if input.consume_key(egui::Modifiers::NONE, egui::Key::F1) {
            acts.push(Act::Guide);
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::F3) {
            acts.push(Act::FindNext);
        }
        acts
    }

    fn apply(&mut self, shell: &mut Shell<Self>, act: Act) {
        match act {
            Act::Host(a) => host::apply(self, shell, a),
            Act::Widget(a) => {
                if let Some(Doc::Widgets(w)) = shell.active_doc_mut()
                    && let Some(line) = w.apply(a)
                {
                    shell.status = line;
                }
            }
            Act::NewNote => self.new_document(shell),
            Act::Guide => self.open_embedded(shell, FILES, GUIDE),
            Act::Widgets => self.widgets_tab(shell),
            Act::StartJob => self.start_count(shell),
            Act::StopJob => {
                if let Some(count) = &self.count {
                    count.stop.store(true, Ordering::Relaxed);
                }
            }
            Act::Ask => shell.ask(
                Question::new(
                    tr("A question"),
                    tr("A question over the window has a yes, a second answer and Cancel."),
                    tr("Yes"),
                    Act::Answered("Yes"),
                )
                .or(tr("Later"), Act::Answered("Later")),
            ),
            Act::Answered(answer) => {
                shell.status = trf("The answer: {answer}", &[("answer", &tr(answer))]);
            }
            Act::FindNext => {
                if let Some(Doc::Note(doc)) = shell.active_doc_mut() {
                    let needle = doc.search.text.trim().to_owned();
                    if needle.is_empty() {
                        doc.search.open = true;
                        doc.search.focus = true;
                    } else if doc.find(&needle, true).is_none() {
                        shell.status = trf("{text} was not found", &[("text", &needle)]);
                    }
                }
            }
            Act::Greet => {
                let greeting = shell.settings.extra.greeting.clone();
                if let Some(Doc::Note(doc)) = shell.active_doc_mut() {
                    if !doc.text.is_empty() && !doc.text.ends_with('\n') {
                        doc.text.push('\n');
                    }
                    doc.text.push_str(&greeting);
                    doc.text.push('\n');
                    doc.dirty = true;
                    doc.changed = Some(Instant::now());
                }
            }
            // The window runs its own acts (`shell_act`); one that comes here goes back to it.
            Act::Shell(a) => shell.pending.push(Act::Shell(a)),
        }
    }

    /// The end of the count: a line in the status bar; frames while it runs.
    fn after_frame(&mut self, shell: &mut Shell<Self>, ctx: &egui::Context) {
        let Some(count) = &self.count else {
            return;
        };
        if count.ended.load(Ordering::Relaxed) {
            let done = count.done.load(Ordering::Relaxed);
            shell.status = trf(
                "The count stopped at {n} of {to}",
                &[("n", &done), ("to", &count.to)],
            );
            self.count = None;
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
