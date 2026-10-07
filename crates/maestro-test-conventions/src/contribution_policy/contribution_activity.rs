use super::approved_users::{APPROVED_FILE, parse_approved_users};
use super::github::{Github, collaborator, get_permission, get_text_file};
use serde_json::{Value, json};

pub(super) fn check(github: &mut Github<'_>, item: &Value) -> Result<(), String> {
    let author = item["user"]["login"].as_str().ok_or("Missing author")?;
    if author.ends_with("[bot]") {
        github
            .process
            .diagnostic(&format!("Skipping bot: {author}"));
        return Ok(());
    }
    match get_text_file(github, APPROVED_FILE) {
        Ok(content) => {
            if parse_approved_users(
                &content,
                super::approved_users::LineDiagnostics::Silent,
                github.process,
            )
            .capability(author)
            .is_some()
            {
                github
                    .process
                    .diagnostic(&format!("{author} is in APPROVED_CONTRIBUTORS, passing"));
                return Ok(());
            }
        }
        Err(error) => github
            .process
            .diagnostic(&format!("Could not read APPROVED_CONTRIBUTORS: {error}")),
    }
    let permission = get_permission(github, author).ok().flatten();
    if collaborator(permission.as_deref()) {
        github.process.diagnostic(&format!(
            "{author} is a collaborator ({}), passing",
            permission.as_deref().unwrap_or("")
        ));
        return Ok(());
    }
    label_activity(github, item, author)
}

fn label_activity(github: &mut Github<'_>, item: &Value, author: &str) -> Result<(), String> {
    let policy: Value = serde_json::from_slice(
        &std::fs::read(github.root.join(".github/repository-policy.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let gate = &policy["activity_gate"];
    let label = gate["label"].as_str().unwrap_or("");
    if label.is_empty() {
        return Ok(());
    }
    let repositories: Vec<_> = gate["repositories"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    for repository in &repositories {
        if has_activity(github, repository, author) {
            github
                .process
                .diagnostic(&format!("{author} has {repository} activity, adding label"));
            let number = item["number"].as_u64().ok_or("Missing issue/PR number")?;
            github.repo_api(
                "POST",
                &format!("issues/{number}/labels"),
                &json!({"labels":[label]}),
            )?;
            return Ok(());
        }
    }
    github.process.diagnostic(&format!(
        "{author} has no {} activity, passing",
        repositories.join(", ")
    ));
    Ok(())
}

pub(super) fn has_activity(github: &mut Github<'_>, repository: &str, author: &str) -> bool {
    match github.api(
        "GET",
        "search/issues",
        &json!({"q":format!("repo:{repository} author:{author}"),"per_page":1}),
    ) {
        Ok(data) if data["total_count"].as_u64().is_some_and(|count| count > 0) => {
            github.process.diagnostic(&format!(
                "{author} has opened {} issues/PRs on {repository}",
                data["total_count"]
            ));
            true
        }
        Ok(_) => false,
        Err(error) => {
            github
                .process
                .diagnostic(&format!("Search failed: {error}"));
            false
        }
    }
}
