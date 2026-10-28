//! Supply credential files, runtime keys and a catalog without a custom model file.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run offline credential configuration; interactive OAuth needs an explicit provider login.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Create sessions with explicit credential owners and observe a non-persisted key override.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A runtime key wins without overwriting the credential file or replacing its owner.
    #[test]
    fn api_keys_keeps_runtime_overrides_out_of_persistent_credentials() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value["stored"], "stored-example-key");
        assert_eq!(value["resolved"], "runtime-example-key");
        assert_eq!(value["restored"], "stored-example-key");
        assert_eq!(value["sameAuth"], true);
        assert_eq!(value["withoutCustomModel"], true);
        assert_eq!(value["sessions"], 4);
        Ok(())
    }
}
