//! The MCP server of W#: the tools of a program and of its plugins for AI agents, over the
//! protocol MCP 2025-06-18 on JSON-RPC 2.0, by stdio or by HTTP on 127.0.0.1 with a bearer token.
//!
//! A program and each of its plugins are sources of tools (`Source`): a source names its tools
//! with their schemas, the group of each, the mode it needs (`read`, `edit`, `files`) and the
//! options it needs (an access setting of the source, `registry` at `read`), and runs them. The
//! server (`Server`) lists the tools the configuration (`config::Config`, a `.wwmcp` file) has on,
//! checks the mode and the options before a call, keeps a journal of the calls and writes its log
//! (`log`). A call by the name of a tool (`Server::call`) is the same door for the program itself,
//! a script or a plugin. It needs no window: `cli` starts it from the command line. With the
//! feature `ui`, `ui` draws its tab in a window of W#; `bridge` brings the calls of the tools that
//! act on the window to its thread.

pub mod bridge;
pub mod cli;
pub mod config;
pub mod http;
pub mod log;
pub mod server;
pub mod stdio;
pub mod tools;
#[cfg(feature = "ui")]
pub mod ui;
#[cfg(feature = "ui")]
pub mod window;

pub use config::Config;
pub use server::{Entry, Info, Listed, MAX_MESSAGE, PROTOCOL_VERSION, Server, Status};
pub use tools::{
    Args, Call, Caller, Choice, Kind, Mode, Need, Outcome, ResourceDef, Source, ToolDef, ToolError,
    fail, no_arguments, schema, text,
};

/// The folder of the sources, for the tests of translations of a program (`tr` of `ui`).
pub const SOURCES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");

/// A token for the HTTP transport: 40 letters and digits from the random source of the system,
/// starting with a letter.
pub fn new_token() -> Result<String, String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut token = String::with_capacity(40);
    for _ in 0..16 {
        let mut bytes = [0u8; 64];
        getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
        for b in bytes {
            // The first character is a letter (52), the others letters or digits (62). A byte
            // at or above the largest multiple of the size below 256 is skipped, so that every
            // character is equally likely.
            let size = if token.is_empty() { 52 } else { 62 };
            if usize::from(b) < 256 - 256 % size {
                token.push(ALPHABET[usize::from(b) % size] as char);
            }
            if token.len() == 40 {
                return Ok(token);
            }
        }
    }
    Err("the random source gave too few usable bytes".to_owned())
}
