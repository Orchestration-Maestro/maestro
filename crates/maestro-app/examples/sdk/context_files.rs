//! Discover project instructions and add a caller-supplied context file.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the context-file sample in a disposable project.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Return context files and the assembled session prompt.
async fn run(sample: Sample) -> Result<Value, Error> {
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
    writeln!(sample.output.borrow_mut(), "Discovered context files:")?;
    for file in files.iter() {
        writeln!(
            sample.output.borrow_mut(),
            "  - {} ({} chars)",
            file.path,
            file.content.chars().count()
        )?;
    }
    writeln!(
        sample.output.borrow_mut(),
        "Session created with {} context files",
        files.len()
    )?;
    let result = json!({"files": *files, "prompt": created.session.system_prompt()});
    created.session.dispose();
    Ok(result)
}
