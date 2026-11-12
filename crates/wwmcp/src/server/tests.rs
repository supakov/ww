//! The tests of the server: the tools of two sources, the switches and the options that decide
//! them, the journal, the protocol.

use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::tools::{Args, Choice, Mode, ResourceDef, schema, text};

/// A source with a tool of each mode, one that needs an option, and a resource.
struct Demo;

impl Source for Demo {
    fn id(&self) -> String {
        "demo".to_owned()
    }
    fn title(&self) -> String {
        "Demo".to_owned()
    }
    fn tools(&self) -> Vec<ToolDef> {
        let one = schema(json!({ "name": text("a name") }), &["name"]);
        vec![
            ToolDef::new("greet", "words", "Greets a name.", one),
            ToolDef::new(
                "change",
                "edits",
                "Changes something.",
                schema(json!({}), &[]),
            )
            .mode(Mode::Edit),
            ToolDef::new("read_pc", "pc", "Reads the PC.", schema(json!({}), &[]))
                .needs("pc", "on"),
            ToolDef::new("fail", "words", "Fails.", schema(json!({}), &[])),
        ]
    }
    fn options(&self) -> Vec<Choice> {
        vec![Choice::switch("pc", "This PC")]
    }
    fn call(&self, tool: &str, args: &Value, call: &Call<'_>) -> Outcome {
        match tool {
            "greet" => Ok(
                json!({ "text": format!("Hello, {}", Args(args).text("name")?), "password": "x" }),
            ),
            "change" => Ok(json!({ "mode": call.mode.name() })),
            "read_pc" => Ok(json!({ "pc": call.option("pc") })),
            _ => Err(fail(Kind::ReadFailed, "it fails")),
        }
    }
    fn resources(&self) -> Vec<ResourceDef> {
        vec![ResourceDef {
            uri: "demo://words".to_owned(),
            name: "Words".to_owned(),
            mime: "text/plain".to_owned(),
        }]
    }
    fn read(&self, uri: &str, _: &Call<'_>) -> Option<(String, String)> {
        (uri == "demo://words").then(|| ("text/plain".to_owned(), "hello".to_owned()))
    }
    fn instructions(&self) -> String {
        "Greet first.".to_owned()
    }
    fn secrets(&self) -> Vec<String> {
        vec!["product_key".to_owned()]
    }
}

/// A second source with a tool whose name the first has.
struct Other;

impl Source for Other {
    fn id(&self) -> String {
        "other".to_owned()
    }
    fn title(&self) -> String {
        "Other".to_owned()
    }
    fn tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef::new("greet", "x", "Greets too.", schema(json!({}), &[])),
            ToolDef::new("count", "x", "Counts.", schema(json!({}), &[])),
        ]
    }
    fn call(&self, tool: &str, _: &Value, _: &Call<'_>) -> Outcome {
        Ok(Value::from(tool))
    }
}

fn server() -> Server {
    let info = Info {
        name: "demo".to_owned(),
        version: "1".to_owned(),
        instructions: "A demo.".to_owned(),
    };
    Server::new(info, Config::default())
        .with(Arc::new(Demo))
        .with(Arc::new(Other))
}

fn rpc(server: &Server, id: u32, method: &str, params: Value) -> Value {
    let text = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    let answer = server.handle_text(&text.to_string()).unwrap();
    serde_json::from_str(&answer).unwrap()
}

#[test]
fn the_tools_of_every_source_are_listed_once() {
    let s = server();
    let names: Vec<String> = s.tools().into_iter().map(|l| l.tool.name).collect();
    assert_eq!(names, ["greet", "change", "read_pc", "fail", "count"]);
    let init = rpc(
        &s,
        1,
        "initialize",
        json!({ "clientInfo": { "name": "tester" } }),
    );
    assert_eq!(init["result"]["instructions"], "A demo. Greet first.");
    assert_eq!(s.client(), "tester");
    let list = rpc(&s, 2, "tools/list", json!({}));
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 5);
    let resources = rpc(&s, 3, "resources/list", json!({}));
    assert_eq!(resources["result"]["resources"][0]["uri"], "demo://words");
    let read = rpc(&s, 4, "resources/read", json!({ "uri": "demo://words" }));
    assert_eq!(read["result"]["contents"][0]["text"], "hello");
}

#[test]
fn the_mode_the_options_and_the_switches_decide_a_call() {
    let s = server();
    let call = |name: &str, caller: &Caller| s.call(name, &json!({}), caller);
    // read: the mode of every start; the tool that edits and the one of the PC are refused.
    assert_eq!(
        call("change", &Caller::Client).unwrap_err().kind,
        Kind::NotAllowed
    );
    let why = call("read_pc", &Caller::Client).unwrap_err();
    assert!(why.message.contains("--pc"), "{why}");
    // The program calls what its user asked for.
    assert_eq!(call("read_pc", &Caller::Program).unwrap()["pc"], "off");
    let mut config = s.config();
    config.mode = Mode::Edit;
    config
        .source_mut("demo")
        .options
        .insert("pc".to_owned(), "on".to_owned());
    s.set_config(config.clone());
    assert_eq!(call("change", &Caller::Client).unwrap()["mode"], "edit");
    assert_eq!(
        call("read_pc", &Caller::Plugin("p".to_owned())).unwrap()["pc"],
        "on"
    );
    // A tool, a group or a source off is not found by a client, and not listed.
    config
        .source_mut("demo")
        .tools_off
        .insert("change".to_owned());
    config.source_mut("demo").groups_off.insert("pc".to_owned());
    config.source_mut("other").off = true;
    s.set_config(config);
    for name in ["change", "read_pc", "count"] {
        assert_eq!(
            call(name, &Caller::Client).unwrap_err().kind,
            Kind::NotFound,
            "{name}"
        );
    }
    assert!(call("count", &Caller::Program).is_ok());
    let listed: Vec<String> = s
        .definitions()
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(listed, ["greet", "fail"]);
    assert_eq!(
        call("nothing", &Caller::Program).unwrap_err().kind,
        Kind::NotFound
    );
    assert_eq!(
        s.call("greet", &json!([]), &Caller::Program)
            .unwrap_err()
            .kind,
        Kind::Invalid
    );
}

#[test]
fn a_call_goes_into_the_journal_without_its_secrets() {
    let s = server();
    let answer = rpc(
        &s,
        1,
        "tools/call",
        json!({ "name": "greet", "arguments": {
            "name": "Ann",
            "token": "secret-token",
            "New_Password": "hunter2",
            "product_key": "ABCDE-12345",
            "count": 3,
        } }),
    );
    assert_eq!(answer["result"]["isError"], false);
    assert_eq!(answer["result"]["structuredContent"]["text"], "Hello, Ann");
    let failed = rpc(
        &s,
        2,
        "tools/call",
        json!({ "name": "fail", "arguments": {} }),
    );
    assert_eq!(failed["result"]["isError"], true);
    let text = failed["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("read_failed: "), "{text}");
    s.call("count", &json!({}), &Caller::Program).unwrap();
    let journal = s.journal();
    assert_eq!(journal.len(), 3);
    // A secret field shows no value in the line of the journal and of the log, whatever the
    // value looks like: the words of every server, and the fields a source names.
    assert_eq!(
        journal[0].detail,
        "name=Ann token=*** New_Password=*** product_key=*** count=3"
    );
    for secret in ["secret-token", "hunter2", "ABCDE-12345"] {
        assert!(!journal[0].detail.contains(secret), "{secret}");
    }
    assert_eq!(
        journal_detail(&json!({ "api_token": "abc", "path": "a.txt" })),
        "api_token=*** path=a.txt"
    );
    assert_eq!(journal[1].result, Status::Error(Kind::ReadFailed));
    assert_eq!(journal[2].client, "program");
    let whole = s.journal_entry(0).unwrap();
    assert_eq!(whole.arguments["token"], "***");
    assert_eq!(whole.arguments["New_Password"], "***");
    assert_eq!(whole.arguments["product_key"], "***");
    assert_eq!(whole.arguments["count"], 3);
    assert_eq!(whole.answer["password"], "***");
    assert_eq!(journal[0].arguments, Value::Null);
}

#[test]
fn a_failure_inside_a_request_keeps_the_server_going() {
    let s = server();
    let failed = s
        .handle_text(r#"{"jsonrpc": "2.0", "id": 7, "method": "test/panic"}"#)
        .unwrap();
    let failed: Value = serde_json::from_str(&failed).unwrap();
    assert_eq!(failed["id"], 7);
    assert_eq!(failed["error"]["code"], -32603);
    let ping = s
        .handle_text(r#"{"jsonrpc": "2.0", "id": 8, "method": "ping"}"#)
        .unwrap();
    assert!(ping.contains("\"result\""), "{ping}");
    assert!(
        s.handle_text(r#"{"jsonrpc": "2.0", "method": "notified"}"#)
            .is_none()
    );
    let wrong = s.handle_text("not json").unwrap();
    assert!(wrong.contains("-32700"), "{wrong}");
    let unknown = rpc(&s, 9, "nothing/here", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
}

#[test]
fn a_long_text_is_cut() {
    let long = "a".repeat(TEXT_LIMIT + 10);
    assert!(fit(&long).ends_with("(cut: 10 more characters)"));
    let big = Value::from("b".repeat(ANSWER_KEPT + 5));
    let kept = kept(&big, &[]);
    assert!(kept.as_str().unwrap().ends_with("bytes in all)"));
}

#[test]
fn the_journal_keeps_the_start_of_long_arguments() {
    let s = server();
    let long = "x".repeat(4 * ANSWER_KEPT);
    s.call(
        "greet",
        &json!({ "name": "Ann", "password": "hunter2", "notes": long }),
        &Caller::Program,
    )
    .unwrap();
    let whole = s.journal_entry(0).unwrap();
    let text = whole.arguments.as_str().expect("cut to a text");
    assert!(text.len() < ANSWER_KEPT + 100, "{}", text.len());
    assert!(text.ends_with("bytes in all)"), "the cut is named");
    assert!(text.contains("\"name\":\"Ann\""), "the start is kept");
    assert!(!text.contains("hunter2"), "without its secrets");
    assert_eq!(whole.detail, "name=Ann password=*** notes=<text>");
    // Short arguments stay the object they were.
    s.call("greet", &json!({ "name": "Bo" }), &Caller::Program)
        .unwrap();
    assert_eq!(s.journal_entry(1).unwrap().arguments["name"], "Bo");
}
