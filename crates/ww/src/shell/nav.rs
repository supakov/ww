//! The navigation row: the history, the breadcrumb, the star and the search of the tab.

use eframe::egui::{self, Align, Layout, Margin, RichText, Sense};

use super::{
    App, CRUMBS_MARGIN, CRUMBS_MIN, SEARCH_CLOSED_WIDTH, SEARCH_OPEN_WIDTH, STAR_WIDTH, Shell,
    ShellAct, TOOLS_WIDTH, Window,
};
use crate::doc::remember_search;
use crate::i18n::tr;
use crate::{icons, style};

impl<A: App> Window<A> {
    /// The navigation row of the active tab: the history, the breadcrumb (shortened to the first
    /// and the last parts with a menu of the others when the row is narrow), the star, and at the
    /// right end the search and, in a narrow window, the command bar as icons.
    pub(super) fn nav_row(&mut self, ui: &mut egui::Ui, acts: &mut Vec<A::Act>, compact: bool) {
        let p = style::palette(ui);
        let nav = self.app.nav(&self.shell);
        let hint = self.shell.search_hint_now();
        let search_open = self.shell.active_search().is_some_and(|s| s.open);
        let tools = compact && self.shell.has_tools();
        let tools_id = egui::Id::new("nav-tools-width");
        let tools_w = if tools {
            ui.data(|d| d.get_temp::<f32>(tools_id))
                .unwrap_or(TOOLS_WIDTH)
        } else {
            0.0
        };
        // The buttons of the row grow with the text size of the theme.
        let search_w = if hint.is_empty() {
            0.0
        } else if search_open {
            style::at_least(ui, SEARCH_OPEN_WIDTH)
        } else {
            style::at_least(ui, SEARCH_CLOSED_WIDTH)
        };
        let star_width = style::at_least(ui, STAR_WIDTH);
        let tools_height = style::at_least(ui, 30.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let (history, mut crumbs, star) = match nav {
                Some(n) => (n.history, n.crumbs, n.star),
                None => (None, Vec::new(), None),
            };
            if let Some((back, forward, back_act, forward_act)) = history {
                if style::icon_button(ui, icons::BACK, tr("Back (Alt+Left)"), back, None).clicked()
                {
                    acts.push(back_act);
                }
                if style::icon_button(ui, icons::FORWARD, tr("Forward (Alt+Right)"), forward, None)
                    .clicked()
                {
                    acts.push(forward_act);
                }
                ui.add_space(6.0);
            }
            let star_w = if star.is_some() { star_width } else { 0.0 };
            let taken = tools_w + search_w + star_w + CRUMBS_MARGIN;
            let room = (ui.available_width() - taken).max(CRUMBS_MIN);
            let labels: Vec<String> = crumbs.iter().map(|(l, _)| l.clone()).collect();
            let shown = fit_crumbs(ui, &labels, room);
            let hidden: Vec<usize> = (0..crumbs.len())
                .filter(|i| !shown.contains(&Some(*i)))
                .collect();
            let mut clicked = None;
            for (k, entry) in shown.iter().enumerate() {
                if k > 0 {
                    ui.label(style::muted(ui, icons::CRUMB, 13.0));
                }
                match entry {
                    Some(i) => {
                        let label = &labels[*i];
                        let last = *i + 1 == labels.len();
                        let text = if last {
                            style::strong(ui, label.as_str(), 13.0)
                        } else {
                            RichText::new(label.as_str())
                        };
                        let hit = if last && shown.len() == 1 {
                            ui.scope(|ui| {
                                ui.set_max_width(room);
                                ui.add(egui::Label::new(text).truncate().sense(Sense::click()))
                            })
                            .inner
                            .clicked()
                        } else {
                            style::flat_button(ui, text).clicked()
                        };
                        if hit {
                            clicked = Some(*i);
                        }
                    }
                    None => {
                        let more = style::flat_button(ui, RichText::new(icons::ELLIPSIS))
                            .on_hover_text(tr("The other parts of the path"));
                        egui::Popup::menu(&more).show(|ui| {
                            for i in &hidden {
                                let label = labels[*i].as_str();
                                if super::menu_entry(ui, icons::FOLDER, label, "", true) {
                                    clicked = Some(*i);
                                }
                            }
                        });
                    }
                }
            }
            if let Some(i) = clicked
                && let Some(act) = crumbs.get_mut(i).and_then(|(_, a)| a.take())
            {
                acts.push(act);
            }
            if let Some((marked, act)) = star {
                let (icon, label) = if marked {
                    (icons::STAR, tr("Remove the bookmark (Ctrl+B)"))
                } else {
                    (icons::STAR_EMPTY, tr("Bookmark this (Ctrl+B)"))
                };
                if style::icon_button(ui, icon, label, true, Some(p.accent)).clicked() {
                    acts.push(act);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let reserve = if tools { tools_w + 12.0 } else { 0.0 };
                self.shell.search_field(ui, acts, hint, reserve);
                if tools {
                    ui.separator();
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(tools_w, tools_height), Sense::hover());
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layout(Layout::left_to_right(Align::Center)),
                    );
                    self.toolbar(&mut child, acts, true);
                    let used = child.min_rect().width().ceil();
                    if (used - tools_w).abs() > 0.5 {
                        ui.data_mut(|d| d.insert_temp(tools_id, used));
                        ui.ctx().request_repaint();
                    }
                }
            });
        });
    }
}

impl<A: App> Shell<A> {
    /// The search of the active tab; `reserve` is the width the row keeps for what comes after
    /// it.
    pub fn search_field(
        &mut self,
        ui: &mut egui::Ui,
        acts: &mut Vec<A::Act>,
        hint: &'static str,
        reserve: f32,
    ) {
        if hint.is_empty() {
            return;
        }
        let history = self.search_history.clone();
        let Some(search) = self.active_search() else {
            return;
        };
        if !search.open {
            if style::icon_button(ui, icons::FIND, tr(hint), true, None).clicked() {
                search.open = true;
                search.focus = true;
            }
            return;
        }
        let had_text = !search.text.trim().is_empty();
        let mut remember = None;
        if !history.is_empty() {
            let more = style::icon_button(ui, icons::UNFOLDED, tr("Recent searches"), true, None);
            egui::Popup::menu(&more).show(|ui| {
                for h in &history {
                    if super::menu_entry(ui, icons::FIND, h, "", true) {
                        search.text = h.clone();
                    }
                }
            });
        }
        let id = egui::Id::new("search-field");
        let width = (ui.available_width() - 40.0 - reserve).clamp(160.0, 560.0);
        let response = ui.add(
            egui::TextEdit::singleline(&mut search.text)
                .id(id)
                .hint_text(tr(hint))
                .margin(Margin::symmetric(8, 4))
                .desired_width(width),
        );
        if search.focus {
            ui.memory_mut(|m| m.request_focus(id));
            search.focus = false;
        }
        let close = style::icon_button(ui, icons::CLOSE, tr("Clear and close (Esc)"), true, None);
        let escaped = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
        if close.clicked() || escaped {
            search.text.clear();
            search.open = false;
        } else if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            remember = Some(search.text.trim().to_owned());
        }
        if had_text && search.text.trim().is_empty() {
            acts.push(ShellAct::SearchCleared.into());
        }
        if let Some(text) = remember
            && remember_search(&mut self.search_history, &text)
        {
            self.settings.search_history = self.search_history.clone();
            self.settings_dirty = true;
        }
    }
}

/// The parts of the breadcrumb that fit a width: all of them, else the first, an ellipsis
/// (`None`) and the last ones, else the last alone.
fn fit_crumbs(ui: &egui::Ui, crumbs: &[String], room: f32) -> Vec<Option<usize>> {
    let n = crumbs.len();
    let width = |i: usize| {
        let font = if i + 1 == n {
            style::strong_font(ui, 13.0)
        } else {
            style::font(ui, 13.0)
        };
        ui.painter()
            .layout_no_wrap(crumbs[i].clone(), font, egui::Color32::WHITE)
            .size()
            .x
            + 22.0
    };
    let total = |list: &[Option<usize>]| {
        list.iter().map(|x| x.map_or(26.0, width)).sum::<f32>()
            + 16.0 * list.len().saturating_sub(1) as f32
    };
    let mut list: Vec<Option<usize>> = (0..n).map(Some).collect();
    while total(&list) > room && list.len() > 2 {
        if list[1].is_some() {
            list[1] = None;
        } else if list.len() > 3 {
            list.remove(2);
        } else {
            break;
        }
    }
    if total(&list) > room && n > 0 {
        list = vec![Some(n - 1)];
    }
    list
}
