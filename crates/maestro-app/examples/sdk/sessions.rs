//! Create, continue, list and reopen a session in a disposable configuration directory.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the session-persistence sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Exercise in-memory history and a persisted exchange through the public SDK.
async fn run(sample: Sample) -> Result<Value, Error> {
    use maestro_app::{PromptOptions, create_agent_session};
    use maestro_models::{FauxAssistantMessageOptions, FauxResponseStep, faux_assistant_message};
    use std::rc::Rc;
    let options = sample.options()?;
    let supplied = Rc::clone(options.session_manager.as_ref().ok_or("missing manager")?);
    let memory = create_agent_session(options).await?;
    let mut result = json!({"memoryFile": memory.session.session_file(),
        "explicitManager": Rc::ptr_eq(memory.session.session_manager(), &supplied)});
    writeln!(
        sample.output.borrow_mut(),
        "In-memory session: {}",
        memory.session.session_file().as_deref().unwrap_or("(none)")
    )?;
    memory.session.dispose();
    let mut options = sample.options()?;
    options.session_manager = None;
    let created = create_agent_session(options).await?;
    result["newFile"] = json!(created.session.session_file());
    writeln!(
        sample.output.borrow_mut(),
        "New session file: {}",
        created
            .session
            .session_file()
            .ok_or("missing session file")?
    )?;
    sample
        .provider
        .set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message("Remembered.", FauxAssistantMessageOptions::default()),
        ))]);
    created
        .session
        .prompt("Remember this.", PromptOptions::default())
        .await?;
    created.session.dispose();
    continue_and_open(&sample, &mut result).await?;
    Ok(result)
}

/// Continue the saved exchange, list available histories and reopen the selected file.
async fn continue_and_open(sample: &Sample, result: &mut Value) -> Result<(), Error> {
    use maestro_app::{SessionManager, create_agent_session};
    use std::{cell::RefCell, rc::Rc};
    let cwd = sample.root.path().display().to_string();
    let continued = SessionManager::continue_recent(&cwd, None, &cwd)?;
    let mut options = sample.options()?;
    options.session_manager = Some(Rc::new(RefCell::new(continued)));
    let continued = create_agent_session(options).await?;
    if let Some(note) = &continued.model_fallback_message {
        writeln!(sample.output.borrow_mut(), "Note: {note}")?;
    }
    result["continuedFile"] = json!(continued.session.session_file());
    writeln!(
        sample.output.borrow_mut(),
        "Continued session: {}",
        continued
            .session
            .session_file()
            .ok_or("missing continued file")?
    )?;
    continued.session.dispose();
    let sessions = SessionManager::list(&cwd, None, &cwd, None).await?;
    result["count"] = json!(sessions.len());
    print_sessions(sample, &sessions)?;
    let info = sessions.first().ok_or("session was not listed")?;
    result["firstMessage"] = json!(info.first_message);
    let mut options = sample.options()?;
    options.session_manager = Some(Rc::new(RefCell::new(SessionManager::open(
        &info.path, None, None,
    )?)));
    let opened = create_agent_session(options).await?;
    result["openedFile"] = json!(opened.session.session_file());
    result["openedMessages"] = json!(
        opened
            .session
            .messages()
            .read()
            .map_err(|error| error.to_string())?
            .len()
    );
    writeln!(
        sample.output.borrow_mut(),
        "\nOpened: {}",
        opened.session.session_id()
    )?;
    opened.session.dispose();
    Ok(())
}

/// Show up to three saved sessions with their identities and first messages.
fn print_sessions(sample: &Sample, sessions: &[maestro_app::SessionInfo]) -> Result<(), Error> {
    writeln!(
        sample.output.borrow_mut(),
        "\nFound {} sessions:",
        sessions.len()
    )?;
    for info in sessions.iter().take(3) {
        let id = info
            .id
            .present()
            .and_then(Value::as_str)
            .ok_or("missing session identity")?;
        writeln!(
            sample.output.borrow_mut(),
            "  {}... - \"{}...\"",
            id.chars().take(8).collect::<String>(),
            info.first_message.chars().take(30).collect::<String>()
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Session paths, listing and opened identity follow the persistence workflow.
    #[test]
    fn sessions_prints_observations_in_order() -> Result<(), Error> {
        let output = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let value = support::execute(|mut sample| {
            sample.output = output.clone();
            run(sample)
        })?;
        let text = String::from_utf8(output.borrow().clone())?;
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines[0], "In-memory session: (none)");
        let file = value["newFile"].as_str().ok_or("missing file")?;
        assert_eq!(lines[1], format!("New session file: {file}"));
        assert_eq!(lines[2], format!("Continued session: {file}"));
        assert_eq!(lines[3..5], ["", "Found 1 sessions:"]);
        assert!(lines[5].starts_with("  "));
        assert!(lines[5].ends_with("... - \"Remember this....\""));
        assert_eq!(lines[6], "");
        assert!(lines[7].starts_with("Opened: "));
        assert_eq!(lines.len(), 8);
        Ok(())
    }

    /// A default session lives below agentDir; continuation and opening retain its exchange.
    #[test]
    fn sessions_persists_continues_and_reopens_the_same_history() -> Result<(), Error> {
        let mut root = String::new();
        let value = support::execute(|sample| {
            root = sample.root.path().display().to_string();
            run(sample)
        })?;
        assert_eq!(value["memoryFile"], Value::Null);
        assert_eq!(value["explicitManager"], true);
        let file = value["newFile"].as_str().ok_or("missing file")?;
        assert!(file.starts_with(&format!("{root}/sessions/")));
        assert_eq!(value["continuedFile"], file);
        assert_eq!(value["openedFile"], file);
        assert_eq!(value["count"], 1);
        assert_eq!(value["firstMessage"], "Remember this.");
        assert_eq!(value["openedMessages"], 2);
        Ok(())
    }
}
