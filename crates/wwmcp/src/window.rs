//! The tools of the window of a program of W# (the feature `ui`): the tabs listed, a file opened
//! in a tab, a tab closed, a tab saved, and the tools the program adds that act on its state. A
//! call comes on a thread of the server and is answered at the next frame of the window
//! (`answer`, which the program calls in its `App::draw`).

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use ww::eframe::egui;
use ww::{App, Document, Shell};

use crate::bridge::{Bridge, WAIT};
use crate::tools::{
    Args, Call, Kind, Mode, Outcome, Source, ToolDef, fail, no_arguments, schema, text,
};

/// What a tool asks of the window.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowAsk {
    List,
    Open(PathBuf),
    /// A tab by its number from 1 or its title; whether its changes may be lost.
    Close(Value, bool),
    Save(Value),
    /// A tool of the program that acts on its state, with its arguments.
    Program(String, Value),
}

/// The bridge of the window.
pub type WindowBridge = Bridge<WindowAsk>;

/// The answer of the program to a tool of its own that acts on its state.
pub type ProgramTool<'a, A> = dyn FnMut(&mut A, &mut Shell<A>, &str, &Value) -> Outcome + 'a;

/// The tools of the window, a source of the server.
pub struct WindowTools {
    pub bridge: Arc<WindowBridge>,
}

impl WindowTools {
    pub fn new(bridge: Arc<WindowBridge>) -> Arc<Self> {
        Arc::new(Self { bridge })
    }
}

fn tab_property() -> Value {
    json!({ "type": ["integer", "string"], "description": "The tab: its number from 1, in the order of the tabs, or its title" })
}

impl Source for WindowTools {
    fn id(&self) -> String {
        "window".to_owned()
    }

    fn title(&self) -> String {
        "The window".to_owned()
    }

    fn tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef::new(
                "list_files",
                "files",
                "The tabs of the window: the number, the title, the file and whether it has changes that are not saved.",
                no_arguments(),
            ),
            ToolDef::new(
                "open_file",
                "files",
                "Opens a file in a tab of the window, by the plugin of its type.",
                schema(json!({ "path": text("The path of the file") }), &["path"]),
            )
            .mode(Mode::Edit),
            ToolDef::new(
                "close_file",
                "files",
                "Closes a tab of the window; a tab with changes that are not saved stays open unless discard is true.",
                schema(
                    json!({ "tab": tab_property(), "discard": { "type": "boolean", "description": "Lose the changes that are not saved" } }),
                    &["tab"],
                ),
            )
            .mode(Mode::Edit),
            ToolDef::new(
                "save_file",
                "files",
                "Saves a tab of the window into its file.",
                schema(json!({ "tab": tab_property() }), &["tab"]),
            )
            .mode(Mode::Files),
        ]
    }

    fn call(&self, tool: &str, args: &Value, _call: &Call<'_>) -> Outcome {
        let a = Args(args);
        let tab = || {
            a.get("tab")
                .cloned()
                .ok_or_else(|| fail(Kind::Invalid, "the argument tab is needed"))
        };
        let ask = match tool {
            "list_files" => WindowAsk::List,
            "open_file" => WindowAsk::Open(PathBuf::from(a.text("path")?)),
            "close_file" => WindowAsk::Close(tab()?, a.flag_or("discard", false)),
            "save_file" => WindowAsk::Save(tab()?),
            _ => return Err(fail(Kind::NotFound, format!("no tool {tool}"))),
        };
        self.bridge.ask(ask, WAIT)
    }
}

/// The tools of the program that act on its state (a new note, a command): each call is answered
/// by the program at a frame of its window (`WindowAsk::Program`, the `program` of `answer`).
pub struct ProgramTools {
    pub id: String,
    pub title: String,
    pub tools: Vec<ToolDef>,
    pub bridge: Arc<WindowBridge>,
}

impl Source for ProgramTools {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn tools(&self) -> Vec<ToolDef> {
        self.tools.clone()
    }

    fn call(&self, tool: &str, args: &Value, _call: &Call<'_>) -> Outcome {
        let ask = WindowAsk::Program(tool.to_owned(), args.clone());
        self.bridge.ask(ask, WAIT)
    }
}

/// The place of a tab named by its number from 1 or its title (or the name of its file).
fn tab_of<A: App>(shell: &Shell<A>, tab: &Value) -> Result<usize, crate::ToolError> {
    let found = match tab {
        Value::Number(n) => n
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| (1..=shell.docs.len()).contains(n))
            .map(|n| n - 1),
        Value::String(name) => shell.docs.iter().position(|d| {
            d.title() == *name
                || d.path()
                    .and_then(|p| p.file_name())
                    .is_some_and(|f| f.to_string_lossy() == *name)
        }),
        _ => None,
    };
    found.ok_or_else(|| fail(Kind::NotFound, format!("no tab {tab}")))
}

/// The tab of a place, for an answer.
fn described<A: App>(shell: &Shell<A>, i: usize) -> Value {
    let d = &shell.docs[i];
    json!({
        "tab": i + 1,
        "title": d.title(),
        "path": d.path().map(|p| p.display().to_string()),
        "changed": d.dirty(),
        "active": i == shell.active,
    })
}

/// Answers the calls of the tools of the window waiting, at a frame of it: `program` answers the
/// tools of the program itself (`WindowAsk::Program`). The first call sets what wakes the window.
pub fn answer<A: App>(
    app: &mut A,
    shell: &mut Shell<A>,
    bridge: &WindowBridge,
    ctx: &egui::Context,
    program: &mut ProgramTool<'_, A>,
) {
    if !bridge.has_waker() {
        let ctx = ctx.clone();
        bridge.set_waker(move || ctx.request_repaint());
    }
    bridge.answer(|ask| match ask {
        WindowAsk::List => Ok(Value::Array(
            (0..shell.docs.len()).map(|i| described(shell, i)).collect(),
        )),
        WindowAsk::Open(path) => {
            if !path.is_file() {
                return Err(fail(
                    Kind::NotFound,
                    format!("no file {}", path.display()),
                ));
            }
            let before = shell.docs.len();
            app.open_path(shell, path.clone());
            if shell.docs.len() > before || shell.docs.iter().any(|d| d.path() == Some(&path)) {
                let i = shell
                    .docs
                    .iter()
                    .position(|d| d.path() == Some(&path))
                    .unwrap_or(shell.active);
                Ok(described(shell, i))
            } else {
                Err(fail(Kind::ReadFailed, shell.status.clone()))
            }
        }
        WindowAsk::Close(tab, discard) => {
            let i = tab_of(shell, &tab)?;
            if shell.docs[i].dirty() && !discard {
                return Err(fail(
                    Kind::NotAllowed,
                    "the tab has changes that are not saved; save it, or close it with discard true",
                ));
            }
            let closed = described(shell, i);
            shell.close_tab(i, true);
            Ok(closed)
        }
        WindowAsk::Save(tab) => {
            let i = tab_of(shell, &tab)?;
            if shell.docs[i].path().is_none() {
                return Err(fail(
                    Kind::NotAllowed,
                    "the tab has no file; save it as a file in the window first",
                ));
            }
            shell.active = i;
            app.save(shell, false);
            if shell.docs.get(i).is_some_and(|d| d.dirty()) {
                Err(fail(Kind::WriteFailed, shell.status.clone()))
            } else {
                Ok(described(shell, i))
            }
        }
        WindowAsk::Program(tool, args) => program(app, shell, &tool, &args),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_of_the_window_ask_the_window() {
        let bridge = WindowBridge::new();
        let tools = WindowTools::new(bridge.clone());
        let names: Vec<String> = tools.tools().into_iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            ["list_files", "open_file", "close_file", "save_file"]
        );
        let options = Default::default();
        let call = Call {
            mode: Mode::Files,
            options: &options,
            caller: &crate::Caller::Program,
        };
        assert_eq!(
            tools
                .call("close_file", &json!({}), &call)
                .unwrap_err()
                .kind,
            Kind::Invalid
        );
        let asking = tools.clone();
        let thread = std::thread::spawn(move || {
            let options = Default::default();
            let call = Call {
                mode: Mode::Files,
                options: &options,
                caller: &crate::Caller::Program,
            };
            asking.call("open_file", &json!({ "path": "a.txt" }), &call)
        });
        let mut seen = None;
        for _ in 0..500 {
            bridge.answer(|ask| {
                seen = Some(ask);
                Ok(Value::Null)
            });
            if seen.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(thread.join().unwrap().is_ok());
        assert_eq!(seen, Some(WindowAsk::Open(PathBuf::from("a.txt"))));
    }
}
