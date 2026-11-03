//! Dialog tours, notices and literal text of the terminal client example.

use std::io;

use maestro_test_process::child::{leave, write_stderr};
use serde_json::{Value, json};

use super::{Commands, emit, refuse, respond};

/// One terminal client interaction, selected before any protocol output.
#[derive(Clone, Copy)]
pub(in crate::native) enum Kind {
    /// Answer a prompt, four correlated dialogs and six notices in order.
    DialogTour,
    /// Answer the local demonstration prompt and end normally.
    LocalDialogTour,
    /// Reply to one prompt, report four notice severities and end normally.
    Notifications,
    /// Keep a padded and an empty selection value in their correlated replies.
    SelectionReplies,
    /// Report dialog controls as literal text while still answering them.
    DialogText,
    /// Preserve a confirmation message's own line breaks.
    MultilineCommand,
    /// Treat streamed text, tool names and errors as literal data.
    LiteralText,
    /// Report the first termination signal in the file `PEER_MARKER` names.
    #[cfg(unix)]
    TerminationMarker,
    /// Announce readiness, check one prompt and end with status seven.
    PromptExit,
}

/// Read the next command and refuse it unless it carries every authored field.
fn expect(commands: &mut Commands, fields: &Value) -> io::Result<Value> {
    let command = commands.required()?;
    let fields = fields
        .as_object()
        .ok_or_else(|| io::Error::other("expected fields must be an object"))?;
    for (name, value) in fields {
        if command.get(name) != Some(value) {
            return Err(io::Error::other(format!("unexpected {name} in {command}")));
        }
    }
    Ok(command)
}

/// Answer the prompt, then its selection, confirmation, input and editor dialogs.
fn dialog_tour() -> io::Result<()> {
    let mut commands = Commands::new();
    let expected = json!({"type": "prompt", "id": "req_1", "message": "hello"});
    let prompt = expect(&mut commands, &expected)?;
    refuse(&prompt, "controlled refusal")?;
    emit(&json!({"type": "agent_start"}))?;
    emit(
        &json!({"type": "message_update", "assistantMessageEvent": {"type": "text_delta", "delta": "first\u{2028}second"}}),
    )?;
    emit(
        &json!({"type": "extension_ui_request", "id": "s", "method": "select", "title": "Dangerous command?", "options": ["Allow", "Block"]}),
    )?;
    expect(&mut commands, &json!({"id": "s", "value": "Allow"}))?;
    emit(
        &json!({"type": "extension_ui_request", "id": "c", "method": "confirm", "title": "Clear session?", "message": "All messages will be lost."}),
    )?;
    expect(&mut commands, &json!({"id": "c", "confirmed": true}))?;
    emit(
        &json!({"type": "extension_ui_request", "id": "i", "method": "input", "title": "Enter a value", "placeholder": "type something..."}),
    )?;
    expect(&mut commands, &json!({"id": "i", "value": "answer"}))?;
    emit(
        &json!({"type": "extension_ui_request", "id": "e", "method": "editor", "title": "Edit some text", "prefill": "Line 1\nLine 2\nLine 3"}),
    )?;
    expect(
        &mut commands,
        &json!({"id": "e", "value": "Line 1\nLine 2\nLine 3"}),
    )?;
    notices()?;
    expect(&mut commands, &json!({"type": "abort", "id": "req_2"}))?;
    emit(&json!({"type": "response", "id": "req_2", "command": "abort", "success": true}))?;
    emit(&json!({"type": "agent_end", "messages": []}))?;
    write_stderr(b"controlled peer exited\n")?;
    leave(7)
}

/// Report a notice, a title, a status, a widget and editor text without answers.
fn notices() -> io::Result<()> {
    for record in [
        json!({"type": "extension_ui_request", "id": "n", "method": "notify", "message": "Editor prefilled"}),
        json!({"type": "extension_ui_request", "id": "t", "method": "setTitle", "title": "RPC Demo"}),
        json!({"type": "extension_ui_request", "id": "st", "method": "setStatus", "statusKey": "rpc-demo", "statusText": "Turns: 1"}),
        json!({"type": "extension_ui_request", "id": "w", "method": "setWidget", "widgetKey": "rpc-demo", "widgetLines": ["Loaded and ready."]}),
        json!({"type": "extension_ui_request", "id": "p", "method": "set_editor_text", "text": "preset"}),
        json!({"type": "extension_ui_request", "id": "a", "method": "notify", "message": "Abort now"}),
    ] {
        emit(&record)?;
    }
    Ok(())
}

/// Answer the final local demonstration prompt and end normally.
fn local_dialog_tour() -> io::Result<()> {
    let mut commands = Commands::new();
    let expected = json!({"type": "prompt", "id": "req_1", "message": "after local"});
    let prompt = expect(&mut commands, &expected)?;
    respond(&prompt, None)?;
    Ok(())
}

/// Reply to one prompt, then report each notice severity without an answer.
fn notifications() -> io::Result<()> {
    let mut commands = Commands::new();
    let prompt = expect(&mut commands, &json!({"type": "prompt"}))?;
    respond(&prompt, None)?;
    for (id, kind, message) in [
        ("a", None, "default notice"),
        ("b", Some("info"), "information"),
        ("c", Some("warning"), "warning"),
        ("d", Some("error"), "failure"),
    ] {
        let mut record = json!({"type": "extension_ui_request", "id": id, "method": "notify", "message": message});
        if let Some(kind) = kind {
            record["notifyType"] = json!(kind);
        }
        emit(&record)?;
    }
    leave(0)
}

/// Keep a padded and an empty selection value in their correlated replies.
fn selection_replies() -> io::Result<()> {
    let mut commands = Commands::new();
    emit(
        &json!({"type": "extension_ui_request", "id": "padded", "method": "select", "title": "Padded choice", "options": [" Allow "]}),
    )?;
    expect(&mut commands, &json!({"id": "padded", "value": " Allow "}))?;
    emit(
        &json!({"type": "extension_ui_request", "id": "empty", "method": "select", "title": "Empty choice", "options": [""]}),
    )?;
    let reply = expect(&mut commands, &json!({"id": "empty", "value": ""}))?;
    if reply.get("cancelled").is_some() {
        return Err(io::Error::other("empty selection was cancelled"));
    }
    Ok(())
}

/// Report dialog chrome that contains terminal controls while answering normally.
fn dialog_text() -> io::Result<()> {
    let controls = "\u{1b}]52;c;YWJj\u{7}";
    let mut commands = Commands::new();
    emit(
        &json!({"type": "extension_ui_request", "id": "approval", "method": "select", "title": format!("Approve sudo echo {controls}?"), "options": [format!("Allow {controls}")]}),
    )?;
    expect(
        &mut commands,
        &json!({"id": "approval", "value": format!("Allow {controls}")}),
    )?;
    Ok(())
}

/// Preserve the command's own line breaks in a confirmation message.
fn multiline_command() -> io::Result<()> {
    let mut commands = Commands::new();
    emit(
        &json!({"type": "extension_ui_request", "id": "approval", "method": "confirm", "title": "Dangerous command", "message": "sudo echo first\n\necho second"}),
    )?;
    expect(&mut commands, &json!({"id": "approval", "confirmed": true}))?;
    Ok(())
}

/// Treat streamed text, tool names and errors as literal data.
fn literal_text() -> io::Result<()> {
    let controls = "\u{1b}]52;c;?\u{7}";
    let mut commands = Commands::new();
    let prompt = expect(&mut commands, &json!({"type": "prompt"}))?;
    refuse(&prompt, &format!("error {controls} text"))?;
    emit(&json!({"type": "agent_start"}))?;
    for delta in [
        format!("visible {controls} end\nsecond line"),
        " split \u{1b}]".to_owned(),
        "52;c;?\u{7} end".to_owned(),
    ] {
        emit(
            &json!({"type": "message_update", "assistantMessageEvent": {"type": "text_delta", "delta": delta}}),
        )?;
    }
    emit(&json!({"type": "tool_execution_start", "toolName": format!("tool {controls} name")}))?;
    emit(&json!({"type": "agent_end", "messages": []}))?;
    leave(0)
}

/// Report the first termination signal in the marker file and exit normally.
#[cfg(unix)]
fn termination_marker() -> io::Result<()> {
    use std::io::Read as _;

    let marker = super::variable("PEER_MARKER")?;
    let mut signals = super::lifecycle::termination_pipe()?;
    emit(
        &json!({"type": "extension_ui_request", "id": "ready", "method": "notify", "message": "Waiting for termination"}),
    )?;
    if matches!(signals.read(&mut [0]), Ok(1)) {
        std::fs::write(marker, "SIGTERM")?;
        leave(0);
    }
    Err(io::Error::other("termination pipe closed"))
}

/// Announce readiness, check one prompt and end with status seven.
fn prompt_exit() -> io::Result<()> {
    let mut commands = Commands::new();
    emit(
        &json!({"type": "extension_ui_request", "id": "ready", "method": "notify", "message": "Native peer ready"}),
    )?;
    expect(
        &mut commands,
        &json!({"type": "prompt", "message": "hello"}),
    )?;
    leave(7)
}

/// Run one terminal client interaction without a shell or interpreted payload.
pub(super) fn run(scenario: Kind) -> io::Result<()> {
    match scenario {
        Kind::DialogTour => dialog_tour(),
        Kind::LocalDialogTour => local_dialog_tour(),
        Kind::Notifications => notifications(),
        Kind::SelectionReplies => selection_replies(),
        Kind::DialogText => dialog_text(),
        Kind::MultilineCommand => multiline_command(),
        Kind::LiteralText => literal_text(),
        #[cfg(unix)]
        Kind::TerminationMarker => termination_marker(),
        Kind::PromptExit => prompt_exit(),
    }
}
