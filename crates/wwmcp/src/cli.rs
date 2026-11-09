//! The command line of a server without the window: `PROGRAM --mcp FILE|stdio|http [options]`.
//! The options of the sources of tools come from their declarations: `--KEY VALUE` sets the
//! option `KEY` of the source that has it (dashes for underscores), `--KEY` alone turns a switch
//! on, and `--set SOURCE.KEY=VALUE` names the source.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::{Config, EXTENSION, TOKEN_MIN, Transport};
use crate::http::HttpServer;
use crate::tools::{Mode, Source};
use crate::{Server, log, stdio};

/// The lines of `--mcp` for the help of a program.
pub const USAGE: &str =
    "  --mcp FILE|stdio|http      serve the tools over MCP without the window: a configuration
                             file (its path, or its name in the folder mcp), or a transport
                             with the defaults
    --config FILE            the configuration file a transport starts from
    --mode read|edit|files   what the tools may do
    --port N, --token T      the port and the bearer token of http (32 letters and digits or more)
    --detailed-log           the log keeps every call and answer whole
    --off SOURCE[/TOOL]      a source of tools, or one of its tools, off
    --set SOURCE.KEY=VALUE   an option of a source
    --KEY VALUE, --KEY       an option of the source that has it; a switch alone is on";

/// The value of an option such as `--port 8765` or `--port=8765`.
pub fn option(args: &[String], name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    args.iter().enumerate().find_map(|(i, a)| {
        if a == name {
            args.get(i + 1).cloned()
        } else {
            a.strip_prefix(&prefix).map(str::to_owned)
        }
    })
}

/// The file of a configuration given on the command line: the path, or a name (with or without
/// its extension) of the folder `mcp` of the program.
pub fn config_path(text: &str, exe_dir: &Path) -> PathBuf {
    let path = PathBuf::from(text);
    if path.exists() {
        return path;
    }
    let name = if path.extension().is_some_and(|e| e == EXTENSION) {
        text.to_owned()
    } else {
        format!("{text}.{EXTENSION}")
    };
    Config::folder(exe_dir).join(name)
}

/// The configuration of a start without the window: the file of `--mcp FILE` or `--config
/// FILE`, else the defaults, with the options of the command line over it. `args` follow
/// `--mcp`; `sources` are the ones the server will have.
pub fn config(
    args: &[String],
    exe_dir: &Path,
    sources: &[Arc<dyn Source>],
) -> Result<Config, String> {
    let first = args.first().map(String::as_str).unwrap_or("");
    let (file, transport) = match first {
        "stdio" => (option(args, "--config"), Some(Transport::Stdio)),
        "http" => (option(args, "--config"), Some(Transport::Http)),
        "" => return Err("--mcp needs a configuration file, stdio or http".to_owned()),
        other => (Some(other.to_owned()), None),
    };
    let mut config = match &file {
        Some(f) => Config::load(&config_path(f, exe_dir))?,
        None => Config::default(),
    };
    if let Some(t) = transport {
        config.transport = t;
    }
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        i += 1;
        let Some(name) = arg.strip_prefix("--") else {
            return Err(format!("unknown argument {arg}"));
        };
        let (name, inline) = match name.split_once('=') {
            Some((n, v)) => (n, Some(v.to_owned())),
            None => (name, None),
        };
        // The value after the option, when the option takes one and it does not start another.
        let mut value = |takes: bool| -> Option<String> {
            if inline.is_some() || !takes {
                return inline.clone();
            }
            let next = args.get(i).filter(|a| !a.starts_with("--")).cloned();
            if next.is_some() {
                i += 1;
            }
            next
        };
        match name {
            "config" => {
                value(true).ok_or("--config needs a file")?;
            }
            "mode" => {
                let text = value(true).unwrap_or_default();
                config.mode = Mode::parse(&text).ok_or("--mode is read, edit or files")?;
            }
            "port" => {
                let text = value(true).unwrap_or_default();
                config.port = text.parse().map_err(|_| "--port is a number")?;
            }
            "token" => config.token = value(true).ok_or("--token needs a value")?,
            "detailed-log" => config.detailed_log = true,
            "off" => {
                let text = value(true).ok_or("--off needs a source or SOURCE/TOOL")?;
                match text.split_once('/') {
                    Some((source, tool)) => {
                        config.source_mut(source).tools_off.insert(tool.to_owned());
                    }
                    None => config.source_mut(&text).off = true,
                }
            }
            "set" => {
                let text = value(true).unwrap_or_default();
                let (path, v) = text.split_once('=').ok_or("--set takes SOURCE.KEY=VALUE")?;
                let (source, key) = path.split_once('.').ok_or("--set takes SOURCE.KEY=VALUE")?;
                let found = sources.iter().find(|s| s.id() == source);
                let choice = found
                    .and_then(|s| s.options().into_iter().find(|c| c.key == key))
                    .ok_or_else(|| format!("--set: {source} has no option {key}"))?;
                config.set_option(&choice, source, v)?;
            }
            other => {
                let key = other.replace('-', "_");
                let found = sources.iter().find_map(|s| {
                    let choice = s.options().into_iter().find(|c| c.key == key)?;
                    Some((s.id(), choice))
                });
                let Some((source, choice)) = found else {
                    return Err(format!("unknown option --{other}"));
                };
                let switch = choice.values == ["off", "on"];
                let v = match value(!switch) {
                    Some(v) => v,
                    None if switch => "on".to_owned(),
                    None => return Err(format!("--{other} needs a value")),
                };
                config.set_option(&choice, &source, &v)?;
            }
        }
    }
    config.check()?;
    Ok(config)
}

/// Serves the tools of a server by the transport of its configuration until the client stops:
/// stdio until standard input ends, HTTP until the process ends.
pub fn serve(server: Arc<Server>) -> Result<(), String> {
    let config = server.config();
    let info = server.info().clone();
    log::line(
        "INFO",
        &format!(
            "{} {} started for MCP over {} (process {})",
            info.name,
            info.version,
            config.transport.name(),
            std::process::id()
        ),
    );
    match config.transport {
        Transport::Stdio => stdio::serve(&server).map_err(|e| format!("stdio: {e}")),
        Transport::Http => {
            if config.token.len() < TOKEN_MIN {
                return Err(format!(
                    "a token of {TOKEN_MIN} characters or more is needed (--token or the file)"
                ));
            }
            let http = HttpServer::start(server, config.port, config.token)
                .map_err(|e| format!("http: {e}"))?;
            eprintln!(
                "{} MCP on {} (mode {})",
                info.name,
                http.url(),
                config.mode.name()
            );
            loop {
                std::thread::park();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::tools::{Call, Choice, Outcome, ToolDef};

    struct Options;

    impl Source for Options {
        fn id(&self) -> String {
            "demo".to_owned()
        }
        fn title(&self) -> String {
            "Demo".to_owned()
        }
        fn tools(&self) -> Vec<ToolDef> {
            Vec::new()
        }
        fn options(&self) -> Vec<Choice> {
            vec![
                Choice::switch("read_pc", "This PC"),
                Choice::of(
                    "registry",
                    "Registry",
                    &[("off", ""), ("read", ""), ("write", "")],
                ),
                Choice::text("profile", "Profile"),
            ]
        }
        fn call(&self, _: &str, _: &Value, _: &Call<'_>) -> Outcome {
            Ok(Value::Null)
        }
    }

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_options_of_the_sources_come_from_the_command_line() {
        let sources: Vec<Arc<dyn Source>> = vec![Arc::new(Options)];
        let dir = std::env::temp_dir();
        let line = "stdio --mode edit --read-pc --registry write --profile Strict --off demo/x";
        let read = config(&words(line), &dir, &sources).unwrap();
        assert_eq!(read.transport, Transport::Stdio);
        assert_eq!(read.mode, Mode::Edit);
        let demo = read.source("demo");
        assert_eq!(demo.options["read_pc"], "on");
        assert_eq!(demo.options["registry"], "write");
        assert_eq!(demo.options["profile"], "Strict");
        assert!(demo.tools_off.contains("x"));
        let set = config(
            &words("http --set demo.registry=read --port=9000"),
            &dir,
            &sources,
        );
        let set = set.unwrap();
        assert_eq!(set.port, 9000);
        assert_eq!(set.source("demo").options["registry"], "read");
        for wrong in [
            "",
            "http --registry all",
            "http --colour red",
            "http --mode all",
            "http --profile",
            "http --set demo.x=1",
            "http --token short",
        ] {
            assert!(config(&words(wrong), &dir, &sources).is_err(), "{wrong}");
        }
        assert!(config_path("x", &dir).ends_with(Path::new("mcp").join("x.wwmcp")));
    }
}
