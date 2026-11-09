//! The server: the sources of tools, the configuration that decides what they may do, the journal
//! of the calls and the protocol (`initialize`, `tools/list`, `tools/call`, `resources/list`,
//! `resources/read`, `ping`). It speaks JSON-RPC 2.0 messages; a transport (`stdio`, `http`) or
//! the program itself (`Server::call`) brings them.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Value, json};
use ww_base::clock;

use crate::config::Config;
use crate::log;
use crate::tools::{Call, Caller, Kind, Outcome, Source, ToolDef, ToolError, fail};

/// The protocol version the server speaks.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// The largest message the server reads.
pub const MAX_MESSAGE: usize = 4 * 1024 * 1024;
const JOURNAL_SIZE: usize = 500;
/// The longest answer the journal keeps for the details of a call and the detailed log.
const ANSWER_KEPT: usize = 16 * 1024;
/// The longest text of one tool result; a longer one is cut with a note.
const TEXT_LIMIT: usize = 60_000;
/// The fields whose values never reach the journal or the log, for every server.
const SECRETS: [&str; 4] = ["password", "token", "authorization", "secret"];

/// What the server says of itself at `initialize`.
#[derive(Debug, Clone, Default)]
pub struct Info {
    /// The name of the server for the client (`serverInfo.name`), the program in lower case.
    pub name: String,
    pub version: String,
    /// What the client is told first; the words of each source follow.
    pub instructions: String,
}

/// One call in the journal of the server.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub time: String,
    pub client: String,
    pub method: String,
    /// The arguments that are identifiers, numbers or switches; other texts are not shown.
    pub detail: String,
    pub result: Status,
    /// The whole arguments and the answer, without the secret fields; a long answer is cut to
    /// its first characters.
    pub arguments: Value,
    pub answer: Value,
    /// How long the call took, in milliseconds.
    pub ms: u64,
}

/// How a call ended: well, or with the kind of its error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Error(Kind),
}

impl Status {
    pub fn is_ok(self) -> bool {
        self == Status::Ok
    }

    /// `ok`, or the name of the kind of the error.
    pub fn name(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Error(kind) => kind.name(),
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A tool as the server lists it: the source it comes from and whether the configuration has it
/// on.
#[derive(Debug, Clone)]
pub struct Listed {
    pub source: String,
    pub tool: ToolDef,
    pub on: bool,
}

struct State {
    config: Config,
    journal: Vec<Entry>,
    client: String,
}

/// The server: the sources and the state behind a lock, shared by the transports and the window.
pub struct Server {
    info: Info,
    sources: Vec<Arc<dyn Source>>,
    state: Mutex<State>,
}

impl Server {
    pub fn new(info: Info, config: Config) -> Self {
        Self {
            info,
            sources: Vec::new(),
            state: Mutex::new(State {
                config,
                journal: Vec::new(),
                client: String::new(),
            }),
        }
    }

    /// The server with one more source; a tool whose name an earlier source has is left out.
    pub fn with(mut self, source: Arc<dyn Source>) -> Self {
        self.sources.push(source);
        self
    }

    /// The server with these sources.
    pub fn with_all(mut self, sources: impl IntoIterator<Item = Arc<dyn Source>>) -> Self {
        self.sources.extend(sources);
        self
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn sources(&self) -> &[Arc<dyn Source>] {
        &self.sources
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The configuration in use.
    pub fn config(&self) -> Config {
        self.lock().config.clone()
    }

    /// Another configuration from now on: the tab changes it while the server runs.
    pub fn set_config(&self, config: Config) {
        self.lock().config = config;
    }

    /// The name of the client that sent `initialize`.
    pub fn client(&self) -> String {
        self.lock().client.clone()
    }

    /// Every tool of every source in their order, the first of a name only, with whether the
    /// configuration has it on.
    pub fn tools(&self) -> Vec<Listed> {
        let config = self.config();
        let mut out: Vec<Listed> = Vec::new();
        for source in &self.sources {
            let id = source.id();
            for tool in source.tools() {
                if out.iter().any(|l| l.tool.name == tool.name) {
                    continue;
                }
                let on = config.is_on(&id, &tool);
                out.push(Listed {
                    source: id.clone(),
                    tool,
                    on,
                });
            }
        }
        out
    }

    /// The last calls, the newest last, without their arguments and answers (the list of the
    /// tab reads it at every frame).
    pub fn journal(&self) -> Vec<Entry> {
        self.lock()
            .journal
            .iter()
            .map(|e| Entry {
                arguments: Value::Null,
                answer: Value::Null,
                ..e.clone()
            })
            .collect()
    }

    /// One call of the journal whole, by its place in the journal.
    pub fn journal_entry(&self, index: usize) -> Option<Entry> {
        self.lock().journal.get(index).cloned()
    }

    /// The secret fields of every source with the ones of every server.
    fn secrets(&self) -> Vec<String> {
        let mut out: Vec<String> = SECRETS.iter().map(|s| (*s).to_owned()).collect();
        for source in &self.sources {
            out.extend(source.secrets());
        }
        out
    }

    /// A copy of a value without the values of the secret fields.
    pub fn redact(&self, value: &Value) -> Value {
        redact(value, &self.secrets())
    }

    /// Runs a tool by its name, for a caller: the way the program (a command, a script) and its
    /// plugins drive the tools, and what `tools/call` does for a client. A caller other than the
    /// program meets the switches of the configuration (a tool that is off is not found), the
    /// mode and the options the tool needs. The call goes into the journal.
    pub fn call(&self, name: &str, args: &Value, caller: &Caller) -> Outcome {
        let detail = journal_detail(args);
        let started = web_now();
        log::line("INFO", &format!("tool {name} {detail}"));
        let outcome = self.run(name, args, caller);
        let ms = elapsed_ms(started);
        match &outcome {
            Ok(_) => log::line("INFO", &format!("tool {name} ok in {ms} ms")),
            Err(e) => log::line(
                "WARN",
                &format!(
                    "tool {name} {} in {ms} ms: {}",
                    e.kind.name(),
                    e.message.chars().take(200).collect::<String>()
                ),
            ),
        }
        let answer = match &outcome {
            Ok(value) => value.clone(),
            Err(e) => Value::from(e.to_string()),
        };
        let result = match &outcome {
            Ok(_) => Status::Ok,
            Err(e) => Status::Error(e.kind),
        };
        self.record(Record {
            caller,
            method: name,
            detail,
            result,
            arguments: args,
            answer: &answer,
            ms,
        });
        outcome
    }

    fn run(&self, name: &str, args: &Value, caller: &Caller) -> Outcome {
        if !args.is_object() {
            return Err(fail(Kind::Invalid, "the arguments must be an object"));
        }
        let config = self.config();
        let gated = caller.is_gated();
        let mut found = None;
        for source in &self.sources {
            if let Some(tool) = source.tools().into_iter().find(|t| t.name == name) {
                found = Some((source.clone(), tool));
                break;
            }
        }
        let Some((source, tool)) = found else {
            return Err(fail(Kind::NotFound, format!("no tool {name}")));
        };
        let id = source.id();
        if gated && !config.is_on(&id, &tool) {
            return Err(fail(Kind::NotFound, format!("no tool {name}")));
        }
        let options = config.options_of(source.as_ref());
        if gated {
            check(&config, source.as_ref(), &tool, &options)?;
        }
        let call = Call {
            mode: config.mode,
            options: &options,
            caller,
        };
        // No lock is held: a tool may call another one by its name, and a source keeps its own
        // state safe for the threads of the transports.
        source.call(name, args, &call)
    }

    /// Handles one JSON-RPC message given as text; `None` for a notification.
    pub fn handle_text(&self, text: &str) -> Option<String> {
        if text.len() > MAX_MESSAGE {
            log::line(
                "WARN",
                &format!("a message of {} bytes was refused: too large", text.len()),
            );
            return Some(error(Value::Null, -32600, "the message is too large").to_string());
        }
        let value: Value = match ww_base::jsonfile::parse(text, "message") {
            Ok(v) => v,
            Err(e) => {
                log::line(
                    "WARN",
                    &format!("a message that is not JSON was refused: {e}"),
                );
                return Some(error(Value::Null, -32700, &e.to_string()).to_string());
            }
        };
        // A failure inside the program answers this request with an error and keeps the
        // process and the other requests going; the panic hook has written the details.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle(&value))) {
            Ok(answer) => answer.map(|v| v.to_string()),
            Err(payload) => {
                let method = value.get("method").and_then(Value::as_str).unwrap_or("");
                log::line(
                    "ERROR",
                    &format!(
                        "the request {method} failed inside the program: {}; the server goes on",
                        log::panic_text(payload.as_ref())
                    ),
                );
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                let text = "internal error: the request failed inside the program; the MCP log of the program has the details";
                Some(error(id, -32603, text).to_string())
            }
        }
    }

    /// Handles one JSON-RPC message; `None` for a notification.
    pub fn handle(&self, message: &Value) -> Option<Value> {
        let Some(object) = message.as_object() else {
            return Some(error(
                Value::Null,
                -32600,
                "a message is one JSON object; batches are not supported",
            ));
        };
        let id = object.get("id").cloned();
        let method = object.get("method").and_then(Value::as_str).unwrap_or("");
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || method.is_empty() {
            return id.map(|id| error(id, -32600, "not a JSON-RPC 2.0 request"));
        }
        let params = object.get("params").cloned().unwrap_or(json!({}));
        let Some(id) = id else {
            // A notification (initialized, cancelled) needs no answer.
            return None;
        };
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.definitions() })),
            "tools/call" => Ok(self.call_tool(&params)),
            "resources/list" => Ok(json!({ "resources": self.resources() })),
            "resources/read" => self.read_resource(&params),
            #[cfg(test)]
            "test/panic" => panic!("a failure inside a request"),
            _ => {
                log::line("WARN", &format!("unknown method {method}"));
                Err((-32601, format!("unknown method {method}")))
            }
        };
        Some(match result {
            Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
            Err((code, text)) => error(id, code, &text),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let client = params
            .pointer("/clientInfo/name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .chars()
            .filter(|c| c.is_ascii_graphic() || *c == ' ')
            .take(60)
            .collect::<String>();
        let protocol = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or("")
            .chars()
            .filter(char::is_ascii_graphic)
            .take(20)
            .collect::<String>();
        log::line(
            "INFO",
            &format!("client '{client}' connected (protocol {protocol})"),
        );
        self.lock().client = client;
        self.record(Record {
            caller: &Caller::Client,
            method: "initialize",
            detail: String::new(),
            result: Status::Ok,
            arguments: params,
            answer: &Value::Null,
            ms: 0,
        });
        let mut instructions = self.info.instructions.clone();
        for source in &self.sources {
            let words = source.instructions();
            if !words.is_empty() {
                if !instructions.is_empty() {
                    instructions.push(' ');
                }
                instructions.push_str(&words);
            }
        }
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false }, "resources": { "listChanged": false } },
            "serverInfo": { "name": self.info.name, "version": self.info.version },
            "instructions": instructions,
        })
    }

    /// The tools a client sees: the ones that are on.
    pub fn definitions(&self) -> Vec<Value> {
        self.tools()
            .into_iter()
            .filter(|l| l.on)
            .map(|l| {
                json!({ "name": l.tool.name, "description": l.tool.description, "inputSchema": l.tool.schema })
            })
            .collect()
    }

    fn resources(&self) -> Vec<Value> {
        let config = self.config();
        self.sources
            .iter()
            .filter(|s| !config.source(&s.id()).off)
            .flat_map(|s| s.resources())
            .map(|r| json!({ "uri": r.uri, "name": r.name, "mimeType": r.mime }))
            .collect()
    }

    fn call_tool(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        match self.call(name, &args, &Caller::Client) {
            Ok(value) => {
                let text = serde_json::to_string_pretty(&value).unwrap_or_default();
                json!({
                    "content": [{ "type": "text", "text": fit(&text) }],
                    "structuredContent": value,
                    "isError": false,
                })
            }
            Err(e) => json!({
                "content": [{ "type": "text", "text": e.to_string() }],
                "isError": true,
            }),
        }
    }

    fn read_resource(&self, params: &Value) -> Result<Value, (i64, String)> {
        let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
        let config = self.config();
        let mut found = None;
        for source in &self.sources {
            if config.source(&source.id()).off || !source.resources().iter().any(|r| r.uri == uri) {
                continue;
            }
            let options = config.options_of(source.as_ref());
            let call = Call {
                mode: config.mode,
                options: &options,
                caller: &Caller::Client,
            };
            found = source.read(uri, &call);
            break;
        }
        let Some((mime, text)) = found else {
            return Err((-32002, format!("unknown resource {uri}")));
        };
        self.record(Record {
            caller: &Caller::Client,
            method: "resources/read",
            detail: uri.chars().take(80).collect(),
            result: Status::Ok,
            arguments: params,
            answer: &Value::from(format!("{} characters", text.chars().count())),
            ms: 0,
        });
        Ok(json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": fit(&text) }] }))
    }

    /// Puts a call into the journal, and into the log when the log is detailed.
    fn record(&self, call: Record<'_>) {
        let secrets = self.secrets();
        let mut state = self.lock();
        let client = match call.caller {
            Caller::Client => state.client.clone(),
            other => other.name(),
        };
        let entry = Entry {
            time: clock::now_utc_iso(),
            client,
            method: call.method.to_owned(),
            detail: call.detail,
            result: call.result,
            arguments: redact(call.arguments, &secrets),
            answer: kept(call.answer, &secrets),
            ms: call.ms,
        };
        if state.config.detailed_log {
            log::event(
                "DETAIL",
                json!({
                    "client": entry.client,
                    "call": entry.method,
                    "result": entry.result.name(),
                    "ms": entry.ms,
                    "arguments": entry.arguments,
                    "answer": entry.answer,
                }),
            );
        }
        state.journal.push(entry);
        let excess = state.journal.len().saturating_sub(JOURNAL_SIZE);
        state.journal.drain(..excess);
    }
}

/// Whether a tool may run: the mode of the configuration and the options it needs.
fn check(
    config: &Config,
    source: &dyn Source,
    tool: &ToolDef,
    options: &BTreeMap<String, String>,
) -> Result<(), ToolError> {
    for need in &tool.needs {
        let Some(choice) = source.options().into_iter().find(|c| c.key == need.option) else {
            continue;
        };
        let value = options.get(&need.option).map(String::as_str).unwrap_or("");
        let met = if choice.is_text() {
            !value.is_empty()
        } else {
            choice.rank(value) >= choice.rank(&need.at_least)
        };
        if !met {
            let wanted = if choice.is_text() {
                "a value".to_owned()
            } else {
                need.at_least.clone()
            };
            return Err(fail(
                Kind::NotAllowed,
                format!(
                    "the option {} of {} is {}; this tool needs {wanted} (the user sets it in the MCP tab of the window or with --{} on the command line)",
                    choice.key,
                    source.id(),
                    if value.is_empty() { "empty" } else { value },
                    choice.key.replace('_', "-")
                ),
            ));
        }
    }
    if config.mode < tool.mode {
        return Err(fail(
            Kind::NotAllowed,
            format!(
                "the server is in the mode {}; this tool needs the mode {} (the user switches it in the window or with --mode)",
                config.mode.name(),
                tool.mode.name()
            ),
        ));
    }
    Ok(())
}

/// One call for the journal.
struct Record<'a> {
    caller: &'a Caller,
    method: &'a str,
    detail: String,
    result: Status,
    arguments: &'a Value,
    answer: &'a Value,
    ms: u64,
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A copy of a value without the values of the secret fields.
pub fn redact(value: &Value, secrets: &[String]) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let secret = secrets.iter().any(|s| k.eq_ignore_ascii_case(s));
                    let v = if secret && !v.is_null() {
                        Value::from("***")
                    } else {
                        redact(v, secrets)
                    };
                    (k.clone(), v)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(|v| redact(v, secrets)).collect()),
        other => other.clone(),
    }
}

/// An answer for the journal: without secrets, and as the start of its text when it is long.
fn kept(value: &Value, secrets: &[String]) -> Value {
    let value = redact(value, secrets);
    let text = value.to_string();
    if text.len() <= ANSWER_KEPT {
        return value;
    }
    let mut end = ANSWER_KEPT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Value::from(format!("{}... ({} bytes in all)", &text[..end], text.len()))
}

/// The arguments of a call for the journal: identifiers, numbers and switches; other texts are
/// replaced by their kind.
pub fn journal_detail(args: &Value) -> String {
    let Some(map) = args.as_object() else {
        return String::new();
    };
    map.iter()
        .map(|(k, v)| {
            let shown = match v {
                Value::Bool(_) | Value::Number(_) => v.to_string(),
                Value::String(s)
                    if s.len() <= 80
                        && s.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "._- ".contains(c)) =>
                {
                    s.clone()
                }
                Value::String(_) => "<text>".to_owned(),
                _ => "<value>".to_owned(),
            };
            format!("{}={shown}", k.chars().take(40).collect::<String>())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A text cut to the limit of one result.
pub fn fit(text: &str) -> String {
    if text.len() <= TEXT_LIMIT {
        return text.to_owned();
    }
    let mut end = TEXT_LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n... (cut: {} more characters)",
        &text[..end],
        text.len() - end
    )
}

/// The clock of a call; nothing in a browser, where no server runs.
fn web_now() -> Option<std::time::Instant> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(std::time::Instant::now())
    }
}

fn elapsed_ms(started: Option<std::time::Instant>) -> u64 {
    started.map_or(0, |s| {
        u64::try_from(s.elapsed().as_millis()).unwrap_or(u64::MAX)
    })
}

#[cfg(test)]
mod tests;
