//! The command bar of the active tab, drawn from the items the program gives.

use eframe::egui;

use super::{App, Window};
use crate::commands::Item;
use crate::style;

impl<A: App> Window<A> {
    /// The command bar of the active tab; `compact` draws the icons alone.
    pub(super) fn toolbar(&self, ui: &mut egui::Ui, acts: &mut Vec<A::Act>, compact: bool) {
        for item in self.app.commands(&self.shell) {
            match item {
                Item::Separator => {
                    ui.add_space(2.0);
                    ui.separator();
                    ui.add_space(2.0);
                }
                Item::Command(c) => {
                    let hint = match c.keys {
                        Some(k) => format!("{} ({})", c.label, k.text),
                        None => c.label.to_owned(),
                    };
                    let enabled = c.act.is_some();
                    // The background of a switch that is on, painted under the button.
                    let back = ui.painter().add(egui::Shape::Noop);
                    let button =
                        style::tool_button(ui, c.icon, c.label, &hint, enabled, compact, false);
                    if c.pressed {
                        let p = style::palette(ui);
                        let shape = egui::Shape::rect_filled(button.rect, 6, p.sel);
                        ui.painter().set(back, shape);
                    }
                    if button.clicked()
                        && let Some(act) = c.act
                    {
                        acts.push(act);
                    }
                }
                Item::Menu(menu) => {
                    let button = style::tool_button(
                        ui, menu.icon, menu.label, menu.label, true, compact, true,
                    );
                    egui::Popup::menu(&button).show(|ui| {
                        ui.set_min_width(menu.width);
                        self.app.menu_popup(&self.shell, menu.id, ui, acts);
                    });
                }
            }
        }
    }
}
