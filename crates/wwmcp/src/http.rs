//! The HTTP transport: POST of one JSON-RPC message to `/mcp` on 127.0.0.1, answered with JSON
//! (the streamable HTTP transport of MCP without server-sent events). Every request needs the
//! bearer token; the Host and Origin headers must name the loopback address, so that a web page
//! cannot reach the server through DNS rebinding.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::config::TOKEN_MIN;
use crate::server::MAX_MESSAGE;
use crate::{Server, log};

const MAX_HEADERS: usize = 16 * 1024;
const MAX_CONNECTIONS: usize = 8;
const TIMEOUT: Duration = Duration::from_secs(15);

/// A running HTTP transport; dropping it stops the server.
pub struct HttpServer {
    pub port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl HttpServer {
    /// Starts listening on 127.0.0.1 and the port (0: any free port). The token is what every
    /// request must bear: one shorter than `TOKEN_MIN` (an empty one is no authentication at
    /// all) is refused with `InvalidInput`, and nothing listens.
    pub fn start(server: Arc<Server>, port: u16, token: String) -> io::Result<Self> {
        if token.len() < TOKEN_MIN {
            log::line(
                "ERROR",
                &format!("the HTTP server did not start: its token is shorter than {TOKEN_MIN}"),
            );
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the token of the HTTP server has at least {TOKEN_MIN} characters"),
            ));
        }
        let listener = TcpListener::bind(("127.0.0.1", port)).inspect_err(|e| {
            log::line(
                "ERROR",
                &format!("the HTTP server cannot listen on port {port}: {e}"),
            );
        })?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let info = server.info();
        log::line(
            "INFO",
            &format!(
                "HTTP server of {} {} on http://127.0.0.1:{port}/mcp, mode {}",
                info.name,
                info.version,
                server.config().mode.name()
            ),
        );
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let active = Arc::new(AtomicUsize::new(0));
        let token = Arc::new(token);
        let thread = std::thread::Builder::new()
            .name("mcp-http".to_owned())
            .spawn(move || {
                let mut last_error = String::new();
                while !flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, peer)) => {
                            if !peer.ip().is_loopback() {
                                log::line("WARN", &format!("a connection from {peer} was refused"));
                                continue;
                            }
                            if active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                                let _ = respond(stream, 503, "", "the server is busy");
                                continue;
                            }
                            active.fetch_add(1, Ordering::Relaxed);
                            let (srv, tok, act) = (server.clone(), token.clone(), active.clone());
                            let spawned = std::thread::Builder::new()
                                .name("mcp-request".to_owned())
                                .spawn(move || {
                                    // The count goes down also when the request fails inside.
                                    let done =
                                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                            || handle(&srv, stream, peer, port, &tok),
                                        ));
                                    act.fetch_sub(1, Ordering::Relaxed);
                                    match done {
                                        Ok(Ok(())) => {}
                                        Ok(Err(e)) => {
                                            log::line("WARN", &format!("a request failed: {e}"))
                                        }
                                        Err(p) => log::line(
                                            "ERROR",
                                            &format!(
                                                "a request failed inside the program: {}",
                                                log::panic_text(p.as_ref())
                                            ),
                                        ),
                                    }
                                });
                            if let Err(e) = spawned {
                                active.fetch_sub(1, Ordering::Relaxed);
                                log::line("ERROR", &format!("no thread for a request: {e}"));
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(40));
                        }
                        Err(e) => {
                            let text = e.to_string();
                            if text != last_error {
                                log::line(
                                    "ERROR",
                                    &format!("accepting a connection failed: {text}"),
                                );
                                last_error = text;
                            }
                            std::thread::sleep(Duration::from_millis(200));
                        }
                    }
                }
                log::line("INFO", &format!("HTTP server on port {port} stopped"));
            })?;
        Ok(Self {
            port,
            stop,
            thread: Some(thread),
        })
    }

    /// The address clients use.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Compares two texts in a time that does not depend on where they differ.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

fn respond(mut stream: TcpStream, status: u16, body: &str, error: &str) -> io::Result<()> {
    if status >= 400 {
        log::line(
            "WARN",
            &format!("HTTP {status} {}: {error}", reason(status)),
        );
    }
    let body = if body.is_empty() && !error.is_empty() {
        format!("{{\"error\": \"{}\"}}", error.replace('"', "'"))
    } else {
        body.to_owned()
    };
    let mut head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n",
        reason(status),
        body.len()
    );
    if status == 401 {
        head.push_str("WWW-Authenticate: Bearer\r\n");
    }
    if status == 405 {
        head.push_str("Allow: POST\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

/// Whether a Host or Origin value names this server on the loopback address.
fn loopback(value: &str, port: u16, with_scheme: bool) -> bool {
    let value = value.trim().to_ascii_lowercase();
    let value = if with_scheme {
        match value.strip_prefix("http://") {
            Some(v) => v.to_owned(),
            None => return false,
        }
    } else {
        value
    };
    value == format!("127.0.0.1:{port}") || value == format!("localhost:{port}")
}

fn handle(
    server: &Server,
    stream: TcpStream,
    _peer: SocketAddr,
    port: u16,
    token: &str,
) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut head = Vec::new();
    loop {
        let mut line = Vec::new();
        let n = reader
            .by_ref()
            .take(MAX_HEADERS as u64)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            return Ok(());
        }
        head.extend_from_slice(&line);
        if head.len() > MAX_HEADERS {
            return respond(stream, 400, "", "the headers are too large");
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
    }
    let head = String::from_utf8_lossy(&head);
    let mut lines = head.lines();
    let request = lines.next().unwrap_or("");
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let mut length = None;
    let mut host = None;
    let mut origin = None;
    let mut authorization = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = Some(value.trim().to_owned()),
            "host" => host = Some(value.trim().to_owned()),
            "origin" => origin = Some(value.trim().to_owned()),
            "authorization" => authorization = Some(value.trim().to_owned()),
            _ => {}
        }
    }
    if !host.as_deref().is_some_and(|h| loopback(h, port, false)) {
        return respond(
            stream,
            403,
            "",
            "the Host header must name 127.0.0.1 or localhost",
        );
    }
    if origin.as_deref().is_some_and(|o| !loopback(o, port, true)) {
        return respond(stream, 403, "", "requests from web pages are refused");
    }
    let bearer = authorization
        .as_deref()
        .and_then(|a| a.strip_prefix("Bearer "))
        .unwrap_or("");
    if !same_secret(bearer.trim(), token) {
        return respond(stream, 401, "", "a valid bearer token is needed");
    }
    if target.split('?').next() != Some("/mcp") {
        return respond(stream, 404, "", "the endpoint is /mcp");
    }
    if method != "POST" {
        return respond(
            stream,
            405,
            "",
            "only POST is served; there is no event stream",
        );
    }
    let Some(length) = length else {
        return respond(stream, 411, "", "Content-Length is needed");
    };
    if length.is_empty() || !length.bytes().all(|b| b.is_ascii_digit()) {
        return respond(stream, 400, "", "Content-Length is not a number");
    }
    let length: usize = match length.parse() {
        Ok(n) if n <= MAX_MESSAGE => n,
        _ => return respond(stream, 413, "", "the message is too large"),
    };
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let Ok(text) = String::from_utf8(body) else {
        return respond(stream, 400, "", "the body is not UTF-8");
    };
    match server.handle_text(&text) {
        Some(answer) => respond(stream, 200, &answer, ""),
        None => respond(stream, 202, "", ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_and_hosts() {
        assert!(
            same_secret("abc", "abc") && !same_secret("abc", "abd") && !same_secret("abc", "ab")
        );
        assert!(loopback("127.0.0.1:8765", 8765, false) && loopback("LOCALHOST:8765", 8765, false));
        assert!(
            !loopback("evil.example:8765", 8765, false) && !loopback("127.0.0.1:1", 8765, false)
        );
        assert!(
            loopback("http://127.0.0.1:8765", 8765, true) && !loopback("https://x", 8765, true)
        );
    }

    #[test]
    fn a_short_token_starts_no_server() {
        let server = || {
            Arc::new(Server::new(
                crate::Info::default(),
                crate::Config::default(),
            ))
        };
        for token in [String::new(), "a".repeat(TOKEN_MIN - 1)] {
            let Err(refused) = HttpServer::start(server(), 0, token) else {
                panic!("a server started with a short token");
            };
            assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
        }
        let started = HttpServer::start(server(), 0, "a".repeat(TOKEN_MIN)).unwrap();
        assert!(started.port != 0 && started.url().ends_with("/mcp"));
    }
}
