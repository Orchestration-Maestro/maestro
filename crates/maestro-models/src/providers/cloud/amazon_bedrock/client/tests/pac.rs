//! PAC lifetime through the SDK's timeout-keyed connector cache.

use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use aws_smithy_runtime_api::client::http::{HttpClient, HttpConnector, HttpConnectorSettings};
use aws_smithy_runtime_api::client::runtime_components::RuntimeComponentsBuilder;
use aws_smithy_types::body::SdkBody;
use http_body_util::BodyExt;

use super::super::{Auth, Settings, WireRequest, bridge, transport};
use crate::providers::cloud::amazon_bedrock::proxy::Environment;
use crate::tests::loopback;

#[test]
fn pac_bridge_retains_globals_across_protocols_and_timeouts() {
    crate::tests::runtime::run(async {
        let (sent, received) = mpsc::channel();
        let server = loopback::serve_routed(move |request| {
            sent.send((
                request.request_line.clone(),
                request.header("proxy-authorization").unwrap().to_owned(),
            ))
            .unwrap();
            loopback::Reply {
                status: if request.request_line.starts_with("CONNECT ") {
                    407
                } else {
                    200
                },
                content_type: "text/plain",
                headers: &[],
                body: b"observed".to_vec(),
                close: false,
            }
        })
        .unwrap();
        let script = format!(
            "let calls=0;function FindProxyForURL(){{return 'PROXY '+(++calls)+'@{}'}}",
            &server.url[7..]
        );
        let encoded =
            percent_encoding::utf8_percent_encode(&script, percent_encoding::NON_ALPHANUMERIC);
        let source = format!("pac+data:,{encoded}");
        for protocols in [&["http", "https", "http"][..], &["https"][..]] {
            consume(&source, protocols).await;
        }
        server.finish().unwrap();
        assert_eq!(
            received.try_iter().collect::<Vec<_>>(),
            [
                (
                    "GET http://destination.invalid:8081/credentials HTTP/1.1".into(),
                    "Basic MTo=".into()
                ),
                (
                    "CONNECT destination.invalid:8081 HTTP/1.1".into(),
                    "Basic Mjo=".into()
                ),
                (
                    "GET http://destination.invalid:8081/credentials HTTP/1.1".into(),
                    "Basic Mzo=".into()
                ),
                (
                    "CONNECT destination.invalid:8081 HTTP/1.1".into(),
                    "Basic MTo=".into()
                ),
            ]
        );
    });
}

/// A captured environment stays owned by one bridge while sender settings change.
async fn consume(source: &str, protocols: &[&str]) {
    let env = Environment(
        [
            ("HTTP_PROXY".into(), source.into()),
            ("HTTPS_PROXY".into(), source.into()),
        ]
        .into(),
    );
    let settings = Settings {
        endpoint: None,
        region: None,
        profile: None,
        auth: Auth::Placeholder,
        http1: true,
        proxy: true,
    };
    let http = bridge(
        None,
        transport(&settings, env),
        None,
        Arc::new(Mutex::new(None)),
    );
    let components = RuntimeComponentsBuilder::for_tests().build().unwrap();
    for (index, protocol) in protocols.iter().enumerate() {
        let settings = HttpConnectorSettings::builder()
            .connect_timeout(Duration::from_secs(30 + index as u64))
            .read_timeout(Duration::from_secs(5 + index as u64))
            .build();
        let connector = http.http_connector(&settings, &components);
        let mut request = WireRequest::new(SdkBody::empty());
        request
            .set_uri(format!("{protocol}://destination.invalid:8081/credentials"))
            .unwrap();
        let response = connector.call(request).await.unwrap();
        assert_eq!(
            response.status().as_u16(),
            if *protocol == "https" { 407 } else { 200 }
        );
        assert_eq!(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .as_ref(),
            b"observed"
        );
    }
}

#[test]
fn pac_bridge_uses_captured_environment_at_later_consumption() {
    const TEST: &str = "providers::cloud::amazon_bedrock::client::tests::pac::pac_bridge_uses_captured_environment_at_later_consumption";
    if std::env::var_os("MAESTRO_CAPTURE_CHILD").is_some() {
        crate::tests::runtime::run(async {
            for bypass in [false, true] {
                captured_environment(bypass).await;
            }
        });
        return;
    }
    let output = maestro_test_process::TestProcess::current(TEST)
        .unwrap()
        .env_clear()
        .env("MAESTRO_CAPTURE_CHILD", "1")
        .env("HTTP_PROXY", "pac+data:,invalid ! script")
        .env("HTTPS_PROXY", "pac+data:,invalid ! script")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// Ambient re-observation fails; the previously supplied snapshot completes on the peer.
async fn captured_environment(bypass: bool) {
    let (sent, received) = mpsc::channel();
    let server = loopback::serve_routed(move |request| {
        sent.send(request.request_line.clone()).unwrap();
        loopback::Reply {
            status: 200,
            content_type: "text/plain",
            headers: &[],
            body: b"snapshot".to_vec(),
            close: false,
        }
    })
    .unwrap();
    let script = format!(
        "function FindProxyForURL(){{return 'PROXY {}'}}",
        &server.url[7..]
    );
    let source = format!(
        "pac+data:,{}",
        percent_encoding::utf8_percent_encode(&script, percent_encoding::NON_ALPHANUMERIC)
    );
    let captured = Environment(
        [
            (
                "HTTP_PROXY".into(),
                if bypass {
                    "pac+data:,invalid ! script".into()
                } else {
                    source
                },
            ),
            (
                "NO_PROXY".into(),
                if bypass { "*".into() } else { String::new() },
            ),
        ]
        .into(),
    );
    let uri = if bypass {
        format!("{}/captured", server.url)
    } else {
        "http://destination.invalid/captured".into()
    };
    check_environment(captured, &uri).await;
    server.finish().unwrap();
    assert_eq!(
        received.try_iter().collect::<Vec<_>>(),
        [format!(
            "GET {} HTTP/1.1",
            if bypass {
                "/captured"
            } else {
                "http://destination.invalid/captured"
            }
        )]
    );
}

/// Compare retained and ambient routing through the same native SDK connector.
async fn check_environment(captured: Environment, uri: &str) {
    let settings = Settings {
        endpoint: None,
        region: None,
        profile: None,
        auth: Auth::Placeholder,
        http1: true,
        proxy: true,
    };
    let components = RuntimeComponentsBuilder::for_tests().build().unwrap();
    for (env, retained) in [(Environment::capture(), false), (captured, true)] {
        let http = bridge(
            None,
            transport(&settings, env),
            None,
            Arc::new(Mutex::new(None)),
        );
        let connector = http.http_connector(&HttpConnectorSettings::builder().build(), &components);
        let mut request = WireRequest::new(SdkBody::empty());
        request.set_uri(uri).unwrap();
        let result = connector.call(request).await;
        if retained {
            assert_eq!(
                result
                    .unwrap()
                    .into_body()
                    .collect()
                    .await
                    .unwrap()
                    .to_bytes()
                    .as_ref(),
                b"snapshot"
            );
        } else {
            let error = result.unwrap_err();
            let message = aws_smithy_types::error::display::DisplayErrorContext(&error).to_string();
            assert!(message.contains("SyntaxError"), "{message}");
        }
    }
}
