//! The configuration file of a server, `mcp/<name>.wwmcp` next to the executable: the transport
//! and, for HTTP, the port and the token, the mode, the detailed log, and in the branch `plugins`
//! the settings of each source of tools by its id (the program and every plugin): whether it is
//! off, the tools and the groups of tools that are off, and the values of its options. The tab of
//! the server edits one and saves it at every change; a start without the window takes one with
//! `--mcp FILE` or `--config FILE`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ww_base::jsonfile;

use crate::tools::{Choice, Mode, Source, ToolDef};

/// The extension of the files, the folder they live in next to the executable and the name of
/// the one the window opens first.
pub const EXTENSION: &str = "wwmcp";
pub const FOLDER: &str = "mcp";
pub const DEFAULT_NAME: &str = "default";
pub const FORMAT: &str = "ww-mcp";
pub const VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 8765;
/// The shortest token the HTTP transport takes.
pub const TOKEN_MIN: usize = 32;
/// The largest file read.
pub const MAX_BYTES: u64 = 256 * 1024;

/// How a client reaches the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    /// http://127.0.0.1:<port>/mcp with the bearer token.
    Http,
    /// The standard input and output of a process the client starts.
    Stdio,
}

impl Transport {
    pub fn name(self) -> &'static str {
        match self {
            Transport::Http => "http",
            Transport::Stdio => "stdio",
        }
    }
}

/// The settings of a source of tools in the configuration file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SourceConfig {
    /// The whole source is off: none of its tools is listed or runs.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
    /// The tools that are off, by their names.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub tools_off: BTreeSet<String>,
    /// The groups of tools that are off.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub groups_off: BTreeSet<String>,
    /// The values of its options by their keys; an option not named has its default.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

impl SourceConfig {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A configuration file. Every field but the format has a default, so a short file is enough.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub format: String,
    pub version: u32,
    pub transport: Transport,
    pub port: u16,
    /// The bearer token of HTTP; the window makes one when it is empty.
    pub token: String,
    pub mode: Mode,
    /// Whether the log keeps every request and answer whole (passwords, keys and tokens left
    /// out); off, the log keeps the call, its identifiers, its result and its time.
    pub detailed_log: bool,
    /// The settings of the sources of tools by their ids: the program and the plugins.
    pub plugins: BTreeMap<String, SourceConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            transport: Transport::Http,
            port: DEFAULT_PORT,
            token: String::new(),
            mode: Mode::Read,
            detailed_log: false,
            plugins: BTreeMap::new(),
        }
    }
}

impl Config {
    /// The folder of the files next to an executable.
    pub fn folder(exe_dir: &Path) -> PathBuf {
        exe_dir.join(FOLDER)
    }

    /// The file the window opens first.
    pub fn default_path(exe_dir: &Path) -> PathBuf {
        Self::folder(exe_dir).join(format!("{DEFAULT_NAME}.{EXTENSION}"))
    }

    /// Reads a file strictly: an unknown field, a wrong format or a bad value is an error.
    pub fn load(path: &Path) -> Result<Self, String> {
        let config: Self = jsonfile::read(path, MAX_BYTES).map_err(|e| e.to_string())?;
        let name = jsonfile::display_name(path);
        if config.format != FORMAT || config.version > VERSION {
            return Err(format!("{name}: not an MCP configuration of this version"));
        }
        config.check().map_err(|e| format!("{name}: {e}"))?;
        Ok(config)
    }

    /// Reads the text of a file, as `load` reads its bytes.
    pub fn parse(text: &str, name: &str) -> Result<Self, String> {
        let config: Self = jsonfile::parse(text, name).map_err(|e| e.to_string())?;
        if config.format != FORMAT || config.version > VERSION {
            return Err(format!("{name}: not an MCP configuration of this version"));
        }
        config.check().map_err(|e| format!("{name}: {e}"))?;
        Ok(config)
    }

    /// The values a server can start with: a port of the user range, a token of letters and
    /// digits that is empty or long enough.
    pub fn check(&self) -> Result<(), String> {
        if self.port < 1024 {
            return Err(format!("the port {} is below 1024", self.port));
        }
        let letters = self.token.chars().all(|c| c.is_ascii_alphanumeric());
        if !self.token.is_empty() && (self.token.len() < TOKEN_MIN || !letters) {
            return Err(format!(
                "the token has at least {TOKEN_MIN} letters and digits"
            ));
        }
        Ok(())
    }

    /// The text of the file.
    pub fn to_text(&self) -> String {
        let mut config = self.clone();
        config.plugins.retain(|_, s| !s.is_empty());
        let mut text = serde_json::to_string_pretty(&config).unwrap_or_default();
        text.push('\n');
        text
    }

    /// Writes the file, with its folder, through a temporary neighbour that replaces it
    /// (`ww_base::files::write`): a write that fails leaves the old file whole.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.check()?;
        ww_base::files::write(path, self.to_text().as_bytes()).map_err(|e| e.to_string())
    }

    /// The settings of a source, the defaults for one the file does not name.
    pub fn source(&self, id: &str) -> SourceConfig {
        self.plugins.get(id).cloned().unwrap_or_default()
    }

    /// The settings of a source to change.
    pub fn source_mut(&mut self, id: &str) -> &mut SourceConfig {
        self.plugins.entry(id.to_owned()).or_default()
    }

    /// Whether a tool of a source is on: the source, its group and itself.
    pub fn is_on(&self, id: &str, tool: &ToolDef) -> bool {
        match self.plugins.get(id) {
            None => true,
            Some(s) => {
                !s.off && !s.tools_off.contains(&tool.name) && !s.groups_off.contains(&tool.group)
            }
        }
    }

    /// The values of the options of a source: its setting where it is one of the values of the
    /// option (any text for a text option), else the default.
    pub fn options_of(&self, source: &dyn Source) -> BTreeMap<String, String> {
        let set = self.plugins.get(&source.id());
        source
            .options()
            .into_iter()
            .map(|choice| {
                let value = set
                    .and_then(|s| s.options.get(&choice.key))
                    .filter(|v| choice.is_text() || choice.rank(v).is_some())
                    .cloned()
                    .unwrap_or_else(|| choice.default.clone());
                (choice.key, value)
            })
            .collect()
    }

    /// Sets an option of a source; a value that is not one of the option's is refused.
    pub fn set_option(&mut self, choice: &Choice, id: &str, value: &str) -> Result<(), String> {
        if !choice.is_text() && choice.rank(value).is_none() {
            return Err(format!(
                "{} is one of {}",
                choice.key,
                choice.values.join(", ")
            ));
        }
        let options = &mut self.source_mut(id).options;
        if value == choice.default {
            options.remove(&choice.key);
        } else {
            options.insert(choice.key.clone(), value.to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configuration_goes_to_a_file_and_back() {
        let name = format!("wwmcp-config-{}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        let path = Config::default_path(&dir);
        let write = |text: &str| std::fs::write(&path, text).unwrap();
        let mut config = Config {
            token: "a".repeat(40),
            mode: Mode::Edit,
            detailed_log: true,
            ..Config::default()
        };
        let source = config.source_mut("demo");
        source.groups_off.insert("files".to_owned());
        source
            .options
            .insert("greeting".to_owned(), "on".to_owned());
        config
            .plugins
            .insert("empty".to_owned(), SourceConfig::default());
        config.save(&path).unwrap();
        let back = Config::load(&path).unwrap();
        config.plugins.remove("empty");
        assert_eq!(back, config);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"mode\": \"edit\""), "{text}");
        assert!(text.contains("\"groups_off\""), "{text}");
        assert!(!text.contains("tools_off"), "{text}");
        // A short file takes the defaults; an unknown field or a short token is refused.
        write(r#"{"format": "ww-mcp", "transport": "stdio"}"#);
        let short = Config::load(&path).unwrap();
        assert_eq!(short.transport, Transport::Stdio);
        assert_eq!(short.mode, Mode::Read);
        write(r#"{"format": "ww-mcp", "colour": 1}"#);
        assert!(Config::load(&path).is_err());
        write(r#"{"format": "ww-mcp", "token": "short"}"#);
        assert!(Config::load(&path).is_err());
        write(r#"{"format": "ww-mcp", "mode": "all"}"#);
        assert!(Config::load(&path).is_err());
        write(r#"{"format": "ww-mcp", "plugins": {"x": {"of": true}}}"#);
        assert!(Config::load(&path).is_err());
        write(r#"{"format": "other"}"#);
        assert!(Config::load(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn options_take_their_values_or_defaults() {
        let choice = Choice::of("access", "Access", &[("off", "Off"), ("read", "Read")]);
        let mut config = Config::default();
        assert!(config.set_option(&choice, "p", "write").is_err());
        config.set_option(&choice, "p", "read").unwrap();
        assert_eq!(config.source("p").options["access"], "read");
        config.set_option(&choice, "p", "off").unwrap();
        assert!(config.source("p").options.is_empty());
        let tool = ToolDef::new("t", "g", "", crate::tools::no_arguments());
        assert!(config.is_on("p", &tool));
        config.source_mut("p").groups_off.insert("g".to_owned());
        assert!(!config.is_on("p", &tool));
    }
}
