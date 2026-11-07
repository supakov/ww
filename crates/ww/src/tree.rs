//! The tree of a profile and of a catalog tab (`docs/spec/01-ui.md` sections 6.1 and 12): rows
//! painted in the look of the window, one stop of the Tab key for the whole tree and the keys
//! inside it: the arrows, Home and End, Page Up and Page Down, Space for the check box, Enter for
//! a group. A click on a row gives the tree the keyboard focus, so the keys go on from there.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Key, Modifiers, Rect, Sense, Shape, Stroke,
    StrokeKind,
};

use crate::i18n::trf;
use crate::style::{self, Palette};

/// The check box of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// No check box (a form, a policy of a catalog tab).
    None,
    Off,
    On,
    /// A group with some rules on.
    Mixed,
    /// An imported policy a built-in rule writes: shown on, not changed here.
    Linked,
    /// Chosen on but waiting for a rule that is off.
    Blocked,
}

/// A row of the tree, in the order it is shown.
pub struct Row<K> {
    pub key: K,
    pub depth: usize,
    pub title: String,
    /// The count of a group or the parameter of a rule, at the right end.
    pub right: String,
    pub check: Check,
    /// Whether a group is open; `None` for a leaf.
    pub open: Option<bool>,
    pub muted: bool,
    /// An icon in place of the check box.
    pub icon: Option<&'static str>,
    /// An inline switch at the right end, on or off.
    pub switch: Option<bool>,
}

/// What the user did in the tree.
pub enum Event<K> {
    Select(K),
    Toggle(K),
    Expand(K, bool),
    Switch(K, bool),
}

/// How the selected row comes into view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scroll {
    None,
    /// As little as needed, after a move with the keys.
    Near,
    /// To the middle, after a search was cleared.
    Center,
}

impl Scroll {
    /// What a tree or a list does with its selection: to the middle after a reveal, into view
    /// after a move with the keys, else nothing.
    pub fn wanted(reveal: bool, follow: bool) -> Self {
        if reveal {
            Self::Center
        } else if follow {
            Self::Near
        } else {
            Self::None
        }
    }
}

pub struct Output<K> {
    pub events: Vec<Event<K>>,
    /// The selection moved with the keys, so it should follow into view at the next frame.
    pub moved: bool,
    /// The selected row was scrolled into view.
    pub scrolled: bool,
}

fn paint_check(painter: &egui::Painter, rect: Rect, check: Check, p: &Palette) {
    let r = CornerRadius::same(4);
    let mark = |painter: &egui::Painter, color: Color32| {
        let points = vec![
            rect.left_top() + egui::vec2(3.8, 8.2),
            rect.left_top() + egui::vec2(6.8, 11.0),
            rect.left_top() + egui::vec2(12.2, 5.0),
        ];
        painter.add(Shape::line(points, Stroke::new(1.8, color)));
    };
    match check {
        Check::None => {}
        Check::Off => {
            painter.rect_filled(rect, r, p.card);
            painter.rect_stroke(rect, r, Stroke::new(1.0, p.off), StrokeKind::Inside);
        }
        Check::On | Check::Blocked => {
            let fill = if check == Check::Blocked {
                p.check.gamma_multiply(0.45)
            } else {
                p.check
            };
            painter.rect_filled(rect, r, fill);
            mark(painter, p.check_mark);
        }
        Check::Mixed => {
            painter.rect_filled(rect, r, p.check);
            painter.line_segment(
                [
                    egui::pos2(rect.center().x - 4.0, rect.center().y),
                    egui::pos2(rect.center().x + 4.0, rect.center().y),
                ],
                Stroke::new(2.0, p.check_mark),
            );
        }
        Check::Linked => {
            painter.rect_filled(rect, r, p.card);
            let s = rect.shrink(0.5);
            let outline = vec![
                s.left_top(),
                s.right_top(),
                s.right_bottom(),
                s.left_bottom(),
                s.left_top(),
            ];
            painter.extend(Shape::dashed_line(
                &outline,
                Stroke::new(1.0, p.accent),
                3.0,
                2.0,
            ));
            mark(painter, p.accent);
        }
    }
}

fn paint_switch(painter: &egui::Painter, rect: Rect, on: bool, p: &Palette) {
    let radius = rect.height() / 2.0;
    painter.rect_filled(
        rect,
        CornerRadius::same(radius as u8),
        if on { p.accent } else { p.off },
    );
    let x = if on {
        rect.right() - radius
    } else {
        rect.left() + radius
    };
    painter.circle_filled(egui::pos2(x, rect.center().y), radius - 2.0, Color32::WHITE);
}

fn paint_twisty(painter: &egui::Painter, center: egui::Pos2, open: bool, color: Color32) {
    let s = 3.5;
    let points = if open {
        vec![
            center + egui::vec2(-s, -s * 0.6),
            center + egui::vec2(s, -s * 0.6),
            center + egui::vec2(0.0, s * 0.8),
        ]
    } else {
        vec![
            center + egui::vec2(-s * 0.6, -s),
            center + egui::vec2(s * 0.8, 0.0),
            center + egui::vec2(-s * 0.6, s),
        ]
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// The row a key that moves leads to among `n` rows: the arrows up and down, Home, End, Page Up
/// and Page Down (`page` rows). The key is consumed.
pub(crate) fn move_target(
    ui: &egui::Ui,
    n: usize,
    selected: Option<usize>,
    page: usize,
) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let pressed = |key: Key| ui.input_mut(|i| i.consume_key(Modifiers::NONE, key));
    let cur = selected.unwrap_or(0);
    if pressed(Key::ArrowDown) {
        Some(if selected.is_none() {
            0
        } else {
            (cur + 1).min(n - 1)
        })
    } else if pressed(Key::ArrowUp) {
        Some(cur.saturating_sub(1))
    } else if pressed(Key::Home) {
        Some(0)
    } else if pressed(Key::End) {
        Some(n - 1)
    } else if pressed(Key::PageDown) {
        Some((cur + page).min(n - 1))
    } else if pressed(Key::PageUp) {
        Some(cur.saturating_sub(page))
    } else {
        None
    }
}

/// Makes `area` one stop of the Tab key named `label` and keeps the arrows inside it while it has
/// the keyboard focus; returns whether it has it.
pub(crate) fn focus_area(ui: &mut egui::Ui, area: Rect, id: egui::Id, label: &str) -> bool {
    let focus = ui.interact(area, id, Sense::focusable_noninteractive());
    focus.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label));
    let focused = focus.has_focus();
    if focused {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    vertical_arrows: true,
                    horizontal_arrows: true,
                    tab: false,
                    escape: false,
                },
            )
        });
    }
    focused
}

/// The frame of an area with the keyboard focus.
pub(crate) fn paint_focus(ui: &egui::Ui, area: Rect, p: &Palette) {
    ui.painter().rect_stroke(
        area.shrink(1.0),
        CornerRadius::same(2),
        Stroke::new(1.5, p.accent),
        StrokeKind::Inside,
    );
}

/// The keys of a focused tree.
fn keys<K: Clone>(
    ui: &mut egui::Ui,
    rows: &[Row<K>],
    selected: Option<usize>,
    out: &mut Output<K>,
) {
    let n = rows.len();
    if n == 0 {
        return;
    }
    let page = ((ui.available_height() / style::row_height(ui)) as usize).max(2) - 1;
    let pressed = |key: Key| ui.input_mut(|i| i.consume_key(Modifiers::NONE, key));
    if let Some(t) = move_target(ui, n, selected, page) {
        if Some(t) != selected {
            out.events.push(Event::Select(rows[t].key.clone()));
            out.moved = true;
        }
        return;
    }
    let Some(i) = selected else {
        return;
    };
    let row = &rows[i];
    if pressed(Key::ArrowRight) {
        match row.open {
            Some(false) => out.events.push(Event::Expand(row.key.clone(), true)),
            Some(true) if i + 1 < n && rows[i + 1].depth > row.depth => {
                out.events.push(Event::Select(rows[i + 1].key.clone()));
                out.moved = true;
            }
            _ => {}
        }
    } else if pressed(Key::ArrowLeft) {
        if row.open == Some(true) {
            out.events.push(Event::Expand(row.key.clone(), false));
        } else if let Some(parent) = (0..i).rev().find(|&j| rows[j].depth < row.depth) {
            out.events.push(Event::Select(rows[parent].key.clone()));
            out.moved = true;
        }
    } else if pressed(Key::Space) {
        if let Some(on) = row.switch {
            out.events.push(Event::Switch(row.key.clone(), !on));
        } else if !matches!(row.check, Check::None | Check::Linked) {
            out.events.push(Event::Toggle(row.key.clone()));
        }
    } else if pressed(Key::Enter)
        && let Some(open) = row.open
    {
        out.events.push(Event::Expand(row.key.clone(), !open));
    }
}

/// Draws the tree and returns what the user did. `id` names the tree for its focus and scroll.
pub fn show<K: Clone>(
    ui: &mut egui::Ui,
    id: egui::Id,
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
    let focused = focus_area(ui, area, id, crate::i18n::tr("Tree"));
    if focused {
        keys(ui, rows, selected, &mut out);
    }
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
                let cy = rect.center().y;
                let left = rect.left() + 8.0 + row.depth as f32 * 16.0;
                let twisty_rect = Rect::from_min_max(
                    egui::pos2(left - 2.0, rect.top()),
                    egui::pos2(left + 16.0, rect.bottom()),
                );
                let check_rect =
                    Rect::from_center_size(egui::pos2(left + 26.0, cy), egui::vec2(16.0, 16.0));
                let switch_rect = Rect::from_center_size(
                    egui::pos2(rect.right() - 26.0, cy),
                    egui::vec2(32.0, 16.0),
                );
                if is_selected && scroll != Scroll::None {
                    let align = if scroll == Scroll::Center {
                        Some(Align::Center)
                    } else {
                        None
                    };
                    ui.scroll_to_rect(rect, align);
                    out.scrolled = true;
                }
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    if is_selected {
                        painter.rect_filled(rect, CornerRadius::ZERO, p.sel);
                    } else if response.hovered() {
                        painter.rect_filled(rect, CornerRadius::ZERO, p.hover);
                    }
                    if let Some(open) = row.open {
                        paint_twisty(painter, egui::pos2(left + 7.0, cy), open, p.muted);
                    }
                    match (row.check, row.icon) {
                        (Check::None, Some(icon)) => {
                            painter.text(
                                check_rect.center(),
                                Align2::CENTER_CENTER,
                                icon,
                                style::font(ui, 13.0),
                                p.text,
                            );
                        }
                        (check, _) => paint_check(painter, check_rect, check, &p),
                    }
                    let mut right_edge = rect.right() - 10.0;
                    if let Some(on) = row.switch {
                        paint_switch(painter, switch_rect, on, &p);
                        right_edge = switch_rect.left() - 8.0;
                    } else if !row.right.is_empty() {
                        let galley = painter.layout_no_wrap(
                            row.right.clone(),
                            style::font(ui, 11.0),
                            p.muted,
                        );
                        let w = galley.size().x;
                        painter.galley(
                            egui::pos2(right_edge - w, cy - galley.size().y / 2.0),
                            galley,
                            p.muted,
                        );
                        right_edge -= w + 10.0;
                    }
                    let x = if row.check == Check::None && row.icon.is_none() {
                        left + 18.0
                    } else {
                        left + 40.0
                    };
                    let color = if row.muted {
                        p.muted
                    } else if is_selected {
                        p.sel_text
                    } else {
                        p.text
                    };
                    let galley =
                        painter.layout_no_wrap(row.title.clone(), style::font(ui, 13.0), color);
                    let clip = Rect::from_min_max(
                        egui::pos2(x, rect.top()),
                        egui::pos2(right_edge, rect.bottom()),
                    );
                    painter
                        .with_clip_rect(clip.intersect(painter.clip_rect()))
                        .galley(egui::pos2(x, cy - galley.size().y / 2.0), galley, color);
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        is_selected,
                        &row.title,
                    )
                });
                let mut handled = false;
                if row.open.is_some() {
                    let twisty = ui.interact(twisty_rect, response.id.with("twisty"), Sense::CLICK);
                    if twisty.clicked() {
                        out.events
                            .push(Event::Expand(row.key.clone(), row.open != Some(true)));
                        handled = true;
                    }
                }
                if !matches!(row.check, Check::None | Check::Linked) {
                    let check = ui.interact(
                        check_rect.expand(2.0),
                        response.id.with("check"),
                        Sense::CLICK,
                    );
                    check.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Checkbox,
                            true,
                            trf("Turn on or off: {title}", &[("title", &row.title)]),
                        )
                    });
                    if check.clicked() {
                        out.events.push(Event::Toggle(row.key.clone()));
                        handled = true;
                    }
                }
                if let Some(on) = row.switch {
                    let switch = ui.interact(switch_rect, response.id.with("switch"), Sense::CLICK);
                    switch.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Checkbox,
                            true,
                            trf("Turn on or off: {title}", &[("title", &row.title)]),
                        )
                    });
                    if switch.clicked() {
                        out.events.push(Event::Switch(row.key.clone(), !on));
                        handled = true;
                    }
                }
                if !handled
                    && response.double_clicked()
                    && let Some(open) = row.open
                {
                    out.events.push(Event::Expand(row.key.clone(), !open));
                } else if !handled && response.clicked() {
                    out.events.push(Event::Select(row.key.clone()));
                }
                if response.clicked() || handled {
                    take_focus = true;
                }
            }
        });
    if take_focus {
        ui.memory_mut(|m| m.request_focus(id));
    }
    if focused {
        paint_focus(ui, area, &p);
    }
    out
}
