//! The panel of the work in the background over the window, in its lower right corner: what
//! runs, how long it has taken, a Stop for the work that can stop; folded, a small spinner
//! with the count of the works.

use std::time::Duration;

use eframe::egui::{self, Align2};
use ww_base::clock::Instant;

use super::{App, ShellAct, Window};
use crate::i18n::{tr, trf};
use crate::{icons, style};

/// A long work of the program in the background, shown in the panel of the work.
pub struct Job<T> {
    /// What runs, translated.
    pub title: String,
    /// More about it (a file, a folder, how far it is); empty for nothing more.
    pub detail: String,
    /// When it started, for the time it has taken.
    pub since: Option<Instant>,
    /// The act of its Stop button, when it can stop.
    pub stop: Option<T>,
}

impl<T> Job<T> {
    /// A work by its title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: String::new(),
            since: None,
            stop: None,
        }
    }

    /// The work with more about it.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// The work with the time it started.
    pub fn since(mut self, at: Instant) -> Self {
        self.since = Some(at);
        self
    }

    /// The work with the act of its Stop button.
    pub fn stop(mut self, act: T) -> Self {
        self.stop = Some(act);
        self
    }
}

/// The time a work has taken: seconds, then minutes and seconds.
fn elapsed(since: Instant) -> String {
    let s = since.elapsed().as_secs();
    if s < 60 {
        trf("{n} s", &[("n", &s)])
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// The widest the panel grows.
const WIDTH: f32 = 360.0;

impl<A: App> Window<A> {
    /// The panel of the work of the program over the window, above the status bar; nothing when
    /// nothing runs.
    pub(super) fn jobs_view(&self, ctx: &egui::Context, acts: &mut Vec<A::Act>) {
        let jobs = self.app.jobs(&self.shell);
        if jobs.is_empty() {
            return;
        }
        let folded = self.shell.jobs_folded;
        egui::Area::new(egui::Id::new("ww-jobs"))
            .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-14.0, -38.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let p = style::palette(ui);
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::same(10))
                    .shadow(ui.visuals().popup_shadow)
                    .show(ui, |ui| {
                        if folded {
                            ui.horizontal(|ui| {
                                ui.add(egui::Spinner::new().size(style::size(ui, 14.0)));
                                let count = trf("{n} running", &[("n", &jobs.len())]);
                                ui.label(style::muted(ui, count, 12.0));
                                let show = tr("Show the work in the background");
                                if style::icon_button(ui, icons::UP, show, true, None).clicked() {
                                    acts.push(ShellAct::FoldJobs(false).into());
                                }
                            });
                            return;
                        }
                        ui.set_max_width(WIDTH);
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(style::size(ui, 14.0)));
                            ui.label(style::strong(ui, tr("In the background"), 13.0));
                            ui.add_space(24.0);
                            let fold = tr("Fold the panel");
                            if style::icon_button(ui, icons::DOWN, fold, true, None).clicked() {
                                acts.push(ShellAct::FoldJobs(true).into());
                            }
                        });
                        for job in jobs {
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                ui.label(style::strong(ui, &job.title, 12.5));
                                if let Some(since) = job.since {
                                    ui.label(style::muted(ui, elapsed(since), 11.0));
                                }
                            });
                            if !job.detail.is_empty() {
                                let detail = style::muted(ui, &job.detail, 11.0);
                                ui.add(egui::Label::new(detail).wrap());
                            }
                            if let Some(stop) = job.stop
                                && ui.button(tr("Stop")).clicked()
                            {
                                acts.push(stop);
                            }
                        }
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}
