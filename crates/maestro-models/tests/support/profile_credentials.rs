//! Profile credential rows: each runs the public stream in a child whose environment and files
//! come from the row, against services on a loopback server, and compares what both sides saw.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
use maestro_models::{Fetch, StreamOptions};
use serde::Deserialize;

use crate::TestResult;
use crate::bedrock;
use crate::chat::block_on;
use crate::child_process;
use crate::profile_files::Workdir;
use crate::profile_services::{Log, RoleRequest, Sent, serve};
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
    pub container: Option<On>,
    pub container_fail: Option<On>,
    pub deny: Option<On>,
    pub disabled: Option<String>,
    pub duration: Option<String>,
    pub env_keys: Option<On>,
    pub env_partial: Option<On>,
    pub env_profile: Option<On>,
    pub env_web: Option<Toggle>,
    pub external_id: Option<String>,
    pub inline_sso: Option<On>,
    pub instance: Option<On>,
    pub keys: Option<On>,
    pub login: Option<On>,
    pub mfa: Option<On>,
    pub omit_sso: Option<String>,
    #[serde(rename = "override")]
    pub credentials_override: Option<On>,
    pub partial: Option<String>,
    pub process: Option<String>,
    pub role: Option<On>,
    pub session_token: Option<On>,
    pub source: Option<String>,
    pub source_profile: Option<String>,
    pub split: Option<On>,
    pub sso: Option<String>,
    pub sso_cache: Option<On>,
    pub web: Option<Toggle>,
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

/// The text a failed row's error must carry, for the failures this crate words itself.
fn cause(id: &str) -> Option<&'static str> {
    Some(match id {
        "role_mfa" => {
            "Profile selected requires multi-factor authentication, but no MFA code callback was provided."
        }
        "sso_missing" | "sso_missing_sso_role_name" => "Missing required SSO field: sso_role_name.",
        "sso_missing_sso_account_id" => "Missing required SSO field: sso_account_id.",
        "sso_missing_sso_start_url" => "Missing required SSO field: sso_start_url.",
        "sso_missing_sso_region" => "Missing required SSO field: sso_region.",
        "sso_session_missing" => "Missing SSO session: controlled.",
        _ => return None,
    })
}

/// Tell the services that the conversation reached Bedrock, with the headers it signed with.
fn report(endpoint: &str, request: &maestro_models::HttpRequest) {
    let mut head =
        String::from("POST /bedrock HTTP/1.1\r\ncontent-length: 0\r\nconnection: close\r\n");
    for name in ["authorization", "x-amz-security-token"] {
        if let Some(value) = request.headers.get(name) {
            let _ = write!(head, "{name}: {value}\r\n");
        }
    }
    head += "\r\n";
    let address = endpoint.trim_start_matches("http://");
    if let Ok(mut connection) = std::net::TcpStream::connect(address)
        && connection.write_all(head.as_bytes()).is_ok()
    {
        connection.read_to_end(&mut Vec::new()).ok();
    }
}

/// The conversation of a row inside the child: stream through a sender that reports to the
/// services, then check how the stream ended.
async fn converse(row: &Row) -> TestResult {
    let endpoint = std::env::var("AWS_EC2_METADATA_SERVICE_ENDPOINT")?;
    let scripted = bedrock::serve(bedrock::answer("answer")?);
    let inner = scripted.fetch.clone();
    let fetch: Fetch = Arc::new(move |request| {
        report(&endpoint, &request);
        inner(request)
    });
    let options = BedrockOptions {
        common: StreamOptions {
            fetch: Some(fetch),
            ..StreamOptions::default()
        },
        region: Some("us-east-1".to_owned()),
        profile: row
            .input
            .env_profile
            .is_none()
            .then(|| "selected".to_owned()),
        ..BedrockOptions::default()
    };
    let stream = stream_bedrock(
        bedrock::model("https://bedrock-runtime.us-east-1.amazonaws.com")?,
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
    child_process::rerun(test, &row.id, &variables)?;
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
    Ok(())
}

/// Run every row a test owns.
pub fn check_rows(test: &str) -> TestResult {
    let all: Vec<Row> = serde_json::from_str(FIXTURE)?;
    let owned: Vec<Row> = all.into_iter().filter(|row| row.test == test).collect();
    assert!(!owned.is_empty(), "{test} owns no rows");
    if let Some(case) = child_process::child_case() {
        let row = owned
            .iter()
            .find(|row| row.id == case)
            .ok_or("unknown row")?;
        return block_on(false, converse(row));
    }
    owned.iter().try_for_each(|row| check_outside(test, row))
}
