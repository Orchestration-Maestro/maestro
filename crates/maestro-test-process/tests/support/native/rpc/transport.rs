//! Correlated responses, malformed records and byte-stream fragments.

use std::io;

use maestro_test_process::child::write_stdout;
use serde_json::{Map, Value, json};

use super::{Commands, drain, emit, refuse, respond, state};

/// Correlated transport exchanges without additional fixture data.
#[derive(Clone, Copy)]
pub(in crate::native) enum Kind {
    /// Refuse every request with a correlated diagnostic.
    Refuse,
    /// Reply out of order with a duplicate response.
    Reversed,
    /// Emit malformed input before a matching reply.
    Malformed,
}

/// Capture every raw request before answering with its selected data.
pub(super) fn serve(answers: &Map<String, Value>) -> io::Result<()> {
    emit(&json!({"type": "peer_ready"}))?;
    let mut commands = Commands::new();
    while let Some((raw, command)) = commands.next()? {
        emit(&json!({"type": "wire_capture", "raw": raw}))?;
        respond(
            &command,
            command["type"].as_str().and_then(|kind| answers.get(kind)),
        )?;
    }
    Ok(())
}

/// Replay full response records, replacing only the request identifier.
pub(super) fn replay(replies: &[Map<String, Value>]) -> io::Result<()> {
    let mut commands = Commands::new();
    for reply in replies {
        let command = commands.required()?;
        let mut reply = reply.clone();
        reply.insert("id".into(), command["id"].clone());
        emit(&Value::Object(reply))?;
    }
    drain()
}

/// Emit arbitrary JSON values after ignored null lines.
pub(super) fn records(records: &[Value]) -> io::Result<()> {
    for record in records {
        write_stdout(b"null\n")?;
        emit(record)?;
    }
    drain()
}

/// Refuse every command with a correlated Unicode diagnostic.
fn refusing() -> io::Result<()> {
    let mut commands = Commands::new();
    while let Some((_, command)) = commands.next()? {
        let kind = command["type"]
            .as_str()
            .ok_or_else(|| io::Error::other("missing command type"))?;
        refuse(&command, &format!("Denied {kind} é"))?;
    }
    Ok(())
}

/// Answer the second command first, then duplicate the first command's response.
fn reversed() -> io::Result<()> {
    let mut commands = Commands::new();
    let first = commands.required()?;
    let second = commands.required()?;
    respond(&second, Some(&json!({"path": "second.html"})))?;
    respond(&first, Some(&json!({"cancelled": true})))?;
    respond(&first, Some(&json!({"cancelled": false})))?;
    emit(&json!({"type": "done"}))?;
    drain()
}

/// Hold a state response behind malformed records and a command barrier.
fn malformed() -> io::Result<()> {
    let mut commands = Commands::new();
    let first = commands.required()?;
    write_stdout(b"null\n{not json\n")?;
    emit(&json!({"type": "barrier"}))?;
    let second = commands.required()?;
    respond(&first, Some(&state()))?;
    respond(&second, None)?;
    drain()
}

/// Write split UTF-8, CRLF, Unicode separators and an unterminated tail on acknowledged release.
#[cfg(unix)]
pub(super) fn fragments(port: u16) -> io::Result<()> {
    use maestro_test_process::child::{connect, receive};

    let mut control = connect(port)?;
    for chunk in [
        &b"{\"type\":\"a\",\"text\":\"\xc3"[..],
        b"\xa9\xf0\x9f",
        b"\x99\x82\"}\r",
        b"\n{\"type\":\"b\",\"text\":\"\xe2\x80\xa8\xe2\x80\xa9\"}\n{\"type\":\"c\"}\n{\"type\":",
        b"\"tail\"}",
    ] {
        write_stdout(chunk)?;
        maestro_test_process::child::announce(&mut control, "written")?;
        if receive(&mut control)?.is_none() {
            return Err(io::Error::other("fragment release ended early"));
        }
    }
    maestro_test_process::child::close_streams(&[maestro_test_process::child::Stream::Output])?;
    drain()
}

/// Select the finite transport variants without interpreting source text.
pub(super) fn run(scenario: Kind) -> io::Result<()> {
    match scenario {
        Kind::Refuse => refusing(),
        Kind::Reversed => reversed(),
        Kind::Malformed => malformed(),
    }
}
