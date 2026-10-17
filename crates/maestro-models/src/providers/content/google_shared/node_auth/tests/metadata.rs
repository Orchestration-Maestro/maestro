//! Metadata admission and endpoint controls.

#[test]
fn native_metadata_endpoint_chooses_ip_before_host() {
    for (ip, host, expected) in [
        (
            Some("127.0.0.1:9001"),
            Some("decoy.invalid"),
            "http://127.0.0.1:9001/computeMetadata/v1",
        ),
        (
            Some(""),
            Some("HTTPS://host.invalid:443/path"),
            "https://host.invalid/computeMetadata/v1",
        ),
        (
            None,
            Some("http://[::1]:9002"),
            "http://[::1]:9002/computeMetadata/v1",
        ),
        (None, None, "http://169.254.169.254/computeMetadata/v1"),
    ] {
        assert_eq!(
            super::super::metadata::base_url(ip, host).unwrap(),
            expected
        );
    }
}

#[test]
fn metadata_detection_modes_choose_their_required_operations() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for (policy, resident, expected_calls, expected) in [
                (None, false, 1, true),
                (None, true, 0, true),
                (Some("assume-present"), false, 0, true),
                (Some("bios-only"), false, 0, false),
                (Some("bios-only"), true, 0, true),
                (Some("ping-only"), false, 1, true),
                (Some("ping-only"), true, 0, true),
            ] {
                let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let observed = calls.clone();
                let fetch: crate::Fetch = std::sync::Arc::new(move |request| {
                    observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    assert_eq!(
                        request.url,
                        "http://controlled.invalid/computeMetadata/v1/instance"
                    );
                    assert_eq!(request.method, "GET");
                    assert!(
                        request
                            .headers
                            .get("metadata-flavor")
                            .is_some_and(|value| value == "Google")
                    );
                    let mut response = super::response(&serde_json::json!({}));
                    response
                        .headers
                        .insert("metadata-flavor".into(), "Google".into());
                    Box::pin(std::future::ready(Ok(response)))
                });
                let metadata = super::super::metadata::Metadata::new(
                    policy,
                    resident,
                    (None, Some("controlled.invalid")),
                    0,
                )
                .unwrap();
                assert_eq!(metadata.admitted(fetch).await.unwrap(), expected);
                assert_eq!(
                    calls.load(std::sync::atomic::Ordering::SeqCst),
                    expected_calls
                );
            }
        });
}

use super::super::metadata::{Metadata, gcp_residency};
use crate::{FetchError, HttpResponse};

#[test]
fn metadata_disable_wins_over_residency() {
    use crate::tests::child_process;
    const TEST: &str = "metadata_disable_wins_over_residency";
    if child_process::child_case().is_some() {
        let (fetch, urls) = super::recording(|_| Ok(super::google_response()));
        let effects = super::super::base_external_client::Effects {
            fetch,
            ..Default::default()
        };
        let error = super::run(super::super::metadata::credentials(effects))
            .err()
            .unwrap();
        assert_eq!(error.message, super::super::NO_CREDENTIALS);
        assert!(urls.lock().unwrap().is_empty());
        return;
    }
    super::run_paused(async {
        for resident in [false, true] {
            let (fetch, urls) = super::recording(|_| Ok(super::google_response()));
            let metadata =
                Metadata::new(Some("none"), resident, (None, Some("m.invalid")), 0).unwrap();
            assert!(!metadata.admitted(fetch).await.unwrap());
            assert!(urls.lock().unwrap().is_empty());
        }
    });
    child_process::rerun(
        TEST,
        "disabled",
        &[
            ("METADATA_SERVER_DETECTION", "none"),
            ("K_SERVICE", "service"),
        ],
    )
    .unwrap();
}

#[test]
fn metadata_probe_requires_successful_google_flavor() {
    super::run_paused(async {
        for (case, expected) in [(0, true), (1, false), (2, false), (3, false), (4, false)] {
            let (fetch, urls) = super::recording(move |_| match case {
                0 => Ok(super::google_response()),
                1 => Ok(super::response(&serde_json::json!({}))),
                2 => Ok(wrong_flavor()),
                3 => Ok(HttpResponse {
                    status: 404,
                    ..super::google_response()
                }),
                _ => Err(FetchError::Timeout),
            });
            let metadata =
                Metadata::new(Some("ping-only"), false, (None, Some("m.invalid")), 0).unwrap();
            assert_eq!(metadata.admitted(fetch).await.unwrap(), expected);
            assert_eq!(
                urls.lock().unwrap()[0],
                "http://m.invalid/computeMetadata/v1/instance"
            );
        }
    });
}

fn wrong_flavor() -> HttpResponse {
    let mut response = super::google_response();
    response
        .headers
        .insert("metadata-flavor".into(), "Other".into());
    response
}

#[test]
fn metadata_probe_uses_selected_deadline_and_retry_controls() {
    let nonresident = Metadata::new(None, false, (None, Some("m.invalid")), 0).unwrap();
    assert_eq!(
        nonresident.timeout,
        Some(std::time::Duration::from_millis(3000))
    );
    let resident = Metadata::new(Some("ping-only"), true, (None, Some("m.invalid")), 0).unwrap();
    assert_eq!(resident.timeout, None);
    super::run_paused(async {
        for (retries, expected) in [(0, 1), (2, 3), (3, 4), (9, 4)] {
            let (fetch, urls) = super::recording(|_| Err(FetchError::Timeout));
            let metadata =
                Metadata::new(Some("ping-only"), false, (None, Some("m.invalid")), retries)
                    .unwrap();
            assert!(!metadata.admitted(fetch).await.unwrap());
            assert_eq!(urls.lock().unwrap().len(), expected);
        }
    });
}

#[test]
fn metadata_default_candidates_accept_the_first_success() {
    super::run_paused(async {
        for (primary, secondary, expected) in [
            (true, false, true),
            (false, true, true),
            (false, false, false),
        ] {
            let (fetch, urls) = super::recording(move |url| answer(url, (primary, secondary)));
            let metadata = Metadata::new(None, false, (None, None), 0).unwrap();
            assert_eq!(metadata.admitted(fetch).await.unwrap(), expected);
            let urls = urls.lock().unwrap();
            assert!(
                urls.iter()
                    .any(|url| url.starts_with("http://169.254.169.254/"))
            );
            assert_eq!(
                urls.iter()
                    .any(|url| url.starts_with("http://metadata.google.internal./")),
                !primary
            );
        }
        let (fetch, urls) = super::recording(|_| Err(FetchError::Timeout));
        let metadata = Metadata::new(None, false, (Some("10.0.0.1"), None), 0).unwrap();
        assert!(!metadata.admitted(fetch).await.unwrap());
        assert_eq!(urls.lock().unwrap().len(), 1);
    });
}

#[test]
fn linux_residency_reads_serverless_dmi_and_network_prefix() {
    use crate::tests::child_process;
    const TEST: &str = "linux_residency_reads_serverless_dmi_and_network_prefix";
    if let Some(case) = child_process::child_case() {
        assert_eq!(gcp_residency::detect(), case != "none");
        return;
    }
    assert!(gcp_residency::bios(true, Some("Google")));
    assert!(!gcp_residency::bios(true, Some("google")));
    assert!(!gcp_residency::bios(false, Some("Google")));
    assert!(!gcp_residency::bios(true, None));
    assert!(gcp_residency::mac("42:01:0a:00:00:02"));
    assert!(!gcp_residency::mac("02:42:01:00:00:00"));
    for name in ["CLOUD_RUN_JOB", "FUNCTION_NAME", "K_SERVICE"] {
        for value in ["0", "false", "x"] {
            child_process::rerun(TEST, "serverless", &[(name, value)]).unwrap();
        }
    }
}

#[test]
fn metadata_policy_uses_shared_whitespace_and_case_rules() {
    super::run_paused(async {
        for policy in ["\u{feff}NoNe\u{feff}", " none\n"] {
            let (fetch, urls) = super::recording(|_| Ok(super::google_response()));
            let metadata =
                Metadata::new(Some(policy), false, (None, Some("m.invalid")), 0).unwrap();
            assert!(!metadata.admitted(fetch).await.unwrap());
            assert!(urls.lock().unwrap().is_empty());
        }
        let (fetch, _) = super::recording(|_| Ok(super::google_response()));
        let metadata =
            Metadata::new(Some(" Assume-Present"), false, (None, Some("m.invalid")), 0).unwrap();
        assert!(metadata.admitted(fetch).await.unwrap());
    });
    assert!(Metadata::new(Some("\u{85}none"), false, (None, None), 0).is_err());
    assert!(Metadata::new(Some("unknown"), false, (None, None), 0).is_err());
}

/// Success for the candidate whose flag is set, a no-response failure otherwise.
fn answer(url: &str, working: (bool, bool)) -> Result<HttpResponse, FetchError> {
    let ok = if url.starts_with("http://169.254.169.254") {
        working.0
    } else {
        working.1
    };
    if ok {
        Ok(super::google_response())
    } else {
        Err(FetchError::Timeout)
    }
}
