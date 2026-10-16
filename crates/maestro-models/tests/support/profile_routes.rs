//! Credential routes observed at native loopback peers after a public conversation completes.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
use maestro_models::{Fetch, ProviderResponse, StreamOptions, default_fetch};
use serde::Deserialize;

use crate::TestResult;
use crate::profile_credentials::{Input, On, Toggle};
use crate::profile_files::Workdir;
use crate::{bedrock, chat, child_process, loopback, profile_services, transport};

/// One service call, retaining order, route and authentication identity.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Seen {
    route: String,
    service: String,
    signature: Option<String>,
    token: Option<String>,
}

/// The conversation response exposed to the caller.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Hook {
    status: f64,
    headers: BTreeMap<String, String>,
}

/// Choices of files, profile selection and transport policy.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    source: String,
    route: String,
    mode: String,
    #[serde(default)]
    denied: bool,
}

/// The oracle's observable result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    seen: Vec<Seen>,
    response_hooks: Vec<Hook>,
    events: Vec<String>,
    error: bool,
    stop_reason: String,
}

/// A scenario owned by one named test.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    test: String,
    input: Scenario,
    expected: Expected,
}

/// Translate scenario choices into the shared profile-file writer's inputs.
fn profile(scenario: &Scenario) -> TestResult<Input> {
    let mut input = Input {
        route: Some(scenario.route.clone()),
        ..Input::default()
    };
    match scenario.source.as_str() {
        "login" => input.login = Some(Toggle::Mode("expired".to_owned())),
        "role" => {
            input.role = Some(On);
            input.base = Some("static".to_owned());
            input.source_profile = Some("base".to_owned());
        }
        "web" => input.web = Some(Toggle::On(On)),
        "env_web" => input.env_web = Some(Toggle::On(On)),
        "sso" => input.sso = Some("valid".to_owned()),
        "oidc" => {
            input.sso = Some("session".to_owned());
            input.session_refresh = Some(On);
        }
        "container" => input.container = Some(On),
        "instance" => input.instance = Some(On),
        "static" => {
            input.keys = Some(On);
            input.static_key = Some("static-key".to_owned());
        }
        "bearer" | "placeholder" => {}
        _ => return Err("unknown credential source".into()),
    }
    if scenario.route == "named" {
        input.role = Some(On);
        input.source = Some(
            match scenario.source.as_str() {
                "container" => "EcsContainer",
                "instance" => "Ec2InstanceMetadata",
                _ => return Err("unknown named metadata source".into()),
            }
            .to_owned(),
        );
    }
    if scenario.denied {
        input.deny = Some(On);
        if scenario.source == "login" {
            input.failure = Some("AccessDeniedException".to_owned());
        }
    }
    Ok(input)
}

/// Classify a request using the same normalized target as the service handler.
fn service(request: &loopback::Received) -> &'static str {
    if request.request_line.starts_with("CONNECT ") {
        return "connect";
    }
    let target = profile_services::target(request);
    let path = target
        .split_once('?')
        .map_or(target.as_str(), |(path, _)| path);
    match path {
        "/v1/token" => "login",
        "/token" => "oidc",
        "/container" => "container",
        "/latest/api/token" => "instance-token",
        "/latest/meta-data/iam/security-credentials/" => "instance-role",
        _ if path.starts_with("/latest/") => "instance",
        _ if path.starts_with("/federation/") => "sso",
        _ if path.ends_with("/converse-stream") => "bedrock",
        _ => "sts",
    }
}

/// Keep the actual signing identity, without timestamps or signature bytes.
fn observation(route: &str, request: &loopback::Received) -> Seen {
    let authorization = request.header("authorization");
    let signature = authorization
        .and_then(bedrock::credential_scope)
        .map(|(key, _)| key.to_owned());
    Seen {
        route: route.to_owned(),
        service: service(request).to_owned(),
        signature,
        token: request.header("x-amz-security-token").map(str::to_owned),
    }
}

/// Serve credentials and conversation frames through the existing joined listener owner.
fn peer(
    input: &Input,
    route: &'static str,
    seen: &Arc<Mutex<Vec<Seen>>>,
    endpoint: &str,
    socks_proxy: bool,
) -> TestResult<loopback::Routed> {
    let body = bedrock::answer("answer")?.concat();
    let (input, seen) = (input.clone(), Arc::clone(seen));
    let authority = url::Url::parse(endpoint)
        .ok()
        .map(|url| url[url::Position::BeforeHost..url::Position::AfterPort].to_owned());
    let handler = move |request: &loopback::Received| {
        let entry = observation(route, request);
        let connect = entry.service == "connect";
        seen.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(entry);
        if connect {
            assert_eq!(
                request.request_line.split(' ').nth(1),
                authority.as_deref(),
                "CONNECT authority"
            );
            return loopback::Reply {
                status: 407,
                content_type: "text/plain",
                headers: &[],
                body: Vec::new(),
                close: false,
            };
        }
        profile_services::answer(
            &input,
            request,
            &mut profile_services::Log::default(),
            &body,
        )
    };
    if socks_proxy {
        loopback::start_routed(Arc::new(move |mut socket| {
            crate::socks::negotiate(&mut socket, false)?;
            loopback::answer_connection(socket, &handler)
        }))
    } else {
        loopback::serve_routed(handler)
    }
}

/// Add destination-specific proxy variables and distinct service/conversation endpoints.
fn environment(row: &Row, work: &Workdir, input: &Input, urls: [&str; 3]) -> Vec<(String, String)> {
    let [credentials, conversation, proxy] = urls;
    let mut vars = work.environment(input, credentials);
    vars.push((
        "MAESTRO_CONVERSATION_URL".to_owned(),
        conversation.to_owned(),
    ));
    vars.push(("AWS_MAX_ATTEMPTS".to_owned(), "1".to_owned()));
    match row.input.mode.as_str() {
        "off" => {}
        "lowercase" => vars.push(("http_proxy".to_owned(), proxy.to_owned())),
        "https" => {
            vars.push(("HTTPS_PROXY".to_owned(), proxy.to_owned()));
            vars.push(("HTTP_PROXY".to_owned(), "http://127.0.0.1:1".to_owned()));
        }
        _ => vars.push(("HTTP_PROXY".to_owned(), proxy.to_owned())),
    }
    let excluded = match row.input.mode.as_str() {
        "bypass" => Some("*".to_owned()),
        "credentials_only" => Some(conversation.trim_start_matches("http://").to_owned()),
        "bedrock_only" => Some(credentials.trim_start_matches("http://").to_owned()),
        _ => None,
    };
    if let Some(excluded) = excluded {
        vars.push(("NO_PROXY".to_owned(), excluded));
    }
    match row.input.source.as_str() {
        "bearer" => vars.push((
            "AWS_BEARER_TOKEN_BEDROCK".to_owned(),
            "controlled-bearer".to_owned(),
        )),
        "placeholder" => vars.push(("AWS_BEDROCK_SKIP_AUTH".to_owned(), "1".to_owned())),
        _ => {}
    }
    vars
}

/// Observe the real destination TLS attempt independently of the proxy.
fn tls_peer(seen: &Arc<Mutex<Vec<Seen>>>) -> TestResult<loopback::Routed> {
    let seen = Arc::clone(seen);
    loopback::serve_closed(move || {
        seen.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Seen {
                route: "direct".to_owned(),
                service: "tls".to_owned(),
                signature: None,
                token: None,
            });
    })
}

/// Compare peers only after child exit and all admitted connections have joined.
fn outside(row: &Row, supplied: bool) -> TestResult {
    let input = profile(&row.input)?;
    let work = Workdir::create(&input)?;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut direct = input.clone();
    if row.input.mode == "mandatory" {
        direct.deny = Some(On);
    }
    let origin = peer(&direct, "direct", &seen, "", false)?;
    let conversation = peer(&input, "direct", &seen, "", false)?;
    let tls = (row.input.mode == "https")
        .then(|| tls_peer(&seen))
        .transpose()?;
    let endpoint = tls.as_ref().map_or_else(
        || origin.url.clone(),
        |tls| tls.url.replacen("http:", "https:", 1),
    );
    let proxy = peer(&input, "proxy", &seen, &endpoint, row.input.mode == "socks")?;
    let proxy_url = if row.input.mode == "socks" {
        proxy.url.replacen("http", "socks5h", 1)
    } else {
        proxy.url.clone()
    };
    let mut vars = environment(
        row,
        &work,
        &input,
        [&endpoint, &conversation.url, &proxy_url],
    );
    let test = if supplied {
        vars.retain(|(name, _)| !name.to_lowercase().contains("proxy"));
        vars.push(("MAESTRO_SUPPLIED_SENDER".to_owned(), "1".to_owned()));
        "credential_services_reuse_the_supplied_sender"
    } else {
        &row.test
    };
    let variables: Vec<_> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let result = child_process::rerun(test, &row.id, &variables);
    origin.finish()?;
    conversation.finish()?;
    proxy.finish()?;
    if let Some(tls) = tls {
        tls.finish()?;
    }
    result?;
    compare_peers(
        row,
        supplied,
        &seen.lock().map_err(|error| error.to_string())?,
    );
    Ok(())
}

/// Assert the socket transcript, allowing only native CONNECT retry multiplicity.
fn compare_peers(row: &Row, supplied: bool, seen: &[Seen]) {
    if row.input.mode == "https" {
        assert!(!seen.is_empty(), "{}: missing CONNECT", row.id);
        assert!(
            seen.iter().all(|entry| row.expected.seen.contains(entry)),
            "{}: HTTPS route {:?}",
            row.id,
            seen.iter()
                .map(|entry| (&entry.route, &entry.service))
                .collect::<Vec<_>>()
        );
    } else {
        assert_eq!(
            seen.iter()
                .map(|entry| (entry.route.as_str(), entry.service.as_str()))
                .collect::<Vec<_>>(),
            row.expected
                .seen
                .iter()
                .map(|entry| (
                    if supplied {
                        "direct"
                    } else {
                        entry.route.as_str()
                    },
                    entry.service.as_str()
                ))
                .collect::<Vec<_>>(),
            "{}: ordered route/service",
            row.id
        );
        assert!(
            seen.iter()
                .map(|entry| (&entry.signature, &entry.token))
                .eq(row
                    .expected
                    .seen
                    .iter()
                    .map(|entry| (&entry.signature, &entry.token))),
            "{}: signature/token",
            row.id
        );
    }
}

/// Wrap the existing default sender only when the caller supplies it explicitly.
fn supplied_fetch(supplied: bool, sent: &Arc<Mutex<Vec<String>>>) -> Option<Fetch> {
    supplied.then(|| {
        let captured = Arc::clone(sent);
        let inner = default_fetch();
        Arc::new(move |request: maestro_models::HttpRequest| {
            let received = loopback::Received {
                request_line: format!("{} {} HTTP/1.1", request.method, request.url),
                headers: request
                    .headers
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
                body: Vec::new(),
                head: Vec::new(),
            };
            captured
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(service(&received).to_owned());
            inner(request)
        }) as Fetch
    })
}

/// Run the same public stream in the child's isolated environment.
async fn converse(row: &Row, supplied: bool) -> TestResult {
    let hooks = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&hooks);
    let sent = Arc::new(Mutex::new(Vec::new()));
    let fetch = supplied_fetch(supplied, &sent);
    let options = BedrockOptions {
        region: Some("us-east-1".to_owned()),
        profile: (row.input.route == "explicit" || row.input.route == "named")
            .then(|| "selected".to_owned()),
        common: StreamOptions::<maestro_models::Threaded> {
            fetch,
            on_response: Some(Arc::new(move |response: ProviderResponse, _| {
                captured
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(Hook {
                        status: response.status,
                        headers: response.headers,
                    });
                Box::pin(std::future::ready(Ok(())))
            })),
            ..StreamOptions::default()
        },
        ..BedrockOptions::default()
    };
    let stream = stream_bedrock(
        bedrock::model(&std::env::var("MAESTRO_CONVERSATION_URL")?)?,
        bedrock::context("question")?,
        Some(options),
    );
    let (events, message) = transport::drain(&stream).await?;
    assert_eq!(events, row.expected.events, "{}: events", row.id);
    assert_eq!(
        serde_json::to_value(message.stop_reason)?,
        row.expected.stop_reason,
        "{}: stop reason",
        row.id
    );
    assert_eq!(
        message.error_message.is_some(),
        row.expected.error,
        "{}: terminal error",
        row.id
    );
    assert!(
        *hooks.lock().map_err(|error| error.to_string())? == row.expected.response_hooks,
        "{}: response hooks",
        row.id
    );
    if supplied {
        compare_sender(row, &sent.lock().map_err(|error| error.to_string())?);
    }
    Ok(())
}

/// Assert ordered callback traffic, with native metadata outside the caller's sender.
fn compare_sender(row: &Row, sent: &[String]) {
    let expected: Vec<_> = row
        .expected
        .seen
        .iter()
        .filter(|entry| !entry.service.starts_with("instance") && entry.service != "container")
        .map(|entry| entry.service.clone())
        .collect();
    assert_eq!(sent, expected, "{}: supplied sender", row.id);
}

/// Parse the corpus without hiding duplicate IDs or inputs behind test filtering.
fn rows() -> TestResult<Vec<Row>> {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../fixtures/cloud_credential_routes.json"))?;
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    for row in &rows {
        assert!(ids.insert(&row.id), "duplicate route id");
        assert!(
            inputs.insert((
                &row.input.source,
                &row.input.route,
                &row.input.mode,
                row.input.denied
            )),
            "duplicate route input"
        );
    }
    Ok(rows)
}

/// Run the corpus subset owned by this test, or seven existing rows through a supplied sender.
pub fn check(test: &str) -> TestResult {
    let supplied = test == "credential_services_reuse_the_supplied_sender";
    let all = rows()?;
    let owned: Vec<_> = all
        .iter()
        .filter(|row| {
            if supplied {
                matches!(
                    row.id.as_str(),
                    "login_explicit_bypass"
                        | "role_explicit_bypass"
                        | "web_explicit_bypass"
                        | "sso_explicit_bypass"
                        | "oidc_explicit_bypass"
                        | "container_explicit"
                        | "instance_explicit"
                )
            } else {
                row.test == test
            }
        })
        .collect();
    assert!(!owned.is_empty(), "test owns no credential route rows");
    if let Some(case) = child_process::child_case() {
        let row = owned
            .into_iter()
            .find(|row| row.id == case)
            .ok_or("unknown route row")?;
        return chat::block_on(false, converse(row, supplied));
    }
    owned.into_iter().try_for_each(|row| outside(row, supplied))
}
