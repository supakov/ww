//! What a source of tools gives the server: its tools with their schemas, the options a tool may
//! need (the access settings of the source), its resources, and the function that runs a tool.
//! The program is a source, and so is every plugin that has tools (`wwrt`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// What the tools may do, the most a server allows: `read` changes nothing, `edit` changes what
/// the program holds in memory, `files` writes files too. A tool names the least mode it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Read,
    Edit,
    Files,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Read, Mode::Edit, Mode::Files];

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == text)
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Read => "read",
            Mode::Edit => "edit",
            Mode::Files => "files",
        }
    }
}

/// An option of a source: a choice among named values (an access setting, `off`, `read`,
/// `write`) or, without values, a text. The configuration file keeps its value in the branch of
/// the source (`Config::plugins`); the tab of the server shows it in the card of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub key: String,
    /// The name shown (English, translated by the window).
    pub title: String,
    /// The values in their order, the weakest first; empty for a text.
    pub values: Vec<String>,
    /// The names shown of the values (English, translated); empty for the values themselves.
    pub labels: Vec<String>,
    /// The value without a setting.
    pub default: String,
    /// A line under the option (English, translated); may be empty.
    pub help: String,
}

impl Choice {
    /// A choice of values, the first the default.
    pub fn of(key: &str, title: &str, values: &[(&str, &str)]) -> Self {
        Self {
            key: key.to_owned(),
            title: title.to_owned(),
            values: values.iter().map(|(v, _)| (*v).to_owned()).collect(),
            labels: values.iter().map(|(_, l)| (*l).to_owned()).collect(),
            default: values
                .first()
                .map(|(v, _)| (*v).to_owned())
                .unwrap_or_default(),
            help: String::new(),
        }
    }

    /// A switch: `off` (the default) or `on`.
    pub fn switch(key: &str, title: &str) -> Self {
        Self::of(key, title, &[("off", "Off"), ("on", "On")])
    }

    /// A text, empty by default.
    pub fn text(key: &str, title: &str) -> Self {
        Self {
            key: key.to_owned(),
            title: title.to_owned(),
            values: Vec::new(),
            labels: Vec::new(),
            default: String::new(),
            help: String::new(),
        }
    }

    pub fn help(mut self, help: &str) -> Self {
        self.help = help.to_owned();
        self
    }

    pub fn is_text(&self) -> bool {
        self.values.is_empty()
    }

    /// The place of a value among the values; none for a text or a value it does not have.
    pub fn rank(&self, value: &str) -> Option<usize> {
        self.values.iter().position(|v| v == value)
    }

    /// The name shown of a value.
    pub fn label(&self, value: &str) -> String {
        self.rank(value)
            .and_then(|i| self.labels.get(i))
            .cloned()
            .unwrap_or_else(|| value.to_owned())
    }
}

/// An option a tool needs at a value or above it (`registry` at `read`), or a text option that
/// must not be empty (`at_least` empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    pub option: String,
    pub at_least: String,
}

impl Need {
    pub fn new(option: &str, at_least: &str) -> Self {
        Self {
            option: option.to_owned(),
            at_least: at_least.to_owned(),
        }
    }
}

/// A tool: its name (unique among the tools of a server, letters, digits, `_`, `-` and `.`), its
/// description and the schema of its arguments for the client, its group (the tools a switch of
/// the tab turns off together), the mode it needs and the options.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub schema: Value,
    pub group: String,
    pub mode: Mode,
    pub needs: Vec<Need>,
}

impl ToolDef {
    /// A tool of the mode `read` without needs.
    pub fn new(name: &str, group: &str, description: &str, schema: Value) -> Self {
        Self {
            name: name.to_owned(),
            description: description.to_owned(),
            schema,
            group: group.to_owned(),
            mode: Mode::Read,
            needs: Vec::new(),
        }
    }

    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    pub fn needs(mut self, option: &str, at_least: &str) -> Self {
        self.needs.push(Need::new(option, at_least));
        self
    }
}

/// The schema of the arguments of a tool: an object of these properties, these required.
pub fn schema(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
}

/// The schema of a tool without arguments.
pub fn no_arguments() -> Value {
    schema(json!({}), &[])
}

/// A text property with its description.
pub fn text(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

/// A resource of a source: a text the client reads by its URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDef {
    pub uri: String,
    pub name: String,
    pub mime: String,
}

/// Why a call failed; its name starts the text the client sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Invalid,
    NotFound,
    NotAllowed,
    ReadFailed,
    WriteFailed,
    /// The tool failed for another reason.
    Failed,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Invalid => "invalid",
            Kind::NotFound => "not_found",
            Kind::NotAllowed => "not_allowed",
            Kind::ReadFailed => "read_failed",
            Kind::WriteFailed => "write_failed",
            Kind::Failed => "failed",
        }
    }

    /// The kind of a number of the stable interface (`WW_TOOL_*` of ww.h); 0 is no error.
    pub fn of_code(code: i32) -> Kind {
        match code {
            1 => Kind::Invalid,
            2 => Kind::NotFound,
            3 => Kind::NotAllowed,
            4 => Kind::ReadFailed,
            5 => Kind::WriteFailed,
            _ => Kind::Failed,
        }
    }

    /// The number of the kind in the stable interface.
    pub fn code(self) -> i32 {
        match self {
            Kind::Invalid => 1,
            Kind::NotFound => 2,
            Kind::NotAllowed => 3,
            Kind::ReadFailed => 4,
            Kind::WriteFailed => 5,
            Kind::Failed => 6,
        }
    }
}

/// A failed call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    pub kind: Kind,
    pub message: String,
}

impl ToolError {
    pub fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind.name(), self.message)
    }
}

/// A failed call of this kind.
pub fn fail(kind: Kind, message: impl Into<String>) -> ToolError {
    ToolError::new(kind, message)
}

/// The answer of a tool.
pub type Outcome = Result<Value, ToolError>;

/// Who calls a tool: a client of MCP, a plugin (through its host) or the program itself (a
/// command, a script). Every caller but the program meets the switches of the tab, the mode and
/// the options; the program calls what its user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    Client,
    Plugin(String),
    Program,
}

impl Caller {
    /// The name the journal shows; a client shows the name it gave at `initialize`.
    pub fn name(&self) -> String {
        match self {
            Caller::Client => String::new(),
            Caller::Plugin(id) => format!("plugin {id}"),
            Caller::Program => "program".to_owned(),
        }
    }

    /// Whether the switches, the mode and the options apply.
    pub fn is_gated(&self) -> bool {
        *self != Caller::Program
    }
}

/// What a running tool knows: the mode of the server, the options of its source with their
/// values (the defaults where the configuration has none), and who calls it.
pub struct Call<'a> {
    pub mode: Mode,
    pub options: &'a BTreeMap<String, String>,
    pub caller: &'a Caller,
}

impl Call<'_> {
    /// The value of an option of the source, empty for one it does not have.
    pub fn option(&self, key: &str) -> &str {
        self.options.get(key).map(String::as_str).unwrap_or("")
    }

    /// Whether an option of the source is `on`.
    pub fn is_on(&self, key: &str) -> bool {
        self.option(key) == "on"
    }
}

/// A source of tools: the program, or a plugin. Its calls come from the threads of the
/// transports and of the window, several at once, and a tool may call another one by its name
/// (`Server::call`): the source keeps its state safe for them.
pub trait Source: Send + Sync {
    /// The key of its branch in the configuration file: the name of the program or the id of a
    /// plugin (the name of its DLL).
    fn id(&self) -> String;
    /// The name shown (translated by the window).
    fn title(&self) -> String;
    fn tools(&self) -> Vec<ToolDef>;
    /// The options its tools need or read.
    fn options(&self) -> Vec<Choice> {
        Vec::new()
    }
    /// Runs one of its tools; the server checked the switches, the mode and the needs.
    fn call(&self, tool: &str, args: &Value, call: &Call<'_>) -> Outcome;
    fn resources(&self) -> Vec<ResourceDef> {
        Vec::new()
    }
    /// A resource of its own: its MIME type and its text.
    fn read(&self, _uri: &str, _call: &Call<'_>) -> Option<(String, String)> {
        None
    }
    /// What it tells a client at `initialize`, after the words of the program.
    fn instructions(&self) -> String {
        String::new()
    }
    /// The names of the fields whose values never reach the journal or the log, beyond the ones
    /// of every server (passwords, tokens).
    fn secrets(&self) -> Vec<String> {
        Vec::new()
    }
}

/// The arguments of a call: a JSON object, a field that is null as good as absent.
pub struct Args<'a>(pub &'a Value);

impl<'a> Args<'a> {
    pub fn get(&self, name: &str) -> Option<&'a Value> {
        self.0.get(name).filter(|v| !v.is_null())
    }

    /// A text that must be there.
    pub fn text(&self, name: &str) -> Result<String, ToolError> {
        self.str(name)
            .map(str::to_owned)
            .ok_or_else(|| fail(Kind::Invalid, format!("the argument {name} must be a text")))
    }

    /// A text that may be left out.
    pub fn str(&self, name: &str) -> Option<&'a str> {
        self.get(name).and_then(Value::as_str)
    }

    /// A text, empty when left out.
    pub fn optional_text(&self, name: &str) -> String {
        self.str(name).unwrap_or_default().to_owned()
    }

    /// A switch that must be there.
    pub fn flag(&self, name: &str) -> Result<bool, ToolError> {
        self.get(name).and_then(Value::as_bool).ok_or_else(|| {
            fail(
                Kind::Invalid,
                format!("the argument {name} must be true or false"),
            )
        })
    }

    /// A switch with its default.
    pub fn flag_or(&self, name: &str, default: bool) -> bool {
        self.get(name).and_then(Value::as_bool).unwrap_or(default)
    }

    pub fn number(&self, name: &str) -> Option<u64> {
        self.get(name).and_then(Value::as_u64)
    }

    /// A number that may have a fraction.
    pub fn float(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(Value::as_f64)
    }

    /// A count between 1 and `max`, `default` when left out.
    pub fn limit(&self, name: &str, default: usize, max: usize) -> usize {
        match self.number(name) {
            Some(n) => n.clamp(1, max as u64) as usize,
            None => default,
        }
    }

    pub fn list(&self, name: &str) -> Option<&'a Vec<Value>> {
        self.get(name).and_then(Value::as_array)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_rank_their_values() {
        let c = Choice::of("registry", "Registry", &[("off", "Off"), ("read", "Read")]);
        assert_eq!(c.default, "off");
        assert_eq!(c.rank("read"), Some(1));
        assert_eq!(c.rank("write"), None);
        assert_eq!(c.label("read"), "Read");
        assert!(Choice::text("profile", "Profile").is_text());
        assert_eq!(Mode::parse("edit"), Some(Mode::Edit));
        assert!(Mode::Files > Mode::Edit);
        for kind in [Kind::Invalid, Kind::NotFound, Kind::Failed] {
            assert_eq!(Kind::of_code(kind.code()), kind);
        }
        let args = json!({ "a": "x", "b": null, "n": 7 });
        let a = Args(&args);
        assert_eq!(a.text("a").unwrap(), "x");
        assert!(a.text("b").is_err());
        assert_eq!(a.limit("n", 1, 5), 5);
    }
}
