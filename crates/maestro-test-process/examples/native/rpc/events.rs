//! Event ordering and responses held across caller-controlled deadlines.

use std::io;

use maestro_test_process::child::{leave, write_stderr};
use serde_json::{Value, json};

use super::{Commands, drain, emit, refuse, respond, state};

/// Event exchanges selected before any protocol output.
#[derive(Clone, Copy)]
pub(in crate::native) enum Kind {
    /// Hold a reply or event behind the next request.
    Delayed(Delayed),
    /// Mark a completed terminal event and reply.
    Answered,
    /// Emit past and future terminal events.
    Collections,
    /// Reply while emitting a selected event sequence.
    Responding(Response),
    /// Acknowledge receipt without answering.
    Received,
    /// Leave a received request unanswered.
    Withhold,
}

/// Outcomes released by the second request.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::native) enum Delayed {
    /// Answer the state request after the second reply.
    State,
    /// Emit the terminal event before both replies.
    End,
    /// Exit with diagnostics instead of replying.
    Exit,
    /// Reply to both requests in order.
    Answers,
}

/// Event sequences emitted while responding to commands.
#[derive(Clone, Copy)]
pub(in crate::native) enum Response {
    /// Refuse the prompt before its terminal event.
    RejectPrompt,
    /// Emit a turn unless the prompt asks for quiet.
    Prompt,
    /// End a turn only when aborted.
    Cancelled,
    /// Keep calls pending until abort or native exit.
    Settle,
    /// Emit the requested number of indexed records.
    Linear,
}

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
fn delayed(scenario: Delayed) -> io::Result<()> {
    if scenario == Delayed::State {
        write_stderr(b"diag\n")?;
    }
    emit(&json!({"type": "peer_ready"}))?;
    let mut commands = Commands::new();
    let first = commands.required()?;
    emit(&json!({"type": "received"}))?;
    let second = commands.required()?;
    match scenario {
        Delayed::State => {
            respond(&second, None)?;
            respond(&first, Some(&state()))?;
        }
        Delayed::End => {
            emit(&json!({"type": "agent_end"}))?;
            respond(&first, None)?;
            respond(&second, None)?;
        }
        Delayed::Exit => {
            write_stderr(b"late\n")?;
            leave(0);
        }
        Delayed::Answers => {
            respond(&first, None)?;
            respond(&second, None)?;
        }
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
fn responding(scenario: Response) -> io::Result<()> {
    if matches!(
        scenario,
        Response::RejectPrompt | Response::Cancelled | Response::Settle
    ) {
        emit(&json!({"type": "peer_ready"}))?;
    }
    let mut commands = Commands::new();
    while let Some((_, command)) = commands.next()? {
        match scenario {
            Response::RejectPrompt => {
                refuse(&command, "Rejected prompt")?;
                emit(&json!({"type": "agent_end"}))?;
                continue;
            }
            Response::Prompt if command["message"] != "quiet" => {
                emit(&json!({"type": "agent_start"}))?;
                emit(&json!({"type": "agent_end", "messages": []}))?;
            }
            Response::Cancelled if command["type"] == "abort" => {
                emit(&json!({"type": "agent_end"}))?;
            }
            Response::Settle => {
                emit(&json!({"type": "received", "command": command["type"]}))?;
                if command["type"] == "bash" {
                    leave(3);
                }
                if command["type"] != "abort" {
                    continue;
                }
            }
            Response::Linear => linear(&command)?,
            Response::Prompt | Response::Cancelled => {}
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
pub(super) fn run(scenario: Kind) -> io::Result<()> {
    match scenario {
        Kind::Delayed(scenario) => delayed(scenario),
        Kind::Answered => answered(),
        Kind::Collections => collections(),
        Kind::Responding(scenario) => responding(scenario),
        Kind::Received | Kind::Withhold => {
            emit(&json!({"type": "peer_ready"}))?;
            Commands::new().required()?;
            if matches!(scenario, Kind::Received) {
                emit(&json!({"type": "received"}))?;
            }
            drain()
        }
    }
}
