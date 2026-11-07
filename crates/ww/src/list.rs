//! A list with columns: the differences of a diff, the bookmarks, the log of the MCP server. Like
//! the tree it is one stop of the Tab key; the arrows, Home, End, Page Up and Page Down move the
//! selection and bring it into view, Enter opens the selected row, Delete asks to remove it, a
//! click gives the list the keyboard focus. The columns change their width by dragging the lines
//! between their titles; a row may have a mini button at its right end.

use eframe::egui::{self, Align, CornerRadius, CursorIcon, Key, Modifiers, Rect, Sense, Stroke};

use crate::style;
use crate::tree::{self, Scroll};

/// The height of the row of titles.
const HEADER: f32 = 30.0;
/// The narrowest a column gets by dragging.
const MIN_WIDTH: f32 = 48.0;
/// The space inside a cell on the left and on the right.
const PAD: f32 = 10.0;

pub struct Column {
    pub title: String,
    /// The width at the first show; the last column takes the rest of the row.
    pub width: f32,
}

impl Column {
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self {
            title: title.into(),
            width,
        }
    }
}

#[derive(Default)]
pub struct Cell {
    pub text: String,
    pub strong: bool,
    pub muted: bool,
    pub warn: bool,
    /// A value in a tinted box: the old one of a difference (`Some(false)`) or the new one.
    pub chip: Option<bool>,
}

impl Cell {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }

    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    pub fn warn(mut self) -> Self {
        self.warn = true;
        self
    }

    /// The value of a side of a difference in a tinted box: red for the old, green for the new.
    pub fn chip(mut self, new: bool) -> Self {
        self.chip = Some(new);
        self
    }
}

/// A row of the list, in the order it is shown.
pub struct Row<K> {
    pub key: K,
    pub cells: Vec<Cell>,
    /// A mini button at the right end: its icon and its hint.
    pub button: Option<(&'static str, String)>,
}

/// What the user did in the list.
pub enum Event<K> {
    Select(K),
    /// A double click or Enter on a row.
    Open(K),
    /// The mini button of a row.
    Button(K),
    /// Delete on the selected row.
    Delete(K),
}

pub struct Output<K> {
    pub events: Vec<Event<K>>,
    /// The selection moved with the keys, so it should follow into view at the next frame.
    pub moved: bool,
    /// The selected row was scrolled into view.
    pub scrolled: bool,
}

/// The widths of the columns: the stored ones while their number stays, else the first ones.
fn widths(ui: &egui::Ui, id: egui::Id, columns: &[Column]) -> Vec<f32> {
    ui.data(|d| d.get_temp::<Vec<f32>>(id))
        .filter(|w| w.len() == columns.len())
        .unwrap_or_else(|| columns.iter().map(|c| c.width).collect())
}

/// Draws the list and returns what the user did. `id` names the list for its focus, its scroll
/// and the widths of its columns; `label` names it for the screen readers.
pub fn show<K: Clone>(
    ui: &mut egui::Ui,
    id: egui::Id,
    label: &str,
    columns: &[Column],
    rows: &[Row<K>],
    selected: Option<usize>,
    scroll: Scroll,
) -> Output<K> {
    let p = style::palette(ui);
    let mut out = Output {
        events: Vec::new(),
        moved: false,
        scrolled: false,
    };
    let area = ui.available_rect_before_wrap();
    let focused = tree::focus_area(ui, area, id, label);
    if focused {
        let page = (((area.height() - HEADER) / style::row_height(ui)) as usize).max(2) - 1;
        if let Some(t) = tree::move_target(ui, rows.len(), selected, page) {
            if Some(t) != selected {
                out.events.push(Event::Select(rows[t].key.clone()));
                out.moved = true;
            }
        } else if let Some(i) = selected
            && ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::Enter))
        {
            out.events.push(Event::Open(rows[i].key.clone()));
        } else if let Some(i) = selected
            && ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::Delete))
        {
            out.events.push(Event::Delete(rows[i].key.clone()));
        }
    }
    let widths_id = id.with("widths");
    let mut widths = widths(ui, widths_id, columns);
    let (header, _) = ui.allocate_exact_size(egui::vec2(area.width(), HEADER), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(header, CornerRadius::ZERO, p.card2);
    painter.hline(
        header.x_range(),
        header.bottom() - 0.5,
        Stroke::new(1.0, p.border),
    );
    let mut x = header.left();
    let mut spans = Vec::with_capacity(columns.len());
    for (i, column) in columns.iter().enumerate() {
        let last = i + 1 == columns.len();
        let w = if last {
            (header.right() - x).max(MIN_WIDTH)
        } else {
            widths[i]
        };
        spans.push((x, w));
        let cell = Rect::from_min_size(egui::pos2(x, header.top()), egui::vec2(w, HEADER));
        let galley =
            painter.layout_no_wrap(column.title.clone(), style::strong_font(ui, 12.0), p.muted);
        painter
            .with_clip_rect(cell.shrink2(egui::vec2(PAD, 0.0)).intersect(area))
            .galley(
                egui::pos2(x + PAD, cell.center().y - galley.size().y / 2.0),
                galley,
                p.muted,
            );
        if !last {
            let edge = x + w;
            let grip = Rect::from_center_size(
                egui::pos2(edge, header.center().y),
                egui::vec2(8.0, HEADER),
            );
            let drag = ui.interact(grip, id.with(("resize", i)), Sense::drag());
            if drag.hovered() || drag.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            }
            if drag.dragged() {
                widths[i] = (widths[i] + drag.drag_delta().x).max(MIN_WIDTH);
            }
            let color = if drag.hovered() || drag.dragged() {
                p.accent
            } else {
                p.border
            };
            painter.vline(
                edge,
                header.top() + 7.0..=header.bottom() - 7.0,
                Stroke::new(1.0, color),
            );
        }
        x += w;
    }
    ui.data_mut(|d| d.insert_temp(widths_id, widths));
    let mut take_focus = false;
    let row_height = style::row_height(ui);
    egui::ScrollArea::vertical()
        .id_salt(id.with("scroll"))
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, row) in rows.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), row_height),
                    Sense::CLICK,
                );
                let is_selected = Some(i) == selected;
                if is_selected && scroll != Scroll::None {
                    let align = if scroll == Scroll::Center {
                        Some(Align::Center)
                    } else {
                        None
                    };
                    ui.scroll_to_rect(rect, align);
                    out.scrolled = true;
                }
                let button_rect = Rect::from_center_size(
                    egui::pos2(rect.right() - 20.0, rect.center().y),
                    egui::vec2(26.0, 22.0),
                );
                let mut cut = Vec::new();
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    if is_selected {
                        painter.rect_filled(rect, CornerRadius::ZERO, p.sel);
                    } else if response.hovered() {
                        painter.rect_filled(rect, CornerRadius::ZERO, p.hover);
                    }
                    for (j, (cell, &(x, w))) in row.cells.iter().zip(&spans).enumerate() {
                        let w = if row.button.is_some() && j + 1 == spans.len() {
                            (w - 30.0).max(0.0)
                        } else {
                            w
                        };
                        let color = if cell.warn {
                            p.warn
                        } else if cell.muted {
                            p.muted
                        } else if is_selected {
                            p.sel_text
                        } else {
                            p.text
                        };
                        let font = if cell.strong {
                            style::strong_font(ui, 13.0)
                        } else {
                            style::font(ui, 13.0)
                        };
                        let line = cell.text.lines().next().unwrap_or_default();
                        let galley = painter.layout_no_wrap(line.to_owned(), font, color);
                        if galley.size().x > w - 2.0 * PAD || line.len() < cell.text.len() {
                            cut.push(cell.text.as_str());
                        }
                        if let Some(new) = cell.chip
                            && !line.is_empty()
                        {
                            let tint = if new { p.ok } else { p.warn };
                            let chip = Rect::from_min_size(
                                egui::pos2(x + PAD - 6.0, rect.center().y - 10.0),
                                egui::vec2(
                                    (galley.size().x + 12.0).min(w - 2.0 * PAD + 12.0),
                                    20.0,
                                ),
                            );
                            painter.rect_filled(
                                chip,
                                CornerRadius::same(4),
                                tint.gamma_multiply(if p.dark { 0.28 } else { 0.16 }),
                            );
                        }
                        let clip = Rect::from_min_max(
                            egui::pos2(x + PAD, rect.top()),
                            egui::pos2(x + w - PAD, rect.bottom()),
                        );
                        painter
                            .with_clip_rect(clip.intersect(painter.clip_rect()))
                            .galley(
                                egui::pos2(x + PAD, rect.center().y - galley.size().y / 2.0),
                                galley,
                                color,
                            );
                    }
                }
                let text = row
                    .cells
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        is_selected,
                        &text,
                    )
                });
                let response = if cut.is_empty() {
                    response
                } else {
                    response.on_hover_text(cut.join("\n"))
                };
                if let Some((icon, hint)) = &row.button {
                    let button = ui.interact(button_rect, response.id.with("button"), Sense::CLICK);
                    button.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, hint.as_str())
                    });
                    if ui.is_rect_visible(button_rect) {
                        let painter = ui.painter();
                        if button.hovered() {
                            painter.rect_filled(button_rect, CornerRadius::same(4), p.card2);
                            painter.rect_stroke(
                                button_rect,
                                CornerRadius::same(4),
                                Stroke::new(1.0, p.border),
                                egui::StrokeKind::Inside,
                            );
                        }
                        painter.text(
                            button_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            *icon,
                            style::font(ui, 13.0),
                            if button.hovered() { p.accent } else { p.muted },
                        );
                    }
                    let clicked = button.clicked();
                    button.on_hover_text(hint.as_str());
                    if clicked {
                        out.events.push(Event::Button(row.key.clone()));
                        take_focus = true;
                        continue;
                    }
                }
                if response.double_clicked() {
                    out.events.push(Event::Open(row.key.clone()));
                } else if response.clicked() {
                    out.events.push(Event::Select(row.key.clone()));
                }
                if response.clicked() || response.double_clicked() {
                    take_focus = true;
                }
            }
        });
    if take_focus {
        ui.memory_mut(|m| m.request_focus(id));
    }
    if focused {
        tree::paint_focus(ui, area, &p);
    }
    out
}
