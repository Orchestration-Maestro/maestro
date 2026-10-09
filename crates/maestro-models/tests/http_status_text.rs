//! Status text carried by the default native HTTP transport.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;

use futures_util::StreamExt;
use indexmap::IndexMap;
use maestro_models::{HttpRequest, default_fetch};
use serde::Deserialize;

/// One recorded wire response and its expected response data.
#[derive(Deserialize)]
struct Case {
    /// Complete response headers written without modification.
    response_head: Vec<u8>,
    /// Expected numeric status.
    status: u16,
    /// Expected status text.
    status_text: String,
}

/// Exercise every wire input in a named corpus group through the public client.
fn check_group(group: &str) {
    let corpus: BTreeMap<String, Vec<Case>> =
        serde_json::from_str(include_str!("fixtures/http_status_text.json")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for case in &corpus[group] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let head = case.response_head.clone();
        let producer = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            socket.write_all(&head).unwrap();
            socket.write_all(b"body").unwrap();
        });
        runtime.block_on(async {
            let mut response = default_fetch()(HttpRequest {
                method: "GET".into(),
                url: format!("http://{address}/"),
                headers: IndexMap::new(),
                body: Vec::new(),
                signal: None,
            })
            .await
            .unwrap();
            assert_eq!(response.status, case.status);
            assert_eq!(response.status_text, case.status_text);
            assert_eq!(response.headers["x-witness"], "carried");
            let mut body = Vec::new();
            while let Some(chunk) = response.body.next().await {
                body.extend(chunk.unwrap());
            }
            assert_eq!(body, b"body");
        });
        producer.join().unwrap();
    }
}

#[test]
fn http_status_text_preserves_canonical_reasons() {
    check_group("http_status_text_preserves_canonical_reasons");
}

#[test]
fn http_status_text_preserves_custom_reasons() {
    check_group("http_status_text_preserves_custom_reasons");
}

#[test]
fn http_status_text_preserves_empty_reasons() {
    check_group("http_status_text_preserves_empty_reasons");
}

#[test]
fn http_status_text_keeps_spacing() {
    check_group("http_status_text_keeps_spacing");
}

#[test]
fn http_status_text_preserves_http10_reason() {
    check_group("http_status_text_preserves_http10_reason");
}

#[test]
fn http_status_text_keeps_native_non_ascii_limitation() {
    check_group("http_status_text_keeps_native_non_ascii_limitation");
}
