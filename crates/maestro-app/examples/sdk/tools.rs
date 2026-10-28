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
    let project = sample.root.path().join("project");
    std::fs::create_dir(&project)?;
    let mut selections = Vec::new();
    for (names, cwd) in [
        (vec!["read", "grep", "find", "ls"], sample.root.path()),
        (vec!["read", "bash", "grep"], sample.root.path()),
        (vec!["read", "bash", "edit", "write"], project.as_path()),
        (vec!["read", "bash", "grep"], project.as_path()),
    ] {
        let mut options = sample.options()?;
        options.cwd = Some(cwd.display().to_string());
        options.session_manager = Some(std::rc::Rc::new(std::cell::RefCell::new(
            maestro_app::SessionManager::in_memory(cwd.to_str())?,
        )));
        options.tools = Some(names.into_iter().map(str::to_owned).collect());
        let created = maestro_app::create_agent_session(options).await?;
        let context = created.session.extension_runner().create_context();
        selections.push(json!({
            "tools": created.session.get_active_tool_names(), "cwd": context.cwd()?,
        }));
        created.session.dispose();
    }
    Ok(json!(selections))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Allowlisted tools stay in order, including sessions rooted in another directory.
    #[test]
    fn tools_selects_read_only_and_custom_cwd_tools() -> Result<(), Error> {
        let mut root = String::new();
        let value = support::execute(|sample| {
            root = sample.root.path().display().to_string();
            run(sample)
        })?;
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
