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
