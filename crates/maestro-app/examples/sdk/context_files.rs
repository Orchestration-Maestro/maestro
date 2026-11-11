//! Discover project instructions and add a caller-supplied context file.
mod support;
use std::io::Write;
use support::{Error, Sample};

/// Run the context-file sample in a disposable project.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Print the discovered and supplied context files.
async fn run(sample: Sample) -> Result<(), Error> {
    std::fs::write(
        sample.root.path().join("AGENTS.md"),
        "Project rule: answer briefly.",
    )?;
    let mut options = sample.options()?;
    if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
        resources.agents_files_override = Some(Box::new(|mut files| {
            files.push(maestro_models::system_prompt::ContextFile {
                path: "/virtual/AGENTS.md".into(),
                content: "Prefer immutable bindings.".into(),
            });
            files
        }));
    }
    let created = maestro_app::create_agent_session(options).await?;
    let files = created.session.resource_loader().get_agents_files();
    writeln!(std::io::stdout(), "Discovered context files:")?;
    for file in files.iter() {
        writeln!(
            std::io::stdout(),
            "  - {} ({} chars)",
            file.path,
            file.content.chars().count()
        )?;
    }
    writeln!(
        std::io::stdout(),
        "Session created with {} context files",
        files.len()
    )?;
    created.session.dispose();
    Ok(())
}
