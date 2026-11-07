//! The window: the frame around the active document, the acts of the window and the keyboard.

use std::path::Path;

use eframe::egui::{self, Key, Modifiers};

use super::{App, Shell, ShellAct, ShellDoc};
use crate::doc::{Document, Scrolled};
use crate::folders::show_in_folder;
use crate::i18n::{tr, trf};
use crate::question::Question;
use crate::{keys, session, widgets};

/// Below this width of the window the command bar shows icons only, in the navigation row.
pub const COMPACT_WIDTH: f32 = 1100.0;
/// The size of the window at the first start, and the least size it takes.
pub const WINDOW_SIZE: [f32; 2] = [1280.0, 800.0];
pub const WINDOW_MIN_SIZE: [f32; 2] = [900.0, 560.0];

/// The window of a program: its state and the program.
pub struct Window<A: App> {
    pub shell: Shell<A>,
    pub app: A,
    /// The first frame fitted the window to its monitor.
    placed: bool,
}

/// The window fitted to the monitor it opened on: moved inside it, and shrunk when it is larger.
/// A window maximized at the last close, or whose state the first frame does not know yet, stays
/// as it is: a size sent to a maximized window restored it and maximized it again, a blink at
/// the start (item 4 of the list of 30.10.2026). Whether the window is placed; not yet while its
/// rectangles or its state are unknown.
fn fit_window(ctx: &egui::Context, was_maximized: bool) -> bool {
    let (outer, inner, monitor, maximized) = ctx.input(|i| {
        let v = i.viewport();
        (v.outer_rect, v.inner_rect, v.monitor_size, v.maximized)
    });
    let (Some(outer), Some(inner), Some(monitor), Some(maximized)) =
        (outer, inner, monitor, maximized)
    else {
        return false;
    };
    if was_maximized || maximized || monitor.x <= 0.0 || monitor.y <= 0.0 {
        return true;
    }
    let frame = outer.size() - inner.size();
    let room = monitor - frame;
    let least = egui::Vec2::from(WINDOW_MIN_SIZE).min(room);
    let size = inner.size().min(room).max(least);
    let pos = egui::pos2(
        outer
            .left()
            .clamp(0.0, (monitor.x - size.x - frame.x).max(0.0)),
        outer
            .top()
            .clamp(0.0, (monitor.y - size.y - frame.y).max(0.0)),
    );
    if size != inner.size() {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    }
    if pos != outer.min {
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
    }
    true
}

impl<A: App> Window<A> {
    pub fn new(mut app: A, settings: crate::Settings<A::Extra>) -> Self {
        let mut shell = Shell::new(settings);
        app.started(&mut shell);
        Self {
            shell,
            app,
            placed: false,
        }
    }

    /// Where the window is, into the settings for the next start; a maximized window keeps the
    /// place it had before.
    fn remember_window(&mut self, ctx: &egui::Context) {
        let (outer, inner, maximized) = ctx.input(|i| {
            let v = i.viewport();
            (v.outer_rect, v.inner_rect, v.maximized.unwrap_or(false))
        });
        let (Some(outer), Some(inner)) = (outer, inner) else {
            return;
        };
        let now = crate::WindowPlace {
            x: outer.left(),
            y: outer.top(),
            width: inner.width(),
            height: inner.height(),
            maximized,
        };
        let place = match (&self.shell.settings.window, maximized) {
            (Some(before), true) => crate::WindowPlace {
                maximized: true,
                ..before.clone()
            },
            _ => now,
        };
        if self.shell.settings.window.as_ref() != Some(&place) {
            self.shell.settings.window = Some(place);
            self.shell.settings_dirty = true;
        }
    }

    /// Runs the acts of a frame: the ones of the window here, the others by the program.
    pub fn apply(&mut self, acts: Vec<A::Act>) {
        for act in acts {
            match A::shell_act(act) {
                Ok(shell) => self.apply_shell(shell),
                Err(act) => self.app.apply(&mut self.shell, act),
            }
        }
    }

    fn apply_shell(&mut self, act: ShellAct<A>) {
        let shell = &mut self.shell;
        match act {
            ShellAct::Activate(i) => {
                if i < shell.docs.len() {
                    shell.active = i;
                }
            }
            ShellAct::MoveTab(from, to) => shell.move_tab(from, to),
            ShellAct::CloseTab(i, force) => shell.close_tab(i, force),
            ShellAct::New => self.app.new_document(shell),
            ShellAct::Open => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(path) = self.app.open_dialog(shell).pick_file() {
                    self.app.open_path(shell, path);
                }
                // The dialog of the page: the file comes to a later frame by its bytes.
                #[cfg(target_arch = "wasm32")]
                shell.pick_file(self.app.open_filters(shell));
            }
            ShellAct::OpenPath(path) => self.app.open_path(shell, path),
            ShellAct::OpenBytes(name, bytes) => self.app.open_bytes(shell, &name, bytes),
            ShellAct::Save => self.save(false),
            ShellAct::SaveAs => self.save(true),
            ShellAct::ForgetRecent => {
                shell.settings.recent.clear();
                shell.settings_dirty = true;
            }
            ShellAct::OpenSettings => {
                shell.themes = crate::themes::themes(&crate::folders::dir("themes"));
                shell.open_settings();
                self.app.settings_opened(shell);
            }
            ShellAct::OpenSection(section) => {
                shell.open_settings().section = section;
                self.app.settings_opened(shell);
            }
            ShellAct::EditTheme(id) => {
                let theme = crate::themes::find(&shell.themes, &id);
                shell.edit_theme(theme);
            }
            // A browser has no folder of themes: the item is not offered there.
            #[cfg(not(target_arch = "wasm32"))]
            ShellAct::OpenThemeFile => {
                let dir = crate::folders::dir("themes");
                let dialog = rfd::FileDialog::new()
                    .add_filter(tr("Theme files"), &[crate::themes::EXTENSION, "json"])
                    .set_title(tr("Open a theme file"));
                if let Some(path) = widgets::in_folder(dialog, &dir).pick_file() {
                    shell.open_theme_path(path);
                }
            }
            #[cfg(target_arch = "wasm32")]
            ShellAct::OpenThemeFile => {}
            ShellAct::UseTheme(id) => shell.use_theme(id),
            ShellAct::ToggleMessages => shell.messages_open = !shell.messages_open,
            ShellAct::SearchCleared => self.app.search_cleared(shell),
            ShellAct::Revealed => shell.scrolled(Scrolled::Revealed),
            ShellAct::Follow => shell.scrolled(Scrolled::Follow),
            ShellAct::Message(text) => shell.status = text,
            ShellAct::ShowInFolder(path) => {
                // A folder of the program that is not there yet is made first.
                if !path.exists() && path.extension().is_none() {
                    let _ = std::fs::create_dir_all(&path);
                }
                if let Err(e) = show_in_folder(&path) {
                    shell.status = trf("The folder was not opened: {e}", &[("e", &e)]);
                }
            }
            ShellAct::OpenLink(link) => {
                if opens_inside(&link) {
                    self.app.open_link(shell, &link);
                } else {
                    // A file of another kind goes to the browser as a file address.
                    let target = match shell.linked_file(&link) {
                        Some((path, _)) if !is_web(&link) => {
                            let path = path.display().to_string().replace('\\', "/");
                            format!("file:///{}", path.trim_start_matches('/'))
                        }
                        _ => link,
                    };
                    shell.ask(Question::new(
                        tr("Open a link"),
                        trf("Open {link} in the browser?", &[("link", &target)]),
                        tr("Open"),
                        A::Act::from(ShellAct::OpenWeb(target)),
                    ));
                }
            }
            ShellAct::OpenWeb(link) => shell.open_web = Some(link),
            ShellAct::About => shell.about_open = true,
            ShellAct::FoldJobs(folded) => shell.jobs_folded = folded,
            ShellAct::SaveAndClose(i) => {
                if i < self.shell.docs.len() {
                    self.shell.active = i;
                    self.save(false);
                    if self.shell.docs.get(i).is_some_and(|d| !d.dirty()) {
                        self.shell.close_tab(i, true);
                    }
                }
            }
            ShellAct::AnswerAlt => {
                if let Some(question) = self.shell.question.take()
                    && let Some((_, act)) = question.alt
                {
                    self.apply(vec![act]);
                }
            }
            ShellAct::Exit => shell.exit = true,
            ShellAct::Ask(question) => shell.ask(*question),
            ShellAct::Answer(yes) => {
                if let Some(question) = shell.question.take()
                    && yes
                {
                    self.apply(vec![question.act]);
                }
            }
        }
    }

    /// Saves the active document: a document of the window by the window, the others by the
    /// program.
    fn save(&mut self, ask: bool) {
        match self.shell.active_doc().and_then(Document::shell_doc) {
            Some(ShellDoc::Theme(_)) => self.shell.save_theme(ask),
            Some(ShellDoc::Settings(_)) => {}
            None => self.app.save(&mut self.shell, ask),
        }
        // A document saved has no draft any more.
        if let Some(doc) = self.shell.active_doc()
            && !doc.dirty()
            && let Some(key) = doc.draft_key()
        {
            session::remove_draft(&self.shell.drafts_dir, &key);
        }
    }

    /// The keys of the window and of the program, the question answered with Escape, the files
    /// dropped on the window.
    fn shortcuts(&mut self, ctx: &egui::Context) -> Vec<A::Act> {
        let mut acts: Vec<A::Act> = Vec::new();
        let active = self.shell.active;
        let count = self.shell.docs.len();
        let asking = self.shell.question.is_some();
        let about = self.shell.about_open;
        let mut close_about = false;
        let mut focus_search = false;
        ctx.input_mut(|i| {
            if i.consume_shortcut(&keys::FIND.shortcut()) {
                focus_search = true;
            }
            if i.consume_shortcut(&keys::SAVE_AS.shortcut()) {
                acts.push(ShellAct::SaveAs.into());
            }
            if i.consume_shortcut(&keys::SAVE.shortcut()) {
                acts.push(ShellAct::Save.into());
            }
            if i.consume_shortcut(&keys::OPEN.shortcut()) {
                acts.push(ShellAct::Open.into());
            }
            if i.consume_shortcut(&keys::NEW.shortcut()) {
                acts.push(ShellAct::New.into());
            }
            if i.consume_shortcut(&keys::CLOSE.shortcut()) {
                acts.push(ShellAct::CloseTab(active, false).into());
            }
            if i.consume_shortcut(&keys::SETTINGS.shortcut()) {
                acts.push(ShellAct::OpenSettings.into());
            }
            // The keys with Shift first: a shortcut matches with an extra Shift too, and in a
            // browser the moves are the switches with Shift.
            if active > 0 && i.consume_shortcut(&keys::TAB_LEFT.shortcut()) {
                acts.push(ShellAct::MoveTab(active, active - 1).into());
            }
            if active + 1 < count && i.consume_shortcut(&keys::TAB_RIGHT.shortcut()) {
                acts.push(ShellAct::MoveTab(active, active + 1).into());
            }
            if count > 1 && i.consume_shortcut(&keys::PREVIOUS_TAB.shortcut()) {
                acts.push(ShellAct::Activate((active + count - 1) % count).into());
            }
            if count > 1 && i.consume_shortcut(&keys::NEXT_TAB.shortcut()) {
                acts.push(ShellAct::Activate((active + 1) % count).into());
            }
            acts.extend(self.app.shortcuts(&self.shell, i));
            if asking && i.consume_key(Modifiers::NONE, Key::Escape) {
                acts.push(ShellAct::Answer(false).into());
            }
            if about && i.consume_key(Modifiers::NONE, Key::Escape) {
                close_about = true;
            }
            for file in &i.raw.dropped_files {
                #[cfg(not(target_arch = "wasm32"))]
                acts.push(ShellAct::OpenPath(file.path().to_path_buf()).into());
                // A browser gives the name and, later, the bytes.
                #[cfg(target_arch = "wasm32")]
                self.shell.read_dropped(file.clone());
            }
        });
        if close_about {
            self.shell.about_open = false;
        }
        if focus_search && let Some(search) = self.shell.active_search() {
            search.open = true;
            search.focus = true;
        }
        acts
    }

    /// The question over the window, when there is one.
    fn ask_view(&self, ctx: &egui::Context, acts: &mut Vec<A::Act>) {
        let Some(ask) = &self.shell.question else {
            return;
        };
        widgets::Modal::new(ask.title.as_str()).show(ctx, |ui| {
            ui.label(ask.text.as_str());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(ask.yes.as_str()).clicked() {
                    acts.push(ShellAct::Answer(true).into());
                }
                if let Some((label, _)) = &ask.alt
                    && ui.button(label.as_str()).clicked()
                {
                    acts.push(ShellAct::AnswerAlt.into());
                }
                if ui.button(tr("Cancel")).clicked() {
                    acts.push(ShellAct::Answer(false).into());
                }
            });
        });
    }

    /// The window About: the name of the program and the lines it gives, over the tabs.
    fn about_view(&mut self, ctx: &egui::Context) {
        if !self.shell.about_open {
            return;
        }
        let lines = self.app.about(&self.shell);
        let mut close = false;
        widgets::Modal::new(tr("About"))
            .id("about")
            .show(ctx, |ui| {
                ui.label(crate::style::strong(ui, A::name(), 18.0));
                ui.add_space(6.0);
                for line in &lines {
                    ui.add(egui::Label::new(line.as_str()).selectable(true));
                }
                ui.add_space(8.0);
                if ui.button(tr("OK")).clicked() {
                    close = true;
                }
            });
        if close {
            self.shell.about_open = false;
        }
    }

    /// The active document: one of the window drawn here, else by the program.
    fn draw_active(&mut self, ui: &mut egui::Ui, acts: &mut Vec<A::Act>) {
        match self.shell.active_doc().and_then(Document::shell_doc) {
            Some(ShellDoc::Settings(_)) => self.settings_view(ui, acts),
            Some(ShellDoc::Theme(_)) => self.shell.theme_view(ui, acts),
            None if self.shell.docs.is_empty() => {
                egui::CentralPanel::default().show(ui, |ui| {
                    let text = "No document is open. Use Menu, New or {keys}.";
                    ui.label(trf(text, &[("keys", &keys::NEW.text)]));
                });
            }
            None => self.app.draw(&mut self.shell, ui, acts),
        }
    }

    /// Opens the tabs of the last close and the drafts of unsaved changes.
    pub fn restore_session(&mut self) {
        let start_tab_untouched = self
            .shell
            .docs
            .first()
            .is_some_and(|d| !d.dirty() && d.path().is_none());
        let before = self.shell.docs.len();
        let mut missing = 0;
        if self.shell.settings.restore_tabs {
            for path in self.shell.settings.open_tabs.clone() {
                if path.exists() {
                    self.app.open_path(&mut self.shell, path);
                } else {
                    missing += 1;
                }
            }
        }
        let restored_tabs = self.shell.docs.len() - before;
        let (drafts, mut broken) = session::read_drafts(&self.shell.drafts_dir);
        let mut restored = 0;
        for (key, draft) in drafts {
            let path = (!draft.path.is_empty()).then(|| Path::new(&draft.path).to_path_buf());
            let shell_draft = self
                .shell
                .restore_shell_draft(&key, path.clone(), &draft.document);
            if shell_draft
                || self
                    .app
                    .restore_draft(&mut self.shell, key.clone(), path, draft.document)
            {
                restored += 1;
            } else {
                broken.push(format!("{key}: not a draft of {}", A::name()));
            }
        }
        if start_tab_untouched && (restored_tabs > 0 || restored > 0) && self.shell.docs.len() > 1 {
            self.shell.docs.remove(0);
        }
        if restored_tabs > 0 {
            self.shell.active = self
                .shell
                .settings
                .active_tab
                .min(self.shell.docs.len().saturating_sub(1));
        }
        if restored > 0 {
            self.shell.active = self.shell.docs.len() - 1;
        }
        let mut notes = Vec::new();
        if restored > 0 {
            notes.push(trf("{n} drafts restored", &[("n", &restored)]));
        }
        if missing > 0 {
            notes.push(trf(
                "{n} files of the last session are missing",
                &[("n", &missing)],
            ));
        }
        if !broken.is_empty() {
            notes.push(trf("{n} drafts could not be read", &[("n", &broken.len())]));
        }
        if !notes.is_empty() {
            self.shell.status = notes.join("; ");
        }
    }
}

/// Whether a link leads outside the files of this PC: a web address or a mail.
fn is_web(link: &str) -> bool {
    let link = link.to_ascii_lowercase();
    (link.contains("://") && !link.starts_with("file:")) || link.starts_with("mailto:")
}

/// Whether a link opens inside the program: a heading, a `.md` or `.txt` file or a file without an
/// extension, not a web address.
fn opens_inside(link: &str) -> bool {
    if is_web(link) {
        return false;
    }
    let link = link.strip_prefix("file:///").unwrap_or(link);
    let file = link.split('#').next().unwrap_or("");
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    match name.rsplit_once('.') {
        None => true,
        Some((_, ext)) => {
            let ext = ext.to_ascii_lowercase();
            ext == "md" || ext == "txt"
        }
    }
}

impl<A: App> eframe::App for Window<A> {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if self.shell.exit {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if !self.placed {
            let was_maximized = self
                .shell
                .settings
                .window
                .as_ref()
                .is_some_and(|w| w.maximized);
            // A few frames to learn the state of the window, then it is left as it is.
            let late = ui.ctx().cumulative_pass_nr() > 10;
            self.placed = fit_window(ui.ctx(), was_maximized) || late;
        }
        self.shell.apply_visuals(ui.ctx());
        #[cfg(not(target_arch = "wasm32"))]
        self.shell.color_title_bar(ui, frame);
        #[cfg(target_arch = "wasm32")]
        {
            let _ = frame;
            self.shell.ctx = Some(ui.ctx().clone());
        }
        let title = self.shell.window_title();
        if title != self.shell.last_title {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.shell.last_title = title;
        }
        let mut acts = self.shortcuts(ui.ctx());
        #[cfg(target_arch = "wasm32")]
        acts.extend(
            self.shell
                .take_picked()
                .into_iter()
                .map(|(name, bytes)| ShellAct::OpenBytes(name, bytes).into()),
        );
        let (tabs_frame, tool_frame, nav_frame, status_frame) = super::frames(ui);
        egui::Panel::top("tabs")
            .frame(tabs_frame)
            .show_separator_line(false)
            .show(ui, |ui| self.tabs_row(ui, &mut acts));
        let compact = ui.available_width() < COMPACT_WIDTH;
        if !compact && self.shell.has_tools() {
            let search_here = !self.shell.has_nav() && self.shell.has_search();
            let hint = self.shell.search_hint_now();
            egui::Panel::top("toolbar")
                .frame(tool_frame)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.toolbar(ui, &mut acts, false);
                        if search_here {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    self.shell.search_field(ui, &mut acts, hint, 0.0);
                                },
                            );
                        }
                    });
                });
        }
        if self.shell.has_nav() || (compact && self.shell.has_tools()) {
            egui::Panel::top("nav")
                .frame(nav_frame)
                .show(ui, |ui| self.nav_row(ui, &mut acts, compact));
        }
        self.messages_panel(ui, &mut acts);
        egui::Panel::bottom("status")
            .frame(status_frame)
            .show(ui, |ui| self.status_bar(ui, &mut acts));
        self.draw_active(ui, &mut acts);
        self.jobs_view(ui.ctx(), &mut acts);
        self.ask_view(ui.ctx(), &mut acts);
        self.about_view(ui.ctx());
        self.app.dialogs(&mut self.shell, ui.ctx(), &mut acts);
        let links: Vec<String> = ui.ctx().output_mut(|o| {
            let mut found = Vec::new();
            o.commands.retain(|c| match c {
                // Every link goes through the window: a page of the program opens inside,
                // any other one is asked first.
                egui::OutputCommand::OpenUrl(u) => {
                    found.push(u.url.clone());
                    false
                }
                _ => true,
            });
            found
        });
        acts.extend(links.into_iter().map(|l| ShellAct::OpenLink(l).into()));
        acts.append(&mut self.shell.pending);
        self.apply(acts);
        if let Some(link) = self.shell.open_web.take() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(link));
        }
        self.app.after_frame(&mut self.shell, ui.ctx());
        let closing = self.shell.exit || ui.ctx().input(|i| i.viewport().close_requested());
        self.shell.write_drafts(closing);
        if closing {
            self.remember_window(ui.ctx());
            self.app.closing(&mut self.shell);
            self.shell.remember_tabs();
        }
        self.shell.persist_settings();
        self.shell.log_status();
    }
}
