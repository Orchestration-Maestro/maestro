//! Absolute redirects cross both HTTP transports while retaining each response identity.

#[test]
fn source_redirects_cross_secure_and_plain_transports_without_losing_cache_targets() {
    crate::tests::runtime::run(async {
        use crate::tests::{loopback, tls};
        use std::io::Write;
        use std::sync::{Arc, mpsc};
        let (sent, requests) = mpsc::channel();
        let final_source = loopback::serve_routed(move |request| {
            sent.send(request.request_line.clone()).unwrap();
            assert!(request.header("if-none-match").is_none());
            loopback::Reply {
                status: 200,
                content_type: "text/plain",
                headers: &[("cache-control", "max-age=3600"), ("etag", "final")],
                body: b"source".to_vec(),
                close: false,
            }
        })
        .unwrap();
        let target = format!("{}/final", final_source.url);
        let config = tls::server_config(&[b"http/1.1"]).unwrap();
        let secure = loopback::start_routed(Arc::new(move |socket| {
            let mut stream = rustls::StreamOwned::new(rustls::ServerConnection::new(Arc::clone(&config)).unwrap(), socket);
            let request = loopback::read_request(&mut stream)?.unwrap();
            assert_eq!(request.request_line, "GET /secure HTTP/1.1");
            assert!(request.header("if-none-match").is_none());
            write!(stream, "HTTP/1.1 302 Found\r\nCache-Control: max-age=3600\r\nETag: secure\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
            stream.conn.send_close_notify();
            stream.flush()
        })).unwrap();
        let target = format!(
            "{}/secure",
            secure
                .url
                .replacen("http://127.0.0.1", "https://localhost", 1)
        );
        let (uri, peer) = super::source_server(vec![format!("HTTP/1.1 302 Found\r\nCache-Control: max-age=3600\r\nETag: plain\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")]).await;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .add_root_certificate(reqwest::Certificate::from_der(tls::CERT).unwrap())
            .build()
            .unwrap();
        let fetch = crate::providers::http::supplied_fetch(client);
        let source = super::super::load(&uri, None, &fetch).await.unwrap();
        assert_eq!(source.bytes, b"source");
        assert_eq!(peer.await.unwrap().len(), 1);
        secure.finish().unwrap();
        final_source.finish().unwrap();
        assert_eq!(
            requests.try_iter().collect::<Vec<_>>(),
            ["GET /final HTTP/1.1"]
        );
        assert!(
            super::super::load(&uri, Some(&source.cache), &fetch)
                .await
                .unwrap_err()
                .is::<super::super::notmodified::NotModified>()
        );
    });
}
