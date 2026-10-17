//! Metadata admission and endpoint controls.

use super::super::metadata::{Detection, Metadata, gcp_residency};
use crate::{FetchError, HttpResponse};

/// Metadata over `policy` (absent means the default control), which must not disable metadata.
pub(super) fn metadata(
    policy: Option<&str>,
    resident: bool,
    endpoints: (Option<&str>, Option<&str>),
    retries: u64,
) -> Metadata {
    let detection = Detection::parse(policy).unwrap().unwrap();
    Metadata::new(detection, resident, endpoints, retries).unwrap()
}

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
    if crate::tests::child_process::child_case().is_none() {
        crate::tests::child_process::rerun(
            "metadata_detection_modes_choose_their_required_operations",
            "probe",
            &[],
        )
        .unwrap();
        return;
    }
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
                let metadata = metadata(policy, resident, (None, Some("controlled.invalid")), 0);
                assert_eq!(metadata.admitted(fetch).await, expected);
                assert_eq!(
                    calls.load(std::sync::atomic::Ordering::SeqCst),
                    expected_calls
                );
            }
        });
}

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
fn metadata_unknown_policy_admits_resident_without_probe() {
    use crate::tests::child_process;
    const TEST: &str = "metadata_unknown_policy_admits_resident_without_probe";
    if child_process::child_case().is_some() {
        let (fetch, urls) = super::recording(|_| panic!("resident host must not probe"));
        let effects = super::super::base_external_client::Effects {
            fetch,
            ..Default::default()
        };
        assert!(super::run(super::super::metadata::credentials(effects)).is_ok());
        assert!(urls.lock().unwrap().is_empty());
        return;
    }
    child_process::rerun(
        TEST,
        "resident",
        &[
            ("METADATA_SERVER_DETECTION", "unknown"),
            ("K_SERVICE", "service"),
        ],
    )
    .unwrap();
}

#[test]
fn metadata_probe_result_is_kept_once_per_process() {
    use crate::tests::child_process;
    const TEST: &str = "metadata_probe_result_is_kept_once_per_process";
    if let Some(case) = child_process::child_case() {
        super::run_paused(async {
            let succeeds = case == "positive";
            let (fetch, urls) = super::recording(move |_| {
                succeeds
                    .then(super::google_response)
                    .ok_or(FetchError::Timeout)
            });
            let effects = super::super::base_external_client::Effects {
                fetch,
                ..Default::default()
            };
            let first = super::super::metadata::credentials(effects.clone()).await;
            let expected_error = (!succeeds).then(|| super::super::NO_CREDENTIALS.to_owned());
            assert_eq!(first.err().map(|error| error.message), expected_error);
            let probes = urls.lock().unwrap().len();
            assert!(probes > 0);
            let second = super::super::metadata::credentials(effects).await;
            assert_eq!(second.err().map(|error| error.message), expected_error);
            assert_eq!(urls.lock().unwrap().len(), probes);
        });
        return;
    }
    for case in ["positive", "negative"] {
        child_process::rerun(
            TEST,
            case,
            &[
                ("METADATA_SERVER_DETECTION", "ping-only"),
                ("GCE_METADATA_HOST", "m.invalid"),
            ],
        )
        .unwrap();
    }
}

#[test]
fn metadata_probe_requires_successful_google_flavor() {
    if crate::tests::child_process::child_case().is_none() {
        crate::tests::child_process::rerun(
            "metadata_probe_requires_successful_google_flavor",
            "probe",
            &[],
        )
        .unwrap();
        return;
    }
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
            let metadata = metadata(Some("ping-only"), false, (None, Some("m.invalid")), 0);
            assert_eq!(metadata.admitted(fetch).await, expected);
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
    if crate::tests::child_process::child_case().is_none() {
        crate::tests::child_process::rerun(
            "metadata_probe_uses_selected_deadline_and_retry_controls",
            "probe",
            &[],
        )
        .unwrap();
        return;
    }
    let nonresident = metadata(None, false, (None, Some("m.invalid")), 0);
    assert_eq!(
        nonresident.timeout,
        Some(std::time::Duration::from_millis(3000))
    );
    let resident = metadata(Some("ping-only"), true, (None, Some("m.invalid")), 0);
    assert_eq!(resident.timeout, None);
    super::run_paused(async {
        for (retries, expected) in [(0, 1), (2, 3), (3, 4), (9, 4)] {
            let (fetch, urls) = super::recording(|_| Err(FetchError::Timeout));
            let metadata = metadata(Some("ping-only"), false, (None, Some("m.invalid")), retries);
            assert!(!metadata.admitted(fetch).await);
            assert_eq!(urls.lock().unwrap().len(), expected);
        }
    });
}

#[test]
fn metadata_default_candidates_accept_the_first_success() {
    if crate::tests::child_process::child_case().is_none() {
        crate::tests::child_process::rerun(
            "metadata_default_candidates_accept_the_first_success",
            "probe",
            &[],
        )
        .unwrap();
        return;
    }
    super::run_paused(async {
        for (primary, secondary, expected) in [
            (true, false, true),
            (false, true, true),
            (false, false, false),
        ] {
            let (fetch, urls) = super::recording(move |url| answer(url, (primary, secondary)));
            let metadata = metadata(None, false, (None, None), 0);
            assert_eq!(metadata.admitted(fetch).await, expected);
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
        let metadata = metadata(None, false, (Some("10.0.0.1"), None), 0);
        assert!(!metadata.admitted(fetch).await);
        assert_eq!(urls.lock().unwrap().len(), 1);
    });
}

#[test]
fn linux_residency_reads_serverless_dmi_and_network_prefix() {
    use crate::tests::child_process;
    const TEST: &str = "linux_residency_reads_serverless_dmi_and_network_prefix";
    if let Some(case) = child_process::child_case() {
        assert_eq!(gcp_residency::detect_gcp_residency(), case != "none");
        return;
    }
    assert!(gcp_residency::is_google_compute_engine_linux(
        true,
        Some("Google")
    ));
    assert!(!gcp_residency::is_google_compute_engine_linux(
        true,
        Some("google")
    ));
    assert!(!gcp_residency::is_google_compute_engine_linux(
        false,
        Some("Google")
    ));
    assert!(!gcp_residency::is_google_compute_engine_linux(true, None));
    assert!(gcp_residency::is_google_compute_engine_mac_address(
        "42:01:0a:00:00:02"
    ));
    assert!(!gcp_residency::is_google_compute_engine_mac_address(
        "02:42:01:00:00:00"
    ));
    for name in ["CLOUD_RUN_JOB", "FUNCTION_NAME", "K_SERVICE"] {
        for value in ["0", "false", "x"] {
            child_process::rerun(TEST, "serverless", &[(name, value)]).unwrap();
        }
    }
}

#[test]
fn metadata_policy_uses_shared_whitespace_and_case_rules() {
    for policy in ["\u{feff}NoNe\u{feff}", " none\n"] {
        assert!(Detection::parse(Some(policy)).unwrap().is_none());
    }
    super::run_paused(async {
        let (fetch, _) = super::recording(|_| Ok(super::google_response()));
        let metadata = metadata(Some(" Assume-Present"), false, (None, Some("m.invalid")), 0);
        assert!(metadata.admitted(fetch).await);
    });
    assert!(Detection::parse(Some("\u{85}none")).is_err());
    assert!(Detection::parse(Some("unknown")).is_err());
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
