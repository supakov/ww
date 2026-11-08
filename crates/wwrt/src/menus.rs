//! The menus a view shares with the window (docs/plugins.md, 6.11): the context menu of a view,
//! the same items in a submenu of the menu of the window, and Open with, the plugins that open
//! the bytes a view offers (`View::offer`) or a file, the ones of their type first.
//!
//! A view draws its context menu on the response of a part of its area with `context` (or with
//! the closure of `ww::tree::show_with_menu` on a row of its tree): its items (`View::context`)
//! and, when it offers bytes, Open with. The window draws the same items with `items` in a submenu
//! of its menu and answers what was picked: an item by `View::act`, a choice of Open with by
//! `open_offered`, whose request (`Request::OpenWith`) opens the bytes in a tab of their own.

use std::path::Path;

use ww::eframe::egui;
use ww::i18n::{tr, trf};
use ww::icons;
use ww::shell::{menu_entry, menu_entry_colored, sub_menu};
use ww::style;

use crate::mime;
use crate::plugin::{Host, Plugins, Request, View};

/// A plugin and a format of it to open some bytes or a file with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// The index of the plugin among the plugins of the window.
    pub plugin: usize,
    /// The name of the format (the menu translates it).
    pub format: &'static str,
    /// The MIME type the plugin opens the bytes or the file as; empty for a format without one.
    pub mime: &'static str,
    /// Whether the type of the bytes or the name of the file leads to this format: shown first,
    /// in the colour of the accent.
    pub found: bool,
}

/// What was picked in the items of the context of a view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    /// An item of `View::context`, by its id.
    Tool(&'static str),
    /// A plugin to open the bytes the view offers with.
    OpenWith(Choice),
}

/// The types a type leads to: itself, then the ones it falls back to (`mime::fallbacks`).
fn wanted(mime: &str) -> Vec<String> {
    std::iter::once(mime::canonical(mime))
        .chain(mime::fallbacks(mime).into_iter().map(mime::canonical))
        .collect()
}

/// The rank of a format among the wanted types: the place of the first it names.
fn rank(format: &crate::plugin::Format, wanted: &[String]) -> Option<usize> {
    wanted
        .iter()
        .position(|w| format.mimes.iter().any(|m| mime::canonical(m) == *w))
}

impl Plugins {
    /// Every format of a MIME type of every plugin that opens bytes, to open some bytes of the
    /// type `mime` with (`None` when it is not known): the formats of that type first, by how
    /// near their type is, then the others in the order of the plugins.
    pub fn choices_for_bytes(&self, mime: Option<&str>) -> Vec<Choice> {
        let wanted = mime.map(wanted).unwrap_or_default();
        let mut found: Vec<(usize, Choice)> = Vec::new();
        let mut other = Vec::new();
        for (plugin, loaded) in self.loaded.iter().enumerate() {
            if !loaded.plugin.opens_bytes() {
                continue;
            }
            for format in loaded.plugin.formats() {
                let Some(first) = format.mimes.first() else {
                    continue;
                };
                let rank = rank(format, &wanted);
                let choice = Choice {
                    plugin,
                    format: format.name,
                    mime: first,
                    found: rank.is_some(),
                };
                match rank {
                    Some(r) => found.push((r, choice)),
                    None => other.push(choice),
                }
            }
        }
        found.sort_by_key(|(r, _)| *r);
        found.into_iter().map(|(_, c)| c).chain(other).collect()
    }

    /// Every format of every plugin, to open a file with: the ones of its extension first, then
    /// the ones of the type of its first bytes (`sniffed`), then the others.
    pub fn choices_for_file(&self, path: &Path, sniffed: Option<&str>) -> Vec<Choice> {
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let wanted = sniffed.map(wanted).unwrap_or_default();
        let mut found: Vec<(usize, Choice)> = Vec::new();
        let mut other = Vec::new();
        for (plugin, loaded) in self.loaded.iter().enumerate() {
            for format in loaded.plugin.formats() {
                let by_name =
                    !extension.is_empty() && format.extensions.contains(&extension.as_str());
                let rank = if by_name {
                    Some(0)
                } else {
                    rank(format, &wanted).map(|r| r + 1)
                };
                // A format without a type opens by the name of a file alone.
                if format.mimes.is_empty() && rank.is_none() {
                    continue;
                }
                let choice = Choice {
                    plugin,
                    format: format.name,
                    mime: format.mimes.first().copied().unwrap_or(""),
                    found: rank.is_some(),
                };
                match rank {
                    Some(r) => found.push((r, choice)),
                    None => other.push(choice),
                }
            }
        }
        found.sort_by_key(|(r, _)| *r);
        found.into_iter().map(|(_, c)| c).chain(other).collect()
    }
}

/// The text of a choice: its format and its plugin.
pub fn label(plugins: &Plugins, choice: &Choice) -> String {
    trf(
        "{format} ({plugin})",
        &[
            ("format", &tr(choice.format)),
            ("plugin", &tr(plugins.loaded[choice.plugin].plugin.name())),
        ],
    )
}

/// The choices as items of a menu: the ones found first, in the colour of the accent, then under
/// a line the others to try; gives the one clicked.
pub fn open_with(ui: &mut egui::Ui, plugins: &Plugins, choices: &[Choice]) -> Option<Choice> {
    let accent = style::palette(ui).accent;
    let mut picked = None;
    let found = choices.iter().filter(|c| c.found);
    let mut any_found = false;
    for choice in found {
        any_found = true;
        let text = label(plugins, choice);
        if menu_entry_colored(ui, icons::CHECK, &text, "", true, Some(accent)) {
            picked = Some(choice.clone());
        }
    }
    let mut first_other = true;
    for choice in choices.iter().filter(|c| !c.found) {
        if first_other {
            if any_found {
                ui.separator();
            }
            ui.weak(tr("Try as"));
            first_other = false;
        }
        if menu_entry(ui, icons::PAGE, &label(plugins, choice), "", true) {
            picked = Some(choice.clone());
        }
    }
    if choices.is_empty() {
        ui.weak(tr("No plugin opens bytes"));
    }
    picked
}

/// The items of the context of a view: its own (`View::context`), then Open with for the bytes
/// it offers, when the window has plugins; gives what was clicked.
pub fn items(ui: &mut egui::Ui, view: &dyn View, plugins: Option<&Plugins>) -> Option<Picked> {
    let mut picked = None;
    let tools = view.context();
    for tool in &tools {
        if menu_entry(ui, tool.icon, tr(tool.label), "", true) {
            picked = Some(Picked::Tool(tool.id));
        }
    }
    if let (Some(offer), Some(plugins)) = (view.offer(), plugins) {
        let choices = plugins.choices_for_bytes(offer.mime.as_deref());
        if !tools.is_empty() {
            ui.separator();
        }
        let text = tr("Open with");
        if let Some(Some(choice)) = sub_menu(ui, icons::GO_TO, text, |ui| {
            ui.weak(&offer.name);
            open_with(ui, plugins, &choices)
        }) {
            picked = Some(Picked::OpenWith(choice));
        }
    }
    picked
}

/// Answers what was picked in the items of a view: an item by the view, a choice of Open with
/// by a request of the host.
pub fn answer(view: &mut dyn View, picked: Picked, host: &mut Host) {
    match picked {
        Picked::Tool(id) => view.act(id, host),
        Picked::OpenWith(choice) => open_offered(view, &choice, host),
    }
}

/// The context menu of a view on the response of a part of its area: its items, answered at
/// once.
pub fn context(response: &egui::Response, view: &mut dyn View, host: &mut Host) {
    let plugins = host.plugins;
    let mut picked = None;
    response.context_menu(|ui| picked = items(ui, &*view, plugins));
    if let Some(picked) = picked {
        answer(view, picked, host);
    }
}

/// The bytes a view offers, asked for in a tab of their own by the plugin of a choice
/// (`Request::OpenWith`), or the reason they are not there.
pub fn open_offered(view: &mut dyn View, choice: &Choice, host: &mut Host) {
    let Some(offer) = view.offer() else {
        return;
    };
    match view.offered() {
        Ok(bytes) => host.requests.push(Request::OpenWith {
            name: offer.name,
            plugin: choice.plugin,
            mime: choice.mime.to_owned(),
            bytes,
        }),
        Err(why) => host.requests.push(Request::Status(trf(
            "{name}: {why}",
            &[("name", &offer.name), ("why", &why)],
        ))),
    }
}
