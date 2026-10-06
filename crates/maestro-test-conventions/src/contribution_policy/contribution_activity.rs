use super::approved_users::{APPROVED_FILE, parse_approved_users};
use super::github::{Github, collaborator, get_permission, get_text_file};
use serde_json::{Value, json};

pub(super) fn check(github: &mut Github<'_>, item: &Value) -> Result<(), String> {
    let author = item["user"]["login"].as_str().ok_or("Missing author")?;
    if author.ends_with("[bot]") {
        eprintln!("Skipping bot: {author}");
        return Ok(());
    }
    match get_text_file(github, APPROVED_FILE) {
        Ok(content) => {
            if parse_approved_users(&content, false)
                .capability(author)
                .is_some()
            {
                eprintln!("{author} is in APPROVED_CONTRIBUTORS, passing");
                return Ok(());
            }
        }
        Err(_) => eprintln!("Could not read APPROVED_CONTRIBUTORS"),
    }
    if collaborator(get_permission(github, author)) {
        eprintln!("{author} is a collaborator, passing");
        return Ok(());
    }
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
    for repository in gate["repositories"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if has_activity(github, repository, author) {
            let number = item["number"].as_u64().ok_or("Missing issue/PR number")?;
            github.repo_api(
                "POST",
                &format!("issues/{number}/labels"),
                json!({"labels":[label]}),
            )?;
            return Ok(());
        }
    }
    eprintln!("{author} has no configured activity, passing");
    Ok(())
}

pub(super) fn has_activity(github: &mut Github<'_>, repository: &str, author: &str) -> bool {
    match github.api(
        "GET",
        "search/issues",
        json!({"q":format!("repo:{repository} author:{author}"),"per_page":1}),
    ) {
        Ok(data) if data["total_count"].as_u64().is_some_and(|count| count > 0) => {
            eprintln!("{author} has activity on {repository}");
            true
        }
        Ok(_) => false,
        Err(_) => {
            eprintln!("Search failed");
            false
        }
    }
}
