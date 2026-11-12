//! The stdio transport: one JSON-RPC message per line on standard input, one answer per line on
//! standard output. The client starts the program with `--mcp stdio`. A line longer than the
//! largest message is never read whole, and one that is not UTF-8 is no message: each gets an
//! error answer and the server goes on with the next line.

use std::io::{self, BufRead, Read, Write};

use crate::server::{MAX_MESSAGE, refusal};
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

/// Reads past the end of the line the input is in, keeping none of it.
fn skip_line(input: &mut impl BufRead) -> io::Result<()> {
    loop {
        let buffer = match input.fill_buf() {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if buffer.is_empty() {
            return Ok(());
        }
        let (used, ended) = match buffer.iter().position(|b| *b == b'\n') {
            Some(at) => (at + 1, true),
            None => (buffer.len(), false),
        };
        input.consume(used);
        if ended {
            return Ok(());
        }
    }
}

/// Serves the messages of `input`, answering into `output`.
pub fn serve_on(
    server: &Server,
    mut input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    // One byte more than the largest message with its line break: a line that fills it without
    // ending is too long, and the rest of it is skipped, not kept.
    let most = MAX_MESSAGE as u64 + 2;
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = input.by_ref().take(most).read_until(b'\n', &mut line)?;
        if read == 0 {
            return Ok(());
        }
        let ended = line.ends_with(b"\n");
        while line.last().is_some_and(|b| matches!(b, b'\n' | b'\r')) {
            line.pop();
        }
        let answer = if line.len() > MAX_MESSAGE {
            if !ended {
                skip_line(&mut input)?;
            }
            log::line(
                "WARN",
                &format!(
                    "a line of more than {MAX_MESSAGE} bytes was refused: too large; the server goes on"
                ),
            );
            Some(refusal(-32600, "the message is too large"))
        } else {
            match std::str::from_utf8(&line) {
                Ok(text) if text.trim().is_empty() => None,
                Ok(text) => server.handle_text(text),
                Err(_) => {
                    log::line(
                        "WARN",
                        "a line that is not UTF-8 was refused; the server goes on",
                    );
                    Some(refusal(-32700, "the message is not UTF-8"))
                }
            }
        };
        if let Some(answer) = answer {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::{Config, Info};

    fn answers(input: &[u8]) -> Vec<Value> {
        let server = Server::new(Info::default(), Config::default());
        let mut out = Vec::new();
        serve_on(&server, input, &mut out).unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    const PING: &str = r#"{"jsonrpc": "2.0", "id": 5, "method": "ping"}"#;

    #[test]
    fn a_line_too_long_or_not_utf8_gets_an_error_and_the_server_goes_on() {
        let mut input = Vec::new();
        // A line far longer than the largest message, then one that is no UTF-8, an empty one,
        // a message with the line break of Windows, and a last one without a line break.
        input.extend(std::iter::repeat_n(b'a', 3 * MAX_MESSAGE));
        input.push(b'\n');
        input.extend([b'{', 0xFF, 0xFE, b'}', b'\n']);
        input.extend(b"\r\n");
        input.extend(PING.as_bytes());
        input.extend(b"\r\n");
        input.extend(PING.as_bytes());
        let got = answers(&input);
        assert_eq!(got.len(), 4, "{got:?}");
        assert_eq!(got[0]["error"]["code"], -32600);
        assert_eq!(got[0]["error"]["message"], "the message is too large");
        assert_eq!(got[1]["error"]["code"], -32700);
        assert_eq!(got[1]["error"]["message"], "the message is not UTF-8");
        assert_eq!(got[2]["id"], 5);
        assert!(got[2]["result"].is_object(), "{}", got[2]);
        assert!(got[3]["result"].is_object(), "{}", got[3]);
    }

    #[test]
    fn the_largest_message_is_read_and_one_byte_more_is_not() {
        // A message of exactly the largest size is handed to the server (it is no JSON-RPC
        // request, which the server says); one byte more is refused before it is read whole.
        let fits = format!("\"{}\"", "a".repeat(MAX_MESSAGE - 2));
        let mut input = fits.clone().into_bytes();
        input.extend(b"\r\n");
        input.extend(fits.as_bytes());
        input.extend(b"a\n");
        // Too long and the last line, without a line break.
        input.extend(std::iter::repeat_n(b'b', MAX_MESSAGE + 1));
        let got = answers(&input);
        assert_eq!(got.len(), 3, "{got:?}");
        assert_ne!(got[0]["error"]["message"], "the message is too large");
        assert_eq!(got[1]["error"]["message"], "the message is too large");
        assert_eq!(got[2]["error"]["message"], "the message is too large");
    }
}
