//! Private request preparation and one-attempt sending.

#[cfg(not(target_arch = "wasm32"))]
use super::super::request;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn request_hooks_keep_phase_and_response_hook_is_unused() {
    crate::tests::runtime::run(async {
        let mut options = crate::MistralOptions::<crate::Threaded>::default();
        options.common.api_key = Some(String::new());
        let mut model = super::request_corpus::model(&serde_json::json!({}));
        model.provider = "missing-synthetic-provider".into();
        let context = serde_json::from_value(serde_json::json!({"messages":[]})).unwrap();
        let error = request::prepare(std::sync::Arc::new(model), &context, &options, None)
            .await
            .err()
            .unwrap();
        assert_eq!(
            error.message,
            "No API key for provider: missing-synthetic-provider"
        );
        super::request_corpus::async_corpus(
            "request_hooks_keep_phase_and_response_hook_is_unused",
            15,
        )
        .await;
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn request_send_resolves_header_case_and_affinity() {
    crate::tests::runtime::run(super::request_corpus::async_corpus(
        "request_send_resolves_header_case_and_affinity",
        31,
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn request_timeout_setting_is_checked_like_every_other_request() {
    crate::tests::runtime::run(async {
        for (timeout_ms, message) in [
            (-1.0, "timeout must be a positive integer"),
            (1.5, "timeout must be an integer"),
            (f64::NAN, "timeout must be an integer"),
            (f64::INFINITY, "timeout must be an integer"),
        ] {
            let failure = super::request_lifetime::try_prepared(
                crate::default_fetch(),
                None,
                Some(timeout_ms),
            )
            .await
            .err()
            .unwrap();
            assert_eq!(failure.message, message, "timeout {timeout_ms}");
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn request_send_has_one_attempt_and_signal_dependent_deadline() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(super::request_corpus::async_corpus(
            "request_send_has_one_attempt_and_signal_dependent_deadline",
            10,
        ));
    super::request_lifetime::verify();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn request_failure_keeps_status_body_and_fallback() {
    crate::tests::runtime::run(super::request_corpus::async_corpus(
        "request_failure_keeps_status_body_and_fallback",
        45,
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn request_rejects_nonfinite_raw_numbers_without_null_sanitizing() {
    crate::tests::runtime::run(super::request_corpus::async_corpus(
        "request_rejects_nonfinite_raw_numbers_without_null_sanitizing",
        15,
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn request_send_uses_relative_endpoint_and_default_origin() {
    let url = request::transport::base_url("///https://example.test/{not.a-template}").unwrap();
    assert_eq!(
        request::transport::endpoint(&url).unwrap(),
        "https://example.test/%7Bnot.a-template%7D/v1/chat/completions"
    );
    assert_eq!(
        request::transport::base_url("https://example.test/{{tenant}}")
            .unwrap_err()
            .message,
        "Parameter 'tenant' is required"
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(super::request_corpus::async_corpus(
            "request_send_uses_relative_endpoint_and_default_origin",
            17,
        ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn request_admission_checks_status_and_content_type() {
    crate::tests::runtime::run(super::request_corpus::async_corpus(
        "request_admission_checks_status_and_content_type",
        54,
    ));
}
