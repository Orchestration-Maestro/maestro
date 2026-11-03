//! Protocol checks of the client example in `examples/sdk`.

use std::io;

use maestro_test_process::child::{leave, read_input, write_stdout};
use serde_json::{Value, json};

use super::{Commands, emit, refuse, respond};

/// One client check, selected before any protocol output.
#[derive(Clone, Copy)]
pub(in crate::native) enum Kind {
    /// Cancel a startup dialog, then answer state with Unicode separators over CRLF.
    UnicodeState,
    /// Accept a prompt and emit the terminal event before the acceptance.
    PromptAcceptance,
    /// Accept a prompt, then keep streaming until the terminal event.
    EndAfterAcceptance,
    /// Accept a prompt and close the stream before any terminal event.
    AcceptedEof,
    /// End with status three after a successful reply.
    ExitAfterReply,
    /// Refuse the startup request without a terminal event.
    RefusedState,
    /// Close the stream before answering anything.
    EarlyEof,
}

/// Read the next command and refuse it unless it equals the authored record.
fn expect(commands: &mut Commands, expected: &Value) -> io::Result<Value> {
    let command = commands.required()?;
    if &command != expected {
        return Err(io::Error::other(format!("unexpected {command}")));
    }
    Ok(command)
}

/// Require the client to close its input without sending another command.
fn closed() -> io::Result<()> {
    let rest = read_input()?;
    if rest.is_empty() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "unexpected input after the reply: {}",
        String::from_utf8_lossy(&rest).trim_end()
    )))
}

/// Write one record terminated by CRLF rather than LF.
fn emit_crlf(record: &Value) -> io::Result<()> {
    write_stdout(&serde_json::to_vec(record)?)?;
    write_stdout(b"\r\n")
}

/// Answer a startup dialog with its cancellation, then answer state over CRLF.
fn unicode_state() -> io::Result<()> {
    let mut commands = Commands::new();
    emit(&json!({"type": "extension_ui_request", "id": "dialog", "method": "input"}))?;
    expect(&mut commands, &json!({"type": "get_state", "id": "state"}))?;
    expect(
        &mut commands,
        &json!({"type": "extension_ui_response", "id": "dialog", "cancelled": true}),
    )?;
    emit_crlf(
        &json!({"type": "response", "id": "state", "success": true, "data": {"sessionName": "a\u{2028}b\u{2029}c"}}),
    )?;
    drop(commands);
    closed()
}

/// Answer state, then accept the prompt after the terminal event it is still waiting for.
fn prompt_acceptance() -> io::Result<()> {
    let mut commands = Commands::new();
    let state = expect(&mut commands, &json!({"type": "get_state", "id": "state"}))?;
    respond(&state, None)?;
    let prompt = expect(
        &mut commands,
        &json!({"type": "prompt", "id": "prompt", "message": "Check fixture"}),
    )?;
    emit(&json!({"type": "response", "id": "unrelated", "success": true}))?;
    emit(
        &json!({"type": "extension_ui_request", "id": "notice", "method": "notify", "message": "running"}),
    )?;
    emit(
        &json!({"type": "message_update", "assistantMessageEvent": {"type": "text_delta", "delta": "Checked"}}),
    )?;
    emit(&json!({"type": "agent_end", "messages": []}))?;
    respond(&prompt, None)?;
    drop(commands);
    closed()
}

/// Accept the prompt first and keep streaming until the terminal event.
fn end_after_acceptance() -> io::Result<()> {
    let mut commands = Commands::new();
    let state = expect(&mut commands, &json!({"type": "get_state", "id": "state"}))?;
    respond(&state, None)?;
    let prompt = expect(
        &mut commands,
        &json!({"type": "prompt", "id": "prompt", "message": "Check fixture"}),
    )?;
    respond(&prompt, None)?;
    emit(
        &json!({"type": "message_update", "assistantMessageEvent": {"type": "text_delta", "delta": "Later"}}),
    )?;
    emit(&json!({"type": "agent_end", "messages": []}))?;
    drop(commands);
    closed()
}

/// Accept the prompt and close the stream without a terminal event.
fn accepted_eof() -> io::Result<()> {
    let mut commands = Commands::new();
    let state = expect(&mut commands, &json!({"type": "get_state", "id": "state"}))?;
    respond(&state, None)?;
    let prompt = commands.required()?;
    respond(&prompt, None)
}

/// Answer the first request, require silence, then end with status three.
fn exit_after_reply() -> io::Result<()> {
    let request = Commands::new().required()?;
    respond(&request, None)?;
    closed()?;
    leave(3)
}

/// Refuse the startup request without a terminal event.
fn refused_state() -> io::Result<()> {
    let request = Commands::new().required()?;
    refuse(&request, "refused")
}

/// End before answering anything.
fn early_eof() -> io::Result<()> {
    Commands::new().required()?;
    Ok(())
}

/// Run one client check without a shell or interpreted payload.
pub(super) fn run(scenario: Kind) -> io::Result<()> {
    match scenario {
        Kind::UnicodeState => unicode_state(),
        Kind::PromptAcceptance => prompt_acceptance(),
        Kind::EndAfterAcceptance => end_after_acceptance(),
        Kind::AcceptedEof => accepted_eof(),
        Kind::ExitAfterReply => exit_after_reply(),
        Kind::RefusedState => refused_state(),
        Kind::EarlyEof => early_eof(),
    }
}
