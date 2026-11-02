//! Loopback requests through the compiled author and application HTTP owner.
use super::{command_answer, loaded};
use crate::support::{Root, rpc::Rpc};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
};

/// Requires a successful HTTP operation without flattening its returned JSON.
fn call(rpc: &mut Rpc, data: &Value) -> Value {
    let reply = command_answer(rpc, "native-http", data);
    assert!(reply.get("error").is_none(), "{reply}");
    reply["value"].clone()
}

/// Reads only the endpoint's request, preserving raw bytes and absent wire framing.
fn request(stream: &mut std::net::TcpStream) -> (String, Vec<u8>) {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    let head = String::from_utf8(head).unwrap();
    let length = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .map_or(0, |value| value.parse().unwrap());
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    (head, body)
}

#[test]
fn native_fetch_forwards_the_complete_request() {
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/request", endpoint.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for expected in [None, Some(vec![]), Some(vec![0, 255, 226, 130, 172])] {
            let (mut socket, _) = endpoint.accept().unwrap();
            let (head, bytes) = request(&mut socket);
            assert!(head.starts_with(if expected.is_none() {
                "GET /request "
            } else {
                "POST /request "
            }));
            assert!(head.contains("\r\nx-author: exact value\r\n"), "{head}");
            assert_eq!(bytes, expected.unwrap_or_default());
            socket
                .write_all(
                    b"HTTP/1.1 200 Owned reason\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        }
    });
    let root = Root::new();
    let mut rpc = loaded(&root);
    for body in [None, Some(vec![]), Some(vec![0, 255, 226, 130, 172])] {
        let response = call(
            &mut rpc,
            &json!({"op":"fetch", "url":url,
            "method":body.as_ref().map(|_| "POST"), "body":body,
            "headers":[["X-Author", "  exact value\t"]]}),
        );
        assert_eq!(response["status"], 200);
    }
    server.join().unwrap();
    rpc.finish();
}

#[test]
fn native_response_keeps_metadata_and_body_presence() {
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/metadata", endpoint.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for status in [200, 204] {
            let (mut socket, _) = endpoint.accept().unwrap();
            request(&mut socket);
            write!(socket, "HTTP/1.1 {status} Owned reason\r\nContent-Length: 0\r\nX-Response: first\r\nX-Response: second\r\nSet-Cookie: one\r\nSet-Cookie: two\r\nConnection: close\r\n\r\n").unwrap();
        }
    });
    let root = Root::new();
    let mut rpc = loaded(&root);
    for status in [200, 204] {
        let response = call(&mut rpc, &json!({"op":"fetch", "url":url}));
        assert_eq!(response["status"], status);
        assert_eq!(response["ok"], true);
        assert_eq!(response["status_text"], "Owned reason");
        assert_eq!(response["body"], status != 204);
        let headers: Vec<(String, String)> =
            serde_json::from_value(response["headers"].clone()).unwrap();
        let headers: std::collections::BTreeMap<_, _> = headers.into_iter().collect();
        assert_eq!(headers["x-response"], "first, second");
        assert_eq!(headers["set-cookie"], "two");
        if status == 200 {
            assert!(call(&mut rpc, &json!({"op":"read"})).is_null());
        }
    }
    server.join().unwrap();
    rpc.finish();
}

#[test]
fn native_response_yields_bytes_before_body_completion() {
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/body", endpoint.local_addr().unwrap());
    let (release, released) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = endpoint.accept().unwrap();
        request(&mut socket);
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\n\x00\xff\xe2",
            )
            .unwrap();
        released.recv().unwrap();
        socket.write_all(&[130, 172, 1]).unwrap();
    });
    let root = Root::new();
    let mut rpc = loaded(&root);
    assert_eq!(
        call(&mut rpc, &json!({"op":"fetch", "url":url}))["body"],
        true
    );
    let mut bytes = Vec::new();
    while bytes.len() < 3 {
        bytes.extend(
            serde_json::from_value::<Vec<u8>>(call(&mut rpc, &json!({"op":"read"}))).unwrap(),
        );
    }
    assert_eq!(bytes, [0, 255, 226]);
    release.send(()).unwrap();
    loop {
        let next = call(&mut rpc, &json!({"op":"read"}));
        if next.is_null() {
            break;
        }
        bytes.extend(serde_json::from_value::<Vec<u8>>(next).unwrap());
    }
    assert_eq!(bytes, [0, 255, 226, 130, 172, 1]);
    server.join().unwrap();
    rpc.finish();
}

#[test]
fn native_fetch_returns_non_success_responses() {
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/status", endpoint.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for status in [200, 204, 299, 300, 401, 404, 599, 401] {
            let (mut socket, _) = endpoint.accept().unwrap();
            request(&mut socket);
            let body = if status == 204 { "" } else { "denied" };
            write!(socket, "HTTP/1.1 {status} Owned reason\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    let root = Root::new();
    let mut rpc = loaded(&root);
    for (status, ok) in [
        (200, true),
        (204, true),
        (299, true),
        (300, false),
        (401, false),
        (404, false),
        (599, false),
    ] {
        let response = call(&mut rpc, &json!({"op":"fetch", "url":url}));
        assert_eq!(response["status"], status);
        assert_eq!(response["ok"], ok);
    }
    let reply = command_answer(
        &mut rpc,
        "native-http",
        &json!({"op":"exchange", "url":url}),
    );
    assert_eq!(reply["error"], "Token exchange failed: denied");
    server.join().unwrap();
    rpc.finish();
}

/// Aborts the original active-run signal at one selected HTTP phase.
fn abort_phase(body: bool) {
    let root = Root::new();
    let provider = super::HeldProvider::new();
    let mut rpc = super::connected(&root, &provider.base);
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/held", endpoint.local_addr().unwrap());
    assert!(call(&mut rpc, &json!({"op":"arm", "url":url})).is_null());
    rpc.request(json!({"type":"prompt", "message":"held-http"}));
    let (mut socket, _) = endpoint.accept().unwrap();
    request(&mut socket);
    assert_eq!(provider.request.recv().unwrap()["model"], "controlled");
    if body {
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nfirst")
            .unwrap();
        assert_eq!(call(&mut rpc, &json!({"op":"finish"}))["status"], 200);
        let mut prefix = Vec::new();
        while prefix.len() < 5 {
            prefix.extend(
                serde_json::from_value::<Vec<u8>>(call(&mut rpc, &json!({"op":"read"}))).unwrap(),
            );
        }
        assert_eq!(prefix, b"first");
        assert_eq!(call(&mut rpc, &json!({"op":"start_read"})), true);
    }
    rpc.request(json!({"type":"abort"}));
    let reply = command_answer(&mut rpc, "native-http", &json!({"op":"finish"}));
    let aborted = reply["error"].clone();
    assert!(aborted.is_string(), "{reply}");
    assert_eq!(call(&mut rpc, &json!({"op":"aborted"})), true);
    if body {
        assert!(call(&mut rpc, &json!({"op":"read"})).is_null());
    }
    let reply = command_answer(&mut rpc, "native-http", &json!({"op":"fetch", "url":url}));
    assert_eq!(reply["error"], aborted, "pre-abort must fail at fetch");
    rpc.finish();
}

#[test]
fn native_response_errors_keep_their_phase() {
    abort_phase(false);
    abort_phase(true);
    let root = Root::new();
    let mut rpc = loaded(&root);
    let refused = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/refused", refused.local_addr().unwrap());
    drop(refused);
    let reply = command_answer(&mut rpc, "native-http", &json!({"op":"fetch", "url":url}));
    assert!(reply["error"].is_string(), "{reply}");
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/broken", endpoint.local_addr().unwrap());
    let (release, released) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = endpoint.accept().unwrap();
        request(&mut socket);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nfirst")
            .unwrap();
        released.recv().unwrap();
    });
    assert_eq!(
        call(&mut rpc, &json!({"op":"fetch", "url":url}))["status"],
        200
    );
    let mut prefix = Vec::new();
    while prefix.len() < 5 {
        prefix.extend(
            serde_json::from_value::<Vec<u8>>(call(&mut rpc, &json!({"op":"read"}))).unwrap(),
        );
    }
    assert_eq!(prefix, b"first");
    release.send(()).unwrap();
    let reply = command_answer(&mut rpc, "native-http", &json!({"op":"read"}));
    assert!(reply["error"].is_string(), "{reply}");
    assert!(
        call(&mut rpc, &json!({"op":"read"})).is_null(),
        "no replay after error"
    );
    server.join().unwrap();
    rpc.finish();
}

/// Serves exact body bytes without applying text or JSON transformations.
fn bodies(payloads: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<()>) {
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/consume", endpoint.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for bytes in payloads {
            let (mut socket, _) = endpoint.accept().unwrap();
            request(&mut socket);
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            socket.write_all(&bytes).unwrap();
        }
    });
    (url, server)
}

#[test]
fn native_response_consumers_share_one_body() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    let (url, server) = bodies(vec![b"{\"value\":1}".to_vec(); 2]);
    for first in ["text", "json"] {
        call(&mut rpc, &json!({"op":"fetch", "url":url}));
        assert_eq!(call(&mut rpc, &json!({"op":"aliases"})), true);
        let value = call(&mut rpc, &json!({"op":first}));
        assert_eq!(
            value,
            if first == "text" {
                json!("{\"value\":1}")
            } else {
                json!({"value":1})
            }
        );
        for second in ["text", "json"] {
            let reply = command_answer(&mut rpc, "native-http", &json!({"op":second}));
            assert!(reply["error"].is_string(), "{reply}");
        }
        assert!(call(&mut rpc, &json!({"op":"read"})).is_null());
    }
    server.join().unwrap();
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/partial", endpoint.local_addr().unwrap());
    let (release, released) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = endpoint.accept().unwrap();
        request(&mut socket);
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"value\":",
            )
            .unwrap();
        released.recv().unwrap();
        socket.write_all(b"1}").unwrap();
    });
    call(&mut rpc, &json!({"op":"fetch", "url":url}));
    let mut prefix = Vec::new();
    while prefix.len() < 9 {
        prefix.extend(
            serde_json::from_value::<Vec<u8>>(call(&mut rpc, &json!({"op":"read"}))).unwrap(),
        );
    }
    assert_eq!(prefix, b"{\"value\":");
    for operation in ["text", "json"] {
        let reply = command_answer(&mut rpc, "native-http", &json!({"op":operation}));
        assert!(reply["error"].is_string(), "{reply}");
    }
    release.send(()).unwrap();
    let mut tail = Vec::new();
    loop {
        let bytes = call(&mut rpc, &json!({"op":"read"}));
        if bytes.is_null() {
            break;
        }
        tail.extend(serde_json::from_value::<Vec<u8>>(bytes).unwrap());
    }
    assert_eq!(tail, b"1}");
    server.join().unwrap();
    rpc.finish();
}

#[test]
fn native_response_json_keeps_all_value_kinds() {
    let cases = [
        ("null", Value::Null),
        ("\"雪\\u0000\"", json!("雪\0")),
        ("42", json!(42)),
        ("1.2345678901234567", json!(1.234_567_890_123_456_7)),
        ("-0.0", json!(-0.0)),
        ("true", json!(true)),
        ("false", json!(false)),
        (
            "{\"2\":\"two\",\"1\":\"one\",\"a\":null}",
            json!({"2":"two","1":"one","a":null}),
        ),
        ("[1,false,null]", json!([1, false, null])),
        ("1e308", json!(1e308)),
        ("5e-324", json!(5e-324)),
        ("9007199254740993", json!(9_007_199_254_740_993_u64)),
    ];
    let invalid = ["", "{", "undefined"];
    let (url, server) = bodies(
        cases
            .iter()
            .map(|(text, _)| text.as_bytes().to_vec())
            .chain(invalid.iter().map(|text| text.as_bytes().to_vec()))
            .collect(),
    );
    let root = Root::new();
    let mut rpc = loaded(&root);
    for (text, expected) in cases {
        call(&mut rpc, &json!({"op":"fetch", "url":url}));
        let value = call(&mut rpc, &json!({"op":"json"}));
        assert_eq!(value, expected, "{text}");
        if text == "-0.0" {
            assert!(value.as_f64().unwrap().is_sign_negative());
        }
    }
    for text in invalid {
        assert_eq!(
            call(&mut rpc, &json!({"op":"fetch", "url":url}))["status"],
            200
        );
        let reply = command_answer(&mut rpc, "native-http", &json!({"op":"json"}));
        assert!(reply["error"].is_string(), "{text}: {reply}");
    }
    server.join().unwrap();
    rpc.finish();
}

#[test]
fn native_response_text_uses_the_existing_decoder() {
    let cases = [
        (vec![], ""),
        (b"\xef\xbb\xbfhello".to_vec(), "hello"),
        (b"\xef\xbb\xbf\xef\xbb\xbfhello".to_vec(), "\u{feff}hello"),
        (b"\0 \t\r\n".to_vec(), "\0 \t\r\n"),
        (b"\xff\xe2\x82".to_vec(), "\u{fffd}\u{fffd}"),
        ("Ω雪".as_bytes().to_vec(), "Ω雪"),
    ];
    let (url, server) = bodies(cases.iter().map(|(bytes, _)| bytes.clone()).collect());
    let root = Root::new();
    let mut rpc = loaded(&root);
    for (_, expected) in cases {
        call(&mut rpc, &json!({"op":"fetch", "url":url}));
        assert_eq!(call(&mut rpc, &json!({"op":"text"})), expected);
    }
    server.join().unwrap();
    rpc.finish();
}

#[test]
fn native_response_close_keeps_caller_signal_live() {
    for close in [true, false] {
        let root = Root::new();
        let provider = super::HeldProvider::new();
        let mut rpc = super::connected(&root, &provider.base);
        let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/retained", endpoint.local_addr().unwrap());
        call(&mut rpc, &json!({"op":"arm", "url":url}));
        rpc.request(json!({"type":"prompt", "message":"retained-http"}));
        let (mut socket, _) = endpoint.accept().unwrap();
        request(&mut socket);
        assert_eq!(provider.request.recv().unwrap()["model"], "controlled");
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 99\r\nConnection: close\r\n\r\nfirst")
            .unwrap();
        assert_eq!(call(&mut rpc, &json!({"op":"finish"}))["status"], 200);
        call(&mut rpc, &json!({"op":"drop_response"}));
        let mut prefix = Vec::new();
        while prefix.len() < 5 {
            prefix.extend(
                serde_json::from_value::<Vec<u8>>(call(&mut rpc, &json!({"op":"read"}))).unwrap(),
            );
        }
        assert_eq!(
            prefix, b"first",
            "body survives its response and creating callback"
        );
        if close {
            assert_eq!(call(&mut rpc, &json!({"op":"start_read"})), true);
            call(&mut rpc, &json!({"op":"close"}));
            call(&mut rpc, &json!({"op":"close"}));
            assert!(call(&mut rpc, &json!({"op":"finish"})).is_null());
        }
        call(&mut rpc, &json!({"op":"drop_body"}));
        assert_eq!(
            socket.read(&mut [0]).unwrap(),
            0,
            "consumer must release its transport"
        );
        assert_eq!(call(&mut rpc, &json!({"op":"aborted"})), false);
        rpc.request(json!({"type":"abort"}));
        assert_eq!(call(&mut rpc, &json!({"op":"aborted"})), true);
        rpc.finish();
    }
}

/// Checks body identity and independent retention through the generated author adapter.
pub(super) fn adapter_contract(rpc: &mut Rpc) {
    let (url, server) = bodies(vec![b"[]".to_vec()]);
    assert_eq!(call(rpc, &json!({"op":"fetch", "url":url}))["status"], 200);
    assert_eq!(call(rpc, &json!({"op":"aliases"})), true);
    call(rpc, &json!({"op":"drop_response"}));
    let mut bytes = Vec::new();
    loop {
        let chunk = call(rpc, &json!({"op":"read"}));
        if chunk.is_null() {
            break;
        }
        bytes.extend(serde_json::from_value::<Vec<u8>>(chunk).unwrap());
    }
    assert_eq!(bytes, b"[]");
    call(rpc, &json!({"op":"close"}));
    call(rpc, &json!({"op":"drop_body"}));
    server.join().unwrap();
}
