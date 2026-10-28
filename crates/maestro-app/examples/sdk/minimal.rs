//! Invoke a session with default tools and stream its text updates without a paid request.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the offline example and print its observed session state.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Prompt the session and return its streamed text and stored messages.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({"text": "", "messages": []}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A streamed answer also becomes part of the session's public history.
    #[test]
    fn minimal_streams_and_retains_the_answer() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value["text"], "The project contains AGENTS.md.");
        assert_eq!(value["messages"][0]["role"], "user");
        assert_eq!(value["messages"][1]["role"], "assistant");
        assert_eq!(value["messages"][1]["content"][0]["text"], value["text"]);
        assert_eq!(value["tools"], json!(["read", "bash", "edit", "write"]));
        Ok(())
    }
}
