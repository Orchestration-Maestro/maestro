//! Override session preferences and flush a persistent settings change.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the settings sample without touching user preferences.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Return live overrides, durable preferences and in-memory session preferences.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Overrides affect the session while the explicit setter crosses the durability boundary.
    #[test]
    fn settings_applies_overrides_and_flushes_explicit_changes() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value["retry"], true);
        assert_eq!(value["maxRetries"], 5.0);
        assert_eq!(value["compaction"], false);
        assert_eq!(value["saved"]["defaultThinkingLevel"], "low");
        assert!(value["saved"].get("retry").is_none());
        assert_eq!(value["errors"], 0);
        assert_eq!(value["memoryRetry"], false);
        Ok(())
    }
}
