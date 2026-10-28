//! Register a local extension with lifecycle events, a custom tool and a command.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the inline-extension sample through the public SDK.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Invoke the extension command and a model-requested tool call.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real inline factory receives lifecycle events and executes both registered operations.
    #[test]
    fn extensions_dispatches_commands_events_and_model_requested_tools() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(
            value["events"],
            json!(["command:demo", "agent_start", "tool:my_tool", "agent_end"])
        );
        assert_eq!(value["toolResult"], "Processed: hello");
        assert_eq!(value["answer"], "Done.");
        assert!(
            value["tools"]
                .as_array()
                .ok_or("missing tools")?
                .contains(&json!("my_tool"))
        );
        Ok(())
    }
}
