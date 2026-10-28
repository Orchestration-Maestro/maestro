//! Replace the active session and rebind its local subscriptions after each change.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the runtime-replacement sample over disposable histories.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Create, replace and resume the session while observing its live queues.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// New and resumed sessions get fresh subscriptions rather than retaining the old session.
    #[test]
    fn session_runtime_rebinds_after_new_and_resumed_sessions() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value["newIsDifferent"], true);
        assert_eq!(value["resumedOriginal"], true);
        assert_eq!(value["queues"], json!([1, 1, 1]));
        assert_eq!(value["messages"], 2);
        Ok(())
    }
}
