//! Widgets the tabs share: a modal window, the left panel of a tab, the split of a list tab, a
//! file dialog in a folder (natively).

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use eframe::egui;

use crate::style;

/// A window over the others: centred, with a title, not collapsible.
pub struct Modal {
    title: String,
    id: Option<egui::Id>,
    width: Option<f32>,
}

impl Modal {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            id: None,
            width: None,
        }
    }

    /// An id of its own, for a window whose title another one may carry.
    pub fn id(mut self, id: &'static str) -> Self {
        self.id = Some(egui::Id::new(id));
        self
    }

    /// A resizable window of this width at first.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn show<R>(self, ctx: &egui::Context, add: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
        let mut window = egui::Window::new(self.title)
            .collapsible(false)
            .resizable(self.width.is_some())
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0));
        if let Some(id) = self.id {
            window = window.id(id);
        }
        if let Some(width) = self.width {
            window = window.default_width(width);
        }
        window.show(ctx, add).and_then(|shown| shown.inner)
    }
}

/// The left panel of a tab: resizable, on a card, with the tree or the outline of the tab.
pub fn side_panel<R>(
    ui: &mut egui::Ui,
    id: impl Into<egui::Id>,
    width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let p = style::palette(ui);
    egui::Panel::left(id)
        .resizable(true)
        .default_size(width)
        .frame(
            egui::Frame::new()
                .fill(p.card)
                .inner_margin(egui::Margin::symmetric(0, 4)),
        )
        .show(ui, add)
        .inner
}

/// The width of the list of a list tab at first and the range it resizes in.
pub fn list_split(ui: &egui::Ui) -> (f32, std::ops::RangeInclusive<f32>) {
    let width = ui.available_width();
    let max = (width - 300.0).max(320.0);
    ((width * 0.55).clamp(320.0, max), 320.0..=max)
}

/// A file dialog that opens in a folder, when the folder is there.
#[cfg(not(target_arch = "wasm32"))]
pub fn in_folder(dialog: rfd::FileDialog, dir: &Path) -> rfd::FileDialog {
    if dir.is_dir() {
        dialog.set_directory(dir)
    } else {
        dialog
    }
}
