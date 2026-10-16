//! Measure browser prompt descriptions through the provider's token-counting endpoint.

use futures_util::TryStreamExt;
use maestro_app::browser::tools::prompts::{
    ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO, ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW,
    ATTACHMENTS_RUNTIME_DESCRIPTION, artifacts_tool_description, javascript_repl_tool_description,
};
use maestro_app::{Fetch, HttpRequest, LocaleNumberFormat, decode_utf8};
use serde::Deserialize;
use std::{error::Error, io::Write, process::ExitCode};

/// Successful provider response; unrelated fields are ignored.
#[derive(Deserialize)]
struct TokenCountResponse {
    /// Number returned by the provider, without local estimation.
    input_tokens: f64,
}

/// Sends one prompt and collects the complete response before decoding its numeric count.
async fn count_tokens(text: &str, api_key: &str, fetch: &Fetch) -> Result<f64, Box<dyn Error>> {
    let response = fetch(HttpRequest {
        method: "POST".into(),
        url: "https://api.anthropic.com/v1/messages/count_tokens".into(),
        headers: [
            ("content-type".into(), "application/json".into()),
            ("x-api-key".into(), api_key.into()),
            ("anthropic-version".into(), "2023-06-01".into()),
        ]
        .into(),
        body: serde_json::to_vec(&serde_json::json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": text}],
        }))?,
        signal: None,
    })
    .await?;
    let bytes = response.body.try_concat().await?;
    let text = decode_utf8(&bytes);
    if !(200..300).contains(&response.status) {
        return Err(std::io::Error::other(format!("API error: {} {text}", response.status)).into());
    }
    let count: TokenCountResponse = maestro_json::object::from_str(&text)?;
    Ok(count.input_tokens)
}

/// Reports five descriptions sequentially, retaining successes when individual requests fail.
async fn run(
    api_key: &str,
    fetch: &Fetch,
    formatter: &LocaleNumberFormat,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    writeln!(stdout, "Counting tokens in prompts...\n")?;
    let repl = javascript_repl_tool_description(&[]);
    let artifacts = artifacts_tool_description(&[]);
    let prompts = [
        (
            "ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW",
            ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW,
        ),
        (
            "ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO",
            ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO,
        ),
        (
            "ATTACHMENTS_RUNTIME_DESCRIPTION",
            ATTACHMENTS_RUNTIME_DESCRIPTION,
        ),
        (
            "JAVASCRIPT_REPL_TOOL_DESCRIPTION (without runtime providers)",
            repl.as_str(),
        ),
        (
            "ARTIFACTS_TOOL_DESCRIPTION (without runtime providers)",
            artifacts.as_str(),
        ),
    ];
    let mut total = 0.0;
    for (name, text) in prompts {
        match count_tokens(text, api_key, fetch).await {
            Ok(tokens) => {
                total += tokens;
                writeln!(stdout, "{name}: {} tokens", formatter.format(tokens)?)?;
            }
            Err(error) => writeln!(stderr, "Error counting tokens for {name}: {error}")?,
        }
    }
    writeln!(stdout, "\nTotal: {} tokens", formatter.format(total)?)?;
    Ok(())
}

/// Supplies native environment, HTTP, runtime and output owners to the report.
fn main() -> Result<ExitCode, Box<dyn Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        eprintln!("Error: ANTHROPIC_API_KEY environment variable not set");
        return Ok(ExitCode::FAILURE);
    }
    let formatter = LocaleNumberFormat::new(&|key| std::env::var(key).ok())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run(
        &api_key,
        &maestro_app::default_fetch(),
        &formatter,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    ))?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use maestro_app::{FetchError, HttpResponse};
    use serde::Deserialize;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// Recorded report input and expected observations.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        /// Input.
        input: Input,
        /// Output.
        output: Output,
    }
    /// Explicit key, locale and controlled responses.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Input {
        /// Scenario selector.
        name: String,
        /// Explicit environment-key observation.
        api_key: Option<String>,
        /// Locale supplied to the shared formatter.
        locale: String,
        /// Replies.
        replies: Vec<Reply>,
        /// Expected failures.
        expected_failures: usize,
    }
    /// One controlled transport or response outcome.
    #[derive(Clone, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Reply {
        /// Status.
        status: Option<u16>,
        /// Body.
        body: Option<String>,
        /// Body error.
        body_error: Option<String>,
        /// Transport error.
        transport_error: Option<String>,
    }
    /// Independently recorded report and optional protocol transcript.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Output {
        /// Stdout.
        stdout: Vec<String>,
        /// Stderr.
        stderr: Vec<Vec<String>>,
        /// Exit.
        exit: i32,
        /// Requests.
        requests: Option<Vec<Request>>,
        /// Phases.
        phases: Option<Vec<String>>,
    }
    /// Complete authored request envelope.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        /// Url.
        url: String,
        /// Method.
        method: String,
        /// Headers.
        headers: indexmap::IndexMap<String, String>,
        /// Body.
        body: String,
    }

    /// Loads one strict recorded scenario.
    fn case(name: &str) -> Case {
        serde_json::from_str::<Vec<Case>>(include_str!("../tests/fixtures/prompt_measurement.json"))
            .unwrap()
            .into_iter()
            .find(|case| case.input.name == name)
            .unwrap()
    }

    /// Produces a controlled header, body or transport outcome.
    fn response(reply: Reply) -> Result<HttpResponse, FetchError> {
        if let Some(message) = reply.transport_error {
            return Err(FetchError::Connection(
                maestro_app::DiagnosticErrorInfo::new(None, message),
            ));
        }
        let chunk = reply.body_error.map_or_else(
            || Ok(reply.body.unwrap().into_bytes()),
            |message| {
                Err(FetchError::Connection(
                    maestro_app::DiagnosticErrorInfo::new(None, message),
                ))
            },
        );
        Ok(HttpResponse {
            status: reply.status.unwrap(),
            status_text: String::new(),
            headers: std::collections::BTreeMap::default(),
            body: Box::pin(stream::iter([chunk])),
        })
    }

    /// Observed request envelope and body-completion order.
    #[derive(Default)]
    struct Transcript {
        /// Requests.
        requests: Vec<HttpRequest>,
        /// Phases.
        phases: Vec<String>,
    }

    /// Compares protocol fields without exposing authentication data on failure.
    fn assert_request(actual: &HttpRequest, expected: &Request) {
        assert_eq!(actual.url, expected.url);
        assert_eq!(actual.method, expected.method);
        for (key, value) in &expected.headers {
            let matches = actual.headers.get(&key.to_ascii_lowercase()) == Some(value);
            assert!(matches);
        }
        assert_eq!(actual.headers.len(), expected.headers.len());
        assert_eq!(actual.body, expected.body.as_bytes());
        assert!(actual.signal.is_none());
    }

    /// Records when the controlled body is first consumed and completely exhausted.
    fn observed_response(
        reply: Reply,
        phases: Arc<Mutex<Transcript>>,
        index: usize,
    ) -> Result<HttpResponse, FetchError> {
        let mut response = response(reply)?;
        let mut body = response.body;
        response.body = Box::pin(stream::poll_fn(move |cx| {
            let result = body.as_mut().poll_next(cx);
            let std::task::Poll::Ready(chunk) = &result else {
                return result;
            };
            let phase = chunk.as_ref().map_or("settled", |_| "json");
            phases
                .lock()
                .unwrap()
                .phases
                .push(format!("{phase}:{index}"));
            result
        }));
        Ok(response)
    }

    /// Consumes one reply per request, retaining the original key and ordered envelope.
    fn recorded_fetch(
        pending: Arc<Mutex<VecDeque<Reply>>>,
        observed: Arc<Mutex<Transcript>>,
        key: String,
    ) -> Fetch {
        Arc::new(move |request| {
            let matches = request.headers["x-api-key"] == key;
            assert!(matches);
            let index = {
                let mut observed = observed.lock().unwrap();
                let index = observed.requests.len();
                observed.requests.push(request);
                observed.phases.push(format!("request:{index}"));
                index
            };
            let reply = pending.lock().unwrap().pop_front().unwrap();
            Box::pin(futures_util::future::ready(observed_response(
                reply,
                observed.clone(),
                index,
            )))
        })
    }

    /// Replays one independently recorded report through the supplied HTTP boundary.
    fn replay(name: &str) {
        let case = case(name);
        let replies = Arc::new(Mutex::new(VecDeque::from(case.input.replies.clone())));
        let transcript = Arc::new(Mutex::new(Transcript::default()));
        let fetch = recorded_fetch(
            replies.clone(),
            transcript.clone(),
            case.input.api_key.clone().unwrap(),
        );
        let formatter = LocaleNumberFormat::new(&|_| Some(case.input.locale.clone())).unwrap();
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run(
                case.input.api_key.as_deref().unwrap(),
                &fetch,
                &formatter,
                &mut stdout,
                &mut stderr,
            ))
            .unwrap();
        assert_report(&case, &stdout, &stderr);
        assert!(replies.lock().unwrap().is_empty());
        let observed = transcript.lock().unwrap();
        assert_eq!(observed.requests.len(), 5);
        if let Some(requests) = case.output.requests {
            assert_eq!(requests.len(), observed.requests.len());
            for (actual, expected) in observed.requests.iter().zip(requests) {
                assert_request(actual, &expected);
            }
        }
        if let Some(phases) = case.output.phases {
            assert_eq!(observed.phases, phases);
        }
    }

    /// Checks full output, ordered diagnostics and complete failing body contents.
    fn assert_report(case: &Case, stdout: &[u8], stderr: &[u8]) {
        assert_eq!(stdout, case.output.stdout.concat().as_bytes());
        let diagnostics = std::str::from_utf8(stderr).unwrap();
        assert_eq!(
            diagnostics.matches("Error counting tokens for ").count(),
            case.input.expected_failures
        );
        let mut remaining = diagnostics;
        for prefix in &case.output.stderr {
            assert_eq!(prefix.len(), 1);
            let offset = remaining.find(&prefix[0]).unwrap();
            remaining = &remaining[offset + prefix[0].len()..];
        }
        for reply in &case.input.replies {
            if let Some(status) = reply.status.filter(|status| !(200..300).contains(status)) {
                assert!(diagnostics.contains(&format!(
                    "API error: {status} {}",
                    reply.body.as_ref().unwrap()
                )));
            }
            if let Some(message) = reply.transport_error.as_ref().or(reply.body_error.as_ref()) {
                assert!(diagnostics.contains(message));
            }
        }
        assert_eq!(case.output.exit, 0);
    }

    #[test]
    fn token_measurement_reports_grouped_counts_and_total() {
        replay("grouped_counts");
        replay("numeric_format_boundaries");
    }
    /// Invokes the real entry point with isolated absent and empty key observations.
    #[test]
    fn token_measurement_requires_an_environment_key() {
        if std::env::var_os("MAESTRO_COUNT_ENTRY_CHILD").is_some() {
            std::process::exit(i32::from(main().unwrap() == ExitCode::FAILURE));
        }
        for name in ["missing_key", "empty_key"] {
            let case = case(name);
            assert!(case.input.replies.is_empty());
            assert_eq!(case.input.expected_failures, 0);
            assert!(case.output.stdout.is_empty());
            assert!(case.output.requests.is_none());
            assert!(case.output.phases.is_none());
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "tests::token_measurement_requires_an_environment_key",
                    "--nocapture",
                ])
                .env("MAESTRO_COUNT_ENTRY_CHILD", "1")
                .env("LC_ALL", &case.input.locale)
                .env("LANG", &case.input.locale)
                .env("LC_MESSAGES", &case.input.locale)
                .env_remove("ANTHROPIC_API_KEY");
            if let Some(key) = case.input.api_key {
                command.env("ANTHROPIC_API_KEY", key);
            }
            let output = command.output().unwrap();
            assert_eq!(output.status.code(), Some(case.output.exit));
            assert_eq!(
                String::from_utf8(output.stderr).unwrap(),
                case.output.stderr[0][0].clone() + "\n"
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                "\nrunning 1 test\n"
            );
        }
    }

    #[test]
    fn token_measurement_accepts_nonempty_environment_values() {
        replay("nonempty_zero_key");
        replay("nonempty_whitespace_key");
    }

    #[test]
    fn token_measurement_uses_the_supplied_locale() {
        replay("german_counts");
    }

    #[test]
    fn token_measurement_continues_after_http_and_transport_errors() {
        replay("status_boundaries");
        replay("transport_failure");
    }

    #[test]
    fn token_measurement_continues_after_json_and_body_errors() {
        replay("malformed_and_body_failures");
    }

    #[test]
    fn token_measurement_keeps_zero_total_when_all_requests_fail() {
        replay("all_failed");
    }

    /// Rejects out-of-contract successful responses without contaminating later counts.
    #[test]
    fn token_measurement_rejects_invalid_count_records_before_summing() {
        for body in [
            "{}",
            r#"{"input_tokens":null}"#,
            r#"{"input_tokens":"7"}"#,
            r#"{"input_tokens":false}"#,
            r#"{"input_tokens":{}}"#,
            r#"{"input_tokens":[]}"#,
            "null",
            "[]",
        ] {
            let replies = Arc::new(Mutex::new(VecDeque::from([
                body.to_owned(),
                r#"{"input_tokens":1}"#.into(),
                r#"{"input_tokens":1}"#.into(),
                r#"{"input_tokens":1}"#.into(),
                r#"{"input_tokens":1}"#.into(),
            ])));
            let pending = replies.clone();
            let fetch: Fetch = Arc::new(move |_| {
                let body = pending.lock().unwrap().pop_front().unwrap();
                let response = response(Reply {
                    status: Some(200),
                    body: Some(body),
                    body_error: None,
                    transport_error: None,
                });
                Box::pin(futures_util::future::ready(response))
            });
            let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
            let formatter = LocaleNumberFormat::new(&|_| Some("en_US.UTF-8".into())).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(run(
                    "controlled",
                    &fetch,
                    &formatter,
                    &mut stdout,
                    &mut stderr,
                ))
                .unwrap();
            assert!(stdout.ends_with(b"\nTotal: 4 tokens\n"));
            assert_eq!(String::from_utf8(stderr).unwrap().lines().count(), 1);
            assert!(replies.lock().unwrap().is_empty());
        }
    }

    /// Holds the first response at independently controlled header and body boundaries.
    async fn gated_response(
        first: bool,
        headers: Arc<tokio::sync::Notify>,
        body: Arc<tokio::sync::Notify>,
    ) -> Result<HttpResponse, FetchError> {
        if first {
            headers.notified().await;
        }
        Ok(HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: std::collections::BTreeMap::default(),
            body: Box::pin(stream::once(async move {
                if first {
                    body.notified().await;
                }
                Ok(br#"{"input_tokens":1}"#.to_vec())
            })),
        })
    }

    /// Polls the same report while headers and then the first body remain unresolved.
    #[test]
    fn token_measurement_waits_for_each_body_before_next_request() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let headers = Arc::new(tokio::sync::Notify::new());
        let body = Arc::new(tokio::sync::Notify::new());
        let (observed, header_gate, body_gate) = (calls.clone(), headers.clone(), body.clone());
        let fetch: Fetch = Arc::new(move |_| {
            let first = observed.fetch_add(1, Ordering::SeqCst) == 0;
            let (headers, body) = (header_gate.clone(), body_gate.clone());
            Box::pin(gated_response(first, headers, body))
        });
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let formatter = LocaleNumberFormat::new(&|_| Some("en_US.UTF-8".into())).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let work = run("controlled", &fetch, &formatter, &mut stdout, &mut stderr);
                let mut work = std::pin::pin!(work);
                let mut context = std::task::Context::from_waker(std::task::Waker::noop());
                assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                headers.notify_one();
                assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                body.notify_one();
                work.await.unwrap();
            });
        assert_eq!(calls.load(Ordering::SeqCst), 5);
        assert!(stdout.ends_with(b"\nTotal: 5 tokens\n"));
        assert!(stderr.is_empty());
    }

    /// Reads a complete native request without normalizing its authored body.
    fn read_request(socket: &mut std::net::TcpStream) -> Request {
        use std::io::{BufRead, BufReader, Read};
        let mut reader = BufReader::new(socket);
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut headers = indexmap::IndexMap::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.split_once(':').unwrap();
            headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
        }
        let mut bytes = vec![0; headers["content-length"].parse().unwrap()];
        reader.read_exact(&mut bytes).unwrap();
        Request {
            url: first,
            method: String::new(),
            headers,
            body: String::from_utf8(bytes).unwrap(),
        }
    }

    /// Serves the recorded responses and retains complete native request bytes.
    fn serve(listener: &std::net::TcpListener, replies: Vec<Reply>) -> Vec<Request> {
        let mut requests = Vec::new();
        for reply in replies {
            let (mut socket, _) = listener.accept().unwrap();
            let request = read_request(&mut socket);
            let body = reply.body.unwrap();
            write!(
                socket,
                "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                reply.status.unwrap(),
                body.len()
            )
            .unwrap();
            requests.push(request);
        }
        requests
    }

    /// Runs the native witness in a child with explicit locale, key and proxy observations.
    fn native_child_completed() -> bool {
        const CHILD: &str = "MAESTRO_COUNT_NATIVE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            for name in [
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "ALL_PROXY",
                "http_proxy",
                "https_proxy",
                "all_proxy",
                "NO_PROXY",
                "no_proxy",
            ] {
                command.env_remove(name);
            }
            let output = command
                .args([
                    "--exact",
                    "tests::token_measurement_sends_five_current_descriptions_in_order",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("ANTHROPIC_API_KEY", "controlled")
                .env("LC_ALL", "en_US.UTF-8")
                .env("LC_MESSAGES", "en_US.UTF-8")
                .env("LANG", "en_US.UTF-8")
                .output()
                .unwrap();
            assert!(output.status.success(), "native transport child failed");
            return true;
        }
        false
    }

    /// Qualifies the real transport in an isolated process against all five full bodies.
    #[test]
    fn token_measurement_sends_five_current_descriptions_in_order() {
        if native_child_completed() {
            return;
        }
        let case = case("grouped_counts");
        let server = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let origin = format!("http://{}", server.local_addr().unwrap());
        let peer = std::thread::spawn(move || serve(&server, case.input.replies));
        let transport = maestro_app::default_fetch();
        let envelopes = Arc::new(Mutex::new(Vec::new()));
        let observed = envelopes.clone();
        let fetch: Fetch = Arc::new(move |mut request| {
            assert_eq!(
                request.url,
                "https://api.anthropic.com/v1/messages/count_tokens"
            );
            observed.lock().unwrap().push(request.clone());
            request.url = format!("{origin}/v1/messages/count_tokens");
            transport(request)
        });
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let formatter = LocaleNumberFormat::new(&|key| std::env::var(key).ok()).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run(
                &std::env::var("ANTHROPIC_API_KEY").unwrap(),
                &fetch,
                &formatter,
                &mut stdout,
                &mut stderr,
            ))
            .unwrap();
        let wire = peer.join().unwrap();
        let expected = case.output.requests.unwrap();
        assert_eq!(wire.len(), expected.len());
        for ((actual, envelope), expected) in wire
            .iter()
            .zip(envelopes.lock().unwrap().iter())
            .zip(expected)
        {
            assert_request(envelope, &expected);
            assert_eq!(actual.url, "POST /v1/messages/count_tokens HTTP/1.1\r\n");
            assert_eq!(actual.body, expected.body);
            for (key, value) in expected.headers {
                let matches = actual.headers.get(&key.to_ascii_lowercase()) == Some(&value);
                assert!(matches);
            }
        }
        assert_eq!(stdout, case.output.stdout.concat().as_bytes());
        assert!(stderr.is_empty());
    }
}
