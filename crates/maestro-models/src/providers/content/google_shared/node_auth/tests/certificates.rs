//! Certificate configuration and ordered subject-token material.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use std::fmt::Write as _;

use super::super::certificate_subject_token_supplier::CertificateSubjectTokenSupplier;

const CLIENT: &[u8] =
    include_bytes!("../../../../../../tests/fixtures/cloud-authentication/client.pem");
const CA: &[u8] = include_bytes!("../../../../../../tests/fixtures/cloud-authentication/ca.pem");
/// The authority of the controlled server's certificate, separate from the client authority.
const SERVER_AUTHORITY: &[u8] =
    include_bytes!("../../../../../../tests/fixtures/cloud-authentication/server-ca.pem");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainRow {
    case: String,
    expected: ChainExpected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainExpected {
    result: String,
    chain: Vec<String>,
}

#[test]
fn certificate_chain_prepends_or_retains_the_leaf() {
    let rows: Vec<ChainRow> = serde_json::from_str(include_str!(
        "../../../../../../tests/fixtures/cloud_authentication.json"
    ))
    .unwrap();
    for row in rows {
        let files = super::Files::new();
        let cert = files.write("client.pem", CLIENT);
        let key = files.write("key.pem", super::client_key_pem().as_bytes());
        let config = files.write(
            "certificate.json",
            &serde_json::to_vec(
                &serde_json::json!({"cert_configs":{"workload":{"cert_path":cert,"key_path":key}}}),
            )
            .unwrap(),
        );
        let chain = match row.case.as_str() {
            "certificate:explicit-config" => None,
            "certificate:chain-prepends-leaf" => Some(CA.to_vec()),
            "certificate:chain-keeps-first-leaf" => Some([CLIENT, CA].concat()),
            "certificate:chain-keeps-duplicates" => Some([CLIENT, CA, CA].concat()),
            "certificate:chain-without-certificates" => Some(b"no certificates here".to_vec()),
            _ => panic!("unknown certificate row"),
        };
        let chain_path = chain.map(|bytes| files.write("chain.pem", &bytes));
        let supplier = CertificateSubjectTokenSupplier::new(&serde_json::json!({"certificate_config_location":config,"trust_chain_path":chain_path})).unwrap();
        let material = supplier.material().unwrap();
        let chain: Vec<String> = serde_json::from_str(&material.subject).unwrap();
        let hashes: Vec<String> = chain
            .iter()
            .map(|cert| {
                Sha256::digest(STANDARD.decode(cert).unwrap()).iter().fold(
                    String::new(),
                    |mut text, byte| {
                        write!(&mut text, "{byte:02x}").unwrap();
                        text
                    },
                )
            })
            .collect();
        assert_eq!(row.expected.result, "success");
        assert_eq!(hashes, row.expected.chain);
    }
}

/// Identity-bearing real client with only the controlled server authority added to trust.
fn trusted_client(
    identity: reqwest::Identity,
) -> Result<crate::Fetch, google_cloud_auth::errors::CredentialsError> {
    let root = reqwest::Certificate::from_pem(SERVER_AUTHORITY)
        .map_err(|_| super::super::base_external_client::invalid_token())?;
    let client = super::super::base_external_client::identity_client(identity)
        .add_root_certificate(root)
        .build()
        .map_err(|_| super::super::base_external_client::invalid_token())?;
    Ok(crate::providers::http::supplied_fetch(client))
}

/// Build a server that verifies the controlled cloud-authentication client certificate.
fn mutual_server_config() -> Result<std::sync::Arc<rustls::ServerConfig>, Box<dyn std::error::Error>>
{
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
    let ca = CertificateDer::from_pem_slice(include_bytes!(
        "../../../../../../tests/fixtures/cloud-authentication/ca.pem"
    ))?;
    let cert = CertificateDer::from_pem_slice(include_bytes!(
        "../../../../../../tests/fixtures/cloud-authentication/server.pem"
    ))?;
    let key = PrivateKeyDer::try_from(
        include_bytes!("../../../../../../tests/fixtures/cloud-authentication/server-key.der")
            .to_vec(),
    )?;
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca)?;
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        std::sync::Arc::new(roots),
        std::sync::Arc::new(rustls::crypto::aws_lc_rs::default_provider()),
    )
    .build()?;
    let config = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_client_cert_verifier(verifier)
    .with_single_cert(vec![cert], key)?;
    Ok(std::sync::Arc::new(config))
}

/// Write a selected certificate configuration backed by the controlled client identity.
fn configuration(files: &super::Files) -> serde_json::Value {
    let cert = files.write("client.pem", CLIENT);
    let key = files.write("client-key.pem", super::client_key_pem().as_bytes());
    let config = files.write(
        "certificate.json",
        &serde_json::to_vec(
            &serde_json::json!({"cert_configs":{"workload":{"cert_path":cert,"key_path":key}}}),
        )
        .unwrap(),
    );
    serde_json::json!({"certificate_config_location":config})
}

#[test]
fn certificate_exchange_proves_mutual_tls_and_content_header_handoff() {
    use std::sync::Arc;
    let (server, received) = tls_server(granted);
    let files = super::Files::new();
    let source = configuration(&files);
    let value = serde_json::json!({"type":"external_account","audience":"//iam.googleapis.com/projects/controlled/locations/global/workloadIdentityPools/pool/providers/provider","subject_token_type":"urn:ietf:params:oauth:token-type:mtls","token_url":server.url.replacen("http:","https:",1)+"/token","quota_project_id":"certificate-quota","credential_source":{"certificate":source}});
    let effects = super::super::base_external_client::Effects {
        fetch: crate::default_fetch(),
        clock: Arc::new(|| 1000.0),
        identity: trusted_client,
    };
    let credentials = super::super::google_auth::from_json(value, effects).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = super::client(credentials, indexmap::IndexMap::new());
    let prepared = runtime.block_on(super::prepare(&client));
    assert!(
        prepared
            .request
            .headers
            .get("authorization")
            .is_some_and(|value| value == "Bearer controlled-access")
    );
    assert!(
        prepared
            .request
            .headers
            .get("x-goog-user-project")
            .is_some_and(|value| value == "certificate-quota")
    );
    server.finish().unwrap();
    let request = received.recv().unwrap();
    assert_eq!(request.request_line, "POST /token HTTP/1.1");
    let fields: Vec<_> = url::form_urlencoded::parse(&request.body)
        .into_owned()
        .collect();
    assert_eq!(
        fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [
            "grant_type",
            "audience",
            "scope",
            "requested_token_type",
            "subject_token",
            "subject_token_type"
        ]
    );
    assert!(
        fields
            .iter()
            .any(|(name, value)| name == "scope" && value == super::super::CLOUD_SCOPE)
    );
    assert!(request.header("authorization").is_none());
    assert!(request.header("x-goog-user-project").is_none());
}

/// Serve certificate-authenticated exchanges; `reply` maps the exchange index to a status and body.
pub(super) fn tls_server(
    reply: impl Fn(usize) -> (u16, Vec<u8>) + Send + Sync + 'static,
) -> (
    crate::tests::loopback::Routed,
    std::sync::mpsc::Receiver<crate::tests::loopback::Received>,
) {
    use std::sync::Arc;
    let (sent, received) = std::sync::mpsc::channel();
    let sent = std::sync::Mutex::new(sent);
    let config = mutual_server_config().unwrap();
    let count = std::sync::atomic::AtomicUsize::new(0);
    let server = crate::tests::loopback::start_routed(Arc::new(move |socket| {
        let connection =
            rustls::ServerConnection::new(config.clone()).map_err(std::io::Error::other)?;
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let request = crate::tests::loopback::read_request(&mut stream)?
            .ok_or_else(|| std::io::Error::other("missing request"))?;
        assert!(
            stream
                .conn
                .peer_certificates()
                .is_some_and(|certs| !certs.is_empty())
        );
        sent.lock().unwrap().send(request).unwrap();
        let (status, body) = reply(count.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        crate::tests::loopback::answer_connection_reply(
            &mut stream,
            &crate::tests::loopback::Reply {
                status,
                content_type: "application/json",
                headers: &[],
                body,
                close: false,
            },
        )
    }))
    .unwrap();
    (server, received)
}

/// The usual successful token body.
pub(super) fn granted(_: usize) -> (u16, Vec<u8>) {
    (
        200,
        br#"{"access_token":"controlled-access","expires_in":3600}"#.to_vec(),
    )
}

/// Certificate-source credentials against `server`, with a controlled clock.
pub(super) fn credentials(
    server: &crate::tests::loopback::Routed,
    files: &super::Files,
    extra: &serde_json::Value,
    clock: std::sync::Arc<dyn Fn() -> f64 + Send + Sync>,
) -> google_cloud_auth::credentials::Credentials {
    let mut value = serde_json::json!({"type":"external_account","audience":"//iam.googleapis.com/projects/controlled/locations/global/workloadIdentityPools/pool/providers/provider","subject_token_type":"urn:ietf:params:oauth:token-type:mtls","token_url":server.url.replacen("http:","https:",1)+"/token","credential_source":{"certificate":configuration(files)}});
    value
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let effects = super::super::base_external_client::Effects {
        fetch: crate::default_fetch(),
        clock,
        identity: trusted_client,
    };
    super::super::google_auth::from_json(value, effects).unwrap()
}

/// Certificate material failures never echo file paths or PEM content.
fn assert_safe(error: &google_cloud_auth::errors::CredentialsError) {
    let text = format!("{error} {error:?}");
    assert!(!text.contains("BEGIN") && !text.contains("maestro-cloud"));
}

/// Copy a valid configuration under `name` at the platform default layout; returns the root and gcloud directory.
fn default_layout(files: &super::Files, location: &str, name: &str) -> (String, String) {
    let dir = files.0.path().join(name).join(".config").join("gcloud");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(location, dir.join("certificate_config.json")).unwrap();
    let text = |path: &std::path::Path| path.to_str().unwrap().to_owned();
    (text(&files.0.path().join(name)), text(&dir))
}

#[test]
fn certificate_configuration_uses_explicit_environment_then_default() {
    const TEST: &str = "certificate_configuration_uses_explicit_environment_then_default";
    let case = maestro_test_process::selection();
    if case.is_child() {
        let source: serde_json::Value =
            serde_json::from_str(&std::env::var("SOURCE").unwrap()).unwrap();
        let outcome = CertificateSubjectTokenSupplier::new(&source)
            .and_then(|supplier| supplier.material().map(|_| ()));
        assert_eq!(outcome.is_ok(), case.runs_case("ok"));
        return;
    }
    check_configuration_precedence(TEST);
}

/// Run every explicit and default certificate-source precedence case.
fn check_configuration_precedence(test: &str) {
    let files = super::Files::new();
    let location = configuration(&files)["certificate_config_location"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, sdk_config) = default_layout(&files, &location, "sdk");
    let (home_name, home) = super::home_variable(&default_layout(&files, &location, "home").0);
    let explicit = serde_json::json!({"certificate_config_location":location}).to_string();
    let default = serde_json::json!({"use_default_certificate_config":true}).to_string();
    let missing = format!("{sdk_config}/missing.json");
    for (case, source, mut variables) in [
        (
            "ok",
            &explicit,
            vec![
                ("GOOGLE_API_CERTIFICATE_CONFIG", missing.as_str()),
                ("CLOUDSDK_CONFIG", "/nonexistent"),
            ],
        ),
        (
            "ok",
            &default,
            vec![
                ("GOOGLE_API_CERTIFICATE_CONFIG", location.as_str()),
                ("CLOUDSDK_CONFIG", "/nonexistent"),
            ],
        ),
        (
            "ok",
            &default,
            vec![
                ("CLOUDSDK_CONFIG", sdk_config.as_str()),
                ("HOME", "/nonexistent"),
            ],
        ),
        ("ok", &default, vec![(home_name, home.as_str())]),
        (
            "err",
            &default,
            vec![
                ("GOOGLE_API_CERTIFICATE_CONFIG", missing.as_str()),
                ("CLOUDSDK_CONFIG", sdk_config.as_str()),
                (home_name, home.as_str()),
            ],
        ),
        ("err", &default, vec![("HOME", "/nonexistent")]),
    ] {
        variables.push(("SOURCE", source.as_str()));
        {
            let output = maestro_test_process::TestProcess::current(test)
                .unwrap()
                .nocapture()
                .env_clear()
                .envs(variables.iter().copied())
                .case(case)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
    }
}

#[test]
fn certificate_configuration_rejects_missing_or_conflicting_sources() {
    let files = super::Files::new();
    let location = configuration(&files)["certificate_config_location"].clone();
    for source in [
        serde_json::json!({"certificate_config_location":location,"use_default_certificate_config":true}),
        serde_json::json!({}),
        serde_json::json!({"certificate_config_location":""}),
        serde_json::json!({"use_default_certificate_config":false}),
    ] {
        assert!(CertificateSubjectTokenSupplier::new(&source).is_err());
    }
    let broken = [
        b"not json".to_vec(),
        br#"{"cert_configs":{}}"#.to_vec(),
        br#"{"cert_configs":{"workload":{"key_path":"k"}}}"#.to_vec(),
        br#"{"cert_configs":{"workload":{"cert_path":"c"}}}"#.to_vec(),
        br#"{"cert_configs":{"workload":{"cert_path":"/missing-c","key_path":"/missing-k"}}}"#
            .to_vec(),
    ];
    for (index, config) in broken.iter().enumerate() {
        let path = files.write(&format!("broken-{index}.json"), config);
        let supplier = CertificateSubjectTokenSupplier::new(
            &serde_json::json!({"certificate_config_location":path}),
        )
        .unwrap();
        assert_safe(&supplier.material().err().unwrap());
    }
    let missing = CertificateSubjectTokenSupplier::new(
        &serde_json::json!({"certificate_config_location":files.0.path().join("absent.json").to_str().unwrap()}),
    )
    .unwrap();
    assert!(missing.material().is_err());
}

#[test]
fn certificate_chain_rejects_a_late_leaf_or_invalid_material() {
    let key_pem = super::client_key_pem();
    let garbage = b"-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n";
    for (index, (cert, key, chain)) in [
        (
            CLIENT.to_vec(),
            key_pem.as_bytes().to_vec(),
            Some([CA, CLIENT].concat()),
        ),
        (
            CLIENT.to_vec(),
            key_pem.as_bytes().to_vec(),
            Some(garbage.to_vec()),
        ),
        (garbage.to_vec(), key_pem.as_bytes().to_vec(), None),
        (CLIENT.to_vec(), b"no key".to_vec(), None),
        (
            b"no certificate".to_vec(),
            key_pem.as_bytes().to_vec(),
            None,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let files = super::Files::new();
        let cert = files.write("cert.pem", &cert);
        let key = files.write("key.pem", &key);
        let config = files.write(
            "config.json",
            serde_json::json!({"cert_configs":{"workload":{"cert_path":cert,"key_path":key}}})
                .to_string()
                .as_bytes(),
        );
        let chain = chain.map(|bytes| files.write("chain.pem", &bytes));
        let supplier = CertificateSubjectTokenSupplier::new(
            &serde_json::json!({"certificate_config_location":config,"trust_chain_path":chain}),
        )
        .unwrap();
        let error = supplier
            .material()
            .err()
            .unwrap_or_else(|| panic!("case {index} accepted"));
        assert_safe(&error);
    }
}

#[test]
fn certificate_exchange_preserves_workforce_and_client_auth_options() {
    use super::super::sts_credentials::StsCredentials;
    super::run(async {
        let workforce =
            "//iam.googleapis.com/locations/global/workforcePools/pool/providers/provider";
        for (extra, audience, options, authorization) in [
            (
                serde_json::json!({"workforce_pool_user_project":"project"}),
                workforce,
                Some(r#"{"userProject":"project"}"#),
                false,
            ),
            (
                serde_json::json!({"workforce_pool_user_project":"project","client_id":"id","client_secret":"secret"}),
                workforce,
                None,
                true,
            ),
            (
                serde_json::json!({"client_id":"id","client_secret":"secret","credential_source":{"headers":{"x-source":"v"},"format":{"type":"json"}},"transporterOptions":{"x":1}}),
                "//audience",
                None,
                true,
            ),
        ] {
            let mut value = serde_json::json!({"audience":audience,"subject_token_type":"type","token_url":"https://sts.invalid/token"});
            value
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let (send, seen) =
                super::refresh::script(vec![super::refresh::grant("controlled-access", 3600)]);
            let sts = StsCredentials::new(&value).unwrap();
            sts.exchange("subject", send).await.unwrap();
            let seen = seen.lock().unwrap();
            let fields: std::collections::HashMap<_, _> =
                url::form_urlencoded::parse(&seen[0].body)
                    .into_owned()
                    .collect();
            assert_eq!(fields.get("options").map(String::as_str), options);
            assert_eq!(seen[0].headers.contains_key("authorization"), authorization);
            assert!(!seen[0].headers.contains_key("x-source"));
        }
        let rejected = serde_json::json!({"audience":"//other","subject_token_type":"type","workforce_pool_user_project":"p"});
        assert!(StsCredentials::new(&rejected).is_err());
    });
}

#[test]
fn certificate_impersonation_preserves_scope_lifetime_and_final_token() {
    super::run(async {
        for (extra, lifetime) in [
            (serde_json::json!({}), "3600s"),
            (
                serde_json::json!({"service_account_impersonation":{"token_lifetime_seconds":null}}),
                "3600s",
            ),
            (
                serde_json::json!({"service_account_impersonation":{"token_lifetime_seconds":0}}),
                "3600s",
            ),
            (
                serde_json::json!({"service_account_impersonation":{"token_lifetime_seconds":600}}),
                "600s",
            ),
            (
                serde_json::json!({"service_account_impersonation":{"token_lifetime_seconds":1.5}}),
                "1.5s",
            ),
        ] {
            let (server, received) = tls_server(impersonation_reply);
            let files = super::Files::new();
            let mut extra = extra;
            extra["service_account_impersonation_url"] =
                (server.url.replacen("http:", "https:", 1) + "/impersonate").into();
            extra["quota_project_id"] = "quota".into();
            let credentials = credentials(&server, &files, &extra, std::sync::Arc::new(|| 1000.0));
            let client = super::client(credentials, indexmap::IndexMap::new());
            let prepared = super::prepare(&client).await;
            assert_eq!(
                prepared.request.headers["authorization"],
                "Bearer final-token"
            );
            assert_eq!(prepared.request.headers["x-goog-user-project"], "quota");
            server.finish().unwrap();
            let requests: Vec<_> = received.try_iter().collect();
            assert_eq!(requests[1].request_line, "POST /impersonate HTTP/1.1");
            assert_eq!(
                requests[1].header("authorization"),
                Some("Bearer controlled-access")
            );
            let body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
            assert_eq!(
                body,
                serde_json::json!({"scope":[super::super::CLOUD_SCOPE],"lifetime":lifetime})
            );
        }
    });
}

/// The certificate exchange first, then the impersonated final token.
fn impersonation_reply(index: usize) -> (u16, Vec<u8>) {
    if index == 0 {
        return granted(index);
    }
    (
        200,
        br#"{"accessToken":"final-token","expireTime":"2099-01-01T00:00:00Z"}"#.to_vec(),
    )
}
