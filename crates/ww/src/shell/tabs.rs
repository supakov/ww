//! The tab row: the menu button, the tabs (a click, a drag, the menu of the right button) and
//! the new button.

use eframe::egui::{self, Align2, CornerRadius, Sense, Stroke};

use super::{
    App, ShellAct, TAB_CLOSE, TAB_DOT, TAB_GAP, TAB_HEIGHT, TAB_ICON, TAB_PADDING, TAB_TITLE_WIDTH,
    Window,
};
use crate::doc::Document;
use crate::i18n::{tr, trf};
use crate::style::{self, Palette};
use crate::{icons, keys};

impl<A: App> Window<A> {
    /// The tab row: the menu button, the tabs with their icons, the new tab button.
    pub(super) fn tabs_row(&mut self, ui: &mut egui::Ui, acts: &mut Vec<A::Act>) {
        let p = style::palette(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let menu = style::icon_button(ui, icons::MENU, tr("Menu"), true, Some(p.text));
            egui::Popup::menu(&menu).show(|ui| {
                ui.set_min_width(260.0);
                self.app.menu(&self.shell, ui, acts);
            });
            ui.add_space(4.0);
            let active = self.shell.active;
            let docs = &self.shell.docs;
            let last = docs.len().saturating_sub(1);
            // The row of the last frame: its active tab, its count of tabs and whether they were
            // wider than the row. A new active tab or count scrolls the active one into view.
            let row_id = egui::Id::new("ww-tab-row");
            let before: Option<(usize, usize, bool)> = ui.data(|d| d.get_temp(row_id));
            let overflow = before.is_some_and(|b| b.2);
            let follow = before.is_none_or(|b| b.0 != active || b.1 != docs.len());
            let buttons = if overflow { 2.0 } else { 1.0 };
            let button = style::at_least(ui, TAB_BUTTON);
            let width = (ui.available_width() - buttons * (button + 2.0)).max(button);
            use egui::containers::scroll_area::{ScrollBarVisibility, ScrollSource};
            let shown = egui::ScrollArea::horizontal()
                .id_salt("ww-tabs")
                .max_width(width)
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                .scroll_source(ScrollSource::MOUSE_WHEEL)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let tabs: Vec<egui::Response> = docs
                            .iter()
                            .enumerate()
                            .map(|(i, doc)| tab::<A>(ui, &p, i, i == active, last, doc, acts))
                            .collect();
                        if follow && let Some(t) = tabs.get(active) {
                            t.scroll_to_me(None);
                        }
                        // The wheel turns the row sideways.
                        let wheel = ui.input(|i| i.smooth_scroll_delta);
                        if wheel.x == 0.0
                            && wheel.y != 0.0
                            && ui.rect_contains_pointer(ui.clip_rect())
                        {
                            ui.scroll_with_delta(egui::vec2(wheel.y, 0.0));
                        }
                        tabs
                    })
                    .inner
                });
            let tabs = shown.inner;
            let wider = shown.content_size.x > shown.inner_rect.width() + 0.5;
            ui.data_mut(|d| d.insert_temp(row_id, (active, docs.len(), wider)));
            let rects: Vec<egui::Rect> = tabs.iter().map(|r| r.rect).collect();
            let pointer = ui.ctx().pointer_interact_pos();
            if let (Some(from), Some(pos)) = (tabs.iter().position(|r| r.dragged()), pointer) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                let (_, x) = drop_place(&rects, from, pos.x);
                let row = rects[from];
                ui.painter().vline(
                    x,
                    row.top() + 4.0..=row.bottom() - 4.0,
                    Stroke::new(2.0, p.accent),
                );
            }
            if let (Some(from), Some(pos)) = (tabs.iter().position(|r| r.drag_stopped()), pointer) {
                let (to, _) = drop_place(&rects, from, pos.x);
                if to != from {
                    acts.push(ShellAct::MoveTab(from, to).into());
                }
            }
            if style::icon_button(ui, "+", tr(A::NEW_HINT), true, None).clicked() {
                acts.push(ShellAct::New.into());
            }
            if wider {
                let all = style::icon_button(ui, icons::UNFOLDED, tr("All tabs"), true, None);
                egui::Popup::menu(&all).show(|ui| {
                    for (i, doc) in docs.iter().enumerate() {
                        let title = doc.title();
                        if menu_entry(ui, doc.icon(), &title, "", true) {
                            acts.push(ShellAct::Activate(i).into());
                        }
                    }
                });
            }
        });
    }
}

/// The width of a button of the tab row (the menu, the new tab, the list of all tabs).
const TAB_BUTTON: f32 = 30.0;

/// A tab of the row: a click brings it to the front, a drag moves it, the right button opens a
/// menu that moves or closes it.
fn tab<A: App>(
    ui: &mut egui::Ui,
    p: &Palette,
    i: usize,
    active: bool,
    last: usize,
    doc: &A::Doc,
    acts: &mut Vec<A::Act>,
) -> egui::Response {
    let title = doc.title();
    let font = style::font(ui, 12.5);
    // The parts of the tab grow with the text size of the theme.
    let room = |at: f32| style::at_least(ui, at);
    let (padding, icon_w, dot_w, gap, close_w, height) = (
        room(TAB_PADDING),
        room(TAB_ICON),
        room(TAB_DOT),
        room(TAB_GAP),
        room(TAB_CLOSE),
        room(TAB_HEIGHT),
    );
    let (close_side, close_inset, icon_inset, dot_inset, dot_radius) =
        (room(18.0), room(15.0), room(6.0), room(2.0), room(3.0));
    let text_w = ui
        .painter()
        .layout_no_wrap(title.clone(), font.clone(), p.text)
        .size()
        .x
        .min(room(TAB_TITLE_WIDTH));
    let dirty = doc.dirty();
    let dot = if dirty { dot_w } else { 0.0 };
    let width = padding + icon_w + dot + text_w + gap + close_w;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), Sense::click_and_drag());
    let close_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - close_inset, rect.center().y),
        egui::vec2(close_side, close_side),
    );
    let close = ui.interact(close_rect, response.id.with("close"), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let corners = CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        };
        if active {
            painter.rect_filled(rect, corners, p.card);
        } else if response.hovered() {
            painter.rect_filled(rect, corners, p.hover);
        }
        let color = if active { p.text } else { p.muted };
        let mut x = rect.left() + padding;
        style::paint_icon(
            ui,
            painter,
            egui::pos2(x + icon_inset, rect.center().y),
            doc.icon(),
            12.0,
            color,
            true,
        );
        x += icon_w;
        if dirty {
            let center = egui::pos2(x + dot_inset, rect.center().y);
            painter.circle_filled(center, dot_radius, p.accent);
            x += dot_w;
        }
        let galley = painter.layout_no_wrap(title.clone(), font, color);
        let clip = egui::Rect::from_min_max(
            egui::pos2(x, rect.top()),
            egui::pos2(x + text_w, rect.bottom()),
        );
        painter.with_clip_rect(clip).galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
        if close.hovered() {
            painter.rect_filled(close_rect, CornerRadius::same(4), p.hover);
        }
        painter.text(
            close_rect.center(),
            Align2::CENTER_CENTER,
            icons::CLOSE,
            style::font(ui, 10.0),
            if active || response.hovered() {
                p.muted
            } else {
                p.off
            },
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, &title)
    });
    close.widget_info(|| {
        let label = trf("Close ({keys})", &[("keys", &keys::CLOSE.text)]);
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label)
    });
    if close.clicked() {
        acts.push(ShellAct::CloseTab(i, false).into());
    } else if response.clicked() {
        acts.push(ShellAct::Activate(i).into());
    }
    if response.middle_clicked() {
        acts.push(ShellAct::CloseTab(i, false).into());
    }
    response.context_menu(|ui| {
        let left = trf(
            "Move the tab left ({keys})",
            &[("keys", &keys::TAB_LEFT.text)],
        );
        if menu_entry(ui, icons::BACK, &left, "", i > 0) {
            acts.push(ShellAct::MoveTab(i, i - 1).into());
        }
        let right = trf(
            "Move the tab right ({keys})",
            &[("keys", &keys::TAB_RIGHT.text)],
        );
        if menu_entry(ui, icons::FORWARD, &right, "", i < last) {
            acts.push(ShellAct::MoveTab(i, i + 1).into());
        }
        ui.separator();
        let close = trf("Close ({keys})", &[("keys", &keys::CLOSE.text)]);
        if menu_entry(ui, icons::CLOSE, &close, "", true) {
            acts.push(ShellAct::CloseTab(i, false).into());
        }
    });
    let hint = doc.path().map(|p| p.display().to_string());
    match hint {
        Some(h) => response.on_hover_text(h),
        None => response,
    }
}

/// Where a dragged tab goes: its new index and the x of the line that shows the place, from the
/// tabs of the row, the index of the dragged one and the x of the pointer.
fn drop_place(rects: &[egui::Rect], from: usize, x: f32) -> (usize, f32) {
    let others: Vec<&egui::Rect> = rects
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != from)
        .map(|(_, r)| r)
        .collect();
    let to = others.iter().filter(|r| r.center().x < x).count();
    let line = match others.get(to) {
        Some(r) => r.left() - 1.0,
        None => others
            .last()
            .map_or(rects[from].right(), |r| r.right() + 1.0),
    };
    (to, line)
}

/// The size of the text of an icon of a menu, in the points of the layout.
const MENU_ICON: f32 = 12.0;

/// The side of the place of an icon of a menu.
fn menu_icon_side(ui: &egui::Ui) -> f32 {
    (style::size(ui, MENU_ICON) * 1.3).round()
}

/// An empty image of the size of an icon of a menu: it keeps the place the icon is painted on,
/// so the texts of a menu line up whether an item has an icon or not.
fn icon_place(ui: &egui::Ui) -> egui::Image<'static> {
    let side = egui::vec2(menu_icon_side(ui), menu_icon_side(ui));
    let place = egui::load::SizedTexture::new(egui::TextureId::default(), side);
    egui::Image::new(place)
        .fit_to_exact_size(side)
        .tint(egui::Color32::TRANSPARENT)
}

/// Paints the icon of a menu item on the place `icon_place` kept at the left of its button.
fn paint_menu_icon(ui: &egui::Ui, response: &egui::Response, icon: &str) {
    if icon.is_empty() || !ui.is_rect_visible(response.rect) {
        return;
    }
    let side = menu_icon_side(ui);
    let center = egui::pos2(
        response.rect.left() + ui.spacing().button_padding.x + side / 2.0,
        response.rect.center().y,
    );
    let color = ui.style().interact(response).text_color();
    let enabled = response.enabled();
    style::paint_icon(ui, ui.painter(), center, icon, MENU_ICON, color, enabled);
}

/// An entry of a menu: an icon (empty for none), a text and the text of its keys, enabled or
/// not; true when it was clicked, and the menu closes then.
pub fn menu_entry(ui: &mut egui::Ui, icon: &str, text: &str, keys: &str, enabled: bool) -> bool {
    menu_entry_colored(ui, icon, text, keys, enabled, None)
}

/// An entry of a menu whose text has a colour of its own (`None` for the colour of the text):
/// a loaded catalog among the others, for one.
pub fn menu_entry_colored(
    ui: &mut egui::Ui,
    icon: &str,
    text: &str,
    keys: &str,
    enabled: bool,
    color: Option<egui::Color32>,
) -> bool {
    let label = match color {
        Some(c) => egui::RichText::new(text).color(c),
        None => egui::RichText::new(text),
    };
    let mut button = egui::Button::image_and_text(icon_place(ui), label);
    if !keys.is_empty() {
        button = button.shortcut_text(keys);
    }
    let response = ui.add_enabled(enabled, button);
    paint_menu_icon(ui, &response, icon);
    let clicked = response.clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// An item of a menu: an icon (empty for none), a text, the text of its keys and the act of a
/// click.
pub fn menu_item<T>(
    ui: &mut egui::Ui,
    icon: &str,
    text: &str,
    keys: &str,
    act: T,
    acts: &mut Vec<T>,
) {
    if menu_entry(ui, icon, text, keys, true) {
        acts.push(act);
    }
}

/// A menu inside a menu, with an icon before its title like the items beside it.
pub fn sub_menu<R>(
    ui: &mut egui::Ui,
    icon: &str,
    text: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    let place = icon_place(ui);
    let inner = ui.menu_image_text_button(place, text, add_contents);
    paint_menu_icon(ui, &inner.response, icon);
    inner.inner
}
