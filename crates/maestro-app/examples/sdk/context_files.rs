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
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Discovered and supplied files both reach the public prompt with their paths.
    #[test]
    fn context_files_merges_discovered_and_supplied_instructions() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
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
