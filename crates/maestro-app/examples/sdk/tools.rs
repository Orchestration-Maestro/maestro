//! Select read-only or custom built-in tools for a supplied working directory.
mod support;
use std::io::Write;
use support::{Error, Sample};

/// Run the offline tool-selection sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Create sessions with four explicit tool selections.
async fn run(sample: Sample) -> Result<(), Error> {
    let project = sample.root.path().join("project");
    std::fs::create_dir(&project)?;
    for (label, names, cwd) in [
        (
            "Read-only",
            vec!["read", "grep", "find", "ls"],
            sample.root.path(),
        ),
        (
            "Custom tools",
            vec!["read", "bash", "grep"],
            sample.root.path(),
        ),
        (
            "Custom cwd",
            vec!["read", "bash", "edit", "write"],
            project.as_path(),
        ),
        (
            "Specific tools with custom cwd",
            vec!["read", "bash", "grep"],
            project.as_path(),
        ),
    ] {
        let mut options = sample.options()?;
        options.cwd = Some(cwd.display().to_string());
        options.session_manager = Some(std::rc::Rc::new(std::cell::RefCell::new(
            maestro_app::SessionManager::in_memory(cwd.to_str())?,
        )));
        options.tools = Some(names.into_iter().map(str::to_owned).collect());
        let created = maestro_app::create_agent_session(options).await?;
        writeln!(std::io::stdout(), "{label} session created")?;
        created.session.dispose();
    }
    Ok(())
}
