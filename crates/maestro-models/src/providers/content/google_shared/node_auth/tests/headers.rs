//! Authentication choice and header preparation.

use super::super::super::{ApiClient, project_content_request};
use indexmap::IndexMap;

#[test]
fn api_key_preparation_never_loads_native_credentials() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for key in ["", "controlled-key"] {
            let client = ApiClient::new(
                "https://content.invalid".into(),
                "v1".into(),
                Some(key.into()),
                IndexMap::new(),
            );
            let projected =
                project_content_request(serde_json::json!({"model":"model", "contents":"hello"}))
                    .unwrap();
            let prepared = client.prepare_request(projected, None).await.unwrap();
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

#[test]
fn native_headers_retain_authored_authorization_and_quota() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for authored in [serde_json::json!({}), serde_json::json!({"Authorization":"authored"}), serde_json::json!({"X-Goog-User-Project":"authored-quota"}), serde_json::json!({"AUTHORIZATION":"authored", "x-goog-user-project":"authored-quota"}), serde_json::json!({"Authorization":"", "x-goog-user-project":""})] {
            let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = calls.clone();
            let fetch: crate::Fetch = std::sync::Arc::new(move |request| {
                observed.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                assert_eq!(request.url,"https://auth.invalid/token");
                assert!(!request.headers.contains_key("x-goog-user-project"));
                Box::pin(std::future::ready(Ok(super::response(&serde_json::json!({"access_token":"controlled-access","expires_in":3600})))))
            });
            let value = serde_json::json!({"type":"external_account_authorized_user","client_id":"controlled-id","client_secret":"controlled-secret","refresh_token":"controlled-refresh","token_url":"https://auth.invalid/token","quota_project_id":"file-quota"});
            let effects = super::super::base_external_client::Effects {fetch,clock: std::sync::Arc::new(|| 1000.0), ..super::super::base_external_client::Effects::default()};
            let credentials = super::super::google_auth::from_json(value,effects).unwrap();
            let client = ApiClient {
                base_url: "https://content.invalid".into(),api_version:"v1".into(),
                authentication: super::super::super::api_client::Authentication::Native(super::super::NodeAuth {credentials: tokio::sync::OnceCell::new_with(Some(credentials))}),
                headers: serde_json::from_value(authored.clone()).unwrap(),
            };
            let projection = project_content_request(serde_json::json!({"model":"model","contents":"hello"})).unwrap();
            let prepared = client.prepare_request(projection,None).await.unwrap();
            let expected_auth = authored.as_object().unwrap().iter().find(|(name,_)| name.eq_ignore_ascii_case("authorization")).map_or("Bearer controlled-access",|(_,value)| value.as_str().unwrap());
            let expected_quota = authored.as_object().unwrap().iter().find(|(name,_)| name.eq_ignore_ascii_case("x-goog-user-project")).map_or("file-quota",|(_,value)| value.as_str().unwrap());
            assert!(prepared.request.headers.get("authorization").is_some_and(|value| value == expected_auth));
            assert!(prepared.request.headers.get("x-goog-user-project").is_some_and(|value| value == expected_quota));
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst),1);
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
