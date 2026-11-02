//! Event ordering and responses held across caller-controlled deadlines.

use std::io;

use maestro_test_process::child::{leave, write_stderr};
use maestro_test_process::rpc_peer::Scenario;
use serde_json::{Value, json};

use super::{Commands, drain, emit, refuse, respond, state};

/// Answer a request before emitting every supplied event in order.
pub(super) fn burst(records: &[Value]) -> io::Result<()> {
    let mut commands = Commands::new();
    while let Some((_, command)) = commands.next()? {
        respond(&command, None)?;
        for record in records {
            emit(record)?;
        }
    }
    Ok(())
}

/// Hold a first response or terminal event until the caller sends another command.
fn delayed(scenario: Scenario) -> io::Result<()> {
    if scenario == Scenario::LateState {
        write_stderr(b"diag\n")?;
    }
    emit(&json!({"type": "peer_ready"}))?;
    let mut commands = Commands::new();
    let first = commands.required()?;
    emit(&json!({"type": "received"}))?;
    let second = commands.required()?;
    match scenario {
        Scenario::LateState => {
            respond(&second, None)?;
            respond(&first, Some(&state()))?;
        }
        Scenario::LateEnd => {
            emit(&json!({"type": "agent_end"}))?;
            respond(&first, None)?;
            respond(&second, None)?;
        }
        Scenario::LateExit => {
            write_stderr(b"late\n")?;
            leave(0);
        }
        Scenario::LateAnswers => {
            respond(&first, None)?;
            respond(&second, None)?;
        }
        _ => return Err(io::Error::other("not a delayed scenario")),
    }
    drain()
}

/// Emit a terminal event and mark the completed response before awaiting EOF.
fn answered() -> io::Result<()> {
    emit(&json!({"type": "peer_ready"}))?;
    let command = Commands::new().required()?;
    emit(&json!({"type": "agent_end"}))?;
    respond(&command, None)?;
    emit(&json!({"type": "answered"}))?;
    drain()
}

/// Emit past and future terminal events separated by a command.
fn collections() -> io::Result<()> {
    emit(&json!({"type": "agent_end", "past": true}))?;
    let mut commands = Commands::new();
    while let Some((_, command)) = commands.next()? {
        for kind in [
            "message_update",
            "turn_end",
            "turn_end",
            "agent_end",
            "late",
        ] {
            emit(&json!({"type": kind}))?;
        }
        respond(&command, None)?;
    }
    Ok(())
}

/// Serve prompt, cancellation and linear-dispatch event sequences.
fn responding(scenario: Scenario) -> io::Result<()> {
    if matches!(
        scenario,
        Scenario::RejectPrompt | Scenario::Cancelled | Scenario::Settle
    ) {
        emit(&json!({"type": "peer_ready"}))?;
    }
    let mut commands = Commands::new();
    while let Some((_, command)) = commands.next()? {
        match scenario {
            Scenario::RejectPrompt => {
                refuse(&command, "Rejected prompt")?;
                emit(&json!({"type": "agent_end"}))?;
                continue;
            }
            Scenario::Prompt if command["message"] != "quiet" => {
                emit(&json!({"type": "agent_start"}))?;
                emit(&json!({"type": "agent_end", "messages": []}))?;
            }
            Scenario::Cancelled if command["type"] == "abort" => {
                emit(&json!({"type": "agent_end"}))?;
            }
            Scenario::Settle => {
                emit(&json!({"type": "received", "command": command["type"]}))?;
                if command["type"] == "bash" {
                    leave(3);
                }
                if command["type"] != "abort" {
                    continue;
                }
            }
            Scenario::Linear => linear(&command)?,
            _ => {}
        }
        respond(&command, None)?;
    }
    Ok(())
}

/// Emit a finite number of indexed records followed by a completion event.
fn linear(command: &Value) -> io::Result<()> {
    let count = command["command"]
        .as_str()
        .ok_or_else(|| io::Error::other("missing frame count"))?
        .parse::<usize>()
        .map_err(io::Error::other)?;
    for index in 0..count {
        emit(&json!({"type": "tick", "index": index}))?;
    }
    emit(&json!({"type": "done"}))
}

/// Run event scenarios whose control comes from native command input.
pub(super) fn run(scenario: Scenario) -> io::Result<()> {
    match scenario {
        Scenario::Burst => Err(io::Error::other("burst records are missing")),
        Scenario::LateState | Scenario::LateEnd | Scenario::LateAnswers | Scenario::LateExit => {
            delayed(scenario)
        }
        Scenario::Answered => answered(),
        Scenario::Collections => collections(),
        Scenario::Prompt
        | Scenario::RejectPrompt
        | Scenario::Cancelled
        | Scenario::Settle
        | Scenario::Linear => responding(scenario),
        Scenario::Received | Scenario::Withhold => {
            emit(&json!({"type": "peer_ready"}))?;
            Commands::new().required()?;
            if scenario == Scenario::Received {
                emit(&json!({"type": "received"}))?;
            }
            drain()
        }
        _ => Err(io::Error::other("not an event scenario")),
    }
}
