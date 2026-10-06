//! Executes trusted repository contribution policy metadata.
#[path = "../contribution_policy/mod.rs"]
mod contribution_policy;

fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), String> {
    use std::io::Write;
    let mut args = std::env::args().skip(1);
    let workflow = args.next().ok_or("Expected workflow argument")?;
    if args.next().is_some()
        || !matches!(
            workflow.as_str(),
            "approve-contributor" | "issue-gate" | "pr-gate" | "contribution-policy"
        )
    {
        return Err("Expected one supported workflow argument".into());
    }
    let event = std::env::var("GITHUB_EVENT_NAME").map_err(|e| e.to_string())?;
    let path = std::env::var("GITHUB_EVENT_PATH").map_err(|e| e.to_string())?;
    let payload = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let slug = std::env::var("MAESTRO_APPROVAL_APP_SLUG").ok();
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let outputs = contribution_policy::run(
        &root,
        &workflow,
        &event,
        &payload,
        slug.as_deref(),
        &mut contribution_policy::SystemProcess,
    )?;
    if !outputs.is_empty() {
        let path = std::env::var("GITHUB_OUTPUT").map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        for (key, value) in outputs {
            writeln!(file, "{key}={value}").map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
