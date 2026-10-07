use super::github::{Github, contributing};
use serde_json::{Value, json};

pub(super) fn close_pull_request(github: &mut Github<'_>, item: &Value) -> Result<(), String> {
    let number = item["number"].as_u64().ok_or("Missing PR number")?;
    let link = contributing(github);
    let message = format!(
        "This PR was auto-closed. Only contributors approved with `lgtm` can open PRs. Open an issue first.\n\nMaintainers review auto-closed issues daily. Issues that do not meet the quality bar in [CONTRIBUTING.md]({link}) will not be reopened or receive a reply.\n\nIf a maintainer replies `lgtmi`, your future issues will stay open. If a maintainer replies `lgtm`, your future issues and PRs will stay open.\n\nSee [CONTRIBUTING.md]({link})."
    );
    github.repo_api(
        "POST",
        &format!("issues/{number}/comments"),
        &json!({"body":message}),
    )?;
    github.repo_api(
        "PATCH",
        &format!("pulls/{number}"),
        &json!({"state":"closed"}),
    )?;
    Ok(())
}
