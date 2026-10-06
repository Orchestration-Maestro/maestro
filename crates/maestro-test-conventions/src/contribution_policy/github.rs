use super::Process;
use serde_json::{Value, json};
use std::path::Path;

pub(super) struct Github<'a> {
    pub root: &'a Path,
    pub repository: &'a str,
    pub branch: &'a str,
    pub process: &'a mut dyn Process,
}

impl Github<'_> {
    pub fn api(&mut self, method: &str, endpoint: &str, data: Value) -> Result<Value, String> {
        let mut args = vec![
            "api".into(),
            "--method".into(),
            method.into(),
            endpoint.into(),
            "--input".into(),
            "-".into(),
        ];
        if method == "GET" {
            for (key, value) in data.as_object().into_iter().flatten() {
                let value = if let Some(value) = value.as_str() {
                    value.to_owned()
                } else {
                    value.to_string()
                };
                args.extend(["--raw-field".into(), format!("{key}={value}")]);
            }
        }
        let input = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
        let output = self.process.output(self.root, "gh", &args, Some(&input))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().into());
        }
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
    }
    pub fn repo_api(&mut self, method: &str, suffix: &str, data: Value) -> Result<Value, String> {
        self.api(method, &format!("repos/{}/{suffix}", self.repository), data)
    }
}

pub(super) fn get_permission(
    github: &mut Github<'_>,
    username: &str,
) -> Result<Option<String>, String> {
    let response = github.repo_api(
        "GET",
        &format!("collaborators/{username}/permission"),
        json!({}),
    )?;
    Ok(response["permission"].as_str().map(str::to_owned))
}

pub(super) fn collaborator(permission: Option<&str>) -> bool {
    matches!(permission, Some("admin" | "maintain" | "write"))
}

pub(super) fn get_text_file(github: &mut Github<'_>, path: &str) -> Result<String, String> {
    use base64::Engine;
    let response = github.repo_api(
        "GET",
        &format!("contents/{path}"),
        json!({"ref":github.branch}),
    )?;
    let content = response["content"]
        .as_str()
        .ok_or_else(|| format!("Expected file content for {path}"))?;
    let mut normalized: String = content
        .chars()
        .take_while(|c| *c != '=')
        .filter_map(|c| match c {
            '-' => Some('+'),
            '_' => Some('/'),
            c if c.is_ascii_alphanumeric() || c == '+' || c == '/' => Some(c),
            _ => None,
        })
        .collect();
    if normalized.len() % 4 == 1 {
        normalized.pop();
    }
    let decoder = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new()
            .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    let bytes = decoder.decode(normalized).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub(super) fn contributing(github: &Github<'_>) -> String {
    format!(
        "https://github.com/{}/blob/{}/CONTRIBUTING.md",
        github.repository, github.branch
    )
}
