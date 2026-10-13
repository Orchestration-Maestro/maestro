//! Connector deadline tests against a controlled stalled HTTP response.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use aws_smithy_runtime_api::client::http::{HttpClient, HttpConnector, HttpConnectorSettings};
use aws_smithy_runtime_api::client::runtime_components::RuntimeComponentsBuilder;
use aws_smithy_types::body::SdkBody;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{Route, WireRequest, bridge};

#[test]
fn stalled_response_is_a_connector_timeout() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        tokio::time::pause();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (received, mut ready) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            received.send(()).unwrap();
            released.await.unwrap();
            // The faulty connector receives success; the correct connector has closed already.
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n")
                .await;
        });
        let http = bridge(None, Route::default(), None, Arc::new(Mutex::new(None)));
        let settings = HttpConnectorSettings::builder()
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(5))
            .build();
        let components = RuntimeComponentsBuilder::for_tests().build().unwrap();
        let connector = http.http_connector(&settings, &components);
        let mut request = WireRequest::new(SdkBody::empty());
        request
            .set_uri(format!("http://{address}/credentials"))
            .unwrap();
        let attempt = tokio::spawn(async move { connector.call(request).await });
        // Stay runnable so paused time cannot advance before the server receives the request.
        while ready.try_recv().is_err() {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(Duration::from_secs(6)).await;
        release.send(()).unwrap();
        let result = attempt.await.unwrap();
        server.await.unwrap();
        assert!(
            result.is_err_and(|error| error.is_timeout()),
            "expected a typed connector timeout"
        );
    });
}

#[test]
fn credential_body_can_arrive_after_the_read_deadline() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        tokio::time::pause();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (release, released) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 11\r\n\r\n")
                .await
                .unwrap();
            released.await.unwrap();
            socket.write_all(b"credentials").await.unwrap();
        });
        let http = bridge(None, Route::default(), None, Arc::new(Mutex::new(None)));
        let settings = HttpConnectorSettings::builder()
            .read_timeout(Duration::from_secs(5))
            .build();
        let components = RuntimeComponentsBuilder::for_tests().build().unwrap();
        let connector = http.http_connector(&settings, &components);
        let mut request = WireRequest::new(SdkBody::empty());
        request
            .set_uri(format!("http://{address}/credentials"))
            .unwrap();
        // Keep paused time runnable until the connector has observed the headers.
        let attempt = tokio::spawn(async move { connector.call(request).await });
        while !attempt.is_finished() {
            tokio::task::yield_now().await;
        }
        let response = attempt.await.unwrap().unwrap();
        let body = tokio::spawn(async move {
            use http_body_util::BodyExt;
            response.into_body().collect().await
        });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(6)).await;
        release.send(()).unwrap();
        let bytes = body
            .await
            .unwrap()
            .expect("body is not subject to the header deadline")
            .to_bytes();
        server.await.unwrap();
        assert_eq!(bytes.as_ref(), b"credentials");
    });
}

#[test]
fn sequential_metadata_requests_reuse_one_connection() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(serve_two_metadata_requests(listener));
        let http = bridge(None, Route::default(), None, Arc::new(Mutex::new(None)));
        let settings = HttpConnectorSettings::builder()
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(5))
            .build();
        let components = RuntimeComponentsBuilder::for_tests().build().unwrap();
        for _ in 0..2 {
            use http_body_util::BodyExt;
            let connector = http.http_connector(&settings, &components);
            let mut request = WireRequest::new(SdkBody::empty());
            request
                .set_uri(format!("http://{address}/credentials"))
                .unwrap();
            let response = connector.call(request).await.unwrap();
            assert_eq!(
                response
                    .into_body()
                    .collect()
                    .await
                    .unwrap()
                    .to_bytes()
                    .as_ref(),
                b"credentials"
            );
        }
        assert_eq!(
            server.await.unwrap(),
            1,
            "metadata transmissions must share their connection pool"
        );
    });
}

/// Serve two complete requests, counting accepted keep-alive connections.
async fn serve_two_metadata_requests(listener: tokio::net::TcpListener) -> usize {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut accepted = 1;
    for _ in 0..2 {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            if socket.read(&mut byte).await.unwrap() == 0 {
                (socket, _) = listener.accept().await.unwrap();
                accepted += 1;
            } else {
                bytes.push(byte[0]);
            }
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 11\r\n\r\ncredentials")
            .await
            .unwrap();
    }
    accepted
}
