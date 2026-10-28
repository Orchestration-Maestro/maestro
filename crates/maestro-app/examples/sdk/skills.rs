//! Filter discovered skills and append an explicitly attributed SDK skill.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the skill-selection sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Load, filter and merge skills into the session prompt.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Filtering preserves discovered provenance while the custom skill retains its SDK source.
    #[test]
    fn skills_filters_discovery_and_preserves_custom_provenance() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value["skills"][0]["name"], "browser");
        assert_eq!(value["skills"][1]["name"], "my-skill");
        assert_eq!(value["skills"][1]["sourceInfo"]["source"], "sdk");
        assert_eq!(value["skills"][1]["filePath"], "/virtual/SKILL.md");
        assert_eq!(value["skills"].as_array().ok_or("missing skills")?.len(), 2);
        let prompt = value["prompt"].as_str().ok_or("missing prompt")?;
        assert!(prompt.contains("my-skill"));
        assert!(prompt.contains("browser"));
        assert!(!prompt.contains("unrelated-skill"));
        Ok(())
    }
}
