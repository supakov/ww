//! The stdio transport: one JSON-RPC message per line on standard input, one answer per line on
//! standard output. The client starts the program with `--mcp stdio`.

use std::io::{self, BufRead, Write};

use crate::{Server, log};

/// Serves until standard input ends.
pub fn serve(server: &Server) -> io::Result<()> {
    let info = server.info();
    log::line(
        "INFO",
        &format!(
            "stdio server of {} {} started, mode {}",
            info.name,
            info.version,
            server.config().mode.name()
        ),
    );
    let result = serve_on(server, io::stdin().lock(), io::stdout().lock());
    match &result {
        Ok(()) => log::line("INFO", "the client closed standard input; the server stops"),
        Err(e) => log::line("ERROR", &format!("standard input or output failed: {e}")),
    }
    result
}

/// Serves the messages of `input`, answering into `output`.
pub fn serve_on(server: &Server, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(answer) = server.handle_text(&line) {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
    Ok(())
}
