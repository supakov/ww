//! The start of a window: the settings file, the renderers to try, the first frame; and the
//! smoke test of the renderers for CI.

use eframe::egui;

use crate::i18n::trf;
use crate::renderer::{self, Renderer};
use crate::settings::Settings;
use crate::shell::{App, Window};
use crate::style;

pub use crate::shell::{WINDOW_MIN_SIZE, WINDOW_SIZE};

/// The size of the window of the smoke test.
const SMOKE_WINDOW_SIZE: [f32; 2] = [400.0, 300.0];

/// The renderer of `--renderer` (an error for a name this executable lacks), else of the
/// settings (Automatic for a name it lacks); `None` is Automatic.
pub fn chosen_renderer(cli: Option<&str>, setting: &str) -> Result<Option<Renderer>, String> {
    match cli.map(str::trim) {
        Some("auto") => Ok(None),
        Some(name) => Renderer::parse(name).map(Some).ok_or_else(|| {
            let names: Vec<&str> = Renderer::ALL.iter().map(|r| r.id()).collect();
            format!(
                "unknown renderer {name}: this executable has auto, {}",
                names.join(", ")
            )
        }),
        None => Ok(Renderer::parse(setting)),
    }
}

/// Opens a small window with the renderers of `--renderer` or the settings, draws two frames and
/// closes it: what it drew with and the renderers that did not start before. For CI
/// (`--smoke-window`).
pub fn smoke_window<A: App>(cli_renderer: Option<&str>) -> Result<String, String> {
    struct Smoke {
        name: &'static str,
        frames: u32,
    }
    impl eframe::App for Smoke {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            egui::CentralPanel::default().show(ui, |ui| ui.label(self.name));
            self.frames += 1;
            if self.frames >= 2 {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ui.ctx().request_repaint();
            }
        }
    }
    let settings = Settings::<A::Extra>::load(A::name()).0.unwrap_or_default();
    let order = renderer::order(chosen_renderer(cli_renderer, &settings.renderer)?);
    let info = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let title = format!("{} smoke test", A::name());
    let viewport = egui::ViewportBuilder::default()
        .with_title(&title)
        .with_inner_size(SMOKE_WINDOW_SIZE);
    let (_, failures) = renderer::start(&title, &viewport, &order, |_, _| {
        let seen = info.clone();
        Box::new(move |cc| {
            *seen.lock().unwrap_or_else(|e| e.into_inner()) = renderer::graphics_text(cc);
            Ok(Box::new(Smoke {
                name: A::name(),
                frames: 0,
            }))
        })
    })?;
    let mut text = info.lock().unwrap_or_else(|e| e.into_inner()).clone();
    for failure in failures {
        text.push_str("; ");
        text.push_str(&failure);
    }
    Ok(text)
}

/// Opens the window of a program with the renderer of `--renderer` or of the settings first;
/// `make` builds the program from the settings, once per renderer tried. The error text when the
/// window cannot start.
pub fn run<A: App>(
    cli_renderer: Option<&str>,
    make: impl Fn(&Settings<A::Extra>) -> A + Clone + 'static,
) -> Result<(), String> {
    let (loaded, settings_path) = Settings::<A::Extra>::load(A::name());
    let (settings, warning) = match loaded {
        Ok(s) => (s, None),
        Err(e) => (Settings::default(), Some(e)),
    };
    let order = renderer::order(chosen_renderer(cli_renderer, &settings.renderer)?);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(A::name())
        .with_inner_size(WINDOW_SIZE)
        .with_min_inner_size(WINDOW_MIN_SIZE);
    // Where the window was at the last close; the first frame fits it to the monitor.
    if let Some(w) = &settings.window {
        viewport = viewport
            .with_inner_size([w.width, w.height])
            .with_position([w.x, w.y])
            .with_maximized(w.maximized);
    }
    renderer::start(A::name(), &viewport, &order, |drawn, failures| {
        let make = make.clone();
        let settings = settings.clone();
        let settings_path = settings_path.clone();
        let warning = warning.clone();
        let failures = failures.to_vec();
        Box::new(move |cc| {
            style::install_fonts(&cc.egui_ctx);
            let app = make(&settings);
            let mut window = Window::new(app, settings);
            window.shell.settings_path = settings_path;
            window.shell.graphics = renderer::graphics_text(cc);
            window.shell.log(
                "INFO",
                &format!(
                    "{} {} started; graphics: {}",
                    A::name(),
                    A::BUILD,
                    window.shell.graphics
                ),
            );
            window.shell.apply_language();
            window.app.load(&mut window.shell);
            if let Some(w) = warning {
                window.shell.status = trf(
                    "Settings not read, the defaults are used: {w}",
                    &[("w", &w)],
                );
            }
            window.restore_session();
            window.app.restored(&mut window.shell);
            if !failures.is_empty() {
                window.shell.status = trf(
                    "{failed}; the window draws with {renderer}",
                    &[
                        ("failed", &failures.join("; ")),
                        ("renderer", &drawn.title()),
                    ],
                );
            }
            Ok(Box::new(window))
        })
    })
    .map(|_| ())
}
