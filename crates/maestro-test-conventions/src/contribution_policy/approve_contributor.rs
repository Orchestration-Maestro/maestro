use super::approved_users::{APPROVED_FILE, Entry, parse_approved_users, stringify_approved_users};
use super::github::{Github, collaborator, get_permission};
use super::semantics::command;
use base64::Engine;
use serde_json::{Value, json};

pub(super) fn approve(
    github: &mut Github<'_>,
    e: &Value,
    app_slug: Option<&str>,
) -> Result<Vec<(String, String)>, String> {
    let Some(target) = command(e["comment"]["body"].as_str().unwrap_or("")) else {
        eprintln!("Comment does not match lgtm or lgtmi");
        return Ok(status("skipped", None));
    };
    let commenter = e["comment"]["user"]["login"]
        .as_str()
        .ok_or("Missing commenter")?;
    if !collaborator(get_permission(github, commenter)) {
        eprintln!("Commenter does not have write access");
        return Ok(status("skipped", None));
    }
    let content =
        std::fs::read_to_string(github.root.join(APPROVED_FILE)).map_err(|e| e.to_string())?;
    let mut users = parse_approved_users(&content, true);
    let author = e["issue"]["user"]["login"]
        .as_str()
        .ok_or("Missing author")?;
    if let Some(capability) = users
        .capability(author)
        .filter(|c| *c == "pr" || *c == target)
    {
        eprintln!("{author} is already approved for {capability}");
        github.repo_api(
            "POST",
            &format!("issues/{}/comments", e["issue"]["number"]),
            json!({"body":format!("@{author} is already approved.")}),
        )?;
        return Ok(status("already", Some(capability)));
    }
    let outcome = if let Some(index) = users.users.get(&author.to_lowercase()) {
        if let Entry::User { capability, .. } = &mut users.entries[*index] {
            *capability = target.into();
        }
        "updated"
    } else {
        users.entries.push(Entry::User {
            username: author.into(),
            capability: target.into(),
        });
        "added"
    };
    let _app = app_slug
        .filter(|s| !s.is_empty())
        .ok_or("Missing approval App slug")?;
    let issue = e["issue"]["number"]
        .as_u64()
        .ok_or("Missing issue number")?;
    let comment = e["comment"]["id"].as_u64().ok_or("Missing comment ID")?;
    let branch = format!("chore/approve-contributor-{issue}-{comment}");
    let head = github.repo_api(
        "GET",
        &format!("git/ref/heads/{}", github.branch),
        json!({}),
    )?;
    let sha = head["object"]["sha"]
        .as_str()
        .ok_or("Missing default head SHA")?;
    let checkout = github.process.output(
        github.root,
        "git",
        &["rev-parse".into(), "HEAD".into()],
        None,
    )?;
    if !checkout.status.success() {
        return Err("Could not read trusted checkout HEAD".into());
    }
    let checkout_sha = std::str::from_utf8(&checkout.stdout)
        .map_err(|_| "Invalid trusted checkout HEAD")?
        .trim();
    if checkout_sha != sha {
        return Err("Trusted checkout HEAD does not match default branch SHA".into());
    }
    github.repo_api(
        "POST",
        "git/refs",
        json!({"ref":format!("refs/heads/{branch}"),"sha":sha}),
    )?;
    let signed = github.api("POST", "graphql", json!({
        "query":"mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid } } }",
        "variables":{"input":{"branch":{"repositoryNameWithOwner":github.repository,"branchName":branch},"expectedHeadOid":sha,
          "message":{"headline":format!("chore: approve contributor {author}")},
          "fileChanges":{"additions":[{"path":APPROVED_FILE,"contents":base64::engine::general_purpose::STANDARD.encode(stringify_approved_users(&users.entries))}]}}}
    }))?;
    let signed_sha = signed
        .pointer("/data/createCommitOnBranch/commit/oid")
        .and_then(Value::as_str)
        .ok_or("Missing signed commit SHA")?;
    let marker = json!({"issue":issue,"comment":comment,"author":author,"capability":target});
    let pr = github.repo_api("POST", "pulls", json!({"title":format!("chore: approve contributor {author}"),"head":branch,"base":github.branch,"body":format!("<!-- maestro-approval:{marker} -->")}))?;
    let number = pr["number"].as_u64().ok_or("Missing approval PR number")?;
    let args = [
        "pr",
        "merge",
        &number.to_string(),
        "--repo",
        github.repository,
        "--squash",
        "--auto",
        "--match-head-commit",
        signed_sha,
    ]
    .map(str::to_owned);
    let output = github.process.output(github.root, "gh", &args, None)?;
    if !output.status.success() {
        return Err("Approval enqueue failed".into());
    }
    eprintln!("Set {author} pending capability to {target}");
    Ok(status(outcome, Some(target)))
}

fn status(status: &str, capability: Option<&str>) -> Vec<(String, String)> {
    let mut result = vec![("status".into(), status.into())];
    if let Some(capability) = capability {
        result.push(("capability".into(), capability.into()));
    }
    result
}

pub(super) fn complete(
    github: &mut Github<'_>,
    e: &Value,
    app_slug: Option<&str>,
) -> Result<(), String> {
    let Some(app) = app_slug.filter(|s| !s.is_empty()) else {
        return Err("Missing approval App slug".into());
    };
    let Some(number) = e["pull_request"]["number"].as_u64() else {
        return Ok(());
    };
    let pr = github.repo_api("GET", &format!("pulls/{number}"), json!({}))?;
    if pr["merged"] != true
        || pr["user"]["login"] != format!("{app}[bot]")
        || pr["base"]["ref"] != github.branch
        || pr["base"]["repo"]["full_name"] != github.repository
        || pr["head"]["repo"]["full_name"] != github.repository
        || pr["changed_files"] != 1
    {
        return Ok(());
    }
    let Some(body) = pr["body"].as_str() else {
        return Ok(());
    };
    let Some(marker) = body
        .strip_prefix("<!-- maestro-approval:")
        .and_then(|s| s.strip_suffix(" -->"))
    else {
        return Ok(());
    };
    let Ok(marker) = serde_json::from_str::<Value>(marker) else {
        return Ok(());
    };
    let (Some(issue), Some(comment), Some(author), Some(target)) = (
        marker["issue"].as_u64(),
        marker["comment"].as_u64(),
        marker["author"].as_str(),
        marker["capability"].as_str(),
    ) else {
        return Ok(());
    };
    if !super::approved_users::VALID_CAPABILITIES.contains(&target)
        || pr["head"]["ref"] != format!("chore/approve-contributor-{issue}-{comment}")
    {
        return Ok(());
    }
    let files = github.repo_api(
        "GET",
        &format!("pulls/{number}/files"),
        json!({"per_page":100}),
    )?;
    if !files.as_array().is_some_and(|files| {
        files.len() == 1
            && files[0]["filename"] == APPROVED_FILE
            && files[0]["status"] == "modified"
    }) {
        return Ok(());
    }
    let original = github.repo_api("GET", &format!("issues/{issue}"), json!({}))?;
    let request = github.repo_api("GET", &format!("issues/comments/{comment}"), json!({}))?;
    if original.get("pull_request").is_some()
        || original["number"] != issue
        || original["user"]["login"] != author
        || request["id"] != comment
        || request["issue_url"]
            != format!(
                "https://api.github.com/repos/{}/issues/{issue}",
                github.repository
            )
        || command(request["body"].as_str().unwrap_or("")) != Some(target)
    {
        return Ok(());
    }
    let Some(commenter) = request["user"]["login"].as_str() else {
        return Ok(());
    };
    if !collaborator(get_permission(github, commenter)) {
        return Ok(());
    }
    let content = super::github::get_text_file(github, APPROVED_FILE)?;
    let users = parse_approved_users(&content, true);
    let Some(capability) = users
        .capability(author)
        .filter(|c| *c == target || *c == "pr")
    else {
        return Ok(());
    };
    let guidance = if capability == "issue" {
        format!(
            "@{author} approved for issues. Your future issues will not be auto-closed. PRs still require `lgtm`."
        )
    } else {
        format!(
            "@{author} approved for issues and PRs. Your future issues and PRs will not be auto-closed."
        )
    };
    let body = format!(
        "{guidance}\n\nSee [CONTRIBUTING.md]({}).",
        super::github::contributing(github)
    );
    github.repo_api(
        "POST",
        &format!("issues/{issue}/comments"),
        json!({"body":body}),
    )?;
    Ok(())
}
