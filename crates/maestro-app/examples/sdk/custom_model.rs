//! Select a catalog model explicitly and request a thinking level.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the model-selection sample with the scripted protocol.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Resolve a model from the catalog and invoke it through the created session.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Explicit model selection keeps the caller's catalog and clamps unsupported thinking.
    #[test]
    fn custom_model_selects_and_prompts_the_catalog_model() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value["selected"], "faux/faux-1");
        assert_eq!(value["thinking"], "off");
        assert_eq!(value["sameCatalog"], true);
        assert_eq!(value["builtInFound"], true);
        assert!(
            value["available"]
                .as_array()
                .ok_or("missing available models")?
                .contains(&json!("faux/faux-1"))
        );
        assert_eq!(value["answer"], "Hello.");
        Ok(())
    }
}
