mod approve_contributor;
mod approved_users;
mod contribution_activity;
mod github;
mod issue_gate;
mod pr_gate;
mod semantics;
use std::path::Path;
use std::process::Output;

pub(crate) trait Process {
    fn output(
        &mut self,
        cwd: &Path,
        program: &str,
        args: &[String],
        input: Option<&[u8]>,
    ) -> Result<Output, String>;
}

pub(crate) fn run(
    root: &Path,
    workflow: &str,
    event_name: &str,
    payload: &str,
    app_slug: Option<&str>,
    process: &mut dyn Process,
) -> Result<Vec<(String, String)>, String> {
    let e: serde_json::Value = serde_json::from_str(payload).map_err(|e| e.to_string())?;
    if workflow == "approve-contributor"
        && event_name == "issue_comment"
        && e["action"] == "created"
        && e["issue"].get("pull_request").is_none()
    {
        if semantics::command(e["comment"]["body"].as_str().unwrap_or("")).is_none() {
            eprintln!("Comment does not match lgtm or lgtmi");
            return Ok(vec![("status".into(), "skipped".into())]);
        }
        let mut github = github::Github {
            root,
            repository: e["repository"]["full_name"]
                .as_str()
                .ok_or("Missing repository")?,
            branch: e["repository"]["default_branch"]
                .as_str()
                .ok_or("Missing default branch")?,
            process,
        };
        return approve_contributor::approve(&mut github, &e, app_slug);
    }
    if e["action"] == "opened"
        && ((workflow == "issue-gate" && event_name == "issues")
            || (workflow == "pr-gate" && event_name == "pull_request_target"))
    {
        let item = if workflow == "pr-gate" {
            &e["pull_request"]
        } else {
            &e["issue"]
        };
        let author = item["user"]["login"].as_str().ok_or("Missing author")?;
        if author.ends_with("[bot]") {
            eprintln!("Skipping bot: {author}");
            return Ok(vec![]);
        }
        let mut github = github::Github {
            root,
            repository: e["repository"]["full_name"]
                .as_str()
                .ok_or("Missing repository")?,
            branch: e["repository"]["default_branch"]
                .as_str()
                .ok_or("Missing default branch")?,
            process,
        };
        if github::collaborator(github::get_permission(&mut github, author)) {
            eprintln!("{author} is a collaborator");
            return Ok(vec![]);
        }
        let content = github::get_text_file(&mut github, approved_users::APPROVED_FILE)?;
        let users = approved_users::parse_approved_users(&content, true);
        if users
            .capability(author)
            .is_some_and(|c| c == "pr" || workflow == "issue-gate")
        {
            eprintln!("{author} is approved");
            return Ok(vec![]);
        }
        if workflow == "issue-gate" {
            issue_gate::close(&mut github, item)?;
        } else {
            pr_gate::close_pull_request(&mut github, item)?;
        }
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
        let mut github = github::Github {
            root,
            repository: e["repository"]["full_name"]
                .as_str()
                .ok_or("Missing repository")?,
            branch: e["repository"]["default_branch"]
                .as_str()
                .ok_or("Missing default branch")?,
            process,
        };
        contribution_activity::check(&mut github, item)?;
    }
    if workflow == "approve-contributor"
        && event_name == "pull_request_target"
        && e["action"] == "closed"
    {
        let mut github = github::Github {
            root,
            repository: e["repository"]["full_name"]
                .as_str()
                .ok_or("Missing repository")?,
            branch: e["repository"]["default_branch"]
                .as_str()
                .ok_or("Missing default branch")?,
            process,
        };
        approve_contributor::complete(&mut github, &e, app_slug)?;
    }
    Ok(vec![])
}

pub(crate) struct SystemProcess;

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
