//! The template of a native plugin of wview (the SDK, docs/plugins.md). It opens `.hello` files
//! and shows their first line large in the middle of the tab: the wheel and + and - change its
//! size, a drag and the arrows move it, a double click and 0 put it back. Its setting: the colour
//! of the text. Every part a plugin has is here once: the formats with their MIME type, bytes in
//! memory (a hello inside another file), the view with the keys and the mouse, the cursor, a
//! command of the bar, the items of the menu of the plugin and of the view, a setting, the status
//! bar and the translations.

use std::path::Path;

use wwrt::egui::{self, Align2, Key, Vec2};
use wwrt::input::{self, Cursor};
use wwrt::plugin::{Format, Host, Plugin, Request, Tool, View, set_setting, setting};
use wwrt::serde_json::Value;
use wwrt::ww::i18n::{tr, trf};
use wwrt::ww::style;

/// The name of the plugin (translated by the host).
const NAME: &str = "Hello, the template of a plugin";
/// The largest file the plugin reads.
const MAX_BYTES: u64 = 64 * 1024;
/// The size of the text at first, in points.
const SIZE: f32 = 48.0;
/// The keys of the view while it has the focus.
const KEYS: [Key; 8] = [
    Key::Plus,
    Key::Equals,
    Key::Minus,
    Key::Num0,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::ArrowUp,
    Key::ArrowDown,
];

static FORMATS: [Format; 1] = [Format {
    name: "Hello files",
    extensions: &["hello"],
    mimes: &["application/x-wview-hello"],
}];

/// The texts of the plugin in other languages: a file `lang/<code>.json` each, in the form of the
/// `ui.json` of a program on W#. Two of them here; a plugin gives the ones it has.
static TRANSLATIONS: [(&str, &str); 2] = [
    ("uk", include_str!("../lang/uk.json")),
    ("ru", include_str!("../lang/ru.json")),
];

/// The view of the first line of a text.
fn hello(text: &str) -> Box<dyn View> {
    let line = text.lines().next().unwrap_or("").trim();
    let text = if line.is_empty() { "Hello" } else { line };
    Box::new(HelloView {
        text: text.to_owned(),
        size: SIZE,
        offset: Vec2::ZERO,
    })
}

pub struct HelloPlugin;

impl Plugin for HelloPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn formats(&self) -> &'static [Format] {
        &FORMATS
    }

    fn open(&self, path: &Path, _settings: &Value) -> Result<Box<dyn View>, String> {
        let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if size > MAX_BYTES {
            return Err(tr("The file is too large for a hello").to_owned());
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Ok(hello(&text))
    }

    /// A hello of bytes in memory, a part of another file shown inside its view.
    fn open_bytes(
        &self,
        _name: &str,
        _mime: &str,
        bytes: Vec<u8>,
        _settings: &Value,
    ) -> Result<Box<dyn View>, String> {
        if bytes.len() as u64 > MAX_BYTES {
            return Err(tr("The file is too large for a hello").to_owned());
        }
        Ok(hello(&String::from_utf8_lossy(&bytes)))
    }

    /// An item in the submenu of the plugin in the menu of the window.
    fn menu(&self) -> Vec<Tool> {
        vec![Tool {
            id: "say",
            icon: "\u{1F44B}",
            label: "Say hello",
        }]
    }

    fn act(&self, id: &str, host: &mut Host) {
        if id == "say" {
            let line = tr("Hello from the menu of the plugin").to_owned();
            host.requests.push(Request::Status(line));
        }
    }

    fn translations(&self) -> &'static [(&'static str, &'static str)] {
        &TRANSLATIONS
    }

    fn has_settings(&self) -> bool {
        true
    }

    fn settings_ui(&self, ui: &mut egui::Ui, settings: &mut Value) -> bool {
        let mut accent: bool = setting(settings, "accent", true);
        let changed = ui
            .checkbox(&mut accent, tr("Draw the text in the accent colour"))
            .changed();
        if changed {
            set_setting(settings, "accent", accent);
        }
        changed
    }
}

struct HelloView {
    text: String,
    size: f32,
    /// Where the text is from the centre of the view, in points.
    offset: Vec2,
}

impl HelloView {
    fn reset(&mut self) {
        self.size = SIZE;
        self.offset = Vec2::ZERO;
    }
}

impl View for HelloView {
    fn ui(&mut self, ui: &mut egui::Ui, host: &mut Host) {
        let accent: bool = setting(host.settings, "accent", true);
        egui::CentralPanel::default().show(ui, |ui| {
            let rect = ui.max_rect();
            let (_, input) = input::area(ui, ui.id().with("hello"), rect, &KEYS);
            for (key, _) in &input.keys {
                match key {
                    Key::Plus | Key::Equals => self.size = (self.size * 1.2).min(400.0),
                    Key::Minus => self.size = (self.size / 1.2).max(8.0),
                    Key::Num0 => self.reset(),
                    Key::ArrowLeft => self.offset.x -= 10.0,
                    Key::ArrowRight => self.offset.x += 10.0,
                    Key::ArrowUp => self.offset.y -= 10.0,
                    Key::ArrowDown => self.offset.y += 10.0,
                    _ => {}
                }
            }
            if input.wheel.y != 0.0 {
                self.size = (self.size * (-input.wheel.y * 0.002).exp()).clamp(8.0, 400.0);
            }
            if input.double_clicked {
                self.reset();
            }
            if input.dragging {
                self.offset += input.drag;
                input::cursor(ui, Cursor::Grabbing);
            } else if input.hovered {
                input::cursor(ui, Cursor::Grab);
            }
            let colour = if accent {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().text_color()
            };
            ui.painter_at(rect).text(
                rect.center() + self.offset,
                Align2::CENTER_CENTER,
                &self.text,
                style::font(ui, self.size),
                colour,
            );
        });
    }

    fn tools(&self) -> Vec<Tool> {
        vec![Tool {
            id: "reset",
            icon: "\u{21BA}",
            label: "Reset",
        }]
    }

    /// Items of the view in the menu of the window, answered by `act` as the tools are.
    fn menu(&self) -> Vec<Tool> {
        vec![
            Tool {
                id: "bigger",
                icon: "\u{2795}",
                label: "Bigger",
            },
            Tool {
                id: "smaller",
                icon: "\u{2796}",
                label: "Smaller",
            },
        ]
    }

    fn act(&mut self, id: &str, _host: &mut Host) {
        match id {
            "reset" => self.reset(),
            "bigger" => self.size = (self.size * 1.2).min(400.0),
            "smaller" => self.size = (self.size / 1.2).max(8.0),
            _ => {}
        }
    }

    fn status(&self) -> String {
        trf(
            "{size} points, moved by {x}, {y}",
            &[
                ("size", &format!("{:.0}", self.size)),
                ("x", &format!("{:.0}", self.offset.x)),
                ("y", &format!("{:.0}", self.offset.y)),
            ],
        )
    }
}

wwrt::plugin!(HelloPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_items_of_the_menu_and_bytes_in_memory() {
        let settings = Value::Null;
        let mut host = Host::new(true, wwrt::ww::PageWidth::View, &settings);
        let plugin = HelloPlugin;
        assert_eq!(plugin.menu()[0].id, "say");
        plugin.act("say", &mut host);
        assert!(matches!(&host.requests[..], [Request::Status(s)] if s.starts_with("Hello")));
        let mut view = plugin
            .open_bytes(
                "a",
                "application/x-wview-hello",
                b"Hi there\nmore".to_vec(),
                &settings,
            )
            .unwrap();
        assert!(view.menu().iter().any(|t| t.id == "bigger"));
        view.act("bigger", &mut host);
        assert!(view.status().starts_with("58 points"), "{}", view.status());
        let big = vec![b'x'; MAX_BYTES as usize + 1];
        assert!(plugin.open_bytes("b", "", big, &settings).is_err());
    }

    #[test]
    fn every_text_of_the_plugin_is_translated() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let wanted = wwrt::testing::plugin_texts(&src);
        let missing = wwrt::testing::missing(&super::TRANSLATIONS, &wanted);
        assert!(missing.is_empty(), "{missing:#?}");
    }
}
