use super::github::{Github, contributing};
use serde_json::{Value, json};
const ISSUE_GATE_MESSAGE_MODE: &str = "refactor";

pub(super) fn build_normal_gate_message(
    github: &Github<'_>,
    policy: &Value,
    weekend: bool,
) -> String {
    let link = contributing(github);
    let extra = if weekend {
        policy["issue_gate"]["weekend_message"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| format!("\n{s}"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    format!(
        "This issue was auto-closed. All issues from new contributors are auto-closed by default.{extra}\n\nMaintainers review auto-closed issues daily and reopen worthwhile ones. Issues that do not meet the quality bar in [CONTRIBUTING.md]({link}) will not be reopened or receive a reply.\n\nIf a maintainer replies `lgtmi` on one of your issues, your future issues will stay open. If a maintainer replies `lgtm`, your future issues and PRs will stay open.\n\nSee [CONTRIBUTING.md]({link})."
    )
}

pub(super) fn close(github: &mut Github<'_>, item: &Value) -> Result<(), String> {
    let number = item["number"].as_u64().ok_or("Missing issue number")?;
    let policy: Value = serde_json::from_slice(
        &std::fs::read(github.root.join(".github/repository-policy.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let weekend =
        super::semantics::weekday(item["created_at"].as_str().unwrap_or("")).is_some_and(|day| {
            policy["issue_gate"]["weekend_days"]
                .as_array()
                .is_some_and(|days| days.iter().any(|v| v.as_u64() == Some(u64::from(day))))
        });
    let refactor = policy["issue_gate"]["message_mode"] == ISSUE_GATE_MESSAGE_MODE;
    let message = if refactor {
        build_refactor_gate_message(github, &policy)
    } else {
        build_normal_gate_message(github, &policy, weekend)
    };
    github.repo_api(
        "POST",
        &format!("issues/{number}/comments"),
        &json!({"body":message}),
    )?;
    let mut labels = if weekend {
        policy["issue_gate"]["weekend_labels"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        vec![]
    };
    if refactor {
        labels.extend(
            policy["issue_gate"]["refactor_labels"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        );
    }
    if !labels.is_empty() {
        github.repo_api(
            "POST",
            &format!("issues/{number}/labels"),
            &json!({"labels":labels}),
        )?;
    }
    github.repo_api(
        "PATCH",
        &format!("issues/{number}"),
        &json!({"state":"closed"}),
    )?;
    Ok(())
}

pub(super) fn build_refactor_gate_message(github: &Github<'_>, policy: &Value) -> String {
    let gate = &policy["issue_gate"];
    let until = gate["refactor_until"].as_str().unwrap_or("");
    let mut paragraphs = vec![if until.is_empty() {
        "This issue was auto-closed.".into()
    } else {
        format!(
            "This issue was auto-closed. All issues will be closed until {until} because the project is undergoing a large refactor."
        )
    }];
    if let Some(branch) = gate["refactor_branch"].as_str().filter(|s| !s.is_empty()) {
        paragraphs.push(format!(
            "See the `{branch}` branch: https://github.com/{}/tree/{branch}",
            github.repository
        ));
    }
    if let Some(reason) = gate["refactor_reason"].as_str().filter(|s| !s.is_empty()) {
        paragraphs.push(reason.into());
    }
    if let Some(help) = policy["help_url"].as_str().filter(|s| !s.is_empty()) {
        paragraphs.push(format!("In case of emergency, ask here: {help}"));
    }
    paragraphs.join("\n\n")
}
