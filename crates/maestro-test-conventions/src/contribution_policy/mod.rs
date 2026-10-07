mod approve_contributor;
mod approved_users;
mod contribution_activity;
mod github;
mod issue_gate;
mod pr_gate;
mod semantics;
use std::path::Path;
use std::process::Output;

/// Supplies process execution and diagnostics for repository policy.
pub trait Process {
    /// Emits an informational policy diagnostic.
    fn diagnostic(&mut self, message: &str) {
        eprintln!("{message}");
    }
    /// Runs a program in the supplied directory with optional standard input.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when the program cannot be started or waited for.
    fn output(
        &mut self,
        cwd: &Path,
        program: &str,
        args: &[String],
        input: Option<&[u8]>,
    ) -> Result<Output, String>;
}

/// Trusted event metadata supplied to a repository policy.
pub struct Invocation<'a> {
    /// Directory containing the trusted repository policy files.
    pub root: &'a Path,
    /// Repository workflow name to execute.
    pub workflow: &'a str,
    /// GitHub event name.
    pub event_name: &'a str,
    /// JSON event payload.
    pub payload: &'a str,
    /// Expected approval application identity, when required.
    pub app_slug: Option<&'a str>,
}

/// Applies a repository policy and returns workflow output key-value pairs.
///
/// # Errors
///
/// Returns a diagnostic for invalid metadata or failed policy operations.
pub fn run(
    invocation: &Invocation<'_>,
    process: &mut dyn Process,
) -> Result<Vec<(String, String)>, String> {
    let Invocation {
        root,
        workflow,
        event_name,
        payload,
        app_slug,
    } = *invocation;
    let e: serde_json::Value = serde_json::from_str(payload).map_err(|e| e.to_string())?;
    if workflow == "approve-contributor"
        && event_name == "issue_comment"
        && e["action"] == "created"
        && e["issue"].get("pull_request").is_none()
    {
        if semantics::command(e["comment"]["body"].as_str().unwrap_or("")).is_none() {
            process.diagnostic("Comment does not match lgtm or lgtmi");
            return Ok(vec![("status".into(), "skipped".into())]);
        }
        let mut github = github(root, &e, process)?;
        return approve_contributor::approve(&mut github, &e, app_slug);
    }
    if e["action"] == "opened"
        && ((workflow == "issue-gate" && event_name == "issues")
            || (workflow == "pr-gate" && event_name == "pull_request_target"))
    {
        gate(root, workflow, &e, process)?;
    }
    if workflow == "contribution-policy"
        && e["action"] == "opened"
        && matches!(event_name, "issues" | "pull_request_target")
    {
        let item = if e.get("pull_request").is_some() {
            &e["pull_request"]
        } else {
            &e["issue"]
        };
        let mut github = github(root, &e, process)?;
        contribution_activity::check(&mut github, item)?;
    }
    if workflow == "approve-contributor"
        && event_name == "pull_request_target"
        && e["action"] == "closed"
    {
        let mut github = github(root, &e, process)?;
        approve_contributor::complete(&mut github, &e, app_slug)?;
    }
    Ok(vec![])
}

/// Executes policy commands as native subprocesses.
pub struct SystemProcess;

impl Process for SystemProcess {
    fn output(
        &mut self,
        cwd: &Path,
        program: &str,
        args: &[String],
        input: Option<&[u8]>,
    ) -> Result<Output, String> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start {program}: {e}"))?;
        if let Some(input) = input {
            let result = child
                .stdin
                .take()
                .ok_or("Missing child stdin")
                .and_then(|mut stdin| {
                    stdin
                        .write_all(input)
                        .map_err(|_| "Child stdin write failed")
                });
            if let Err(error) = result {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        child
            .wait_with_output()
            .map_err(|e| format!("Could not wait for {program}: {e}"))
    }
}

fn gate(
    root: &Path,
    workflow: &str,
    e: &serde_json::Value,
    process: &mut dyn Process,
) -> Result<(), String> {
    let item = if workflow == "pr-gate" {
        &e["pull_request"]
    } else {
        &e["issue"]
    };
    let author = item["user"]["login"].as_str().ok_or("Missing author")?;
    if author.ends_with("[bot]") {
        process.diagnostic(&format!("Skipping bot: {author}"));
        return Ok(());
    }
    let mut github = github(root, e, process)?;
    let permission = github::get_permission(&mut github, author).ok().flatten();
    if github::collaborator(permission.as_deref()) {
        github.process.diagnostic(&format!(
            "{author} is a collaborator with {} access",
            permission.as_deref().unwrap_or("")
        ));
        return Ok(());
    }
    let content = github::get_text_file(&mut github, approved_users::APPROVED_FILE)?;
    let users = approved_users::parse_approved_users(
        &content,
        approved_users::LineDiagnostics::Raw,
        github.process,
    );
    if let Some(capability) = users
        .capability(author)
        .filter(|c| *c == "pr" || workflow == "issue-gate")
    {
        github.process.diagnostic(&if workflow == "pr-gate" {
            format!("{author} is approved for PRs")
        } else {
            format!("{author} is approved for {capability}")
        });
        return Ok(());
    }
    if workflow == "issue-gate" {
        issue_gate::close(&mut github, item)?;
    } else {
        github
            .process
            .diagnostic(&format!("{author} is not approved, closing PR"));
        pr_gate::close_pull_request(&mut github, item)?;
    }
    Ok(())
}

fn github<'a>(
    root: &'a Path,
    e: &'a serde_json::Value,
    process: &'a mut dyn Process,
) -> Result<github::Github<'a>, String> {
    Ok(github::Github {
        root,
        repository: e["repository"]["full_name"]
            .as_str()
            .ok_or("Missing repository")?,
        branch: e["repository"]["default_branch"]
            .as_str()
            .ok_or("Missing default branch")?,
        process,
    })
}

/// Executes a workflow from command-line arguments and GitHub environment values.
///
/// # Errors
///
/// Returns a diagnostic for invalid inputs, failed policy operations or outputs.
pub fn execute() -> Result<(), String> {
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
    let outputs = run(
        &Invocation {
            root: &root,
            workflow: &workflow,
            event_name: &event,
            payload: &payload,
            app_slug: slug.as_deref(),
        },
        &mut SystemProcess,
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
