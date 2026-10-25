//! Script-selected cloud routes through the unchanged public stream callers.
#![cfg(not(target_arch = "wasm32"))]

use crate::chat::TestResult;
use crate::{bedrock, cloud_proxy_transports, loopback};
use maestro_models::StreamOptions;

#[test]
fn pac_bedrock_routes_reach_raw_and_simple_streams() -> TestResult {
    let selection = maestro_test_process::selection();
    if selection.is_child() {
        return crate::runtime::run(async {
            let (labels, message) =
                cloud_proxy_transports::conversation(StreamOptions::default()).await?;
            cloud_proxy_transports::completed(&labels, &message)?;
            if selection.runs_case("PROXY") {
                direct_conversation().await?;
            }
            Ok(())
        });
    }
    for simple in [false, true] {
        for directive in ["DIRECT", "PROXY", "SOCKS", "SOCKS5", "SOCKS4"] {
            conversation(directive, simple)?;
        }
    }
    Ok(())
}

/// Consume the same script with a destination that selects DIRECT instead of the proxy.
async fn direct_conversation() -> TestResult {
    use maestro_models::providers::cloud::amazon_bedrock::{stream_bedrock, stream_simple_bedrock};
    let model = bedrock::model(&std::env::var("MAESTRO_DIRECT_ORIGIN")?)?;
    let context = bedrock::context("question")?;
    let stream = if std::env::var_os("MAESTRO_SIMPLE").is_some() {
        stream_simple_bedrock(model, context, None)
    } else {
        stream_bedrock(model, context, None)
    };
    let (labels, message) = crate::transport::drain(&stream).await?;
    cloud_proxy_transports::completed(&labels, &message)
}

/// Record one admitted destination and route for a complete public conversation.
fn conversation(directive: &str, simple: bool) -> TestResult {
    let body = bedrock::answer("answer")?.concat();
    let (sent, received) = std::sync::mpsc::channel();
    let socks = directive.starts_with("SOCKS");
    let server = loopback::start_routed(std::sync::Arc::new(move |mut socket| {
        if socks {
            use crate::socks::{DEFAULT_REPLY, negotiate_observing};
            let (address, ()) =
                negotiate_observing(&mut socket, false, DEFAULT_REPLY, |_, _, _| ())?;
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
        "async function FindProxyForURL(url,host){{if(!url.endsWith('/a%20b/model/meta.llama-controlled/converse-stream')||!url.includes(host+':'))throw Error('lost URL');return {{then(resolve){{resolve(host==='127.0.0.1'?'DIRECT':'{result}')}}}}}}"
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
    .env("MAESTRO_ORIGIN", format!("{origin}/a%20b?x=1&x=2"))
    .env(
        "MAESTRO_DIRECT_ORIGIN",
        format!("{}/a%20b?x=1&x=2", server.url),
    )
    .env("AWS_REGION", "us-east-1")
    .env("AWS_BEARER_TOKEN_BEDROCK", "controlled-bearer")
    .env("AWS_MAX_ATTEMPTS", "1")
    .case(directive);
    if simple {
        child = child.env("MAESTRO_SIMPLE", "1");
    }
    let output = child.output()?;
    server.finish()?;
    assert!(output.status.success(), "{output:?}");
    check_request_lines(directive, &received.try_iter().collect::<Vec<_>>());
    Ok(())
}

/// Both destination branches retain the SDK's encoded path and omit its discarded query.
fn check_request_lines(directive: &str, requests: &[String]) {
    let target = if directive == "PROXY" {
        "http://destination.invalid:8081/a%20b/model/meta.llama-controlled/converse-stream"
    } else {
        "/a%20b/model/meta.llama-controlled/converse-stream"
    };
    let mut expected = vec![format!("POST {target} HTTP/1.1")];
    if directive == "PROXY" {
        expected.push("POST /a%20b/model/meta.llama-controlled/converse-stream HTTP/1.1".into());
    }
    assert_eq!(requests, expected);
}

#[test]
#[cfg(unix)]
fn pac_credential_services_reuse_the_existing_bridge() -> TestResult {
    crate::profile_routes::check("pac_credential_services_reuse_the_existing_bridge")
}

#[test]
fn pac_bypass_and_supplied_fetch_avoid_script_loading() -> TestResult {
    let selection = maestro_test_process::selection();
    if selection.is_child() {
        let case = ["selected", "bypass", "replacement"]
            .into_iter()
            .find(|id| selection.runs_case(id))
            .ok_or("unknown child case")?;
        return crate::runtime::run(cloud_proxy_transports::bypass_conversation(case));
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
        .case(case);
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

#[test]
fn pac_ftp_sources_load_and_revalidate_scripts() -> TestResult {
    let selection = maestro_test_process::selection();
    if selection.is_child() {
        return crate::runtime::run(ftp_source_conversations());
    }
    let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let body = bedrock::answer("answer")?.concat();
    let origin = routed_replies(&observed, "origin", body.clone())?;
    let proxy = routed_replies(&observed, "proxy", body)?;
    let ftp = crate::ftp_server::serve(ftp_scenarios(&proxy.url))?;
    let output = ftp_child(&origin.url, &ftp)?;
    let commands = ftp.commands();
    let closed = ftp.finish()?;
    assert!(
        closed.control && closed.data,
        "the child closed the FTP control and data connections"
    );
    origin.finish()?;
    proxy.finish()?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        *observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["origin", "origin", "proxy", "proxy"]
    );
    assert_eq!(
        commands
            .iter()
            .filter(|line| line.starts_with("MDTM"))
            .count(),
        5,
        "every send re-validated the source"
    );
    Ok(())
}

/// Run the child that sends four conversations and one through a refused source.
fn ftp_child(origin: &str, ftp: &crate::ftp_server::Peer) -> TestResult<std::process::Output> {
    Ok(
        maestro_test_process::TestProcess::current("pac_ftp_sources_load_and_revalidate_scripts")?
            .nocapture()
            .env_clear()
            .env(
                "HTTP_PROXY",
                format!("pac+ftp://{}/script.pac", ftp.address()),
            )
            .env("MAESTRO_ORIGIN", origin)
            .env("AWS_REGION", "us-east-1")
            .env("AWS_BEARER_TOKEN_BEDROCK", "controlled-bearer")
            .env("AWS_MAX_ATTEMPTS", "1")
            .case("*")
            .output()?,
    )
}

/// One scripted source per send: an unchanged pair, then a changed pair, then a refusal.
fn ftp_scenarios(proxy: &str) -> Vec<crate::ftp_server::Scenario> {
    let mut scenarios: Vec<crate::ftp_server::Scenario> = ["DIRECT", "DIRECT", "PROXY", "PROXY"]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| crate::ftp_server::Scenario {
            modification: crate::ftp_server::Modification::Instant(if index < 2 {
                "20240101120000"
            } else {
                "20240101120001"
            }),
            transfer: crate::ftp_server::Transfer::body(ftp_script(proxy, kind).into_bytes()),
            ..crate::ftp_server::Scenario::default()
        })
        .collect();
    scenarios.push(crate::ftp_server::Scenario {
        modification: crate::ftp_server::Modification::MISSING,
        ..crate::ftp_server::Scenario::default()
    });
    scenarios
}

/// One script selecting the transport every send through it uses.
fn ftp_script(proxy: &str, kind: &str) -> String {
    let chosen = if kind == "DIRECT" {
        "'DIRECT'".to_owned()
    } else {
        format!("'PROXY {}'", &proxy[7..])
    };
    format!(
        "function FindProxyForURL(url,host){{if(!url.endsWith('/converse-stream'))throw Error('lost URL');return {chosen}}}"
    )
}

/// A loopback server that answers cloud frames and records each admitted request's role.
fn routed_replies(
    observed: &std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
    role: &'static str,
    body: Vec<u8>,
) -> TestResult<loopback::Routed> {
    let recorded = std::sync::Arc::clone(observed);
    loopback::serve_routed(move |_| {
        recorded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(role);
        loopback::Reply {
            status: 200,
            content_type: "application/vnd.amazon.eventstream",
            headers: &[],
            body: body.clone(),
            close: false,
        }
    })
}

/// Four sends over an FTP-held script, then a refused source that ends the send.
async fn ftp_source_conversations() -> TestResult {
    use maestro_models::providers::cloud::amazon_bedrock::stream_bedrock;
    let origin = std::env::var("MAESTRO_ORIGIN")?;
    for _ in 0..4 {
        let stream = stream_bedrock(
            bedrock::model(&origin)?,
            bedrock::context("question")?,
            None,
        );
        let (labels, message) = crate::transport::drain(&stream).await?;
        cloud_proxy_transports::completed(&labels, &message)?;
    }
    let stream = stream_bedrock(
        bedrock::model(&origin)?,
        bedrock::context("question")?,
        None,
    );
    let (labels, message) = crate::transport::drain(&stream).await?;
    assert_eq!(labels.last().map(String::as_str), Some("error"));
    let failure = message
        .error_message
        .ok_or("the refused source still answered")?;
    assert!(
        failure.contains("does not exist at the specified endpoint"),
        "{failure}"
    );
    Ok(())
}
