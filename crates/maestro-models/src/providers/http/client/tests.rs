//! Retained connection diagnostics and native status-text extraction.
#![cfg(not(target_arch = "wasm32"))]

use std::convert::Infallible;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::ext::ReasonPhrase;
use hyper::service::service_fn;
use hyper::{Response, Version};
use hyper_util::rt::{TokioExecutor, TokioIo};

use super::{classify, status_text};

/// Browser exception names survive the retained client error chain.
#[test]
fn connection_retains_browser_exception_names() {
    for (message, name) in [
        (
            "error sending request: JsValue(TypeError: Failed to fetch)",
            "TypeError",
        ),
        (
            "error sending request: JsValue(NetworkError: A network error occurred)",
            "NetworkError",
        ),
        (
            "error sending request: JsValue(TypeError: NetworkError when attempting to fetch resource.)",
            "TypeError",
        ),
        ("error sending request: connection refused", "FetchError"),
    ] {
        let error = classify(&std::io::Error::other(message));
        let crate::FetchError::Connection(diagnostic) = error else {
            panic!("expected a connection failure");
        };
        assert_eq!(diagnostic.name.as_deref(), Some(name), "{message}");
        assert_eq!(diagnostic.message, message);
    }
}

#[test]
fn http_status_text_empty_for_http2() {
    crate::tests::runtime::run(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                .serve_connection(
                    TokioIo::new(socket),
                    service_fn(|_| async {
                        Ok::<_, Infallible>(
                            Response::builder()
                                .status(503)
                                .body(Full::new(Bytes::from_static(b"body")))
                                .unwrap(),
                        )
                    }),
                )
                .await
                .unwrap();
        });
        let client = reqwest::Client::builder()
            .http2_prior_knowledge()
            .build()
            .unwrap();
        let response = client
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.version(), Version::HTTP_2);
        assert_eq!(response.status(), 503);
        assert_eq!(status_text(&response), "");
        assert_eq!(response.bytes().await.unwrap().as_ref(), b"body");
        drop(client);
        server.await.unwrap();
    });
}

#[test]
fn http_status_text_no_phrase_fallback() {
    for version in [Version::HTTP_10, Version::HTTP_11] {
        let response = Response::builder()
            .version(version)
            .status(599)
            .body("")
            .unwrap();
        assert_eq!(status_text(&response.into()), "");
    }
    for version in [Version::HTTP_2, Version::HTTP_3] {
        let mut response = Response::builder()
            .version(version)
            .status(503)
            .body("")
            .unwrap();
        response
            .extensions_mut()
            .insert(ReasonPhrase::from_static(b"Supplied"));
        assert_eq!(status_text(&response.into()), "");
    }
}

mod pac;

#[test]
fn http_request_keeps_absent_and_empty_bodies() {
    use crate::HttpRequest;
    let client = reqwest::Client::new();
    for body in [None, Some(Vec::new()), Some(vec![0, 255, 226, 130, 172])] {
        let request: HttpRequest<Option<Vec<u8>>> = HttpRequest {
            method: "POST".into(),
            url: "http://127.0.0.1/literal%20path#fragment".into(),
            headers: indexmap::IndexMap::from([("X-Supplied".into(), " \tvalue\t ".into())]),
            body: body.clone(),
            signal: None,
        };
        let prepared = super::prepare(&client, request).unwrap();
        assert_eq!(
            prepared.body().and_then(reqwest::Body::as_bytes),
            body.as_deref()
        );
        assert_eq!(prepared.headers()["x-supplied"], "value");
        assert_eq!(prepared.url().as_str(), "http://127.0.0.1/literal%20path");
    }
    let existing: HttpRequest = HttpRequest {
        method: "POST".into(),
        url: "http://127.0.0.1/".into(),
        headers: indexmap::IndexMap::new(),
        body: Vec::new(),
        signal: None,
    };
    assert_eq!(
        super::prepare(&client, existing)
            .unwrap()
            .body()
            .unwrap()
            .as_bytes(),
        Some([].as_slice())
    );
}

#[test]
fn optional_http_body_reaches_the_native_endpoint() {
    use crate::{HttpRequest, tests::loopback};
    crate::tests::runtime::run(async {
        for method in ["GET", "POST"] {
            for body in [None, Some(Vec::new()), Some(vec![0, 255, 226, 130, 172])] {
                let server = loopback::serve(
                    "HTTP/1.1 299 Supplied\r\nContent-Length: 3\r\nX-Native: retained\r\nConnection: close\r\n\r\n",
                    vec![loopback::Step::Write(vec![0, 255, 1])],
                ).unwrap();
                let response = crate::fetch_optional_body(HttpRequest {
                    method: method.into(),
                    url: format!("{}/literal%20path?x=1&x=2#fragment", server.url),
                    headers: indexmap::IndexMap::from([(
                        "X-Supplied".into(),
                        " \tvalue\t ".into(),
                    )]),
                    body: body.clone(),
                    signal: None,
                })
                .await
                .unwrap();
                assert_eq!(response.status, 299);
                assert_eq!(response.status_text, "Supplied");
                assert_eq!(response.headers["x-native"], "retained");
                assert_eq!(
                    crate::providers::http::collect_body(response.body)
                        .await
                        .unwrap(),
                    [0, 255, 1]
                );
                let seen = server.finish().unwrap();
                assert_eq!(
                    seen.request_line,
                    format!("{method} /literal%20path?x=1&x=2 HTTP/1.1")
                );
                assert_eq!(seen.header_octets("x-supplied"), Some(b"value".as_slice()));
                assert_eq!(seen.body, body.unwrap_or_default());
            }
        }
    });
}
