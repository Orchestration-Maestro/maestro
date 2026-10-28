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
    let result = json!({"files": *files, "prompt": created.session.system_prompt()});
    created.session.dispose();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Discovered and supplied files both reach the public prompt with their paths.
    #[test]
    fn context_files_merges_discovered_and_supplied_instructions() -> Result<(), Error> {
        let value = support::execute(run)?;
        let prompt = value["prompt"].as_str().ok_or("missing prompt")?;
        assert!(prompt.contains("Project rule: answer briefly."));
        assert!(prompt.contains("Prefer immutable bindings."));
        let files = value["files"].as_array().ok_or("missing context files")?;
        assert!(
            files
                .iter()
                .any(|file| file["path"] == "/virtual/AGENTS.md")
        );
        assert!(
            files
                .iter()
                .any(|file| file["content"] == "Project rule: answer briefly.")
        );
        Ok(())
    }
}
