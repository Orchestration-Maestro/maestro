//! Create, continue, list and reopen a session in a disposable configuration directory.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the session-persistence sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Exercise in-memory history and a persisted exchange through the public SDK.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A default session lives below agentDir; continuation and opening retain its exchange.
    #[test]
    fn sessions_persists_continues_and_reopens_the_same_history() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let root = sample.root.path().display().to_string();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value["memoryFile"], Value::Null);
        assert_eq!(value["explicitManager"], true);
        let file = value["newFile"].as_str().ok_or("missing file")?;
        assert!(file.starts_with(&format!("{root}/sessions/")));
        assert_eq!(value["continuedFile"], file);
        assert_eq!(value["openedFile"], file);
        assert_eq!(value["count"], 1);
        assert_eq!(value["firstMessage"], "Remember this.");
        assert_eq!(value["openedMessages"], 2);
        Ok(())
    }
}
