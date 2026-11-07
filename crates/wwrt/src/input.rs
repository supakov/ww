//! The keys and the mouse of a view (docs/plugins.md, 6.8): a view takes a rectangle with `area`
//! and gets what happened in it since the last frame as an `Input`.
//!
//! A click gives the area the focus of the keyboard, Tab and Escape give it back to the window. An
//! area in focus takes the keys it names, pressed without Ctrl or Alt, so the shortcuts of the
//! window stay the window's. The mouse counts while it is over the area, and a drag goes on outside
//! it until the button is up. A view sets its cursor with `cursor` and asks for another frame with
//! `egui::Context::request_repaint` while it is at work.

use crate::egui::{self, Event, Id, Key, Modifiers, Pos2, Rect, Response, Sense, Vec2};

/// What happened in the area of a view since the last frame.
#[derive(Debug, Clone)]
pub struct Input {
    /// The area, in the points of the window.
    pub rect: Rect,
    /// The pointer in the area (its top left corner is zero) while it is over it or a drag of it
    /// goes on.
    pub pointer: Option<Pos2>,
    pub hovered: bool,
    pub focused: bool,
    /// The movement with the primary button down, and whether the drag started or ended now.
    pub drag: Vec2,
    pub dragging: bool,
    pub drag_started: bool,
    pub drag_stopped: bool,
    pub clicked: bool,
    pub double_clicked: bool,
    pub secondary_clicked: bool,
    pub middle_clicked: bool,
    /// The wheel over the area, in points (down is positive), and the zoom of Ctrl with the wheel
    /// or of a pinch (1 is none).
    pub wheel: Vec2,
    pub zoom: f32,
    pub modifiers: Modifiers,
    /// The keys of the view pressed while it had the focus, with their modifiers; a key held down
    /// comes again with every repeat.
    pub keys: Vec<(Key, Modifiers)>,
}

impl Input {
    /// Whether a key was pressed, with any modifiers but Ctrl and Alt.
    pub fn pressed(&self, key: Key) -> bool {
        self.keys.iter().any(|(k, _)| *k == key)
    }
}

/// The cursors a view may show over its area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    Default,
    Cross,
    Hand,
    Grab,
    Grabbing,
    ZoomIn,
    ZoomOut,
    Move,
}

/// Takes `rect` for the mouse and the keys of a view: the response of egui and the input of the
/// frame. `keys` are the keys the view takes while it has the focus; a thin frame shows the focus.
pub fn area(ui: &mut egui::Ui, id: Id, rect: Rect, keys: &[Key]) -> (Response, Input) {
    let response = ui.interact(rect, id, Sense::click_and_drag());
    if response.clicked() || response.drag_started() || response.secondary_clicked() {
        response.request_focus();
    }
    let focused = response.has_focus();
    let mut taken = Vec::new();
    if focused {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    tab: false,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: false,
                },
            );
        });
        ui.input_mut(|i| {
            i.events.retain(|event| match event {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if keys.contains(key)
                    && !modifiers.ctrl
                    && !modifiers.alt
                    && !modifiers.command =>
                {
                    taken.push((*key, *modifiers));
                    false
                }
                _ => true,
            });
        });
    }
    let hovered = response.hovered();
    let dragging = response.dragged();
    let (pointer, wheel, zoom, modifiers) = ui.input(|i| {
        let pointer = i
            .pointer
            .latest_pos()
            .filter(|_| hovered || dragging)
            .map(|p| Pos2::ZERO + (p - rect.min));
        let over = hovered && !dragging;
        let wheel = if over {
            -i.smooth_scroll_delta()
        } else {
            Vec2::ZERO
        };
        let zoom = if over { i.zoom_delta() } else { 1.0 };
        (pointer, wheel, zoom, i.modifiers)
    });
    if focused {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            0.0,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    let input = Input {
        rect,
        pointer,
        hovered,
        focused,
        drag: if dragging {
            response.drag_delta()
        } else {
            Vec2::ZERO
        },
        dragging,
        drag_started: response.drag_started(),
        drag_stopped: response.drag_stopped(),
        clicked: response.clicked(),
        double_clicked: response.double_clicked(),
        secondary_clicked: response.secondary_clicked(),
        middle_clicked: response.middle_clicked(),
        wheel,
        zoom,
        modifiers,
        keys: taken,
    };
    (response, input)
}

/// Shows a cursor over the area this frame.
pub fn cursor(ui: &egui::Ui, cursor: Cursor) {
    let icon = match cursor {
        Cursor::Default => return,
        Cursor::Cross => egui::CursorIcon::Crosshair,
        Cursor::Hand => egui::CursorIcon::PointingHand,
        Cursor::Grab => egui::CursorIcon::Grab,
        Cursor::Grabbing => egui::CursorIcon::Grabbing,
        Cursor::ZoomIn => egui::CursorIcon::ZoomIn,
        Cursor::ZoomOut => egui::CursorIcon::ZoomOut,
        Cursor::Move => egui::CursorIcon::Move,
    };
    ui.ctx().set_cursor_icon(icon);
}
