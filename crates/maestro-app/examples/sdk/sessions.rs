//! Create, continue, list and reopen a session in a disposable configuration directory.
mod support;
use serde_json::Value;
use std::io::Write;
use support::{Error, Sample};

/// Run the session-persistence sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Exercise in-memory history and a persisted exchange through the public SDK.
async fn run(sample: Sample) -> Result<(), Error> {
    use maestro_app::{PromptOptions, create_agent_session};
    use maestro_models::{FauxAssistantMessageOptions, FauxResponseStep, faux_assistant_message};
    let options = sample.options()?;
    let memory = create_agent_session(options).await?;
    writeln!(
        std::io::stdout(),
        "In-memory session: {}",
        memory.session.session_file().as_deref().unwrap_or("(none)")
    )?;
    memory.session.dispose();
    let mut options = sample.options()?;
    options.session_manager = None;
    let created = create_agent_session(options).await?;
    writeln!(
        std::io::stdout(),
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
    continue_and_open(&sample).await?;
    Ok(())
}

/// Continue the saved exchange, list available histories and reopen the selected file.
async fn continue_and_open(sample: &Sample) -> Result<(), Error> {
    use maestro_app::{SessionManager, create_agent_session};
    use std::{cell::RefCell, rc::Rc};
    let cwd = sample.root.path().display().to_string();
    let continued = SessionManager::continue_recent(&cwd, None, &cwd)?;
    let mut options = sample.options()?;
    options.session_manager = Some(Rc::new(RefCell::new(continued)));
    let continued = create_agent_session(options).await?;
    if let Some(note) = &continued.model_fallback_message {
        writeln!(std::io::stdout(), "Note: {note}")?;
    }
    writeln!(
        std::io::stdout(),
        "Continued session: {}",
        continued
            .session
            .session_file()
            .ok_or("missing continued file")?
    )?;
    continued.session.dispose();
    let sessions = SessionManager::list(&cwd, None, &cwd, None).await?;
    print_sessions(&sessions)?;
    let info = sessions.first().ok_or("session was not listed")?;
    let mut options = sample.options()?;
    options.session_manager = Some(Rc::new(RefCell::new(SessionManager::open(
        &info.path, None, None,
    )?)));
    let opened = create_agent_session(options).await?;
    writeln!(
        std::io::stdout(),
        "\nOpened: {}",
        opened.session.session_id()
    )?;
    opened.session.dispose();
    Ok(())
}

/// Show up to three saved sessions with their identities and first messages.
fn print_sessions(sessions: &[maestro_app::SessionInfo]) -> Result<(), Error> {
    writeln!(std::io::stdout(), "\nFound {} sessions:", sessions.len())?;
    for info in sessions.iter().take(3) {
        let id = info
            .id
            .present()
            .and_then(Value::as_str)
            .ok_or("missing session identity")?;
        writeln!(
            std::io::stdout(),
            "  {}... - \"{}...\"",
            id.chars().take(8).collect::<String>(),
            info.first_message.chars().take(30).collect::<String>()
        )?;
    }
    Ok(())
}
