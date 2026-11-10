//! The log of the server and the crash handlers of the process. The server writes a file of the
//! day in the logs folder of the program, `mcp-<date>.jsonl`, apart from the log of the window:
//! one JSON object a line (`time`, `level`, then `text` or the fields of an event) for the start
//! and the stop of a transport, every call with its result and its time, the refusals and errors
//! of the transports, and, when the configuration asks for the detailed log, every call and
//! answer whole (`level` DETAIL). A panic is written before the process ends: into the MCP log
//! from a thread of the server or a process that only serves MCP, into the log of the window
//! (`<program>-<date>.log`) from any other thread; a program that catches the fatal exceptions of
//! its system hands them to `fatal`. No token or password is ever written.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};
use ww_base::clock;

/// Where the logs go, set once by [`init`].
struct Target {
    dir: PathBuf,
    /// The name of the log of the window, the program in lower case.
    window: String,
    /// The program and its version, for the lines of a crash.
    program: String,
    /// Whether the process only serves MCP.
    headless: bool,
}

static TARGET: OnceLock<Target> = OnceLock::new();
/// One line at a time, so that lines of two threads never mix.
static WRITING: Mutex<()> = Mutex::new(());

/// The kind of the MCP log in the name of its file.
const MCP: &str = "mcp";

/// The file of the MCP log of today, once [`init`] ran.
pub fn file_of_today() -> Option<PathBuf> {
    TARGET.get().map(|t| t.dir.join(name(MCP)))
}

/// The logs folder, once [`init`] ran.
pub fn dir() -> Option<PathBuf> {
    TARGET.get().map(|t| t.dir.clone())
}

fn name(kind: &str) -> String {
    let now = clock::now_utc_iso();
    let ext = if kind == MCP { "jsonl" } else { "log" };
    format!("{kind}-{}.{ext}", now.get(..10).unwrap_or("unknown"))
}

/// The line of a log: a JSON object for the MCP log, `[time] [level] text` for the window.
fn formatted(file: &Path, level: &str, text: &str) -> String {
    let json = file.extension().is_some_and(|e| e == "jsonl");
    if json {
        json!({ "time": clock::now_utc_iso(), "level": level, "text": text }).to_string()
    } else {
        format!("[{}] [{level}] {text}", clock::now_utc_iso())
    }
}

/// Writes a line. A crash takes the lock only when it is free: the thread that crashed may hold
/// it.
fn append(file: &Path, level: &str, text: &str, crash: bool) {
    write_line(file, &formatted(file, level, text), crash);
}

fn write_line(file: &Path, line: &str, crash: bool) {
    let guard = if crash {
        WRITING.try_lock().ok()
    } else {
        Some(WRITING.lock().unwrap_or_else(|e| e.into_inner()))
    };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        let _ = writeln!(f, "{line}");
        let _ = f.flush();
    }
    drop(guard);
}

/// Appends a line to the MCP log; nothing before [`init`] (the tests).
pub fn line(level: &str, text: &str) {
    if let Some(t) = TARGET.get() {
        let text = text.replace(['\r', '\n'], " ");
        append(&t.dir.join(name(MCP)), level, &text, false);
    }
}

/// Appends an event to the MCP log: the fields of an object after its time and level; nothing
/// before [`init`].
pub fn event(level: &str, fields: Value) {
    if let Some(t) = TARGET.get() {
        let mut line = serde_json::Map::new();
        line.insert("time".to_owned(), Value::from(clock::now_utc_iso()));
        line.insert("level".to_owned(), Value::from(level));
        if let Value::Object(fields) = fields {
            line.extend(fields);
        }
        let text = Value::Object(line).to_string();
        write_line(&t.dir.join(name(MCP)), &text, false);
    }
}

/// Appends a line to the log of the window, for what fails before the window can write it.
pub fn window_line(level: &str, text: &str) {
    if let Some(t) = TARGET.get() {
        let text = text.replace(['\r', '\n'], " ");
        append(&t.dir.join(name(&t.window)), level, &text, false);
    }
}

/// The log a crash of the current thread belongs to.
fn crash_file() -> Option<PathBuf> {
    let t = TARGET.get()?;
    let server_thread = std::thread::current()
        .name()
        .is_some_and(|n| n.starts_with("mcp"));
    let kind = if t.headless || server_thread {
        MCP
    } else {
        &t.window
    };
    Some(t.dir.join(name(kind)))
}

/// Sets the logs folder and installs the panic hook, once per process. `program` names the
/// program and its version for a crash, its first word in lower case
/// names the log of the window; `headless` is true for a process that only serves MCP.
pub fn init(dir: &Path, program: &str, headless: bool) {
    let window = program
        .split_whitespace()
        .next()
        .unwrap_or("program")
        .to_lowercase();
    let target = Target {
        dir: dir.to_owned(),
        window,
        program: program.to_owned(),
        headless,
    };
    if TARGET.set(target).is_err() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let place = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let message = panic_text(info.payload());
        if let Some(file) = crash_file() {
            let trace = std::backtrace::Backtrace::force_capture().to_string();
            append(
                &file,
                "PANIC",
                &format!(
                    "thread '{}' at {place}: {} | {} | backtrace: {}",
                    thread.name().unwrap_or("unnamed"),
                    message.replace(['\r', '\n'], " "),
                    process_line(),
                    trace.replace(['\r', '\n'], " ")
                ),
                true,
            );
        }
        previous(info);
    }));
}

/// The program, its version and the process, for the lines of a crash.
fn process_line() -> String {
    let program = TARGET.get().map(|t| t.program.as_str()).unwrap_or("");
    format!("{program} process {}", std::process::id())
}

/// The name of a few exception codes of Windows.
fn exception_name(code: u32) -> &'static str {
    match code {
        0xC000_0005 => "access violation",
        0xC000_00FD => "stack overflow",
        0xC000_0409 => "stack buffer overrun",
        0xC000_0374 => "heap corruption",
        0xC000_001D => "illegal instruction",
        0xC000_0094 => "integer division by zero",
        0x8000_0003 => "breakpoint",
        _ => "exception",
    }
}

/// Writes an exception of the system that nothing handles, given by the filter of the program;
/// the system ends the process after it.
pub fn fatal(code: u32, address: usize) {
    if let Some(file) = crash_file() {
        let thread = std::thread::current();
        append(
            &file,
            "FATAL",
            &format!(
                "{} 0x{code:08X} at 0x{address:X} in thread '{}' | {}",
                exception_name(code),
                thread.name().unwrap_or("unnamed"),
                process_line()
            ),
            true,
        );
    }
}

/// The text of the payload of a caught panic.
pub fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mcp_log_is_json_lines() {
        let file = Path::new("logs").join(name(MCP));
        assert!(file.to_string_lossy().ends_with(".jsonl"), "{file:?}");
        let line: Value =
            serde_json::from_str(&formatted(&file, "INFO", "a \"quoted\" text")).unwrap();
        assert_eq!(line["level"], "INFO");
        assert_eq!(line["text"], "a \"quoted\" text");
        assert!(name("demo").ends_with(".log"));
        let text = formatted(&Path::new("logs").join(name("demo")), "WARN", "x");
        assert!(text.ends_with("[WARN] x"), "{text}");
    }

    #[test]
    fn exceptions_have_names() {
        assert_eq!(exception_name(0xC000_0005), "access violation");
        assert_eq!(exception_name(0xC000_00FD), "stack overflow");
        assert_eq!(exception_name(1), "exception");
        let payload: Box<dyn std::any::Any + Send> = Box::new("boom");
        assert_eq!(panic_text(payload.as_ref()), "boom");
        let payload: Box<dyn std::any::Any + Send> = Box::new(String::from("bang"));
        assert_eq!(panic_text(payload.as_ref()), "bang");
    }
}
