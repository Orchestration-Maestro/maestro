//! Release comparison and hosted request observations.
#![cfg(test)]
#![cfg(unix)]
use maestro_app::configuration::version_check::{
    compare_package_versions, is_newer_package_version,
};
use maestro_text::collation::Collator;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionCase {
    input: VersionInput,
    output: VersionOutput,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionInput {
    left: String,
    right: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionOutput {
    compare: Option<f64>,
    newer: bool,
}
fn versions(indices: std::ops::Range<usize>) {
    let cases: Vec<VersionCase> =
        serde_json::from_str(include_str!("fixtures/services/versions.json")).unwrap();
    let collator = Collator::new(&|_| Some("en_US.UTF-8".into())).unwrap();
    for case in &cases[indices] {
        assert_eq!(
            compare_package_versions(&case.input.left, &case.input.right, &collator),
            case.output.compare
        );
        assert_eq!(
            is_newer_package_version(&case.input.left, &case.input.right, &collator),
            case.output.newer
        );
    }
}
#[test]
fn package_version_cases_preserve_comparison() {
    versions(0..3);
}

#[test]
fn version_components_return_their_numeric_difference() {
    versions(3..6);
}
#[test]
fn version_grammar_keeps_whitespace_and_build_rules() {
    versions(6..14);
    versions(21..28);
    versions(35..44);
}
#[test]
fn prereleases_use_the_prepared_locale_comparator() {
    versions(14..21);
}
#[test]
fn unparsed_versions_compare_trimmed_text() {
    versions(28..35);
}
use crate::service_observations as services;
use maestro_app::configuration::version_check::{VersionCheckOptions, get_latest_maestro_release};
use services::{ORIGIN, Transport};
#[cfg(unix)]
#[test]
fn release_request_keeps_endpoint_headers_and_version() {
    services::run(release_request_keeps_endpoint_headers_and_version_run());
}
async fn release_request_keeps_endpoint_headers_and_version_run() {
    if services::isolated("release_request_keeps_endpoint_headers_and_version") {
        return;
    }
    for case in services::cases("release-status") {
        let status = u16::try_from(case.input["status"].as_u64().unwrap()).unwrap();
        let transport = Transport::new(
            status,
            if status == 204 {
                String::new()
            } else {
                r#"{"version":"1.2.4"}"#.into()
            },
            "",
        );
        let options = VersionCheckOptions {
            timeout_ms: case.input["timeout"].as_u64(),
            fetch: Some(transport.fetch.clone()),
            offline: None,
            skip_version_check: None,
        };
        let result = get_latest_maestro_release("1.2.3", ORIGIN, &options).await;
        if case.output["branch"] == "error" {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().map(|r| r.version),
                case.output["result"].as_str().map(str::to_owned)
            );
        }
        assert_eq!(transport.calls(), services::expected_calls(&case.output));
        if status >= 300 {
            assert_eq!(
                transport
                    .body_polls
                    .load(std::sync::atomic::Ordering::SeqCst),
                0
            );
        }
    }
}
#[cfg(unix)]
#[test]
fn release_fields_keep_their_selected_type_domain() {
    services::run(release_fields_keep_their_selected_type_domain_run());
}
async fn release_fields_keep_their_selected_type_domain_run() {
    if services::isolated("release_fields_keep_their_selected_type_domain") {
        return;
    }
    for group in ["release-payload", "release-body"] {
        for case in services::cases(group) {
            if group == "release-body"
                && ["", "{", "{\"version\":"].contains(&case.input["body"].as_str().unwrap())
            {
                continue;
            }
            let body = if group == "release-body" {
                case.input["body"].as_str().unwrap().to_owned()
            } else {
                case.input["payload"].to_string()
            };
            let t = Transport::new(200, body, "");
            let options = VersionCheckOptions {
                fetch: Some(t.fetch.clone()),
                ..Default::default()
            };
            let result = get_latest_maestro_release("1.2.3", ORIGIN, &options).await;
            if case.output["branch"] == "error" {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    services::release_value(result.unwrap()),
                    case.output["result"]
                );
            }
            assert_eq!(t.calls(), services::expected_calls(&case.output));
        }
    }
    let t = Transport::new(200, "{\"version\":\" \u{feff} \"}".into(), "");
    let options = VersionCheckOptions {
        fetch: Some(t.fetch.clone()),
        ..Default::default()
    };
    assert!(
        get_latest_maestro_release("1.2.3", ORIGIN, &options)
            .await
            .unwrap()
            .is_none()
    );
}
use maestro_app::configuration::version_check::{
    check_for_new_maestro_version, get_latest_maestro_version,
};
#[cfg(unix)]
#[test]
fn version_projection_and_newer_check_keep_distinct_results() {
    services::run(version_projection_and_newer_check_keep_distinct_results_run());
}
async fn version_projection_and_newer_check_keep_distinct_results_run() {
    if services::isolated("version_projection_and_newer_check_keep_distinct_results") {
        return;
    }
    for case in services::cases("newer") {
        let t = Transport::new(
            200,
            serde_json::json!({"version": case.input["candidate"]}).to_string(),
            "",
        );
        let options = VersionCheckOptions {
            fetch: Some(t.fetch.clone()),
            ..Default::default()
        };
        let current = case.input["current"].as_str().unwrap();
        assert_eq!(
            get_latest_maestro_version(current, ORIGIN, &options)
                .await
                .unwrap()
                .as_deref(),
            case.input["candidate"].as_str()
        );
        let before = t.count();
        assert_eq!(
            check_for_new_maestro_version(current, ORIGIN, &options)
                .await
                .as_deref(),
            case.output["result"].as_str()
        );
        assert_eq!(
            t.calls().as_array().unwrap()[before..],
            services::expected_calls(&case.output).as_array().unwrap()[..]
        );
    }
}
#[cfg(unix)]
#[test]
fn release_flags_skip_before_transport() {
    services::run(release_flags_skip_before_transport_run());
}
async fn release_flags_skip_before_transport_run() {
    let Some(case) = services::environment_cases(
        "release_flags_skip_before_transport",
        "release-gates",
        &(0..15).collect::<Vec<_>>(),
    ) else {
        return;
    };
    let t = Transport::new(200, r#"{"version":"1.2.4"}"#.into(), "");
    let options = VersionCheckOptions {
        fetch: Some(t.fetch.clone()),
        ..Default::default()
    };
    assert_eq!(
        get_latest_maestro_version("1.2.3", ORIGIN, &options)
            .await
            .unwrap()
            .as_deref(),
        case.output["result"].as_str()
    );
    assert_eq!(t.calls(), services::expected_calls(&case.output));
}
#[cfg(unix)]
#[test]
fn release_failures_keep_the_lower_error_boundary() {
    services::run(release_failures_keep_the_lower_error_boundary_run());
}
async fn release_failures_keep_the_lower_error_boundary_run() {
    if services::isolated("release_failures_keep_the_lower_error_boundary") {
        return;
    }
    for case in services::cases("release-errors") {
        lower_failure(case).await;
    }
    malformed_bodies().await;
}
/// Observe lower and catching operations for one failure.
async fn lower_failure(case: services::Case) {
    let mode = case.input["response"].as_str().unwrap_or("");
    let body = if mode == "JSON_ERROR" { "{" } else { "null" };
    let t = Transport::new(200, body.into(), mode);
    let options = VersionCheckOptions {
        fetch: Some(t.fetch.clone()),
        ..Default::default()
    };
    let error = get_latest_maestro_release("1.2.3", ORIGIN, &options)
        .await
        .unwrap_err();
    assert_eq!(case.output["lower"], "error");
    assert_cause(&error, mode);
    let before = t.count();
    assert!(
        get_latest_maestro_version("1.2.3", ORIGIN, &options)
            .await
            .is_err()
    );
    assert_eq!(t.count(), before + 1);
    let before = t.count();
    assert_eq!(
        check_for_new_maestro_version("1.2.3", ORIGIN, &options)
            .await
            .as_deref(),
        case.output["convenience"].as_str()
    );
    let calls = t.calls();
    let expected = services::expected_calls(&case.output);
    assert_eq!(calls[0], expected[0]);
    assert_eq!(
        calls.as_array().unwrap()[before..],
        expected.as_array().unwrap()[1..]
    );
}
/// JSON failures retain the decoder cause.
async fn malformed_bodies() {
    for case in services::cases("release-body")
        .into_iter()
        .filter(|c| ["", "{", "{\"version\":"].contains(&c.input["body"].as_str().unwrap()))
    {
        let t = Transport::new(200, case.input["body"].as_str().unwrap().into(), "");
        let options = VersionCheckOptions {
            fetch: Some(t.fetch.clone()),
            ..Default::default()
        };
        assert_eq!(case.output["branch"], "error");
        assert!(
            get_latest_maestro_release("1.2.3", ORIGIN, &options)
                .await
                .unwrap_err()
                .get_ref()
                .unwrap()
                .is::<serde_json::Error>()
        );
        assert!(
            get_latest_maestro_version("1.2.3", ORIGIN, &options)
                .await
                .is_err()
        );
        assert!(
            check_for_new_maestro_version("1.2.3", ORIGIN, &options)
                .await
                .is_none()
        );
        let expected = services::expected_calls(&case.output);
        assert_eq!(
            t.calls(),
            serde_json::json!([expected[0], expected[0], expected[0]])
        );
    }
}
#[cfg(unix)]
#[test]
fn release_deadline_covers_headers_and_body() {
    crate::support::paused_io_runtime().block_on(release_deadline_covers_headers_and_body_run());
}
async fn release_deadline_covers_headers_and_body_run() {
    if services::isolated("release_deadline_covers_headers_and_body") {
        return;
    }
    for (timeout, mode) in [
        (None, "HEADERS_PENDING"),
        (None, "HEADERS_DELAYED"),
        (Some(17), "HEADERS_PENDING"),
        (Some(17), "HEADERS_DELAYED"),
    ] {
        deadline(timeout, mode).await;
    }
    let t = Transport::new(200, r#"{"version":"1.2.4"}"#.into(), "");
    let options = VersionCheckOptions {
        timeout_ms: Some(0),
        fetch: Some(t.fetch.clone()),
        offline: None,
        skip_version_check: None,
    };
    assert_eq!(
        get_latest_maestro_version("1.2.3", ORIGIN, &options)
            .await
            .unwrap()
            .as_deref(),
        Some("1.2.4")
    );
}
#[cfg(unix)]
#[test]
fn default_release_transport_reaches_the_controlled_server() {
    services::run(default_release_transport_reaches_the_controlled_server_run());
}
async fn default_release_transport_reaches_the_controlled_server_run() {
    const NAME: &str = "default_release_transport_reaches_the_controlled_server";
    if !maestro_test_process::selection().is_child() {
        let mut removed = services::FLAGS.to_vec();
        removed.extend([
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "NO_PROXY",
            "no_proxy",
        ]);
        services::native_probe(NAME, &[], &removed);
        return;
    }
    let server = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let peer = std::thread::spawn(move || observe_native_requests(&server));
    let release =
        get_latest_maestro_release("1.2.3", &origin, &VersionCheckOptions::default()).await;
    let settings =
        maestro_settings::SettingsManager::in_memory(maestro_settings::Settings::default());
    maestro_app::configuration::telemetry::report_install_telemetry(
        "1.2.3", &settings, &origin, None, None,
    )
    .await;
    let observed = peer.join().unwrap();
    assert_eq!(
        release.unwrap(),
        Some(
            maestro_app::configuration::version_check::LatestMaestroRelease {
                version: "1.2.4".into(),
                package_name: Some("maestro".into())
            }
        )
    );
    assert_eq!(observed[0].0, "GET /api/latest-version HTTP/1.1");
    assert_eq!(
        observed[1].0,
        "GET /api/report-install?version=1.2.3 HTTP/1.1"
    );
    assert_eq!(observed[0].1["accept"], "application/json");
    for (_, headers) in observed {
        assert_eq!(
            headers["user-agent"],
            maestro_app::configuration::maestro_user_agent::get_maestro_user_agent(
                "1.2.3",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        );
    }
}

/// Poll the same request around its unchanged whole-operation deadline.
async fn deadline(timeout: Option<u64>, mode: &str) {
    let t = Transport::new(200, String::new(), mode);
    let options = VersionCheckOptions {
        timeout_ms: timeout,
        fetch: Some(t.fetch.clone()),
        offline: None,
        skip_version_check: None,
    };
    let mut work = std::pin::pin!(get_latest_maestro_release("1.2.3", ORIGIN, &options));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
    assert_eq!(t.count(), 1);
    tokio::time::advance(std::time::Duration::from_millis(5)).await;
    if mode == "HEADERS_DELAYED" {
        t.headers.notify_one();
        assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
        assert_eq!(t.body_polls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    expire(work.as_mut(), &t, timeout.unwrap_or(10_000) - 6).await;
}

/// Serve exactly the two requests emitted by the public operations.
fn observe_native_requests(
    server: &std::net::TcpListener,
) -> Vec<(String, std::collections::BTreeMap<String, String>)> {
    use std::io::{Read, Write};
    let mut observed = Vec::new();
    for _ in 0..2 {
        let (mut stream, _) = server.accept().unwrap();
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8(head).unwrap();
        let lines: Vec<_> = head.split("\r\n").collect();
        let headers: std::collections::BTreeMap<_, _> = lines
            .iter()
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
            .collect();
        assert!(!headers.contains_key("transfer-encoding"));
        assert_eq!(
            headers.get("content-length").map_or("0", String::as_str),
            "0"
        );
        observed.push((lines[0].to_owned(), headers));
        let body = r#"{"version":"1.2.4","packageName":"maestro"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    }
    observed
}

/// Native failure causes survive the release boundary.
fn assert_cause(error: &std::io::Error, mode: &str) {
    if matches!(mode, "FETCH_ERROR" | "BODY_ERROR") {
        assert_eq!(
            error
                .get_ref()
                .unwrap()
                .downcast_ref::<maestro_models::FetchError>(),
            Some(&maestro_models::FetchError::Aborted)
        );
    } else if mode == "JSON_ERROR" {
        assert!(error.get_ref().unwrap().is::<serde_json::Error>());
    } else {
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}

/// Expiry revokes the same pending producer at the exact boundary.
async fn expire(
    mut work: std::pin::Pin<
        &mut impl std::future::Future<
            Output = std::io::Result<
                Option<maestro_app::configuration::version_check::LatestMaestroRelease>,
            >,
        >,
    >,
    t: &Transport,
    remaining: u64,
) {
    tokio::time::advance(std::time::Duration::from_millis(remaining)).await;
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
    assert_eq!(t.dropped.load(std::sync::atomic::Ordering::SeqCst), 0);
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    let std::task::Poll::Ready(result) = std::future::Future::poll(work.as_mut(), &mut context)
    else {
        panic!("deadline did not settle");
    };
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(t.dropped.load(std::sync::atomic::Ordering::SeqCst), 1);
}
