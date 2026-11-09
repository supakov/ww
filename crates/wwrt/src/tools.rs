//! The tools of the plugins (`wwmcp`): a plugin with tools is a source of the server of its
//! program (`PluginSource`), and a plugin calls any tool of the program by its name
//! (`Host::call_tool`, `call`, the function `call_tool` of the host of the stable interface),
//! through the server the program installs (`install`) and with the switches of its
//! configuration.

use std::sync::{Arc, RwLock};

use serde_json::Value;
use wwmcp::{Call, Caller, Choice, Kind, Outcome, Server, Source, ToolDef, fail};

use crate::plugin::{Loaded, Plugin, Plugins};

static SERVER: RwLock<Option<Arc<Server>>> = RwLock::new(None);

/// The tests that install a server, one at a time.
#[cfg(test)]
pub(crate) static TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The server whose tools the plugins call: the program installs it when it makes it, and
/// again when it makes another.
pub fn install(server: Arc<Server>) {
    if let Ok(mut s) = SERVER.write() {
        *s = Some(server);
    }
}

/// The server installed, if any.
pub fn server() -> Option<Arc<Server>> {
    SERVER.read().ok().and_then(|s| s.clone())
}

/// Calls a tool of the program by its name for a plugin: the switches, the mode and the options
/// of the configuration of the server apply.
pub fn call(name: &str, args: &Value, plugin: &str) -> Outcome {
    let Some(server) = server() else {
        return Err(fail(Kind::NotFound, "the program has no tools"));
    };
    server.call(name, args, &Caller::Plugin(plugin.to_owned()))
}

/// A plugin as a source of tools: its id is the name of its DLL.
pub struct PluginSource {
    pub id: String,
    pub plugin: Arc<dyn Plugin>,
}

impl Source for PluginSource {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn title(&self) -> String {
        self.plugin.name().to_owned()
    }

    fn tools(&self) -> Vec<ToolDef> {
        self.plugin.tools()
    }

    fn options(&self) -> Vec<Choice> {
        self.plugin.tool_options()
    }

    fn call(&self, tool: &str, args: &Value, call: &Call<'_>) -> Outcome {
        self.plugin.call_tool(tool, args, call)
    }
}

impl Plugins {
    /// The plugins that have tools or options of tools, as sources of a server.
    pub fn sources(&self) -> Vec<Arc<dyn Source>> {
        self.loaded
            .iter()
            .filter(|l| has_tools(l))
            .map(|l| {
                Arc::new(PluginSource {
                    id: l.id.clone(),
                    plugin: l.plugin.clone(),
                }) as Arc<dyn Source>
            })
            .collect()
    }
}

fn has_tools(loaded: &Loaded) -> bool {
    !loaded.plugin.tools().is_empty() || !loaded.plugin.tool_options().is_empty()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::json;
    use wwmcp::{Config, Info, Mode, schema};

    use super::*;
    use crate::plugin::{Format, View};

    /// A plugin of tools alone: no format, a tool that needs its option, a tool that calls
    /// another tool of the program by its name.
    struct Tools;

    impl Plugin for Tools {
        fn name(&self) -> &'static str {
            "Tools"
        }
        fn formats(&self) -> &'static [Format] {
            &[]
        }
        fn open(&self, _: &Path, _: &Value) -> Result<Box<dyn View>, String> {
            Err("no files".to_owned())
        }
        fn tools(&self) -> Vec<ToolDef> {
            vec![
                ToolDef::new("double", "math", "Doubles n.", schema(json!({}), &[]))
                    .needs("math", "on"),
                ToolDef::new(
                    "again",
                    "math",
                    "Doubles through the server.",
                    schema(json!({}), &[]),
                ),
            ]
        }
        fn tool_options(&self) -> Vec<Choice> {
            vec![Choice::switch("math", "Mathematics")]
        }
        fn call_tool(&self, name: &str, args: &Value, _: &Call<'_>) -> Outcome {
            let n = args.get("n").and_then(Value::as_i64).unwrap_or(0);
            match name {
                "double" => Ok(json!(n * 2)),
                _ => call("double", &json!({ "n": n }), "tools"),
            }
        }
    }

    #[test]
    fn a_plugin_is_a_source_and_calls_the_tools_by_name() {
        let _one = TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let mut plugins = Plugins::default();
        plugins.add(PathBuf::from("tools.dll"), Box::new(Tools));
        let sources = plugins.sources();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id(), "tools");
        let mut config = Config {
            mode: Mode::Read,
            ..Config::default()
        };
        let server = Arc::new(Server::new(Info::default(), config.clone()).with_all(sources));
        install(server.clone());
        let n = json!({ "n": 4 });
        // The option is off: the call of the plugin meets it, the one of the program does not.
        assert_eq!(
            server.call("again", &n, &Caller::Program).unwrap_err().kind,
            Kind::NotAllowed
        );
        assert_eq!(
            server.call("double", &n, &Caller::Program).unwrap(),
            json!(8)
        );
        config
            .source_mut("tools")
            .options
            .insert("math".to_owned(), "on".to_owned());
        server.set_config(config);
        assert_eq!(server.call("again", &n, &Caller::Client).unwrap(), json!(8));
        let journal = server.journal();
        assert!(journal.iter().any(|e| e.client == "plugin tools"));
    }
}
