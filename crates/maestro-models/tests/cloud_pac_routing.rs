//! Script-selected cloud routes through the unchanged public stream callers.
#![cfg(not(target_arch = "wasm32"))]

use crate::chat::TestResult;
use crate::{bedrock, cloud_proxy_transports, loopback};
use maestro_models::StreamOptions;

#[test]
fn pac_bedrock_routes_reach_raw_and_simple_streams() -> TestResult {
    if std::env::var_os("MAESTRO_CHILD_CASE").is_some() {
        return crate::runtime::run(async {
            let (labels, message) =
                cloud_proxy_transports::conversation(StreamOptions::default()).await?;
            cloud_proxy_transports::completed(&labels, &message)
        });
    }
    for simple in [false, true] {
        for directive in ["DIRECT", "PROXY", "SOCKS", "SOCKS5", "SOCKS4"] {
            conversation(directive, simple)?;
        }
    }
    Ok(())
}

/// Record one admitted destination and route for a complete public conversation.
fn conversation(directive: &str, simple: bool) -> TestResult {
    let body = bedrock::answer("answer")?.concat();
    let (sent, received) = std::sync::mpsc::channel();
    let socks = directive.starts_with("SOCKS");
    let server = loopback::start_routed(std::sync::Arc::new(move |mut socket| {
        if socks {
            let address = crate::socks::negotiate(&mut socket, false)?;
            assert_eq!(address.kind, 3);
            assert_eq!(address.host, b"destination.invalid");
            assert_eq!(address.port, 8081);
        }
        loopback::answer_connection(socket, &|request| {
            assert!(sent.send(request.request_line.clone()).is_ok());
            loopback::Reply {
                status: 200,
                content_type: "application/vnd.amazon.eventstream",
                headers: &[],
                body: body.clone(),
                close: false,
            }
        })
    }))?;
    let (origin, result) = if directive == "DIRECT" {
        (server.url.as_str(), directive.to_owned())
    } else {
        (
            "http://destination.invalid:8081",
            format!("{directive} {}", &server.url[7..]),
        )
    };
    let script = format!(
        "async function FindProxyForURL(url,host){{if(!url.endsWith('/converse-stream'))throw Error('lost URL');return {{then(resolve){{resolve('{result}')}}}}}}"
    );
    let encoded =
        percent_encoding::utf8_percent_encode(&script, percent_encoding::NON_ALPHANUMERIC);
    let proxy = format!("pac+data:,{encoded}");
    let mut child = maestro_test_process::TestProcess::current(
        "pac_bedrock_routes_reach_raw_and_simple_streams",
    )?
    .nocapture()
    .env_clear()
    .env("HTTP_PROXY", &proxy)
    .env("MAESTRO_ORIGIN", origin)
    .env("AWS_REGION", "us-east-1")
    .env("AWS_BEARER_TOKEN_BEDROCK", "controlled-bearer")
    .env("AWS_MAX_ATTEMPTS", "1")
    .env("MAESTRO_CHILD_CASE", directive);
    if simple {
        child = child.env("MAESTRO_SIMPLE", "1");
    }
    let output = child.output()?;
    server.finish()?;
    assert!(output.status.success(), "{output:?}");
    let requests: Vec<_> = received.try_iter().collect();
    let target = if directive == "PROXY" {
        "http://destination.invalid:8081/model/meta.llama-controlled/converse-stream"
    } else {
        "/model/meta.llama-controlled/converse-stream"
    };
    assert_eq!(requests, [format!("POST {target} HTTP/1.1")]);
    Ok(())
}

#[test]
#[cfg(unix)]
fn pac_credential_services_reuse_the_existing_bridge() -> TestResult {
    crate::profile_routes::check("pac_credential_services_reuse_the_existing_bridge")
}

#[test]
fn pac_bypass_and_supplied_fetch_avoid_script_loading() -> TestResult {
    if let Ok(case) = std::env::var("MAESTRO_CHILD_CASE") {
        return crate::runtime::run(cloud_proxy_transports::bypass_conversation(&case));
    }
    for case in ["selected", "bypass", "replacement"] {
        let source_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = source_calls.clone();
        let source = loopback::serve_routed(move |_| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            loopback::Reply {
                status: 200,
                content_type: "application/x-ns-proxy-autoconfig",
                headers: &[],
                body: b"invalid ! script".to_vec(),
                close: false,
            }
        })?;
        let body = bedrock::answer("answer")?.concat();
        let (sent, received) = std::sync::mpsc::channel();
        let origin = loopback::serve_routed(move |request| {
            sent.send(request.request_line.clone()).unwrap();
            loopback::Reply {
                status: 200,
                content_type: "application/vnd.amazon.eventstream",
                headers: &[],
                body: body.clone(),
                close: false,
            }
        })?;
        let mut child = maestro_test_process::TestProcess::current(
            "pac_bypass_and_supplied_fetch_avoid_script_loading",
        )?
        .nocapture()
        .env_clear()
        .env("HTTP_PROXY", format!("pac+{}", source.url))
        .env("MAESTRO_ORIGIN", &origin.url)
        .env("AWS_REGION", "us-east-1")
        .env("AWS_BEARER_TOKEN_BEDROCK", "controlled-bearer")
        .env("AWS_MAX_ATTEMPTS", "1")
        .env("MAESTRO_CHILD_CASE", case);
        if case == "bypass" {
            child = child.env("NO_PROXY", "*");
        }
        let output = child.output()?;
        source.finish()?;
        origin.finish()?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            source_calls.load(std::sync::atomic::Ordering::SeqCst),
            usize::from(case == "selected")
        );
        assert_eq!(received.try_iter().count(), usize::from(case == "bypass"));
    }
    Ok(())
}
