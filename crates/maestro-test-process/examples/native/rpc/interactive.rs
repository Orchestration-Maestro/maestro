//! Session peers for the interactive client example's existing integration tests.

use std::io::{self, Read};

use maestro_test_process::child::{announce, connect, leave, write_stderr};
use serde_json::{Value, json};

use super::{Commands, emit, respond};

/// Write the native termination marker before exiting normally.
fn terminate() -> io::Result<()> {
    let mut signals = super::lifecycle::termination_pipe()?;
    std::thread::Builder::new().spawn(move || -> io::Result<()> {
        if signals.read(&mut [0])? != 0 {
            write_stderr(b"terminated\n")?;
            leave(0);
        }
        Ok(())
    })?;
    Ok(())
}

/// Renderable turn records, including a delta the illustration must ignore.
fn turn() -> [Value; 7] {
    [
        json!({"type": "message_update", "assistantMessageEvent": {"type": "text_delta", "delta": "Hi there"}}),
        json!({"type": "message_update", "assistantMessageEvent": {"type": "toolcall_delta", "delta": "skipped"}}),
        json!({"type": "message_update", "assistantMessageEvent": {"type": "thinking_delta", "delta": " (thinking)"}}),
        json!({"type": "tool_execution_start", "toolName": "read"}),
        json!({"type": "tool_execution_end", "result": {"content": [{"type": "text", "text": "ok"}]}}),
        json!({"type": "turn_end"}),
        json!({"type": "agent_end", "messages": []}),
    ]
}

/// Answer session commands while capturing them verbatim on standard error.
pub(super) fn session(state: &Value) -> io::Result<()> {
    terminate()?;
    let mut commands = Commands::new();
    while let Some((raw, command)) = commands.next()? {
        write_stderr(raw.as_bytes())?;
        let kind = command["type"].as_str();
        respond(&command, (kind == Some("get_state")).then_some(state))?;
        match kind {
            Some("prompt") if command["message"] == "hold" => {
                emit(&json!({"type": "agent_start"}))?;
            }
            Some("prompt") => {
                for record in turn() {
                    emit(&record)?;
                }
            }
            Some("steer" | "abort") => emit(&json!({"type": "agent_end", "messages": []}))?,
            _ => {}
        }
    }
    Ok(())
}

/// Observe TERM and input closure separately after the client's executor ends.
pub(super) fn idle(state: &Value, marker: &str, port: u16) -> io::Result<()> {
    let mut signals = super::lifecycle::termination_pipe()?;
    let mut control = connect(port)?;
    let state = state.clone();
    let commands = std::thread::Builder::new().spawn(move || -> io::Result<()> {
        let mut commands = Commands::new();
        while let Some((_, command)) = commands.next()? {
            respond(&command, Some(&state))?;
        }
        Ok(())
    })?;
    signals.read_exact(&mut [0])?;
    std::fs::write(marker, "terminated")?;
    announce(&mut control, "terminated")?;
    commands
        .join()
        .map_err(|_| io::Error::other("command reader failed"))??;
    announce(&mut control, "input closed")?;
    leave(0)
}
