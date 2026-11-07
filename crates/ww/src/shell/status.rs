//! The message bar and the status bar.

use eframe::egui::{self, Align, Layout, Margin, RichText, Stroke};

use super::{App, Messages, ShellAct, Window};
use crate::i18n::tr;
use crate::{icons, style};

impl<A: App> Window<A> {
    /// The message bar of the active tab, folded to one line or open with its own height that
    /// the user drags; nothing for a tab without messages.
    pub(super) fn messages_panel(&mut self, ui: &mut egui::Ui, acts: &mut Vec<A::Act>) {
        let Some(Messages {
            error,
            summary,
            clear,
        }) = self.app.messages(&self.shell)
        else {
            return;
        };
        let p = style::palette(ui);
        let frame = egui::Frame::new()
            .fill(p.msg_bg)
            .stroke(Stroke::new(1.0, p.msg_border))
            .inner_margin(Margin::symmetric(12, 4));
        let open = self.shell.messages_open;
        let header = |ui: &mut egui::Ui, acts: &mut Vec<A::Act>| {
            ui.horizontal(|ui| {
                let icon = if error { icons::ERROR } else { icons::WARNING };
                ui.label(RichText::new(icon).color(p.msg_fg));
                ui.label(RichText::new(summary).color(p.msg_fg));
                let toggle = if open { tr("Hide") } else { tr("Show") };
                if ui.link(toggle).clicked() {
                    acts.push(ShellAct::ToggleMessages.into());
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if style::icon_button(
                        ui,
                        icons::CLOSE,
                        tr("Hide the messages"),
                        true,
                        Some(p.msg_fg),
                    )
                    .clicked()
                    {
                        acts.push(clear);
                    }
                });
            });
        };
        if !open {
            egui::Panel::top("messages-bar")
                .frame(frame)
                .show_separator_line(false)
                .show(ui, |ui| header(ui, acts));
            return;
        }
        let max = (ui.available_height() * 0.6).max(120.0);
        egui::Panel::top("messages")
            .frame(frame)
            .resizable(true)
            .default_size(150.0)
            .size_range(60.0..=max)
            .show(ui, |ui| {
                header(ui, acts);
                egui::ScrollArea::vertical()
                    .id_salt("messages-list")
                    .auto_shrink(false)
                    .show(ui, |ui| self.app.messages_body(&self.shell, ui, acts));
            });
    }

    /// The status bar: the texts of the program at the left; at the right its own part, the
    /// button of the folder of the file the message names, the message, a spinner while the
    /// program is busy.
    pub(super) fn status_bar(&self, ui: &mut egui::Ui, acts: &mut Vec<A::Act>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            for text in self.app.status_left(&self.shell) {
                ui.label(style::muted(ui, text, 11.0));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.app.status_right(&self.shell, ui);
                if let Some((text, path)) = &self.shell.shown_file
                    && text == &self.shell.status
                {
                    let open = ui.small_button(tr("Open the folder"));
                    if open.on_hover_text(path.display().to_string()).clicked() {
                        acts.push(ShellAct::ShowInFolder(path.clone()).into());
                    }
                }
                // Cut at the room left after the texts of the program, so a large text size
                // never lets the message run over them.
                let status = style::muted(ui, self.shell.status.as_str(), 11.0);
                ui.add(egui::Label::new(status).truncate());
                if !self.app.jobs(&self.shell).is_empty() {
                    ui.add(egui::Spinner::new().size(style::size(ui, 12.0)));
                }
            });
        });
    }
}
