//! Private request preparation and one-attempt sending.

use super::super::request;

#[test]
fn request_hooks_keep_phase_and_response_hook_is_unused() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut options = crate::MistralOptions::default();
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

#[test]
fn request_send_resolves_header_case_and_affinity() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(super::request_corpus::async_corpus(
            "request_send_resolves_header_case_and_affinity",
            31,
        ));
}

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

#[test]
fn request_failure_keeps_status_body_and_fallback() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(super::request_corpus::async_corpus(
            "request_failure_keeps_status_body_and_fallback",
            45,
        ));
}

#[test]
fn request_rejects_nonfinite_raw_numbers_without_null_sanitizing() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(super::request_corpus::async_corpus(
            "request_rejects_nonfinite_raw_numbers_without_null_sanitizing",
            15,
        ));
}

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

#[test]
fn request_admission_checks_status_and_content_type() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(super::request_corpus::async_corpus(
            "request_admission_checks_status_and_content_type",
            54,
        ));
}
