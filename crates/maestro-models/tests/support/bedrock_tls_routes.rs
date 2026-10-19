//! Real destination TLS exchanges observed after isolated provider calls finish.

use crate::{TestResult, bedrock, loopback, tls, transport};
use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
use maestro_models::{Context, Model, StreamOptions};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Common invocation and independently recorded observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Owned invocation data.
    common: Common,
    /// Complete native case set.
    rows: Vec<Row>,
}
/// Invocation data shared by every recorded call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Common {
    /// Controlled model.
    model: Model,
    /// Controlled conversation.
    context: Context,
}
/// One complete recorded exchange.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Authored environment and identity.
    input: Input,
    /// Completed peer observations.
    observations: Observations,
    /// Returned stream and callback counts.
    returned: Returned,
}
/// One complete input, serialized for uniqueness checking.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Stable case name.
    id: String,
    /// Owning witness.
    test: String,
    /// Explicit OS strings, with runtime endpoint placeholders.
    env: BTreeMap<String, String>,
}
/// Ordered, closed sets of real peer observations.
#[derive(Default, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Observations {
    /// Destination handshakes, including tunneled destinations.
    origin: Vec<Origin>,
    /// Completed CONNECT phases.
    proxy: Vec<Proxy>,
    /// Decoy admissions.
    decoy: usize,
}
/// One destination handshake and optional decrypted HTTP request.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Origin {
    /// Negotiated destination server name.
    sni: String,
    /// Observable application protocol.
    protocol: String,
    /// Decrypted request line, absent for negotiation-only controls.
    request_line: Option<String>,
}
/// One selected proxy's completed handshake and CONNECT target.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Proxy {
    /// Whether an outer TLS handshake completed before CONNECT.
    secure: bool,
    /// Observed authority from the request line.
    target: String,
}
/// Ordered stream results and independent hook counts.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Returned {
    /// Calls made through supplied hooks.
    counts: Counts,
    /// Ordered event labels, including repetitions.
    events: Vec<String>,
    /// Terminal result projection.
    result: ResultRecord,
}
/// Independent callback invocation counts.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Counts {
    /// Payload hook calls.
    payload: usize,
    /// Response hook calls.
    response: usize,
}
/// Terminal content and stop reason; foreign disconnect diagnostics are not compared.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ResultRecord {
    /// All terminal content, without synthesized labels.
    content: Value,
    /// Terminal stop reason.
    stop_reason: Value,
}

/// Read the corpus and reject duplicate identities or complete inputs before selection.
fn corpus() -> TestResult<Corpus> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/cloud_runtime_witnesses.json"))?;
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    for row in &corpus.rows {
        assert!(ids.insert(&row.input.id), "duplicate case identity");
        assert!(
            inputs.insert(serde_json::to_string(&row.input.env)?),
            "duplicate complete input"
        );
    }
    Ok(corpus)
}

/// Serve the destination on any direct or tunneled byte stream with the same peer routine.
fn destination(mut socket: impl Read + Write, seen: &Mutex<Observations>) -> std::io::Result<()> {
    let config = tls::server_config(&[b"h2", b"http/1.1"]).map_err(std::io::Error::other)?;
    let mut connection = rustls::ServerConnection::new(config).map_err(std::io::Error::other)?;
    while connection.is_handshaking() {
        connection.complete_io(&mut socket)?;
    }
    let origin = Origin {
        sni: connection
            .server_name()
            .ok_or_else(|| std::io::Error::other("missing destination SNI"))?
            .to_owned(),
        protocol: match connection.alpn_protocol() {
            Some(b"http/1.1") => "HTTP/1.1".into(),
            Some(b"h2") => "h2".into(),
            _ => "unknown".into(),
        },
        request_line: None,
    };
    let mut stream = rustls::StreamOwned::new(connection, socket);
    if origin.protocol == "h2" {
        seen.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .origin
            .push(origin);
        return Ok(());
    }
    let body = bedrock::answer("controlled answer")
        .map_err(|error| std::io::Error::other(error.to_string()))?
        .concat();
    loopback::answer_connection(&mut stream, &|request| {
        seen.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .origin
            .push(Origin {
                sni: origin.sni.clone(),
                protocol: origin.protocol.clone(),
                request_line: Some(request.request_line.clone()),
            });
        loopback::Reply {
            status: 200,
            content_type: "application/vnd.amazon.eventstream",
            headers: &[],
            body: body.clone(),
            close: false,
        }
    })?;
    stream.conn.send_close_notify();
    stream.flush()
}

/// Acknowledge a real CONNECT request, then host the destination inside its tunnel.
fn tunnel(
    mut socket: impl Read + Write,
    secure: bool,
    seen: &Mutex<Observations>,
) -> std::io::Result<()> {
    let request = loopback::read_request(&mut socket)?
        .ok_or_else(|| std::io::Error::other("missing CONNECT"))?;
    let parts: Vec<_> = request.request_line.split(' ').collect();
    assert_eq!(parts.len(), 3, "CONNECT request line shape");
    assert_eq!(parts[0], "CONNECT", "selected proxy method");
    assert_eq!(parts[2], "HTTP/1.1", "selected proxy HTTP version");
    seen.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .proxy
        .push(Proxy {
            secure,
            target: parts[1].into(),
        });
    socket.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")?;
    socket.flush()?;
    destination(socket, seen)
}

/// Start the selected proxy, completing outer TLS only for the HTTPS proxy input.
fn proxy(secure: bool, seen: Arc<Mutex<Observations>>) -> TestResult<loopback::Routed> {
    let config = tls::server_config(&[b"http/1.1"])?;
    loopback::start_routed(Arc::new(move |socket| {
        if secure {
            let connection = rustls::ServerConnection::new(Arc::clone(&config))
                .map_err(std::io::Error::other)?;
            tunnel(rustls::StreamOwned::new(connection, socket), true, &seen)
        } else {
            tunnel(socket, false, &seen)
        }
    }))
}

/// Execute the provider in the isolated child with no replacement sender.
fn child(common: Common, row: &Row) -> TestResult {
    let payload = Arc::new(AtomicUsize::new(0));
    let response = Arc::new(AtomicUsize::new(0));
    let p = Arc::clone(&payload);
    let r = Arc::clone(&response);
    let options = BedrockOptions {
        bearer_token: Some("fixture-token".into()),
        common: StreamOptions::<maestro_models::Threaded> {
            on_payload: Some(Arc::new(move |input, _| {
                p.fetch_add(1, Ordering::SeqCst);
                Box::pin(std::future::ready(Ok(input)))
            })),
            on_response: Some(Arc::new(move |_, _| {
                r.fetch_add(1, Ordering::SeqCst);
                Box::pin(std::future::ready(Ok(())))
            })),
            ..StreamOptions::default()
        },
        ..BedrockOptions::default()
    };
    let mut model = common.model;
    model.base_url = std::env::var("MAESTRO_ORIGIN")?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let (events, message) =
                transport::drain(&stream_bedrock(model, common.context, Some(options))).await?;
            check_returned(
                row,
                &events,
                &message,
                &Counts {
                    payload: payload.load(Ordering::SeqCst),
                    response: response.load(Ordering::SeqCst),
                },
            )?;
            Ok(())
        })
}

/// Compare the same drained stream's ordered updates, terminal content and callback effects.
fn check_returned(
    row: &Row,
    events: &[String],
    message: &maestro_models::AssistantMessage,
    counts: &Counts,
) -> TestResult {
    assert_eq!(
        events, row.returned.events,
        "{} returned events",
        row.input.id
    );
    assert_eq!(
        serde_json::to_value(&message.content)?,
        row.returned.result.content,
        "{} content",
        row.input.id
    );
    assert_eq!(
        serde_json::to_value(&message.stop_reason)?,
        row.returned.result.stop_reason,
        "{} stop reason",
        row.input.id
    );
    assert_eq!(
        counts.payload, row.returned.counts.payload,
        "payload hook count"
    );
    assert_eq!(
        counts.response, row.returned.counts.response,
        "response hook count"
    );
    Ok(())
}

/// Run a case, closing all peer owners before inspecting the closed observation set.
fn parent(row: &Row, test: &str) -> TestResult {
    let seen = Arc::new(Mutex::new(Observations::default()));
    let observed = Arc::clone(&seen);
    let direct = Arc::new(AtomicUsize::new(0));
    let admissions = Arc::clone(&direct);
    let origin = loopback::start_routed(Arc::new(move |socket| {
        admissions.fetch_add(1, Ordering::SeqCst);
        destination(socket, &observed)
    }))?;
    let secure = row
        .input
        .env
        .get("HTTPS_PROXY")
        .is_some_and(|value| value == "HTTPS_PROXY_URL");
    let proxy = proxy(secure, Arc::clone(&seen))?;
    let observed = Arc::clone(&seen);
    let decoy = loopback::serve_closed(move || {
        observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .decoy += 1;
    })?;
    let endpoint = origin.url.replace("http://127.0.0.1", "https://localhost");
    let authority = endpoint
        .strip_prefix("https://")
        .ok_or("origin authority")?
        .to_owned();
    let variables = environment(row, &endpoint, &proxy.url, &decoy.url);
    let variables: Vec<_> = variables
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let output = maestro_test_process::TestProcess::current(test)?
        .nocapture()
        .env_clear()
        .envs(variables.iter().copied())
        .env("MAESTRO_CHILD_CASE", &row.input.id)
        .output()?;
    let origin_done = origin.finish();
    let proxy_done = proxy.finish();
    let decoy_done = decoy.finish();
    origin_done?;
    proxy_done?;
    decoy_done?;
    let seen = std::mem::take(
        &mut *seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    check_observations(row, &seen, direct.load(Ordering::SeqCst), &authority);
    assert!(
        output.status.success(),
        "{} child failed:\n{}\n{}",
        row.input.id,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Resolve only authored placeholders, then explicitly supply the isolated process environment.
fn environment(row: &Row, endpoint: &str, proxy: &str, decoy: &str) -> BTreeMap<String, String> {
    let mut variables = row.input.env.clone();
    for value in variables.values_mut() {
        *value = match value.as_str() {
            "HTTP_PROXY_URL" => proxy.to_owned(),
            "HTTPS_PROXY_URL" => proxy.replace("http://127.0.0.1", "https://localhost"),
            "DECOY_URL" => decoy.to_owned(),
            _ => continue,
        };
    }
    variables.extend([
        ("MAESTRO_ORIGIN".into(), endpoint.into()),
        ("AWS_REGION".into(), "us-east-1".into()),
        ("AWS_MAX_ATTEMPTS".into(), "1".into()),
        ("AWS_EC2_METADATA_DISABLED".into(), "true".into()),
        (
            "SSL_CERT_FILE".into(),
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/socks-tls/cert.pem"
            )
            .into(),
        ),
    ]);
    variables
}

/// Check routing before protocols and child results so a removed effect fails at its own observation.
fn check_observations(row: &Row, seen: &Observations, direct: usize, authority: &str) {
    let expected_direct = if row.observations.proxy.is_empty() {
        row.observations.origin.len()
    } else {
        0
    };
    assert_eq!(
        (seen.proxy.len(), direct),
        (row.observations.proxy.len(), expected_direct),
        "{} selected proxy CONNECT and direct destination admissions",
        row.input.id
    );
    assert_eq!(
        seen.origin.len(),
        row.observations.origin.len(),
        "{} destination admissions",
        row.input.id
    );
    for (actual, expected) in seen.origin.iter().zip(&row.observations.origin) {
        assert_eq!(
            actual.protocol, expected.protocol,
            "{} negotiated destination protocol",
            row.input.id
        );
        assert_eq!(
            actual.sni, expected.sni,
            "{} destination name",
            row.input.id
        );
        assert_eq!(
            actual.request_line, expected.request_line,
            "{} decrypted destination request",
            row.input.id
        );
    }
    check_proxy(row, seen, authority);
    assert_eq!(
        seen.decoy, row.observations.decoy,
        "{} decoy admissions",
        row.input.id
    );
}

/// Resolve the recorded authority placeholder against the supplied destination, not a proxy URL.
fn check_proxy(row: &Row, seen: &Observations, authority: &str) {
    for (actual, expected) in seen.proxy.iter().zip(&row.observations.proxy) {
        assert_eq!(
            expected.target, "$ORIGIN_AUTHORITY",
            "recorded authority placeholder"
        );
        assert_eq!(
            actual.target, authority,
            "{} selected CONNECT destination",
            row.input.id
        );
        assert_eq!(
            actual.secure, expected.secure,
            "{} completed proxy TLS phase",
            row.input.id
        );
    }
}

/// Run exactly the recorded rows owned by this witness in fresh child environments.
pub(crate) fn run_cases(test: &str) -> TestResult {
    let corpus = corpus()?;
    if let Ok(id) = std::env::var("MAESTRO_CHILD_CASE") {
        let row = corpus
            .rows
            .iter()
            .find(|row| row.input.id == id && row.input.test == test)
            .ok_or("unknown child case")?;
        return child(corpus.common, row);
    }
    let rows: Vec<_> = corpus
        .rows
        .iter()
        .filter(|row| row.input.test == test)
        .collect();
    assert!(!rows.is_empty(), "no recorded cases selected");
    let mut failed = 0;
    for row in rows {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parent(row, test))) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("{} peer failure: {error}", row.input.id);
                failed += 1;
            }
            Err(_) => failed += 1,
        }
    }
    assert_eq!(failed, 0, "runtime cases failed; see per-case assertions");
    Ok(())
}
