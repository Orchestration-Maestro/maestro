//! Native credential acquisition through the shared HTTP effect.

use super::super::base_external_client::Effects;
use base64::Engine as _;
use google_cloud_auth::credentials::CacheableResource;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn native_metadata_acquires_and_refreshes_controlled_tokens() {
    super::run(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let fetch: crate::Fetch = Arc::new(move |request| {
            observed.fetch_add(1, Ordering::SeqCst);
            assert_eq!(
                request.url,
                "http://controlled.invalid/computeMetadata/v1/instance/service-accounts/default/token?scopes=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform"
            );
            assert!(
                request
                    .headers
                    .get("metadata-flavor")
                    .is_some_and(|value| value == "Google")
            );
            let mut response = super::response(
                &serde_json::json!({"access_token":"controlled-access","expires_in":3600,"token_type":"Bearer"}),
            );
            response
                .headers
                .insert("metadata-flavor".into(), "Google".into());
            Box::pin(std::future::ready(Ok(response)))
        });
        let effects = Effects {
            fetch,
            clock: Arc::new(|| 1000.0),
            ..Effects::default()
        };
        let credentials = super::metadata::compute("controlled.invalid", effects, false);
        for _ in 0..2 {
            let CacheableResource::New { data, .. } = credentials
                .headers(hyper::http::Extensions::new())
                .await
                .unwrap()
            else {
                panic!("expected generated headers")
            };
            assert!(
                data.get("authorization")
                    .is_some_and(|value| value == "Bearer controlled-access")
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn adc_quota_override_reaches_generated_headers() {
    const TEST: &str = "adc_quota_override_reaches_generated_headers";
    if maestro_test_process::selection().is_child() {
        let expected = std::env::var("EXPECTED_QUOTA").unwrap();
        let client = super::super::super::ApiClient::new(
            "https://content.invalid".into(),
            "v1".into(),
            None,
            indexmap::IndexMap::new(),
        );
        let prepared = super::run(super::prepare(&client));
        assert!(
            prepared
                .request
                .headers
                .get("x-goog-user-project")
                .is_some_and(|value| value == &expected)
        );
        return;
    }
    let server = crate::tests::loopback::serve_routed(|_| crate::tests::loopback::Reply {
        status: 200,
        content_type: "application/json",
        headers: &[],
        body: br#"{"access_token":"controlled-access","expires_in":3600}"#.to_vec(),
        close: false,
    })
    .unwrap();
    for (file, environment, expected) in [
        ("file-one", "environment-two", "environment-two"),
        ("file-two", "environment-one", "environment-one"),
        ("file-one", "", "file-one"),
        ("file-one", " ", ""),
    ] {
        let files = super::Files::new();
        let path = files.write(
            "credentials.json",
            &serde_json::to_vec(&super::workforce(&server.url, file)).unwrap(),
        );
        {
            let output = maestro_test_process::TestProcess::current(TEST)
                .unwrap()
                .nocapture()
                .env_clear()
                .envs(
                    [
                        ("GOOGLE_APPLICATION_CREDENTIALS", path.as_str()),
                        ("GOOGLE_CLOUD_QUOTA_PROJECT", environment),
                        ("EXPECTED_QUOTA", expected),
                    ]
                    .iter()
                    .copied(),
                )
                .case("quota")
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
    }
    server.finish().unwrap();
}

#[test]
fn adc_errors_never_include_credential_material() {
    let server = crate::tests::loopback::serve_routed(|_| crate::tests::loopback::Reply {
        status: 400,
        content_type: "application/json",
        headers: &[],
        body: br#"{"error":"invalid_grant","error_description":"controlled-sensitive-echo"}"#
            .to_vec(),
        close: false,
    })
    .unwrap();
    super::run(async {
        let native = google_cloud_auth::credentials::user_account::Builder::new(serde_json::json!({"type":"authorized_user","client_id":"controlled-id","client_secret":"controlled-secret","refresh_token":"controlled-refresh"})).with_token_uri(&server.url).build().unwrap();
        let owned = super::super::google_auth::from_json(
            super::workforce(&server.url, "quota"),
            Effects::default(),
        )
        .unwrap();
        let metadata = super::metadata::compute(&server.url, Effects::default(), false);
        for credentials in [native, owned, metadata] {
            let client = super::client(credentials, indexmap::IndexMap::new());
            let projected = super::super::super::project_content_request(
                serde_json::json!({"model":"model","contents":"hello"}),
            )
            .unwrap();
            let error = client.prepare_request(projected, None).await.err().unwrap();
            assert!(matches!(
                error.code,
                Some(crate::DiagnosticCode::Number(400.0))
            ));
            for rendered in [
                error.to_string(),
                format!("{error:?}"),
                serde_json::to_string(&error).unwrap(),
            ] {
                assert!(!rendered.contains("controlled-sensitive-echo"));
                assert!(!rendered.contains("controlled-refresh"));
                assert!(!rendered.contains(&server.url));
            }
        }
    });
    server.finish().unwrap();
}

#[test]
fn adc_without_files_falls_through_to_metadata() {
    const TEST: &str = "adc_without_files_falls_through_to_metadata";
    let case = maestro_test_process::selection();
    if case.is_child() {
        let result = super::run(super::super::NodeAuth::default().headers());
        if case.runs_case("metadata") {
            assert_eq!(result.unwrap()["authorization"], "Bearer metadata-access");
        } else {
            assert_eq!(result.err().unwrap().message, super::super::NO_CREDENTIALS);
        }
        return;
    }
    let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let record = requests.clone();
    let server = crate::tests::loopback::serve_routed(move |request| {
        record.lock().unwrap().push(request.request_line.clone());
        crate::tests::loopback::Reply {
            status: 200,
            content_type: "application/json",
            headers: &[("metadata-flavor", "Google")],
            body: br#"{"access_token":"metadata-access","expires_in":3600}"#.to_vec(),
            close: false,
        }
    })
    .unwrap();
    let files = super::Files::new();
    let home = files.path("home");
    for (case, detection, expected) in [("disabled", "none", 0), ("metadata", "assume-present", 1)]
    {
        {
            let output = maestro_test_process::TestProcess::current(TEST)
                .unwrap()
                .nocapture()
                .env_clear()
                .envs(
                    [
                        ("GOOGLE_APPLICATION_CREDENTIALS", ""),
                        ("google_application_credentials", ""),
                        ("HOME", home.as_str()),
                        ("METADATA_SERVER_DETECTION", detection),
                        ("GCE_METADATA_IP", ""),
                        ("GCE_METADATA_HOST", server.url.as_str()),
                    ]
                    .iter()
                    .copied(),
                )
                .case(case)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), expected);
        assert!(requests.iter().all(|line| {
            line.starts_with("GET /computeMetadata/v1/instance/service-accounts/default/token?")
        }));
    }
    server.finish().unwrap();
}

/// A server granting the usual controlled token to every request.
fn token_server() -> crate::tests::loopback::Routed {
    crate::tests::loopback::serve_routed(|_| crate::tests::loopback::Reply {
        status: 200,
        content_type: "application/json",
        headers: &[],
        body: br#"{"access_token":"controlled-access","expires_in":3600}"#.to_vec(),
        close: false,
    })
    .unwrap()
}

/// Write workforce credentials with a distinguishing file quota and return the file path.
fn quota_file(files: &super::Files, name: &str, endpoint: &str, quota: &str) -> String {
    files.write(
        name,
        &serde_json::to_vec(&super::workforce(endpoint, quota)).unwrap(),
    )
}

/// Write the default credential file under a disposable home.
fn credentials_home(files: &super::Files, endpoint: &str) -> String {
    let well_known = files.0.path().join("home").join(".config").join("gcloud");
    std::fs::create_dir_all(&well_known).unwrap();
    std::fs::write(
        well_known.join("application_default_credentials.json"),
        serde_json::to_vec(&super::workforce(endpoint, "well-known")).unwrap(),
    )
    .unwrap();
    files.0.path().join("home").to_str().unwrap().to_owned()
}

#[test]
#[cfg_attr(not(unix), allow(unused_mut))]
fn adc_file_order_prefers_nonempty_explicit_paths() {
    const TEST: &str = "adc_file_order_prefers_nonempty_explicit_paths";
    let expected = maestro_test_process::selection();
    if expected.is_child() {
        let headers = super::run(super::super::NodeAuth::default().headers()).unwrap();
        assert!(expected.runs_case(&headers["x-goog-user-project"]));
        return;
    }
    let server = token_server();
    let files = super::Files::new();
    let upper = quota_file(&files, "upper.json", &server.url, "upper");
    let lower = quota_file(&files, "lower.json", &server.url, "lower");
    let home = credentials_home(&files, &server.url);
    #[cfg(unix)]
    let link = {
        let link = files.0.path().join("link.json");
        std::os::unix::fs::symlink(&upper, &link).unwrap();
        link.to_str().unwrap().to_owned()
    };
    let mut cases = vec![
        (
            "upper",
            vec![
                ("GOOGLE_APPLICATION_CREDENTIALS", upper.as_str()),
                ("google_application_credentials", lower.as_str()),
            ],
        ),
        (
            "lower",
            vec![
                ("GOOGLE_APPLICATION_CREDENTIALS", ""),
                ("google_application_credentials", lower.as_str()),
            ],
        ),
        (
            "well-known",
            vec![
                ("GOOGLE_APPLICATION_CREDENTIALS", ""),
                ("google_application_credentials", ""),
            ],
        ),
    ];
    #[cfg(unix)]
    cases.push((
        "upper",
        vec![("GOOGLE_APPLICATION_CREDENTIALS", link.as_str())],
    ));
    let (home_name, home) = super::home_variable(&home);
    for (expected, mut variables) in cases {
        variables.push((home_name, home.as_str()));
        let output = maestro_test_process::TestProcess::current(TEST)
            .unwrap()
            .nocapture()
            .env_clear()
            .envs(super::settable(&variables))
            .case(expected)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
    server.finish().unwrap();
}

#[test]
fn adc_selected_file_failures_do_not_fall_back() {
    const TEST: &str = "adc_selected_file_failures_do_not_fall_back";
    if maestro_test_process::selection().is_child() {
        let error = super::run(super::super::NodeAuth::default().headers())
            .err()
            .unwrap();
        assert!(error.message.starts_with("Authentication: "));
        assert!(!error.message.contains("controlled"));
        return;
    }
    let server = token_server();
    let files = super::Files::new();
    let valid = quota_file(&files, "valid.json", &server.url, "valid");
    let directory = files.0.path().to_str().unwrap().to_owned();
    let malformed = files.write("malformed.json", b"{not json");
    let unsupported = files.write("unsupported.json", br#"{"type":"controlled-unknown"}"#);
    let invalid = files.write("invalid.json", br#"{"type":"authorized_user"}"#);
    let missing = format!("{directory}/missing.json");
    let host = server.url.trim_start_matches("http://").to_owned();
    for (index, selected) in [missing, directory.clone(), malformed, unsupported, invalid]
        .iter()
        .enumerate()
    {
        {
            let output = maestro_test_process::TestProcess::current(TEST)
                .unwrap()
                .nocapture()
                .env_clear()
                .envs(super::settable(&[
                    ("GOOGLE_APPLICATION_CREDENTIALS", selected.as_str()),
                    ("google_application_credentials", valid.as_str()),
                    ("HOME", directory.as_str()),
                    ("METADATA_SERVER_DETECTION", "assume-present"),
                    ("GCE_METADATA_HOST", host.as_str()),
                ]))
                .case(index.to_string())
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
    }
    server.finish().unwrap();
}

/// Every request a routed server received, in arrival order.
type Log = std::sync::Arc<std::sync::Mutex<Vec<crate::tests::loopback::Received>>>;

/// A server answering `/subject` with a text subject and everything else with a token.
fn recording_server() -> (crate::tests::loopback::Routed, Log) {
    let log = Log::default();
    let record = log.clone();
    let server = crate::tests::loopback::serve_routed(move |request| {
        let subject = request.request_line.contains("/subject");
        record.lock().unwrap().push(crate::tests::loopback::Received {
            request_line: request.request_line.clone(),
            headers: request.headers.clone(),
            body: request.body.clone(),
            head: request.head.clone(),
        });
        crate::tests::loopback::Reply {
            status: 200,
            content_type: if subject { "text/plain" } else { "application/json" },
            headers: &[],
            body: if subject {
                b"subject-url".to_vec()
            } else {
                br#"{"access_token":"controlled-access","expires_in":3600,"issued_token_type":"urn:ietf:params:oauth:token-type:access_token","token_type":"Bearer"}"#.to_vec()
            },
            close: false,
        }
    })
    .unwrap();
    (server, log)
}

/// The decoded fields of the form body of the newest token request.
fn newest_form(log: &Log) -> std::collections::HashMap<String, String> {
    let log = log.lock().unwrap();
    let request = log
        .iter()
        .rev()
        .find(|request| request.request_line.starts_with("POST"))
        .unwrap();
    if request.body.first() == Some(&b'{') {
        let fields: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&request.body).unwrap();
        return fields
            .into_iter()
            .filter_map(|(name, value)| value.as_str().map(|text| (name, text.to_owned())))
            .collect();
    }
    url::form_urlencoded::parse(&request.body)
        .into_owned()
        .collect()
}

/// The executable-sourced script, created through a child process so it is never written and run in-process.
#[cfg(unix)]
fn executable_source(files: &super::Files) -> String {
    let script = files.0.path().join("script.tmp");
    std::fs::write(&script, "#!/bin/sh\nprintf '{\"version\":1,\"success\":true,\"token_type\":\"urn:ietf:params:oauth:token-type:jwt\",\"id_token\":\"subject-exec\",\"expiration_time\":4102444800}'\n").unwrap();
    let program = files.0.path().join("script.sh");
    let status = std::process::Command::new("install")
        .args(["-m", "0700"])
        .arg(&script)
        .arg(&program)
        .status()
        .unwrap();
    assert!(status.success());
    program.to_str().unwrap().to_owned()
}

/// One credential file per supported ordinary and federated form.
fn credential_forms(files: &super::Files, base: &str) -> Vec<(&'static str, serde_json::Value)> {
    let token = format!("{base}/token");
    let audience =
        "//iam.googleapis.com/projects/1/locations/global/workloadIdentityPools/p/providers/q";
    let subject = files.write("subject.json", br#"{"tok":"subject-file"}"#);
    let key = super::client_key_pem();
    let external = |source: serde_json::Value| serde_json::json!({"type":"external_account","audience":audience,"subject_token_type":"urn:ietf:params:oauth:token-type:jwt","token_url":token,"credential_source":source});
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut cases = vec![
        (
            "user",
            serde_json::json!({"type":"authorized_user","client_id":"id","client_secret":"secret","refresh_token":"controlled-refresh","token_uri":token}),
        ),
        (
            "service",
            serde_json::json!({"type":"service_account","client_email":"controlled@example.invalid","private_key":key,"private_key_id":"k","token_uri":token,"project_id":"p"}),
        ),
        (
            "file",
            external(
                serde_json::json!({"file":subject,"format":{"type":"json","subject_token_field_name":"tok"}}),
            ),
        ),
        (
            "url",
            external(
                serde_json::json!({"url":format!("{base}/subject"),"headers":{"x-source":"source-header"},"format":{"type":"text"}}),
            ),
        ),
        (
            "aws",
            external(
                serde_json::json!({"environment_id":"aws1","regional_cred_verification_url":"https://sts.{region}.amazonaws.com?Action=GetCallerIdentity&Version=2011-06-15"}),
            ),
        ),
    ];
    #[cfg(unix)]
    cases.push((
        "exec",
        external(
            serde_json::json!({"executable":{"command":executable_source(files),"timeout_millis":5000}}),
        ),
    ));
    cases
}

/// What the native or owned builder sent to the controlled endpoints for one form.
fn assert_form_requests(name: &str, log: &Log) {
    let form = newest_form(log);
    match name {
        "user" => {
            assert_eq!(form["refresh_token"], "controlled-refresh");
            assert!(!form.contains_key("scopes"));
        }
        "aws" => {
            assert_eq!(form["scope"], super::super::CLOUD_SCOPE);
            assert!(form["subject_token"].contains("GetCallerIdentity"));
        }
        _ => {
            assert_eq!(form["scope"], super::super::CLOUD_SCOPE);
            let expected = match name {
                "file" => "subject-file",
                "url" => "subject-url",
                _ => "subject-exec",
            };
            assert_eq!(form["subject_token"], expected);
        }
    }
    if name == "url" {
        let log = log.lock().unwrap();
        let fetch = log
            .iter()
            .find(|r| r.request_line.contains("/subject"))
            .unwrap();
        assert_eq!(fetch.header("x-source"), Some("source-header"));
    }
}

/// A self-signed service-account token names the cloud-platform scope.
fn assert_service_scope(authorization: &str) {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let jwt = authorization.strip_prefix("Bearer ").unwrap();
    let claims: serde_json::Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(jwt.split('.').nth(1).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(claims["scope"], super::super::CLOUD_SCOPE);
}

#[test]
fn adc_ordinary_and_external_sources_reach_native_builders() {
    const TEST: &str = "adc_ordinary_and_external_sources_reach_native_builders";
    let case = maestro_test_process::selection();
    if case.is_child() {
        let headers = super::run(super::super::NodeAuth::default().headers()).unwrap();
        if case.runs_case("service") {
            assert_service_scope(&headers["authorization"]);
        } else {
            assert_eq!(headers["authorization"], "Bearer controlled-access");
        }
        return;
    }
    let (server, log) = recording_server();
    let files = super::Files::new();
    for (name, value) in credential_forms(&files, &server.url) {
        let path = files.write(&format!("{name}.json"), value.to_string().as_bytes());
        {
            let output = maestro_test_process::TestProcess::current(TEST)
                .unwrap()
                .nocapture()
                .env_clear()
                .envs(
                    [
                        ("GOOGLE_APPLICATION_CREDENTIALS", path.as_str()),
                        ("GOOGLE_EXTERNAL_ACCOUNT_ALLOW_EXECUTABLES", "1"),
                        ("AWS_ACCESS_KEY_ID", "controlled-id"),
                        ("AWS_SECRET_ACCESS_KEY", "controlled-secret"),
                        ("AWS_REGION", "us-east-1"),
                    ]
                    .iter()
                    .copied(),
                )
                .case(name)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
        if name != "service" {
            assert_form_requests(name, &log);
        }
    }
    server.finish().unwrap();
}

#[test]
fn native_sources_preserve_proxy_and_nested_transport_configuration() {
    const TEST: &str = "native_sources_preserve_proxy_and_nested_transport_configuration";
    if maestro_test_process::selection().is_child() {
        let headers = super::run(super::super::NodeAuth::default().headers()).unwrap();
        assert_eq!(headers["authorization"], "Bearer controlled-access");
        return;
    }
    let (proxy, log) = recording_server();
    let files = super::Files::new();
    let endpoint = "http://token.invalid/token";
    let user = serde_json::json!({"type":"authorized_user","client_id":"id","client_secret":"secret","refresh_token":"controlled-refresh","token_uri":endpoint});
    let owned = super::workforce(endpoint, "quota");
    for (name, value) in [("native", user), ("owned", owned)] {
        let path = files.write(&format!("{name}.json"), value.to_string().as_bytes());
        {
            let output = maestro_test_process::TestProcess::current(TEST)
                .unwrap()
                .nocapture()
                .env_clear()
                .envs(
                    [
                        ("GOOGLE_APPLICATION_CREDENTIALS", path.as_str()),
                        ("HTTP_PROXY", proxy.url.as_str()),
                        ("http_proxy", proxy.url.as_str()),
                    ]
                    .iter()
                    .copied(),
                )
                .case(name)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
        let log = log.lock().unwrap();
        assert_eq!(
            log.last().unwrap().request_line,
            "POST http://token.invalid/token HTTP/1.1"
        );
    }
    proxy.finish().unwrap();
}

/// Impersonation URL for a fixed target ending in `suffix`.
fn principal(suffix: &str) -> String {
    format!(
        "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/target@example.invalid{suffix}"
    )
}

/// An outer impersonated file over `source` with the given impersonation URL.
fn outer(url: &str, source: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({"type":"impersonated_service_account","service_account_impersonation_url":url,"source_credentials":source,"delegates":["d@example.invalid"],"quota_project_id":"outer"})
}

/// Principal extraction rejects URLs without a target and over-long values.
fn assert_principal_rules(source: &serde_json::Value) {
    let certificate = serde_json::json!({"type":"external_account","audience":"//a","subject_token_type":"t","credential_source":{"certificate":{"use_default_certificate_config":true}}});
    for (url, source, accepted) in [
        (principal(":generateIdToken"), &certificate, true),
        (principal(""), source, false),
        (principal(":other"), source, false),
        (
            format!("https://x/{}:generateAccessToken", "a".repeat(260)),
            source,
            false,
        ),
        (
            "https://x/serviceAccounts/:generateAccessToken".to_owned(),
            source,
            false,
        ),
    ] {
        let result = super::super::google_auth::from_json(outer(&url, source), Effects::default());
        assert_eq!(result.is_ok(), accepted, "{url}");
    }
    let missing = serde_json::json!({"type":"impersonated_service_account","service_account_impersonation_url":principal(":generateAccessToken")});
    assert!(super::super::google_auth::from_json(missing, Effects::default()).is_err());
}

/// Validate which credential kinds may supply nested impersonation.
fn assert_impersonation_sources(token: &str, user: &serde_json::Value) {
    let sources = [
        (user.clone(), true),
        (super::workforce(token, "source-quota"), true),
        (
            serde_json::json!({"type":"service_account","client_email":"s@example.invalid","private_key":"k","token_uri":token}),
            false,
        ),
    ];
    for (source, accepted) in &sources {
        let result = super::super::google_auth::from_json(
            outer(&principal(":generateAccessToken"), source),
            Effects::default(),
        );
        assert_eq!(result.is_ok(), *accepted);
    }
}

#[test]
fn nested_impersonation_uses_the_retained_source_credentials() {
    const TEST: &str = "nested_impersonation_uses_the_retained_source_credentials";
    if maestro_test_process::selection().is_child() {
        let error = super::run(super::super::NodeAuth::default().headers())
            .err()
            .unwrap();
        assert!(error.message.starts_with("Authentication: "));
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = runtime.enter();
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = hits.clone();
    let failing = crate::tests::loopback::serve_routed(move |_| {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        crate::tests::loopback::Reply {
            status: 400,
            content_type: "application/json",
            headers: &[],
            body: b"{}".to_vec(),
            close: false,
        }
    })
    .unwrap();
    let files = super::Files::new();
    let token = format!("{}/token", failing.url);
    let user = serde_json::json!({"type":"authorized_user","client_id":"id","client_secret":"secret","refresh_token":"controlled-refresh","token_uri":token});
    assert_impersonation_sources(&token, &user);
    let path = files.write(
        "outer.json",
        outer(&principal(":generateAccessToken"), &user)
            .to_string()
            .as_bytes(),
    );
    let variables = [
        ("GOOGLE_APPLICATION_CREDENTIALS", path.as_str()),
        ("HOME", "/nonexistent"),
    ];
    {
        let output = maestro_test_process::TestProcess::current(TEST)
            .unwrap()
            .nocapture()
            .env_clear()
            .envs(variables.iter().copied())
            .case("source")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    };
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_principal_rules(&user);
    failing.finish().unwrap();
}
