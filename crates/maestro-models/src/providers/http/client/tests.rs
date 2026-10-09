//! Native status-text extraction across protocol versions.
#![cfg(not(target_arch = "wasm32"))]

use std::convert::Infallible;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::ext::ReasonPhrase;
use hyper::service::service_fn;
use hyper::{Response, Version};
use hyper_util::rt::{TokioExecutor, TokioIo};

use super::status_text;

#[test]
fn http_status_text_empty_for_http2() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
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
