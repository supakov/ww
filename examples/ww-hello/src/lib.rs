//! The template of a native plugin of a program on W# (docs/plugins.md). It opens `.hello` files,
//! and any text whose first line starts with `Hello`, and shows their first line large in the
//! middle of the tab: the wheel and + and - change its size, a drag and the arrows move it, a
//! double click and 0 put it back. Its setting: the colour of the text. Every part a plugin has
//! is here once: the formats with their MIME type, the type found in the first bytes of a file,
//! bytes read through a `BlockFile` (a hello inside another file, or offered by another view), the
//! view with the keys and the mouse, the cursor, a command of the bar and a switch of it, the
//! items of the menu of the plugin and of the view, the context menu of the view, its text
//! offered to the other plugins (Open with), a setting, the status bar, the files it embeds (its
//! guide in three languages, a sample, the translations of its texts) and a page of them opened
//! from its menu, and tools for the MCP server of the program and a call by name: a greeting that
//! reads an option of the plugin, a count of the words of a text, and its sample opened in a tab
//! through the tool `open_file` of the program, called by its name, which an access option of the
//! plugin allows.

use std::path::Path;

use wwrt::blocks::BlockFile;
use wwrt::egui::{self, Align2, Key, Vec2};
use wwrt::input::{self, Cursor};
use wwrt::menus;
use wwrt::mime;
use wwrt::plugin::{Format, Host, Offer, Plugin, Request, Tool, View, set_setting, setting};
use wwrt::serde_json::{Value, json};
use wwrt::ww::embedded::Embedded;
use wwrt::ww::i18n::{tr, trf};
use wwrt::ww::style;
use wwrt::wwmcp::{self, Args, Call, Choice, Kind, Mode, Outcome, ToolDef, fail};

/// The name of the plugin (translated by the host).
const NAME: &str = "Hello, the template of a plugin";
/// The MIME type of a hello.
const MIME: &str = "application/x-ww-hello";
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
    mimes: &[MIME],
}];

/// The files the plugin embeds: its guide and its translations, a sample, and the texts of the
/// plugin in other languages (`lang/<code>/ui.json`, in the form of the `ui.json` of a program on
/// W#), which the host adds to the window. Two languages here; a plugin gives the ones it has.
static FILES: Embedded = wwrt::ww::embed![
    "docs/guide.md",
    "lang/uk/docs/guide.md",
    "lang/ru/docs/guide.md",
    "samples/sample.hello",
    "lang/uk/ui.json",
    "lang/ru/ui.json",
];

/// The page of the guide of the plugin, among its files.
const GUIDE: &str = "docs/guide.md";
/// The sample of the plugin, among its files.
const SAMPLE: &str = "samples/sample.hello";
/// The id of the plugin, the name of its DLL: the caller of the tools it calls.
const ID: &str = "ww_hello";

/// A greeting of a name in the manner of the option `greeting`.
fn greeting(name: &str, manner: &str) -> String {
    let name = if name.trim().is_empty() {
        "world"
    } else {
        name.trim()
    };
    if manner == "warm" {
        format!("Hello, {name}, so good to see you!")
    } else {
        format!("Hello, {name}!")
    }
}

/// The letters, the words and the lines of a text.
fn counted(text: &str) -> Value {
    json!({
        "letters": text.chars().filter(|c| c.is_alphabetic()).count(),
        "words": text.split_whitespace().count(),
        "lines": text.lines().count(),
    })
}

/// The view of the first line of a text.
fn hello(text: &str) -> Box<dyn View> {
    let line = text.lines().next().unwrap_or("").trim();
    let text = if line.is_empty() { "Hello" } else { line };
    Box::new(HelloView {
        text: text.to_owned(),
        size: SIZE,
        offset: Vec2::ZERO,
        framed: false,
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

    fn opens_bytes(&self) -> bool {
        true
    }

    /// A hello of some bytes: a part of another file shown inside its view, or the bytes
    /// another view offers; the plugin reads them whole, up to its limit.
    fn open_bytes(
        &self,
        _name: &str,
        _mime: &str,
        bytes: BlockFile,
        _settings: &Value,
    ) -> Result<Box<dyn View>, String> {
        if bytes.len() > MAX_BYTES {
            return Err(tr("The file is too large for a hello").to_owned());
        }
        let bytes = bytes.to_vec(MAX_BYTES)?;
        Ok(hello(&String::from_utf8_lossy(&bytes)))
    }

    /// A text whose first line starts with `Hello` is a hello, whatever the name of its file.
    fn sniff(&self, head: &[u8]) -> Option<&'static str> {
        let text = mime::text(head)?;
        text.trim_start().starts_with("Hello").then_some(MIME)
    }

    fn files(&self) -> Embedded {
        FILES
    }

    /// The items in the submenu of the plugin in the menu of the window.
    fn menu(&self) -> Vec<Tool> {
        vec![
            Tool {
                id: "say",
                icon: "\u{1F44B}",
                label: "Say hello",
            },
            Tool {
                id: "guide",
                icon: "\u{1F4D6}",
                label: "Guide of the plugin",
            },
        ]
    }

    fn act(&self, id: &str, host: &mut Host) {
        match id {
            "say" => {
                let line = tr("Hello from the menu of the plugin").to_owned();
                host.requests.push(Request::Status(line));
            }
            "guide" => host
                .requests
                .push(Request::OpenEmbedded(FILES, GUIDE.to_owned())),
            _ => {}
        }
    }

    /// Its tools: a group of the plugin, each with the mode and the options it needs.
    fn tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef::new(
                "hello_greet",
                "hello",
                "Greets a name in the manner the option greeting of the plugin sets.",
                wwmcp::schema(json!({ "name": wwmcp::text("The name to greet") }), &[]),
            ),
            ToolDef::new(
                "hello_count",
                "hello",
                "Counts the letters, the words and the lines of a text.",
                wwmcp::schema(json!({ "text": wwmcp::text("The text") }), &["text"]),
            ),
            ToolDef::new(
                "hello_sample",
                "hello",
                "Writes the sample hello into the temporary folder and opens it in a tab of the window, through the tool open_file of the program.",
                wwmcp::no_arguments(),
            )
            .mode(Mode::Files)
            .needs("open_tabs", "on"),
        ]
    }

    /// Its options: a value a tool reads, and an access setting a tool needs.
    fn tool_options(&self) -> Vec<Choice> {
        vec![
            Choice::of(
                "greeting",
                "Greeting",
                &[("plain", "Plain"), ("warm", "Warm")],
            ),
            Choice::switch("open_tabs", "May open tabs of the window")
                .help("hello_sample opens its sample through the tool open_file of the program"),
        ]
    }

    fn call_tool(&self, name: &str, args: &Value, call: &Call<'_>) -> Outcome {
        let args = Args(args);
        match name {
            "hello_greet" => Ok(json!({
                "text": greeting(&args.optional_text("name"), call.option("greeting")),
            })),
            "hello_count" => Ok(counted(&args.text("text")?)),
            "hello_sample" => {
                let bytes = FILES
                    .get(SAMPLE)
                    .map(|f| f.bytes)
                    .ok_or_else(|| fail(Kind::NotFound, "the sample is not embedded"))?;
                let path = std::env::temp_dir().join("ww-hello-sample.hello");
                std::fs::write(&path, bytes).map_err(|e| fail(Kind::WriteFailed, e.to_string()))?;
                // A tool of the program, by its name: the switches of the server apply to it.
                let opened = wwrt::tools::call(
                    "open_file",
                    &json!({ "path": path.display().to_string() }),
                    ID,
                )?;
                Ok(json!({ "path": path.display().to_string(), "tab": opened }))
            }
            _ => Err(fail(Kind::NotFound, format!("no tool {name}"))),
        }
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
    /// A frame around the text: the switch of the bar.
    framed: bool,
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
            let (response, input) = input::area(ui, ui.id().with("hello"), rect, &KEYS);
            // The context menu of the view on its area: its items and Open with for its text.
            menus::context(&response, self, host);
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
            let painter = ui.painter_at(rect);
            let shown = painter.text(
                rect.center() + self.offset,
                Align2::CENTER_CENTER,
                &self.text,
                style::font(ui, self.size),
                colour,
            );
            if self.framed {
                let stroke = egui::Stroke::new(2.0, colour);
                painter.rect_stroke(shown.expand(8.0), 6, stroke, egui::StrokeKind::Outside);
            }
        });
    }

    fn tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                id: "reset",
                icon: "\u{21BA}",
                label: "Reset",
            },
            Tool {
                id: "frame",
                icon: "\u{25A2}",
                label: "Frame",
            },
        ]
    }

    /// The switch of the bar, drawn pressed while it is on.
    fn pressed(&self, id: &str) -> bool {
        id == "frame" && self.framed
    }

    /// The items of the context menu of the view and of its submenu in the window.
    fn context(&self) -> Vec<Tool> {
        let mut items = self.menu();
        items.extend(self.tools().into_iter().take(1));
        items
    }

    /// The text of the hello, which a plugin of texts opens.
    fn offer(&self) -> Option<Offer> {
        Some(Offer {
            name: self.text.clone(),
            mime: Some("text/plain".to_owned()),
        })
    }

    fn offered(&mut self) -> Result<BlockFile, String> {
        Ok(BlockFile::from_bytes(self.text.as_bytes().to_vec()))
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
            "frame" => self.framed = !self.framed,
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
    fn the_items_of_the_menu_and_some_bytes() {
        let settings = Value::Null;
        let mut host = Host::new(true, wwrt::ww::PageWidth::View, &settings);
        let plugin = HelloPlugin;
        assert_eq!(plugin.menu()[0].id, "say");
        plugin.act("say", &mut host);
        assert!(matches!(&host.requests[..], [Request::Status(s)] if s.starts_with("Hello")));
        plugin.act("guide", &mut host);
        assert!(
            matches!(&host.requests[1], Request::OpenEmbedded(f, p) if f.same(&FILES) && p == GUIDE)
        );
        let bytes = BlockFile::from_bytes(b"Hi there\nmore".to_vec());
        let mut view = plugin.open_bytes("a", MIME, bytes, &settings).unwrap();
        assert!(view.menu().iter().any(|t| t.id == "bigger"));
        assert!(view.context().iter().any(|t| t.id == "reset"));
        view.act("bigger", &mut host);
        assert!(view.status().starts_with("58 points"), "{}", view.status());
        assert!(!view.pressed("frame"));
        view.act("frame", &mut host);
        assert!(view.pressed("frame"), "the switch is on");
        assert_eq!(view.offered().unwrap().to_vec(64).unwrap(), b"Hi there");
        assert!(plugin.opens_bytes());
        let big = BlockFile::from_bytes(vec![b'x'; MAX_BYTES as usize + 1]);
        assert!(plugin.open_bytes("b", "", big, &settings).is_err());
    }

    #[test]
    fn a_hello_is_found_by_its_first_line_and_embeds_its_files() {
        let plugin = HelloPlugin;
        assert_eq!(plugin.sniff(b"  Hello, world"), Some(MIME));
        assert_eq!(plugin.sniff(b"Goodbye"), None);
        assert_eq!(plugin.sniff(&[0xFF, 0x00, 0x01]), None);
        let files = plugin.files();
        let sample = files.get("samples/sample.hello").unwrap();
        assert_eq!(
            plugin.sniff(sample.bytes),
            Some(MIME),
            "the sample is a hello"
        );
        let uk = files.localized(GUIDE, "uk").unwrap();
        assert_eq!(uk.path, "lang/uk/docs/guide.md");
        let link = wwrt::ww::embedded::join(GUIDE, "../samples/sample.hello");
        assert_eq!(link.as_deref(), Some("samples/sample.hello"));
        let codes: Vec<&str> = files.translations().iter().map(|(c, _)| *c).collect();
        assert_eq!(codes, ["uk", "ru"]);
    }

    #[test]
    fn the_tools_greet_count_and_need_their_option() {
        use std::collections::BTreeMap;
        let plugin = HelloPlugin;
        let names: Vec<String> = plugin.tools().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["hello_greet", "hello_count", "hello_sample"]);
        let warm = BTreeMap::from([("greeting".to_owned(), "warm".to_owned())]);
        let call = Call {
            mode: Mode::Read,
            options: &warm,
            caller: &wwmcp::Caller::Program,
        };
        let greeted = plugin
            .call_tool("hello_greet", &json!({ "name": "Ann" }), &call)
            .unwrap();
        assert_eq!(greeted["text"], "Hello, Ann, so good to see you!");
        let counted = plugin
            .call_tool(
                "hello_count",
                &json!({ "text": "Hello, big world\nagain" }),
                &call,
            )
            .unwrap();
        assert_eq!(counted, json!({ "letters": 18, "words": 4, "lines": 2 }));
        let none = plugin
            .call_tool("hello_none", &json!({}), &call)
            .unwrap_err();
        assert_eq!(none.kind, Kind::NotFound);
        let sample = &plugin.tools()[2];
        assert_eq!(
            (sample.mode, sample.needs[0].option.as_str()),
            (Mode::Files, "open_tabs")
        );
    }

    #[test]
    fn every_text_of_the_plugin_is_translated() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let wanted = wwrt::testing::plugin_texts(&src);
        let missing = wwrt::testing::missing(&FILES.translations(), &wanted);
        assert!(missing.is_empty(), "{missing:#?}");
    }
}
