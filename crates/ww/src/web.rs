//! The start of a window in a browser (wasm32): the program in a canvas of the page through the
//! `WebRunner` of eframe, with its settings and drafts in the storage of the page (`store`) and
//! its log in the console. Natively a window starts through `run`.

use eframe::wasm_bindgen::JsCast;

use crate::i18n::trf;
use crate::renderer;
use crate::settings::Settings;
use crate::shell::{App, Window};
use crate::style;

/// The id of the element of the page that shows a text while the program loads: removed when the
/// window is up, given the failure when the start fails.
const LOADING_ID: &str = "loading_text";

/// Starts the window of a program in the canvas of that id of the page and runs it until the
/// page closes; `make` builds the program from the settings. The error text when the page has
/// no such canvas or eframe cannot start, also shown in the page and the console.
pub async fn start<A: App>(
    canvas_id: &str,
    make: impl Fn(&Settings<A::Extra>) -> A + 'static,
) -> Result<(), String> {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    match run::<A>(canvas_id, make).await {
        Ok(()) => {
            if let Some(loading) = element(LOADING_ID) {
                loading.remove();
            }
            Ok(())
        }
        Err(e) => {
            failed(&e);
            Err(e)
        }
    }
}

/// Shows a failure of the start in the page and the console.
pub fn failed(text: &str) {
    web_sys::console::error_1(&text.into());
    if let Some(loading) = element(LOADING_ID) {
        loading.set_text_content(Some(text));
    }
}

fn element(id: &str) -> Option<web_sys::Element> {
    web_sys::window()?.document()?.get_element_by_id(id)
}

async fn run<A: App>(
    canvas_id: &str,
    make: impl Fn(&Settings<A::Extra>) -> A + 'static,
) -> Result<(), String> {
    let canvas = element(canvas_id)
        .ok_or_else(|| format!("the page has no element {canvas_id}"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| format!("{canvas_id} is not a canvas"))?;
    let (loaded, settings_path) = Settings::<A::Extra>::load(A::name());
    let (settings, warning) = match loaded {
        Ok(s) => (s, None),
        Err(e) => (Settings::default(), Some(e)),
    };
    let creator: eframe::AppCreator<'static> = Box::new(move |cc| {
        style::install_fonts(&cc.egui_ctx);
        let app = make(&settings);
        let mut window = Window::new(app, settings);
        window.shell.settings_path = settings_path;
        window.shell.graphics = renderer::graphics_text(cc);
        window.shell.log(
            "INFO",
            &format!(
                "{} {} started in a browser; graphics: {}",
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
        Ok(Box::new(window))
    });
    eframe::WebRunner::new()
        .start(canvas, eframe::WebOptions::default(), creator)
        .await
        .map_err(|e| format!("{e:?}"))
}
