//! The shell: the window around the documents of a program. The program implements `App`; the
//! shell owns the tabs, the settings, the status, the question over the window, the themes and
//! the session (`Shell`), draws the chrome (the tab row, the command bar, the navigation row, the
//! message and status bars, the settings tab, the editor of a theme) and hands every act of its
//! own to its handler, the others to the program (`Window`).

mod bar;
mod jobs;
mod nav;
mod settings;
mod status;
mod tabs;
mod theme;
mod window;

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::commands::Item;
use crate::doc::{Document, Scrolled, Search};
use crate::folders::{dir, exe_dir};
use crate::i18n::{self, tr, trf};
use crate::question::Question;
use crate::settings::{Extra, Settings};
use crate::themes::{self, Theme};
use crate::themeview::ThemeDoc;
use crate::{icons, session};

pub use jobs::Job;
pub use tabs::{menu_entry, menu_entry_colored, menu_item, sub_menu};
pub use window::{COMPACT_WIDTH, WINDOW_MIN_SIZE, WINDOW_SIZE, Window};

/// The height of a tab.
const TAB_HEIGHT: f32 = 32.0;
/// The title of a tab is cut at this width.
const TAB_TITLE_WIDTH: f32 = 200.0;
/// The parts of a tab from left to right: the padding, the icon, the dot of unsaved changes,
/// the gap after the title and the close button.
const TAB_PADDING: f32 = 12.0;
const TAB_ICON: f32 = 18.0;
const TAB_DOT: f32 = 10.0;
const TAB_GAP: f32 = 8.0;
const TAB_CLOSE: f32 = 20.0;
/// The width the navigation row keeps for the command bar until it is measured.
const TOOLS_WIDTH: f32 = 320.0;
/// The width of the search field, open and closed.
const SEARCH_OPEN_WIDTH: f32 = 240.0;
const SEARCH_CLOSED_WIDTH: f32 = 34.0;
/// The width of the star, the margin of the breadcrumb and its least room.
const STAR_WIDTH: f32 = 34.0;
const CRUMBS_MARGIN: f32 = 32.0;
const CRUMBS_MIN: f32 = 80.0;

/// A frame of a bar: the card colour with a margin.
fn bar_frame(p: &crate::style::Palette, margin: egui::Margin) -> egui::Frame {
    egui::Frame::new().fill(p.card).inner_margin(margin)
}

/// The frames of the bars of the chrome: the tab row, the command bar, the navigation row, the
/// status bar.
fn frames(ui: &egui::Ui) -> (egui::Frame, egui::Frame, egui::Frame, egui::Frame) {
    let p = crate::style::palette(ui);
    (
        egui::Frame::new().fill(p.bg).inner_margin(egui::Margin {
            left: 6,
            right: 6,
            top: 4,
            bottom: 0,
        }),
        bar_frame(&p, egui::Margin::symmetric(8, 4)),
        bar_frame(&p, egui::Margin::symmetric(8, 4)),
        bar_frame(&p, egui::Margin::symmetric(12, 3)),
    )
}

/// The settings tab.
#[derive(Default)]
pub struct SettingsDoc {
    /// The section shown: 0 is General, the next ones the sections of the program.
    pub section: usize,
}

/// A document of the window itself.
pub enum ShellDoc {
    Settings(SettingsDoc),
    /// The editor of a theme.
    Theme(Box<ThemeDoc>),
}

impl Document for ShellDoc {
    fn title(&self) -> String {
        match self {
            ShellDoc::Settings(_) => tr("Settings").to_owned(),
            ShellDoc::Theme(t) => t.title(),
        }
    }

    fn dirty(&self) -> bool {
        matches!(self, ShellDoc::Theme(t) if t.dirty)
    }

    fn path(&self) -> Option<&Path> {
        match self {
            ShellDoc::Settings(_) => None,
            ShellDoc::Theme(t) => t.theme.path.as_deref(),
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            ShellDoc::Settings(_) => icons::GEAR,
            ShellDoc::Theme(_) => icons::PALETTE,
        }
    }

    fn has_tools(&self) -> bool {
        matches!(self, ShellDoc::Theme(_))
    }

    fn draft_key(&self) -> Option<String> {
        match self {
            ShellDoc::Theme(t) => Some(t.draft.key.clone()),
            ShellDoc::Settings(_) => None,
        }
    }

    /// The draft of a theme with unsaved changes: the theme and its id, the kind `ww-theme`.
    fn draft_due(&mut self, all: bool) -> Option<crate::doc::Draft> {
        let ShellDoc::Theme(t) = self else {
            return None;
        };
        if !t.draft.ready(t.dirty, all) {
            return None;
        }
        let document = serde_json::json!({
            "kind": THEME_DRAFT,
            "id": t.theme.id,
            "theme": serde_json::to_value(&t.theme).unwrap_or_default(),
        });
        t.draft.offer(t.theme.path.clone(), document)
    }
}

/// The kind of the draft of a theme.
pub const THEME_DRAFT: &str = "ww-theme";

/// The navigation row of a tab: the history, the breadcrumb, the star.
pub struct Nav<T> {
    /// Back and Forward: whether each is enabled, and their acts.
    pub history: Option<(bool, bool, T, T)>,
    /// The parts of the breadcrumb, each with what a click does.
    pub crumbs: Vec<(String, Option<T>)>,
    /// The star: whether it is set, and the act that toggles it.
    pub star: Option<(bool, T)>,
}

/// The head of the message bar of a tab.
pub struct Messages<T> {
    /// Whether an error is among the messages (the icon).
    pub error: bool,
    pub summary: String,
    /// The act that clears the messages.
    pub clear: T,
}

/// The acts of the window itself. A program's act type holds them in one variant.
pub enum ShellAct<A: App> {
    Activate(usize),
    MoveTab(usize, usize),
    /// Closes a tab; `true` discards its unsaved changes.
    CloseTab(usize, bool),
    New,
    Open,
    OpenPath(PathBuf),
    /// Opens a file by its name and its bytes: one the browser hands over, which has no path.
    OpenBytes(String, Vec<u8>),
    Save,
    SaveAs,
    ForgetRecent,
    OpenSettings,
    /// The settings at a section: 0 is General.
    OpenSection(usize),
    EditTheme(String),
    OpenThemeFile,
    UseTheme(String),
    ToggleMessages,
    /// The search of the tab was cleared: the selection comes back into view.
    SearchCleared,
    /// The selection came into view.
    Revealed,
    /// The selection moved with the keys.
    Follow,
    Message(String),
    ShowInFolder(PathBuf),
    /// A link of a page that is not a web address: the program opens what it names.
    OpenLink(String),
    Exit,
    /// Saves a tab, then closes it when the save left no changes (the question of a tab with
    /// changes).
    SaveAndClose(usize),
    /// The window About over the tabs.
    About,
    /// Folds (true) or opens the panel of the work in the background.
    FoldJobs(bool),
    /// Opens a link outside the program (the web, a file of another kind), after its question.
    OpenWeb(String),
    Ask(Box<Question<A::Act>>),
    /// The second answer of the question over the window.
    AnswerAlt,
    /// The answer to the question over the window: yes or no.
    Answer(bool),
}

/// A program on the window: its documents, its acts, its settings and what it draws.
pub trait App: Sized + 'static {
    /// The documents of the tabs.
    type Doc: Document;
    /// The acts of the program, which hold the acts of the window in one variant.
    type Act: From<ShellAct<Self>>;
    /// The fields of the program in the settings file.
    type Extra: Extra;

    /// The name of the program: the title of the window, the name of its log.
    const NAME: &'static str;
    /// The version of the program with its commit, for the first line of the log.
    const BUILD: &'static str = env!("CARGO_PKG_VERSION");

    /// The name of the program at run time: `NAME`, or the name of an edition of it.
    fn name() -> &'static str {
        Self::NAME
    }

    /// Whether the settings offer the editor of a theme; off, the themes of the folder are only
    /// chosen.
    fn edits_themes() -> bool {
        true
    }

    /// Whether the settings offer the fonts of the theme shown (the families and the size), saved
    /// in its file; a program without the editor of themes may keep them, so the size can grow.
    fn chooses_fonts() -> bool {
        true
    }
    /// The hint of the button that opens a new tab.
    const NEW_HINT: &'static str;

    /// The translations the program embeds: the language code and the file `ui.json`.
    fn languages() -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// The themes the program embeds besides Light and Dark: the id and the file.
    fn themes() -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// The language of the system, for the setting that follows it.
    fn system_language() -> Option<String> {
        None
    }

    /// A document of the window as a document of the program.
    fn shell_doc(doc: ShellDoc) -> Self::Doc;

    /// An act of the program that is an act of the window, else the act itself.
    fn shell_act(act: Self::Act) -> Result<ShellAct<Self>, Self::Act>;

    /// When the window is made (the tests too): the first document.
    fn started(&mut self, _shell: &mut Shell<Self>) {}

    /// Before the session is restored, at a start of the program: the files of the program
    /// next to the executable.
    fn load(&mut self, _shell: &mut Shell<Self>) {}

    /// After the tabs of the last session and the drafts are back.
    fn restored(&mut self, _shell: &mut Shell<Self>) {}

    /// When the window closes, before the files of the tabs are remembered: what the program
    /// keeps in its settings for the next start beyond them.
    fn closing(&mut self, _shell: &mut Shell<Self>) {}

    /// Opens a new document (the button + and Ctrl+N).
    fn new_document(&mut self, shell: &mut Shell<Self>);

    /// Opens a file in a tab: of the dialog, of a drop on the window, of the recent files, of
    /// the last session.
    fn open_path(&mut self, shell: &mut Shell<Self>, path: PathBuf);

    /// The dialog of Open with the filters of the program.
    #[cfg(not(target_arch = "wasm32"))]
    fn open_dialog(&self, shell: &Shell<Self>) -> rfd::FileDialog;

    /// The kinds of files Open offers in a browser: a name and its extensions each; none for
    /// any file.
    #[cfg(target_arch = "wasm32")]
    fn open_filters(&self, _shell: &Shell<Self>) -> Vec<(String, Vec<String>)> {
        Vec::new()
    }

    /// Opens a file by its name and its bytes: one the browser hands over (its dialog, a drop on
    /// the page), which has no path.
    fn open_bytes(&mut self, _shell: &mut Shell<Self>, _name: &str, _bytes: Vec<u8>) {}

    /// Saves the active document; `ask` asks for the file.
    fn save(&mut self, shell: &mut Shell<Self>, ask: bool);

    /// Opens a draft of the last session; `false` when the draft is not one of the program.
    fn restore_draft(
        &mut self,
        _shell: &mut Shell<Self>,
        _key: String,
        _path: Option<PathBuf>,
        _document: serde_json::Value,
    ) -> bool {
        false
    }

    /// Opens a page or a file a link names: a `.md` or `.txt` file, a file without an extension
    /// or a page of the program (the window asks before any other link and opens it outside).
    fn open_link(&mut self, _shell: &mut Shell<Self>, _link: &str) {}

    /// The lines of the window About under the name of the program: the version, the settings
    /// file, what the window draws with; a program adds its own.
    fn about(&self, shell: &Shell<Self>) -> Vec<String> {
        shell.about_lines(Self::BUILD)
    }

    /// The search of the active tab was cleared.
    fn search_cleared(&mut self, _shell: &mut Shell<Self>) {}

    /// The settings tab was opened or brought to the front: what its sections list is read
    /// again.
    fn settings_opened(&mut self, _shell: &mut Shell<Self>) {}

    /// Draws the active document (the window draws its own).
    fn draw(&mut self, shell: &mut Shell<Self>, ui: &mut egui::Ui, acts: &mut Vec<Self::Act>);

    /// The command bar of the active document.
    fn commands(&self, shell: &Shell<Self>) -> Vec<Item<Self::Act>>;

    /// The menu a button of the command bar opens, by its number.
    fn menu_popup(
        &self,
        _shell: &Shell<Self>,
        _menu: usize,
        _ui: &mut egui::Ui,
        _acts: &mut Vec<Self::Act>,
    ) {
    }

    /// The items of the menu button of the tab row.
    fn menu(&self, shell: &Shell<Self>, ui: &mut egui::Ui, acts: &mut Vec<Self::Act>);

    /// The navigation row of the active document, for a document that has one.
    fn nav(&self, _shell: &Shell<Self>) -> Option<Nav<Self::Act>> {
        None
    }

    /// The head of the message bar, when the active document has messages.
    fn messages(&self, _shell: &Shell<Self>) -> Option<Messages<Self::Act>> {
        None
    }

    /// The messages themselves, in the open message bar.
    fn messages_body(&self, _shell: &Shell<Self>, _ui: &mut egui::Ui, _acts: &mut Vec<Self::Act>) {}

    /// The texts at the left of the status bar (the state of the document, the catalog).
    fn status_left(&self, _shell: &Shell<Self>) -> Vec<String> {
        Vec::new()
    }

    /// What the program draws at the right of the status bar, before the message.
    fn status_right(&self, _shell: &Shell<Self>, _ui: &mut egui::Ui) {}

    /// The work that runs in the background: the panel of the work over the window shows it,
    /// and a spinner in the status bar while there is any.
    fn jobs(&self, _shell: &Shell<Self>) -> Vec<Job<Self::Act>> {
        Vec::new()
    }

    /// The sections of the settings tab after General, by their titles.
    fn settings_sections(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// More of the General section, in its card of the window.
    fn settings_general(&mut self, _shell: &mut Shell<Self>, _ui: &mut egui::Ui) {}

    /// A section of the program in the settings tab, by its number in `settings_sections`.
    fn settings_section(
        &mut self,
        _shell: &mut Shell<Self>,
        _section: usize,
        _ui: &mut egui::Ui,
        _acts: &mut Vec<Self::Act>,
    ) {
    }

    /// The keys of the program beyond the ones of the window.
    fn shortcuts(&self, _shell: &Shell<Self>, _input: &mut egui::InputState) -> Vec<Self::Act> {
        Vec::new()
    }

    /// Runs an act of the program.
    fn apply(&mut self, shell: &mut Shell<Self>, act: Self::Act);

    /// The windows of the program over the tabs, drawn after the question of the window.
    fn dialogs(
        &mut self,
        _shell: &mut Shell<Self>,
        _ctx: &egui::Context,
        _acts: &mut Vec<Self::Act>,
    ) {
    }

    /// After the acts of a frame: the polls of the work in the background.
    fn after_frame(&mut self, _shell: &mut Shell<Self>, _ctx: &egui::Context) {}
}

/// The colours given to the title bar last: dark, the background, the text, the border.
#[cfg(not(target_arch = "wasm32"))]
type TitleColors = (bool, [u8; 3], [u8; 3], [u8; 3]);

/// The files the browser handed over, shared with the tasks of the page that read them.
#[cfg(target_arch = "wasm32")]
type Picked = std::rc::Rc<std::cell::RefCell<Vec<(String, Vec<u8>)>>>;

/// The state of the window: the tabs and everything around them.
pub struct Shell<A: App> {
    pub docs: Vec<A::Doc>,
    pub active: usize,
    pub settings: Settings<A::Extra>,
    pub settings_dirty: bool,
    /// The file the settings are written to (`Settings::load` names it).
    pub settings_path: PathBuf,
    /// The recent searches of every tab.
    pub search_history: Vec<String>,
    /// A question over the window and the act a yes runs.
    pub question: Option<Question<A::Act>>,
    /// Whether the message bar shows its list or one line.
    pub messages_open: bool,
    pub status: String,
    /// The file the message of the status bar names, which its button shows in its folder while
    /// the message stays.
    pub shown_file: Option<(String, PathBuf)>,
    /// The themes of the folder and the embedded ones, read at the start, when the settings
    /// open and after a theme is saved.
    pub themes: Vec<Theme>,
    applied_theme: Option<(Theme, Theme, egui::ThemePreference)>,
    #[cfg(not(target_arch = "wasm32"))]
    title_colors: Option<TitleColors>,
    last_title: String,
    pub exit: bool,
    /// The window About is open.
    pub about_open: bool,
    /// The panel of the work in the background is folded to its spinner.
    pub jobs_folded: bool,
    /// A link outside the program the user agreed to open, handed to the system after the acts.
    pub open_web: Option<String>,
    pub drafts_dir: PathBuf,
    pub logs_dir: PathBuf,
    /// What the window draws with (`renderer::graphics_text`), empty without a window.
    pub graphics: String,
    /// Acts requested by views that run without the list of the frame.
    pub pending: Vec<A::Act>,
    last_logged: String,
    /// The files the browser handed over (its dialog, a drop on the page), by their names and
    /// bytes, opened at the next frame.
    #[cfg(target_arch = "wasm32")]
    pub picked: Picked,
    /// The context of the window, so that a file that arrives wakes the frame.
    #[cfg(target_arch = "wasm32")]
    pub(crate) ctx: Option<egui::Context>,
}

impl<A: App> Shell<A> {
    pub fn new(settings: Settings<A::Extra>) -> Self {
        i18n::install(A::languages());
        themes::install(A::themes());
        let search_history = settings.search_history.clone();
        Self {
            docs: Vec::new(),
            active: 0,
            settings,
            settings_dirty: false,
            settings_path: Settings::<A::Extra>::candidates(A::name()).remove(0),
            search_history,
            question: None,
            messages_open: true,
            status: "Ready".to_owned(),
            shown_file: None,
            themes: themes::themes(&dir("themes")),
            applied_theme: None,
            #[cfg(not(target_arch = "wasm32"))]
            title_colors: None,
            last_title: String::new(),
            exit: false,
            about_open: false,
            jobs_folded: false,
            open_web: None,
            drafts_dir: exe_dir().join("drafts"),
            logs_dir: exe_dir().join("logs"),
            graphics: String::new(),
            pending: Vec::new(),
            last_logged: String::new(),
            #[cfg(target_arch = "wasm32")]
            picked: Default::default(),
            #[cfg(target_arch = "wasm32")]
            ctx: None,
        }
    }

    /// The name of the log file of the program.
    fn log_name() -> String {
        A::name().to_lowercase()
    }

    pub fn active_doc(&self) -> Option<&A::Doc> {
        self.docs.get(self.active)
    }

    pub fn active_doc_mut(&mut self) -> Option<&mut A::Doc> {
        self.docs.get_mut(self.active)
    }

    /// The search of the active tab, `None` for a tab without one.
    pub fn active_search(&mut self) -> Option<&mut Search> {
        self.docs.get_mut(self.active)?.search()
    }

    /// Whether the active tab has commands in the command bar.
    pub fn has_tools(&self) -> bool {
        self.active_doc().is_none_or(Document::has_tools)
    }

    /// Whether the active tab has a navigation row.
    pub fn has_nav(&self) -> bool {
        self.active_doc().is_some_and(Document::has_nav)
    }

    /// Whether the active tab has something to search.
    pub fn has_search(&self) -> bool {
        !self.search_hint_now().is_empty()
    }

    /// The hint of the search field of the active tab.
    pub fn search_hint_now(&self) -> &'static str {
        self.active_doc().map_or("", Document::search_hint)
    }

    /// Brings the tab the predicate accepts to the front, making it when there is none, and
    /// gives its index.
    pub fn show_or_open(
        &mut self,
        find: impl Fn(&A::Doc) -> bool,
        make: impl FnOnce() -> A::Doc,
    ) -> usize {
        let i = match self.docs.iter().position(find) {
            Some(i) => i,
            None => {
                self.docs.push(make());
                self.docs.len() - 1
            }
        };
        self.active = i;
        i
    }

    /// Brings the first tab the predicate accepts to the front; `false` when there is none.
    pub fn focus_tab(&mut self, find: impl Fn(&A::Doc) -> bool) -> bool {
        match self.docs.iter().position(find) {
            Some(i) => {
                self.active = i;
                true
            }
            None => false,
        }
    }

    /// Opens a document in a new tab in front.
    pub fn open_tab(&mut self, doc: A::Doc) {
        self.docs.push(doc);
        self.active = self.docs.len() - 1;
    }

    /// Opens a file in a new tab in front, unless a tab has it: `same` finds that tab, `load`
    /// reads the file and `make` wraps what it read.
    pub fn open_file<T, E: std::fmt::Display>(
        &mut self,
        path: &Path,
        same: impl Fn(&A::Doc) -> bool,
        load: impl FnOnce(&Path) -> Result<T, E>,
        make: impl FnOnce(T, PathBuf) -> A::Doc,
    ) {
        if self.focus_tab(same) {
            return;
        }
        match load(path) {
            Ok(file) => {
                self.open_tab(make(file, path.to_path_buf()));
                self.opened(path);
            }
            Err(e) => self.open_failed(e),
        }
    }

    /// Moves a tab to another place of the row; the tab in front stays in front.
    pub fn move_tab(&mut self, from: usize, to: usize) {
        let n = self.docs.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let doc = self.docs.remove(from);
        self.docs.insert(to, doc);
        self.active = if self.active == from {
            to
        } else if from < self.active && self.active <= to {
            self.active - 1
        } else if to <= self.active && self.active < from {
            self.active + 1
        } else {
            self.active
        };
    }

    /// Closes a tab; `force` discards its unsaved changes, else a dirty tab stays with a message.
    pub fn close_tab(&mut self, i: usize, force: bool) {
        if i >= self.docs.len() {
            return;
        }
        if self.docs[i].dirty() && !force {
            let question = self.close_question(i);
            self.ask(question);
            return;
        }
        if let Some(key) = self.docs[i].draft_key() {
            session::remove_draft(&self.drafts_dir, &key);
        }
        self.docs.remove(i);
        if self.active >= self.docs.len() {
            self.active = self.docs.len().saturating_sub(1);
        }
    }

    /// Names a saved file in the status bar, with the button that shows it in its folder.
    pub fn saved(&mut self, path: &Path) {
        self.status = trf("Saved {path}", &[("path", &path.display())]);
        self.shown_file = Some((self.status.clone(), path.to_path_buf()));
    }

    /// Names an opened file in the status bar and in the recent files.
    pub fn opened(&mut self, path: &Path) {
        self.status = trf("Opened {path}", &[("path", &path.display())]);
        self.remember_recent(path);
    }

    pub fn save_failed(&mut self, e: impl std::fmt::Display) {
        self.status = trf("Save failed: {e}", &[("e", &e)]);
    }

    pub fn open_failed(&mut self, e: impl std::fmt::Display) {
        self.status = trf("Open failed: {e}", &[("e", &e)]);
    }

    pub fn remember_recent(&mut self, path: &Path) {
        self.settings.remember(path);
        self.settings_dirty = true;
    }

    /// Shows a question over the window; a yes runs its act, a no drops it.
    pub fn ask(&mut self, question: Question<A::Act>) {
        self.question = Some(question);
    }

    /// The file a link of the active page names, with the heading after `#`: a relative link
    /// is taken from the folder of the page, or of the program for a page without a file.
    /// `None` for a link to a heading of the page itself.
    pub fn linked_file(&self, link: &str) -> Option<(PathBuf, Option<String>)> {
        let (file, anchor) = match link.split_once('#') {
            Some((f, a)) => (f, Some(a.to_owned())),
            None => (link, None),
        };
        let file = file.strip_prefix("file:///").unwrap_or(file);
        if file.is_empty() {
            return None;
        }
        let path = PathBuf::from(file.replace("%20", " "));
        if path.is_absolute() {
            return Some((path, anchor));
        }
        let base = self
            .active_doc()
            .and_then(Document::path)
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(exe_dir);
        Some((base.join(path), anchor))
    }

    /// Opens the draft of a document of the window (the editor of a theme); `false` when the
    /// draft is not one of the window.
    pub fn restore_shell_draft(
        &mut self,
        key: &str,
        path: Option<PathBuf>,
        document: &serde_json::Value,
    ) -> bool {
        if document.get("kind").and_then(|k| k.as_str()) != Some(THEME_DRAFT) {
            return false;
        }
        let Some(mut theme) = document
            .get("theme")
            .and_then(|t| serde_json::from_value::<Theme>(t.clone()).ok())
        else {
            return false;
        };
        theme.id = document
            .get("id")
            .and_then(|i| i.as_str())
            .unwrap_or_default()
            .to_owned();
        theme.path = path.filter(|p| p.exists());
        let mut doc = ThemeDoc::new(theme);
        doc.dirty = true;
        doc.draft = crate::doc::DraftState::with_key(key.to_owned());
        self.docs.push(A::shell_doc(ShellDoc::Theme(Box::new(doc))));
        true
    }

    /// The question before a tab with unsaved changes closes: save them, close without them, or
    /// keep the tab.
    pub fn close_question(&self, i: usize) -> Question<A::Act> {
        let title = self.docs.get(i).map(Document::title).unwrap_or_default();
        Question::new(
            tr("Unsaved changes"),
            trf(
                "Save the changes of {title} before the tab closes?",
                &[("title", &title)],
            ),
            tr("Save"),
            A::Act::from(ShellAct::SaveAndClose(i)),
        )
        .or(tr("Don't save"), A::Act::from(ShellAct::CloseTab(i, true)))
    }

    /// The lines of About every program has: the version, the settings file, the renderer.
    pub fn about_lines(&self, build: &str) -> Vec<String> {
        let mut lines = vec![
            trf("Version {version}", &[("version", &build)]),
            trf(
                "Settings file: {path}",
                &[("path", &self.settings_path.display())],
            ),
        ];
        if !self.graphics.is_empty() {
            lines.push(trf(
                "Draws now with {graphics}",
                &[("graphics", &self.graphics)],
            ));
        }
        lines
    }

    /// The question before the recent files are forgotten.
    pub fn forget_recent_act(&self) -> A::Act {
        ShellAct::Ask(Box::new(Question::new(
            tr("Forget the recent files"),
            tr("Forget the list of the recent files? The files stay where they are."),
            tr("Forget"),
            A::Act::from(ShellAct::ForgetRecent),
        )))
        .into()
    }

    /// Brings the settings tab to the front, opening it when there is none, and gives it.
    pub fn open_settings(&mut self) -> &mut SettingsDoc {
        let i = self.show_or_open(
            |d| matches!(d.shell_doc(), Some(ShellDoc::Settings(_))),
            || A::shell_doc(ShellDoc::Settings(SettingsDoc::default())),
        );
        match self.docs[i].shell_doc_mut() {
            Some(ShellDoc::Settings(doc)) => doc,
            _ => unreachable!("the tab at {i} is the settings"),
        }
    }

    pub fn window_title(&self) -> String {
        match self.active_doc() {
            Some(doc) => format!("{} - {}", doc.title(), A::name()),
            None => A::name().to_owned(),
        }
    }

    /// The language the window shows: the setting, or the language of the system when it says
    /// to follow it, English when there is no file for it.
    pub fn effective_language(&self) -> String {
        let wanted = if self.settings.language.is_empty() {
            A::system_language().unwrap_or_else(|| "en".to_owned())
        } else {
            self.settings.language.clone()
        };
        if i18n::languages(&dir("lang"))
            .iter()
            .any(|l| l.code == wanted)
        {
            wanted
        } else {
            "en".to_owned()
        }
    }

    pub fn apply_language(&mut self) {
        let code = self.effective_language();
        i18n::set_language(&code, &dir("lang"));
    }

    pub fn persist_settings(&mut self) {
        if !self.settings_dirty {
            return;
        }
        self.settings_dirty = false;
        if let Err(e) = self.settings.save_to(&self.settings_path) {
            self.status = trf("Settings not saved: {e}", &[("e", &e)]);
        }
    }

    /// Writes every new status line to the log.
    pub fn log_status(&mut self) {
        if self.status != self.last_logged {
            session::log(&self.logs_dir, &Self::log_name(), "INFO", &self.status);
            self.last_logged = self.status.clone();
        }
    }

    /// A line of the log of the program.
    pub fn log(&self, level: &str, text: &str) {
        session::log(&self.logs_dir, &Self::log_name(), level, text);
    }

    /// Remembers the files of the open tabs for the next start.
    pub fn remember_tabs(&mut self) {
        let mut tabs = Vec::new();
        let mut active = 0;
        for (i, doc) in self.docs.iter().enumerate() {
            if let Some(p) = doc.path() {
                if i == self.active {
                    active = tabs.len();
                }
                tabs.push(p.to_path_buf());
            }
        }
        if tabs != self.settings.open_tabs || active != self.settings.active_tab {
            self.settings.open_tabs = tabs;
            self.settings.active_tab = active;
            self.settings_dirty = true;
        }
    }

    /// Writes the drafts the documents give: a while after their last change, and all of them
    /// when the window closes.
    pub fn write_drafts(&mut self, all: bool) {
        let mut failed = None;
        for doc in &mut self.docs {
            let Some(draft) = doc.draft_due(all) else {
                continue;
            };
            if let Err(e) = session::write_draft(
                &self.drafts_dir,
                &draft.key,
                draft.path.as_deref(),
                &draft.document,
            ) {
                failed = Some(e);
            }
        }
        if let Some(e) = failed {
            self.status = trf("Draft not saved: {e}", &[("e", &e)]);
        }
    }

    /// The selection of the active tab came into view, or moved with the keys.
    pub fn scrolled(&mut self, how: Scrolled) {
        if let Some(doc) = self.active_doc_mut() {
            doc.scrolled(how);
        }
    }
}

/// The files of a browser: they come by their bytes, from the dialog of the page or a drop on
/// it, and reach the program at the next frame as `ShellAct::OpenBytes`.
#[cfg(target_arch = "wasm32")]
impl<A: App> Shell<A> {
    /// Asks the browser for a file with its dialog, filtered by kinds (a name and its
    /// extensions each).
    pub fn pick_file(&self, filters: Vec<(String, Vec<String>)>) {
        let slot = self.picked.clone();
        let ctx = self.ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let mut dialog = rfd::AsyncFileDialog::new().set_title(tr("Open"));
            for (name, extensions) in &filters {
                dialog = dialog.add_filter(name, extensions.as_slice());
            }
            let Some(file) = dialog.pick_file().await else {
                return;
            };
            let bytes = file.read().await;
            slot.borrow_mut().push((file.file_name(), bytes));
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Reads a file dropped on the page.
    pub fn read_dropped(&self, file: egui::DroppedFileHandle) {
        let slot = self.picked.clone();
        let ctx = self.ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let name = file.path().display().to_string();
            match file.bytes_async().await {
                Ok(bytes) => {
                    slot.borrow_mut().push((name, bytes));
                    if let Some(ctx) = ctx {
                        ctx.request_repaint();
                    }
                }
                Err(e) => web_sys::console::error_1(&format!("{name}: {e}").into()),
            }
        });
    }

    /// The files that arrived since the last frame.
    pub fn take_picked(&self) -> Vec<(String, Vec<u8>)> {
        std::mem::take(&mut *self.picked.borrow_mut())
    }
}
