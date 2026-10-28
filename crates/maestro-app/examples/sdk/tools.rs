//! Select read-only or custom built-in tools for a supplied working directory.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the offline tool-selection sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Create sessions with four explicit tool selections.
async fn run(sample: Sample) -> Result<Value, Error> {
    let _options = sample.options()?;
    Ok(json!([]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Allowlisted tools stay in order, including sessions rooted in another directory.
    #[test]
    fn tools_selects_read_only_and_custom_cwd_tools() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let root = sample.root.path().display().to_string();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        assert_eq!(value[0]["tools"], json!(["read", "grep", "find", "ls"]));
        assert_eq!(value[1]["tools"], json!(["read", "bash", "grep"]));
        assert_eq!(value[2]["tools"], json!(["read", "bash", "edit", "write"]));
        assert_eq!(value[3]["tools"], json!(["read", "bash", "grep"]));
        assert_eq!(value[0]["cwd"], root);
        assert_eq!(value[2]["cwd"], format!("{root}/project"));
        assert_eq!(value[3]["cwd"], value[2]["cwd"]);
        Ok(())
    }
}
