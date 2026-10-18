//! Authentication choice and header preparation.

use super::super::super::{ApiClient, project_content_request};
use indexmap::IndexMap;

#[test]
fn api_key_preparation_never_loads_native_credentials() {
    super::run(async {
        for key in ["", "controlled-key"] {
            let client = ApiClient::new(
                "https://content.invalid".into(),
                "v1".into(),
                Some(key.into()),
                IndexMap::new(),
            );
            let prepared = super::prepare(&client).await;
            assert!(
                prepared
                    .request
                    .headers
                    .get("x-goog-api-key")
                    .is_some_and(|value| value == key)
            );
        }
    });
}

/// The authored header named `name` in any case, else the generated `fallback`.
fn authored_or<'a>(authored: &'a serde_json::Value, name: &str, fallback: &'a str) -> &'a str {
    authored
        .as_object()
        .unwrap()
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map_or(fallback, |(_, value)| value.as_str().unwrap())
}

#[test]
fn native_headers_retain_authored_authorization_and_quota() {
    super::run(async {
        for authored in [
            serde_json::json!({}),
            serde_json::json!({"Authorization":"authored"}),
            serde_json::json!({"X-Goog-User-Project":"authored-quota"}),
            serde_json::json!({"AUTHORIZATION":"authored", "x-goog-user-project":"authored-quota"}),
            serde_json::json!({"Authorization":"", "x-goog-user-project":""}),
        ] {
            let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = calls.clone();
            let fetch: crate::Fetch = std::sync::Arc::new(move |request| {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(request.url, "https://auth.invalid/token");
                assert!(!request.headers.contains_key("x-goog-user-project"));
                Box::pin(std::future::ready(Ok(super::response(
                    &serde_json::json!({"access_token":"controlled-access","expires_in":3600}),
                ))))
            });
            let effects = super::super::base_external_client::Effects {
                fetch,
                clock: std::sync::Arc::new(|| 1000.0),
                ..super::super::base_external_client::Effects::default()
            };
            let credentials = super::super::google_auth::from_json(
                super::workforce("https://auth.invalid/token", "file-quota"),
                effects,
            )
            .unwrap();
            let client = super::client(
                credentials,
                serde_json::from_value(authored.clone()).unwrap(),
            );
            let prepared = super::prepare(&client).await;
            let headers = &prepared.request.headers;
            assert_eq!(
                headers["authorization"],
                authored_or(&authored, "authorization", "Bearer controlled-access")
            );
            assert_eq!(
                headers["x-goog-user-project"],
                authored_or(&authored, "x-goog-user-project", "file-quota")
            );
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        }
    });
}

#[test]
fn native_acquisition_is_retained_across_prepared_requests() {
    super::run(async {
        let clock = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1000));
        let (send, seen) = super::refresh::script(vec![super::refresh::grant("shared", 3600)]);
        let credentials = super::refresh::workforce(super::refresh::effects(send, &clock));
        let client = super::client(credentials, IndexMap::new());
        for _ in 0..2 {
            let prepared = super::prepare(&client).await;
            assert_eq!(prepared.request.headers["authorization"], "Bearer shared");
        }
        assert_eq!(seen.lock().unwrap().len(), 1);
    });
}

#[test]
fn auth_preparation_fails_after_url_and_header_validation() {
    super::run(async {
        let clock = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1000));
        let (send, seen) = super::refresh::script(vec![super::refresh::grant("shared", 3600)]);
        let credentials = super::refresh::workforce(super::refresh::effects(send, &clock));
        let client = super::client(credentials, IndexMap::new());
        let projected = || {
            project_content_request(serde_json::json!({"model":"model","contents":"hello"}))
                .unwrap()
        };
        for options in [
            serde_json::json!({"baseUrl":"not a url"}),
            serde_json::json!({"baseUrl":7}),
            serde_json::json!({"headers":{"bad name":"v"}}),
            serde_json::json!({"headers":{"x":1}}),
        ] {
            assert!(
                client
                    .prepare_request(projected(), Some(&options))
                    .await
                    .is_err()
            );
            assert!(seen.lock().unwrap().is_empty());
        }
        let options = serde_json::json!({"headers":{"X-Extra":"kept"}});
        let prepared = client
            .prepare_request(projected(), Some(&options))
            .await
            .unwrap();
        assert_eq!(prepared.request.headers["authorization"], "Bearer shared");
        assert_eq!(prepared.request.headers["x-extra"], "kept");
        assert_eq!(seen.lock().unwrap().len(), 1);
    });
}
