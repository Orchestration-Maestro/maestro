//! Merge discovered prompt templates with a caller-authored deployment command.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the prompt-template sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Return merged templates and their expanded deployment command.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both templates survive loading and the SDK template expands with its source intact.
    #[test]
    fn prompt_templates_merges_and_expands_the_sdk_template() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value["templates"][0]["name"], "review");
        assert_eq!(value["templates"][1]["name"], "deploy");
        assert_eq!(value["templates"][1]["sourceInfo"]["source"], "sdk");
        assert_eq!(
            value["expanded"],
            "# Deploy Instructions\n\n1. Build: npm run build\n2. Test: npm test\n3. Deploy: npm run deploy"
        );
        Ok(())
    }
}
