//! The tab of the server in a window of W# (the feature `ui`): the configuration file it edits
//! (`mcp/<name>.wwmcp`, saved at every change), the HTTP server of the window, its mode, the
//! token and the configurations for clients, the sources of tools with their options, their
//! groups and their tools each on or off, a call of a tool by hand, the journal of the calls that
//! the search of the tab filters and the details of the call selected. The program keeps the
//! panel (`McpPanel`), draws it with `draw` and answers its acts with `McpPanel::apply`; the
//! server lives as long as the panel, its transport while it runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use serde_json::{Map, Value, json};
use ww::eframe::egui;
use ww::i18n::{tr, tr_any, trf};
use ww::{Search, list, matches_words, style, tree};

use crate::config::{Config, EXTENSION, Transport};
use crate::http::HttpServer;
use crate::tools::{Caller, Choice, Mode, Outcome, ToolDef};
use crate::{Entry, Server, log};

/// An act of the tab. `OpenLog` is the program's: it opens the log in a tab; `McpPanel::apply`
/// answers the others.
#[derive(Debug, Clone, PartialEq)]
pub enum McpAct {
    Start,
    Stop,
    RenewToken,
    /// Another configuration file, chosen in a dialog.
    OpenConfig,
    /// The configuration saved into another file, which the tab edits from then on.
    SaveAs,
    OpenLog(PathBuf),
    /// Runs the tool of the card of a call by hand.
    Call,
}

/// The call of a tool by hand: the tool, its arguments as JSON, and the answer.
#[derive(Default)]
pub struct Trial {
    pub tool: String,
    pub args: String,
    pub answer: String,
    pub failed: bool,
    waiting: Option<Receiver<Outcome>>,
}

/// The configuration the tab edits, the server, its transport and the state of its journal.
pub struct McpPanel {
    /// The configuration file, saved at every change.
    pub path: PathBuf,
    /// The configuration as last saved, so that a change is written once. None while the file
    /// of `path` is one that did not read: nothing is written over it, whatever changes (a
    /// token made at a start too), until the user saves the configuration under a name
    /// (`save_as`) or opens another file.
    pub saved: Option<Config>,
    pub config: Config,
    /// The tools of the program and of its plugins, with the journal of their calls.
    pub server: Arc<Server>,
    /// The HTTP transport of the window while it runs.
    pub http: Option<HttpServer>,
    pub error: String,
    /// The command of the program in the configurations for clients, and the key of
    /// the server in them.
    pub program: String,
    /// The selected call of the journal, by its place in the journal.
    pub journal_sel: Option<usize>,
    /// The selection moved with the keys: it comes into view at the next frame.
    pub journal_follow: bool,
    /// The search was cleared: the selection comes to the middle of the list.
    pub journal_reveal: bool,
    /// Whether the panel of the details of the selected call is open.
    pub detail_open: bool,
    pub trial: Trial,
}

impl McpPanel {
    /// The tab of a configuration file, read when it exists: one that cannot be read leaves the
    /// defaults, its reason, and is not written over; one that does not exist is written at the
    /// first change.
    pub fn new(path: PathBuf, program: &str, server: Arc<Server>) -> Self {
        let mut panel = Self {
            path,
            saved: Some(Config::default()),
            config: Config::default(),
            server,
            http: None,
            error: String::new(),
            program: program.to_owned(),
            journal_sel: None,
            journal_follow: false,
            journal_reveal: false,
            detail_open: true,
            trial: Trial::default(),
        };
        if panel.path.exists() {
            match Config::load(&panel.path) {
                Ok(config) => {
                    panel.saved = Some(config.clone());
                    panel.config = config;
                }
                Err(e) => {
                    panel.error = trf("The MCP configuration was not read: {e}", &[("e", &e)]);
                    panel.saved = None;
                }
            }
        }
        panel.server.set_config(panel.config.clone());
        panel
    }

    pub fn running(&self) -> bool {
        self.http.is_some()
    }

    /// Another server (new sources, a new start of the program's own): it takes the
    /// configuration of the tab. A running transport keeps the server it has.
    pub fn set_server(&mut self, server: Arc<Server>) {
        server.set_config(self.config.clone());
        self.server = server;
    }

    /// The configuration of the tab into the server, once it changed.
    pub fn sync(&self) {
        if self.server.config() != self.config {
            self.server.set_config(self.config.clone());
        }
    }

    /// Writes the configuration once it changed; a failure is shown once. A file that did not
    /// read is never written over: the configuration lives in the tab until it is saved under
    /// a name.
    pub fn persist(&mut self) {
        self.sync();
        if self
            .saved
            .as_ref()
            .is_none_or(|saved| *saved == self.config)
        {
            return;
        }
        if let Err(e) = self.config.save(&self.path) {
            self.error = trf("The MCP configuration was not saved: {e}", &[("e", &e)]);
        }
        self.saved = Some(self.config.clone());
    }

    /// Starts the HTTP transport of the window on the server; the status line it gives.
    pub fn start(&mut self) -> String {
        if self.config.token.is_empty() {
            match crate::new_token() {
                Ok(t) => self.config.token = t,
                Err(e) => {
                    self.error = e;
                    return tr("The server did not start").to_owned();
                }
            }
        }
        self.sync();
        let started = HttpServer::start(
            self.server.clone(),
            self.config.port,
            self.config.token.clone(),
        );
        match started {
            Ok(http) => {
                let line = trf("The MCP server runs on {url}", &[("url", &http.url())]);
                self.http = Some(http);
                // The reason a file did not read stays shown while that file is not written.
                if self.saved.is_some() {
                    self.error.clear();
                }
                line
            }
            Err(e) => {
                self.error = trf("The server did not start: {e}", &[("e", &e)]);
                self.error.clone()
            }
        }
    }

    pub fn stop(&mut self) -> String {
        self.http = None;
        tr("The MCP server stopped").to_owned()
    }

    /// Opens a configuration file in the tab; a running server keeps the one it has.
    pub fn open_config(&mut self, path: &Path) -> Result<(), String> {
        if self.running() {
            let text = tr("Stop the MCP server before you open another configuration");
            return Err(text.to_owned());
        }
        let config = Config::load(path)?;
        self.saved = Some(config.clone());
        self.config = config;
        self.path = path.to_path_buf();
        self.error.clear();
        self.sync();
        Ok(())
    }

    /// Saves the configuration into another file, which the tab edits from then on. It is the
    /// word of the user for a file that did not read: saved under its own name, it is replaced.
    pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
        self.config.save(path)?;
        self.path = path.to_path_buf();
        if self.saved.is_none() {
            self.error.clear();
        }
        self.saved = Some(self.config.clone());
        Ok(())
    }

    /// Answers an act of the tab but `OpenLog`; the line of the status bar it gives, if any.
    pub fn apply(&mut self, act: McpAct) -> Option<String> {
        match act {
            McpAct::Start => Some(self.start()),
            McpAct::Stop => Some(self.stop()),
            McpAct::RenewToken => {
                match crate::new_token() {
                    Ok(t) => self.config.token = t,
                    Err(e) => self.error = e,
                }
                None
            }
            McpAct::OpenConfig => {
                let path = self.dialog().pick_file()?;
                Some(match self.open_config(&path) {
                    Ok(()) => trf("Opened {path}", &[("path", &path.display())]),
                    Err(e) => trf("Not opened: {e}", &[("e", &e)]),
                })
            }
            McpAct::SaveAs => {
                let mut path = self
                    .dialog()
                    .set_title(tr("Save the MCP configuration as"))
                    .save_file()?;
                if path.extension().is_none() {
                    path.set_extension(EXTENSION);
                }
                Some(match self.save_as(&path) {
                    Ok(()) => trf("Saved {path}", &[("path", &path.display())]),
                    Err(e) => trf(
                        "{path} was not saved: {why}",
                        &[("path", &path.display()), ("why", &e)],
                    ),
                })
            }
            McpAct::Call => {
                self.call();
                None
            }
            McpAct::OpenLog(_) => None,
        }
    }

    fn dialog(&self) -> ww::rfd::FileDialog {
        let mut dialog =
            ww::rfd::FileDialog::new().add_filter(tr("MCP configurations"), &[EXTENSION]);
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
            dialog = dialog.set_directory(dir);
        }
        dialog
    }

    /// Runs the tool of the card on a thread of its own, as the program: a tool of the window
    /// answers at a next frame of it.
    pub fn call(&mut self) {
        if self.trial.waiting.is_some() || self.trial.tool.is_empty() {
            return;
        }
        let args: Value = if self.trial.args.trim().is_empty() {
            json!({})
        } else {
            match serde_json::from_str(&self.trial.args) {
                Ok(v) => v,
                Err(e) => {
                    self.trial.answer = trf("The arguments are not JSON: {e}", &[("e", &e)]);
                    self.trial.failed = true;
                    return;
                }
            }
        };
        let (send, receive) = channel();
        let server = self.server.clone();
        let tool = self.trial.tool.clone();
        let spawned = std::thread::Builder::new()
            .name("mcp-call".to_owned())
            .spawn(move || {
                let _ = send.send(server.call(&tool, &args, &Caller::Program));
            });
        match spawned {
            Ok(_) => {
                self.trial.waiting = Some(receive);
                self.trial.answer.clear();
            }
            Err(e) => {
                self.trial.answer = e.to_string();
                self.trial.failed = true;
            }
        }
    }

    /// Takes the answer of a call by hand once it came.
    pub fn poll(&mut self) {
        let Some(waiting) = &self.trial.waiting else {
            return;
        };
        if let Ok(outcome) = waiting.try_recv() {
            self.trial.waiting = None;
            match outcome {
                Ok(value) => {
                    self.trial.answer = serde_json::to_string_pretty(&value).unwrap_or_default();
                    self.trial.failed = false;
                }
                Err(e) => {
                    self.trial.answer = e.to_string();
                    self.trial.failed = true;
                }
            }
        }
    }
}

/// The configuration a client needs for the HTTP server of the window.
pub fn http_config(name: &str, url: &str, token: &str) -> String {
    let value = json!({
        "mcpServers": {
            name: {
                "type": "http",
                "url": url,
                "headers": { "Authorization": format!("Bearer {token}") },
            }
        }
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

/// The configuration a client needs to start the program itself on stdio with a configuration
/// file.
pub fn stdio_config(name: &str, exe: &str, config: &str) -> String {
    let value = json!({
        "mcpServers": {
            name: {
                "command": exe,
                "args": ["--mcp", "stdio", "--config", config],
            }
        }
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

fn masked(token: &str) -> String {
    if token.len() < 8 {
        return "***".to_owned();
    }
    format!("{}...{}", &token[..4], &token[token.len() - 4..])
}

/// The arguments of a tool to fill in: every property of its schema with an empty value of its
/// type.
pub fn skeleton(tool: &ToolDef) -> String {
    let mut out = Map::new();
    if let Some(properties) = tool.schema.get("properties").and_then(Value::as_object) {
        for (name, property) in properties {
            let empty = match property.get("type").and_then(Value::as_str) {
                Some("number" | "integer") => json!(0),
                Some("boolean") => json!(false),
                Some("array") => json!([]),
                Some("object") => json!({}),
                _ => json!(""),
            };
            out.insert(name.clone(), empty);
        }
    }
    serde_json::to_string_pretty(&Value::Object(out)).unwrap_or_default()
}

/// The whole tab: the settings above, the journal of the calls below them, the details of the
/// call selected at the right. `extra` draws the cards of the program after the one of the
/// server, `places` the buttons of what a call names in its details.
pub fn draw(
    ui: &mut egui::Ui,
    panel: &mut McpPanel,
    search: &Search,
    acts: &mut Vec<McpAct>,
    extra: &mut dyn FnMut(&mut egui::Ui),
    places: &mut dyn FnMut(&mut egui::Ui, &Entry),
) {
    panel.poll();
    panel.sync();
    let p = style::palette(ui);
    let max = (ui.available_height() - 120.0).max(160.0);
    egui::Panel::top("mcp-settings")
        .resizable(true)
        .default_size(440.0)
        .size_range(120.0..=max)
        .frame(egui::Frame::new().fill(p.bg))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| settings_view(ui, panel, acts, extra));
        });
    if panel.detail_open {
        egui::Panel::right("mcp-detail")
            .resizable(true)
            .default_size(380.0)
            .frame(
                egui::Frame::new()
                    .fill(p.bg)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| detail_view(ui, panel, places));
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(p.card))
        .show(ui, |ui| journal_view(ui, panel, search));
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));
}

/// The settings: the server, the cards of the program, the sources of tools, a call by hand.
pub fn settings_view(
    ui: &mut egui::Ui,
    panel: &mut McpPanel,
    acts: &mut Vec<McpAct>,
    extra: &mut dyn FnMut(&mut egui::Ui),
) {
    style::header(ui, |ui| {
        ui.label(style::strong(ui, tr("MCP server"), 18.0));
        ui.label(style::muted(ui, tr("AI agents use the tools of the program and of its plugins through this server. Here you decide what they may do: the mode, the sources of tools, their options and each tool."), 12.0));
    });
    style::block(ui, |ui| server_card(ui, panel, acts));
    extra(ui);
    style::block(ui, |ui| sources_card(ui, panel));
    style::block(ui, |ui| trial_card(ui, panel, acts));
    ui.add_space(12.0);
}

/// The server: its state, the file, the port, the mode, the log, the token and the
/// configurations for clients.
fn server_card(ui: &mut egui::Ui, panel: &mut McpPanel, acts: &mut Vec<McpAct>) {
    style::section_title(ui, tr("Server"));
    ui.add_space(4.0);
    let running = panel.running();
    egui::Grid::new("mcp-grid")
        .num_columns(2)
        .spacing([16.0, 8.0])
        .show(ui, |ui| {
            ui.label(tr("Configuration"));
            ui.horizontal_wrapped(|ui| {
                ui.monospace(panel.path.display().to_string());
                if ui
                    .add_enabled(!running, egui::Button::new(tr("Open...")))
                    .clicked()
                {
                    acts.push(McpAct::OpenConfig);
                }
                if ui.button(tr("Save as...")).clicked() {
                    acts.push(McpAct::SaveAs);
                }
            });
            ui.end_row();

            ui.label(tr("State"));
            match &panel.http {
                Some(http) => {
                    let client = panel.server.client();
                    let client = if client.is_empty() {
                        "-".to_owned()
                    } else {
                        client
                    };
                    ui.label(trf(
                        "Running on {url}; client {client}",
                        &[("url", &http.url()), ("client", &client)],
                    ));
                }
                None => {
                    ui.label(tr("Stopped"));
                }
            }
            ui.end_row();

            ui.label(tr("Port"));
            ui.add_enabled(
                !running,
                egui::DragValue::new(&mut panel.config.port).range(1024..=65535),
            );
            ui.end_row();

            ui.label(tr("Mode"));
            ui.horizontal_wrapped(|ui| {
                let mode = &mut panel.config.mode;
                ui.radio_value(mode, Mode::Read, tr("read: nothing changes"));
                ui.radio_value(
                    mode,
                    Mode::Edit,
                    tr("edit: what the program holds changes in memory"),
                );
                ui.radio_value(mode, Mode::Files, tr("files: files are written too"));
            });
            ui.end_row();

            ui.label(tr("Log"));
            ui.checkbox(
                &mut panel.config.detailed_log,
                tr("Detailed: every request and answer whole, without passwords, keys and tokens"),
            );
            ui.end_row();

            ui.label(tr("Without the window"));
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(&mut panel.config.transport, Transport::Http, "HTTP");
                ui.radio_value(&mut panel.config.transport, Transport::Stdio, "stdio");
                ui.weak(trf(
                    "for {program} --mcp with this file",
                    &[("program", &panel.program)],
                ));
            });
            ui.end_row();

            ui.label(tr("Token"));
            ui.horizontal_wrapped(|ui| {
                ui.monospace(masked(&panel.config.token));
                if ui.button(tr("Copy the client configuration")).clicked() {
                    let url = format!("http://127.0.0.1:{}/mcp", panel.config.port);
                    let text = http_config(&panel.program, &url, &panel.config.token);
                    ui.ctx().copy_text(text);
                }
                if ui
                    .add_enabled(!running, egui::Button::new(tr("Renew")))
                    .clicked()
                {
                    acts.push(McpAct::RenewToken);
                }
            });
            ui.end_row();
        });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if running {
            if ui.button(tr("Stop")).clicked() {
                acts.push(McpAct::Stop);
            }
        } else if ui.button(tr("Start")).clicked() {
            acts.push(McpAct::Start);
        }
        if ui
            .button(tr("Copy the stdio configuration"))
            .on_hover_text(tr(
                "For clients that start the program themselves; it needs no running server",
            ))
            .clicked()
        {
            let exe = std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| format!("{}.exe", panel.program));
            let file = panel.path.display().to_string();
            ui.ctx()
                .copy_text(stdio_config(&panel.program, &exe, &file));
        }
    });
    if !panel.error.is_empty() {
        ui.colored_label(ui.visuals().warn_fg_color, &panel.error);
    }
    ui.weak(tr("The copied configuration holds the token; the clipboard history of Windows keeps what is copied."));
    if let Some(file) = log::file_of_today() {
        ui.horizontal_wrapped(|ui| {
            ui.weak(trf(
                "The log of the server: {path}",
                &[("path", &file.display())],
            ));
            if file.exists() && ui.small_button(tr("Open the log")).clicked() {
                acts.push(McpAct::OpenLog(file.clone()));
            }
        });
    }
}

/// The sources of tools: each on or off, its options, its groups and its tools.
fn sources_card(ui: &mut egui::Ui, panel: &mut McpPanel) {
    style::section_title(ui, tr("Tools"));
    ui.add_space(2.0);
    ui.weak(tr("A source, a group or a tool that is off is not listed for a client and does not run for it or for a plugin; the options of a source decide what its tools may do."));
    ui.add_space(4.0);
    let listed = panel.server.tools();
    for source in panel.server.sources().to_vec() {
        let id = source.id();
        let tools: Vec<&ToolDef> = listed
            .iter()
            .filter(|l| l.source == id)
            .map(|l| &l.tool)
            .collect();
        let options = source.options();
        if tools.is_empty() && options.is_empty() {
            continue;
        }
        let on_count = tools.iter().filter(|t| panel.config.is_on(&id, t)).count();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut on = !panel.config.source(&id).off;
            if ui.checkbox(&mut on, "").changed() {
                panel.config.source_mut(&id).off = !on;
            }
            ui.label(style::strong(ui, tr_any(&source.title()), 14.0));
            ui.weak(trf(
                "{on} of {all} tools on",
                &[("on", &on_count), ("all", &tools.len())],
            ));
        });
        let off = panel.config.source(&id).off;
        egui::CollapsingHeader::new(tr("Options and tools"))
            .id_salt(("mcp-source", &id))
            .default_open(false)
            .show(ui, |ui| {
                ui.add_enabled_ui(!off, |ui| {
                    options_grid(ui, panel, &id, &options);
                    groups(ui, panel, &id, &tools);
                });
            });
    }
}

/// The options of a source: a choice of its values, or a text.
fn options_grid(ui: &mut egui::Ui, panel: &mut McpPanel, id: &str, options: &[Choice]) {
    if options.is_empty() {
        return;
    }
    egui::Grid::new(("mcp-options", id))
        .num_columns(2)
        .spacing([16.0, 6.0])
        .show(ui, |ui| {
            for choice in options {
                ui.label(tr_any(&choice.title));
                let current = panel
                    .config
                    .source(id)
                    .options
                    .get(&choice.key)
                    .cloned()
                    .unwrap_or_else(|| choice.default.clone());
                ui.vertical(|ui| {
                    let mut value = current.clone();
                    if choice.is_text() {
                        ui.add(egui::TextEdit::singleline(&mut value).desired_width(260.0));
                    } else {
                        egui::ComboBox::from_id_salt(("mcp-option", id, &choice.key))
                            .selected_text(tr_any(&choice.label(&current)))
                            .show_ui(ui, |ui| {
                                for v in &choice.values {
                                    let label = tr_any(&choice.label(v));
                                    ui.selectable_value(&mut value, v.clone(), label);
                                }
                            });
                    }
                    if !choice.help.is_empty() {
                        ui.weak(tr_any(&choice.help));
                    }
                    if value != current {
                        let _ = panel.config.set_option(choice, id, &value);
                    }
                });
                ui.end_row();
            }
        });
    ui.add_space(4.0);
}

/// The tools of a source by their groups, each group and each tool with its switch.
fn groups(ui: &mut egui::Ui, panel: &mut McpPanel, id: &str, tools: &[&ToolDef]) {
    let mut names: Vec<&str> = Vec::new();
    for t in tools {
        if !names.contains(&t.group.as_str()) {
            names.push(&t.group);
        }
    }
    for group in names {
        let mut on = !panel.config.source(id).groups_off.contains(group);
        let shown = if group.is_empty() {
            tr("Other tools").to_owned()
        } else {
            tr_any(group)
        };
        if ui
            .checkbox(&mut on, egui::RichText::new(shown).strong())
            .changed()
        {
            let set = &mut panel.config.source_mut(id).groups_off;
            if on {
                set.remove(group);
            } else {
                set.insert(group.to_owned());
            }
        }
        ui.indent(("mcp-group", id, group), |ui| {
            ui.add_enabled_ui(on, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for tool in tools.iter().filter(|t| t.group == group) {
                        let mut tool_on = !panel.config.source(id).tools_off.contains(&tool.name);
                        let mut text = egui::RichText::new(&tool.name).monospace();
                        if tool.mode != Mode::Read {
                            text = egui::RichText::new(format!(
                                "{} ({})",
                                tool.name,
                                tool.mode.name()
                            ))
                            .monospace();
                        }
                        let mut hint = tool.description.clone();
                        for need in &tool.needs {
                            hint.push('\n');
                            hint.push_str(&trf(
                                "Needs the option {option}: {value}",
                                &[("option", &need.option), ("value", &need.at_least)],
                            ));
                        }
                        let changed = ui
                            .checkbox(&mut tool_on, text)
                            .on_hover_text(hint)
                            .changed();
                        if changed {
                            let set = &mut panel.config.source_mut(id).tools_off;
                            if tool_on {
                                set.remove(&tool.name);
                            } else {
                                set.insert(tool.name.clone());
                            }
                        }
                    }
                });
            });
        });
    }
}

/// A call of a tool by hand: the tool, its arguments, the answer.
fn trial_card(ui: &mut egui::Ui, panel: &mut McpPanel, acts: &mut Vec<McpAct>) {
    style::section_title(ui, tr("Call a tool"));
    ui.add_space(2.0);
    ui.weak(tr("The program calls the tool by its name, as a script or a plugin would: the switches above are for the clients and the plugins, so any tool runs. The call goes into the journal."));
    ui.add_space(4.0);
    let listed = panel.server.tools();
    let before = panel.trial.tool.clone();
    ui.horizontal_wrapped(|ui| {
        let shown = if panel.trial.tool.is_empty() {
            tr("Choose a tool").to_owned()
        } else {
            panel.trial.tool.clone()
        };
        egui::ComboBox::from_id_salt("mcp-trial-tool")
            .selected_text(shown)
            .width(260.0)
            .show_ui(ui, |ui| {
                for l in &listed {
                    let label = format!("{}  ({})", l.tool.name, l.source);
                    ui.selectable_value(&mut panel.trial.tool, l.tool.name.clone(), label)
                        .on_hover_text(&l.tool.description);
                }
            });
        let waiting = panel.trial.waiting.is_some();
        let can = !panel.trial.tool.is_empty() && !waiting;
        if ui.add_enabled(can, egui::Button::new(tr("Call"))).clicked() {
            acts.push(McpAct::Call);
        }
        if waiting {
            ui.spinner();
        }
    });
    if panel.trial.tool != before
        && let Some(l) = listed.iter().find(|l| l.tool.name == panel.trial.tool)
    {
        panel.trial.args = skeleton(&l.tool);
        panel.trial.answer.clear();
    }
    if let Some(l) = listed.iter().find(|l| l.tool.name == panel.trial.tool) {
        ui.weak(&l.tool.description);
    }
    ui.label(tr("Arguments"));
    ui.add(
        egui::TextEdit::multiline(&mut panel.trial.args)
            .font(egui::TextStyle::Monospace)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
    if !panel.trial.answer.is_empty() {
        ui.label(tr("Answer"));
        if panel.trial.failed {
            ui.colored_label(ui.visuals().warn_fg_color, &panel.trial.answer);
        } else {
            let mut text = panel.trial.answer.clone();
            ui.add(
                egui::TextEdit::multiline(&mut text)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
        }
    }
}

/// The journal of the server, the newest call first, as the search of the tab filters it.
pub fn journal_view(ui: &mut egui::Ui, panel: &mut McpPanel, search: &Search) {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(18, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                style::section_title(ui, tr("Journal"));
                let label = if panel.detail_open {
                    tr("Hide the details")
                } else {
                    tr("Show the details")
                };
                if ui.small_button(label).clicked() {
                    panel.detail_open = !panel.detail_open;
                }
            });
        });
    let words = search.words();
    let entries = panel.server.journal();
    let rows: Vec<list::Row<usize>> = entries
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, e)| {
            let hay = format!(
                "{} {} {} {} {}",
                e.time, e.client, e.method, e.detail, e.result
            );
            matches_words(&hay, &words)
        })
        .map(|(i, e)| {
            let result = list::Cell::new(e.result.name());
            list::Row {
                key: i,
                cells: vec![
                    list::Cell::new(e.time.get(11..19).unwrap_or(&e.time)).muted(),
                    list::Cell::new(&e.client).muted(),
                    list::Cell::new(&e.method).strong(),
                    list::Cell::new(&e.detail).muted(),
                    if e.result.is_ok() {
                        result
                    } else {
                        result.warn()
                    },
                ],
                button: None,
            }
        })
        .collect();
    let columns = [
        list::Column::new(tr("Time"), 90.0),
        list::Column::new(tr("Client"), 150.0),
        list::Column::new(tr("Call"), 170.0),
        list::Column::new(tr("Arguments"), 320.0),
        list::Column::new(tr("Result"), 120.0),
    ];
    let selected = panel
        .journal_sel
        .and_then(|s| rows.iter().position(|r| r.key == s));
    let scroll = if panel.journal_reveal {
        tree::Scroll::Center
    } else if panel.journal_follow {
        tree::Scroll::Near
    } else {
        tree::Scroll::None
    };
    let out = list::show(
        ui,
        egui::Id::new("mcp-journal"),
        tr("Journal"),
        &columns,
        &rows,
        selected,
        scroll,
    );
    for event in out.events {
        if let list::Event::Select(i) | list::Event::Open(i) = event {
            panel.journal_sel = Some(i);
        }
    }
    if out.moved {
        panel.journal_follow = true;
    } else if out.scrolled {
        panel.journal_follow = false;
        panel.journal_reveal = false;
    }
}

/// The details of the selected call: its arguments and answer, and the buttons of the program
/// for what it names (`places`).
pub fn detail_view(
    ui: &mut egui::Ui,
    panel: &McpPanel,
    places: &mut dyn FnMut(&mut egui::Ui, &Entry),
) {
    style::section_title(ui, tr("Details of the call"));
    ui.add_space(4.0);
    let Some(entry) = panel
        .journal_sel
        .and_then(|i| panel.server.journal_entry(i))
    else {
        ui.weak(tr("Select a call in the journal."));
        return;
    };
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            egui::Grid::new("mcp-detail")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    let rows = [
                        (tr("Call"), entry.method.clone()),
                        (tr("Time"), entry.time.clone()),
                        (tr("Client"), entry.client.clone()),
                        (tr("Result"), entry.result.to_string()),
                        (tr("Duration"), trf("{ms} ms", &[("ms", &entry.ms)])),
                    ];
                    for (name, value) in rows {
                        ui.weak(name);
                        ui.label(value);
                        ui.end_row();
                    }
                });
            places(ui, &entry);
            for (title, value) in [
                (tr("Arguments"), &entry.arguments),
                (tr("Answer"), &entry.answer),
            ] {
                ui.add_space(8.0);
                style::section_title(ui, title);
                let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
                ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .interactive(true),
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Info;

    #[test]
    fn configurations_for_clients_are_json() {
        let http: Value =
            serde_json::from_str(&http_config("demo", "http://127.0.0.1:1/mcp", "T")).unwrap();
        assert_eq!(
            http["mcpServers"]["demo"]["headers"]["Authorization"],
            "Bearer T"
        );
        let config = stdio_config("demo", "C:\\W\\demo.exe", "C:\\W\\mcp\\a.wwmcp");
        let stdio: Value = serde_json::from_str(&config).unwrap();
        assert_eq!(stdio["mcpServers"]["demo"]["command"], "C:\\W\\demo.exe");
        assert_eq!(
            stdio["mcpServers"]["demo"]["args"][3],
            "C:\\W\\mcp\\a.wwmcp"
        );
        assert_eq!(masked("abcdefghijkl"), "abcd...ijkl");
        let tool = ToolDef::new(
            "t",
            "g",
            "",
            crate::tools::schema(
                json!({ "a": { "type": "string" }, "n": { "type": "integer" } }),
                &[],
            ),
        );
        let args: Value = serde_json::from_str(&skeleton(&tool)).unwrap();
        assert_eq!(args, json!({ "a": "", "n": 0 }));
    }

    #[test]
    fn a_panel_keeps_its_configuration_in_its_file() {
        let dir = std::env::temp_dir().join(format!("wwmcp-ui-{}", std::process::id()));
        let path = Config::default_path(&dir);
        let server = Arc::new(Server::new(Info::default(), Config::default()));
        let mut panel = McpPanel::new(path.clone(), "demo", server.clone());
        assert!(panel.error.is_empty());
        panel.persist();
        assert!(!path.exists(), "nothing changed, nothing written");
        panel.config.mode = Mode::Files;
        panel.persist();
        assert_eq!(Config::load(&path).unwrap().mode, Mode::Files);
        assert_eq!(server.config().mode, Mode::Files);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_did_not_read_is_not_written_over() {
        let dir = std::env::temp_dir().join(format!("wwmcp-ui-unread-{}", std::process::id()));
        let path = Config::default_path(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let broken = "{ not json: the work of the user";
        std::fs::write(&path, broken).unwrap();
        let server = Arc::new(Server::new(Info::default(), Config::default()));
        let mut panel = McpPanel::new(path.clone(), "demo", server.clone());
        assert!(!panel.error.is_empty());
        assert!(panel.saved.is_none());
        // A change of the tab and the token a start makes reach the server, never the file.
        panel.config.mode = Mode::Edit;
        panel.persist();
        assert_eq!(server.config().mode, Mode::Edit);
        panel.config.port = 0;
        let line = panel.start();
        assert!(panel.running(), "{line}");
        assert!(!panel.config.token.is_empty());
        panel.persist();
        panel.stop();
        panel.config.port = crate::config::DEFAULT_PORT;
        panel.persist();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
        assert!(!panel.error.is_empty(), "the reason stays shown");
        // Saved under a name, the configuration is written there at every change from then on.
        let named = dir.join("named.wwmcp");
        panel.save_as(&named).unwrap();
        assert!(panel.error.is_empty());
        panel.config.mode = Mode::Files;
        panel.persist();
        assert_eq!(Config::load(&named).unwrap().mode, Mode::Files);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
        // Another file opened is written as ever.
        std::fs::write(&path, broken).unwrap();
        let mut again = McpPanel::new(path.clone(), "demo", server);
        again.open_config(&named).unwrap();
        again.config.mode = Mode::Read;
        again.persist();
        assert_eq!(Config::load(&named).unwrap().mode, Mode::Read);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
