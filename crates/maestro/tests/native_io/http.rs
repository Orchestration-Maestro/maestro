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
