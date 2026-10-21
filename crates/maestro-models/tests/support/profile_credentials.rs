//! Profile credential rows: each runs the public stream in a child whose environment and files
//! come from the row, against services on a loopback server, and compares what both sides saw.

use std::sync::{Arc, Mutex};

use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};

use serde::Deserialize;

use crate::TestResult;
use crate::bedrock;
use crate::profile_files::Workdir;
use crate::profile_services::{Log, RoleRequest, Sent, SigninRequest, SsoRequest, serve};
use crate::transport;

const FIXTURE: &str = include_str!("../fixtures/cloud_profile_credentials.json");

/// A setting that a row only ever turns on.
#[derive(Clone, Deserialize)]
#[serde(try_from = "bool")]
pub struct On;

impl TryFrom<bool> for On {
    type Error = &'static str;

    fn try_from(on: bool) -> Result<Self, Self::Error> {
        on.then_some(Self)
            .ok_or("a setting is either true or absent")
    }
}

/// A setting that is either on or names a variant.
#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum Toggle {
    /// Plain on.
    On(On),
    /// A named variant.
    Mode(String),
}

impl Toggle {
    /// The variant name, absent for plain on.
    pub fn mode(&self) -> Option<&str> {
        match self {
            Self::On(_) => None,
            Self::Mode(name) => Some(name),
        }
    }
}

/// What a row sets up.
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub absent: Option<On>,
    pub base: Option<String>,
    pub cache_override: Option<On>,
    pub container: Option<On>,
    pub container_fail: Option<On>,
    pub deny: Option<On>,
    pub disabled: Option<String>,
    pub duration: Option<String>,
    pub env_keys: Option<On>,
    pub env_partial: Option<On>,
    pub env_web: Option<Toggle>,
    pub error_code: Option<String>,
    pub external_id: Option<String>,
    pub failure: Option<String>,
    pub inline_sso: Option<On>,
    pub instance: Option<On>,
    pub keys: Option<On>,
    pub login: Option<Toggle>,
    pub message: Option<String>,
    pub mfa: Option<On>,
    pub omit_sso: Option<String>,
    #[serde(rename = "override")]
    pub credentials_override: Option<On>,
    pub partial: Option<String>,
    pub process: Option<String>,
    pub read_only: Option<On>,
    pub role: Option<On>,
    pub route: Option<String>,
    pub session_decoy: Option<On>,
    pub session_omit: Option<String>,
    pub session_refresh: Option<On>,
    pub session_token: Option<On>,
    pub source: Option<String>,
    pub source_profile: Option<String>,
    pub split: Option<On>,
    pub sso: Option<String>,
    pub sso_cache: Option<On>,
    pub static_key: Option<String>,
    pub web: Option<Toggle>,
}

/// How a row's login token cache ends.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LoginCache {
    /// The cache keeps the bytes the row wrote.
    Unchanged,
    /// The cache holds the refreshed token.
    Renewed,
}

/// What a row expects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    authorization: Option<String>,
    security_token: Option<String>,
    process_calls: usize,
    sequence: Vec<String>,
    roles: Vec<RoleRequest>,
    sso: Option<Vec<SsoRequest>>,
    signin: Option<Vec<SigninRequest>>,
    login_cache: Option<LoginCache>,
    events: Vec<String>,
    error: Option<bool>,
    stop_reason: String,
}

/// One row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    test: String,
    input: Input,
    expected: Expected,
}

/// The text a failed row's error must carry: this crate's own wording, or the stable cause the
/// native provider reports for a controlled failure.
fn cause(id: &str) -> Option<&'static str> {
    Some(match id {
        "role_mfa" => {
            "Profile selected requires multi-factor authentication, but no MFA code callback was provided."
        }
        "sso_missing" | "sso_missing_sso_role_name" => "Missing required SSO field: sso_role_name.",
        "sso_missing_sso_account_id" => "Missing required SSO field: sso_account_id.",
        "sso_missing_sso_start_url" | "sso_session_without_start_url" => {
            "Missing required SSO field: sso_start_url."
        }
        "sso_missing_sso_region" | "sso_session_without_region" => {
            "Missing required SSO field: sso_region."
        }
        "sso_session_missing" => "Missing SSO session: controlled.",
        "web_sts_denied" | "role_denied" => "AccessDenied",
        "sso_denied" => "controlled SSO denial",
        "denied_token_expired" | "role_login_denied" => {
            "Your session has expired. Please reauthenticate."
        }
        "denied_user_credentials_changed" => {
            "Unable to refresh credentials because of a change in your password. Please reauthenticate with your new password."
        }
        "denied_insufficient_permissions" => {
            "Unable to refresh credentials due to insufficient permissions. You may be missing permission for the 'CreateOAuth2Token' action."
        }
        "sso_expired" | "sso_refreshable_cache" => "controlled SSO expiry",
        "web_missing_file" | "env_web_missing_file" | "sso_token_missing" => {
            "No such file or directory (os error 2)"
        }
        _ => return None,
    })
}

/// Run the conversation against the real loopback service and check its terminal events.
async fn converse(row: &Row) -> TestResult {
    let endpoint = std::env::var("AWS_EC2_METADATA_SERVICE_ENDPOINT")?;
    let options = BedrockOptions {
        region: Some("us-east-1".to_owned()),
        profile: (row.input.route.as_deref().unwrap_or("explicit") == "explicit")
            .then(|| "selected".to_owned()),
        ..BedrockOptions::default()
    };
    let stream = stream_bedrock(
        bedrock::model(&endpoint)?,
        bedrock::context("question")?,
        Some(options),
    );
    let (events, message) = transport::drain(&stream).await?;
    let id = &row.id;
    assert_eq!(events, row.expected.events, "{id}: events");
    let stop = serde_json::to_value(message.stop_reason)?;
    assert_eq!(stop, row.expected.stop_reason, "{id}: stop reason");
    assert_eq!(
        message.error_message.is_some(),
        row.expected.error == Some(true),
        "{id}: error"
    );
    if let (Some(text), Some(expected)) = (&message.error_message, cause(id)) {
        assert!(text.contains(expected), "{id}: cause {text}");
    }
    Ok(())
}

/// Adjacent repeats of the container request are retries the SDK owns.
fn coalesced(sequence: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for entry in sequence {
        if !(entry == "container" && kept.last() == Some(entry)) {
            kept.push(entry.clone());
        }
    }
    kept
}

/// Run a row's child and compare what the services and the process counter saw.
fn check_outside(test: &str, row: &Row) -> TestResult {
    let work = Workdir::create(&row.input)?;
    let log = Arc::new(Mutex::new(Log::default()));
    let services = serve(&row.input, &log)?;
    let environment = work.environment(&row.input, &services.url);
    let variables: Vec<(&str, &str)> = environment
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    {
        let output = maestro_test_process::TestProcess::current(test)?
            .nocapture()
            .env_clear()
            .envs(variables.iter().copied())
            .case(&row.id)
            .output()?;
        assert!(output.status.success(), "{output:?}");
    };
    services.finish()?;
    let log = log.lock().map_err(|error| error.to_string())?;
    let (id, expected) = (&row.id, &row.expected);
    assert_eq!(
        work.process_calls()?,
        expected.process_calls,
        "{id}: process runs"
    );
    assert_eq!(
        coalesced(&log.sequence),
        expected.sequence,
        "{id}: sequence"
    );
    assert_eq!(log.roles, expected.roles, "{id}: role requests");
    if let Some(sso) = &expected.sso {
        assert_eq!(&log.sso, sso, "{id}: sso requests");
    }
    if let Some(signin) = &expected.signin {
        assert_eq!(&log.signin, signin, "{id}: sign-in requests");
    }
    if let Some(cache) = expected.login_cache {
        assert_eq!(
            work.login_cache_effect(&row.input)?,
            cache,
            "{id}: login cache"
        );
    }
    let sent = log.sent.as_ref();
    let want = (
        expected.authorization.as_ref(),
        expected.security_token.as_ref(),
    );
    let got = (
        sent.and_then(|sent: &Sent| sent.authorization.as_ref()),
        sent.and_then(|sent: &Sent| sent.security_token.as_ref()),
    );
    assert_eq!(got, want, "{id}: signature");
    if input_has_cache(&row.input) {
        assert!(work.sso_cache_unchanged(&row.input)?, "{id}: token cache");
    }
    Ok(())
}

/// Whether the row writes an SSO token cache file.
pub fn input_has_cache(input: &Input) -> bool {
    input.sso_cache.is_some()
        || matches!(
            input.sso.as_deref(),
            Some("valid" | "session" | "expired" | "refreshable")
        )
}

/// Parse rows, rejecting a repeated id before any filtering can hide it.
fn parse_rows(json: &str) -> TestResult<Vec<Row>> {
    let rows: Vec<Row> = serde_json::from_str(json)?;
    let mut seen = std::collections::HashSet::new();
    for row in &rows {
        if !seen.insert(row.id.as_str()) {
            return Err(format!("duplicate row id {}", row.id).into());
        }
    }
    Ok(rows)
}

/// Run every row a test owns.
pub fn check_rows(test: &str) -> TestResult {
    let all = parse_rows(FIXTURE)?;
    let owned: Vec<Row> = all.into_iter().filter(|row| row.test == test).collect();
    assert!(!owned.is_empty(), "{test} owns no rows");
    let case = maestro_test_process::selection();
    if case.is_child() {
        let row = owned
            .iter()
            .find(|row| case.runs_case(&row.id))
            .ok_or("unknown row")?;
        return crate::runtime::run(converse(row));
    }
    owned.iter().try_for_each(|row| check_outside(test, row))
}

/// Reject a fixture whose ids repeat.
pub fn duplicate_ids_are_rejected() -> TestResult {
    let row = r#"{"id":"same","test":"t","input":{},"expected":{"processCalls":0,"sequence":[],"roles":[],"events":[],"stopReason":"stop"}}"#;
    let error = parse_rows(&format!("[{row},{row}]"))
        .err()
        .ok_or("accepted")?;
    assert_eq!(error.to_string(), "duplicate row id same");
    Ok(())
}
