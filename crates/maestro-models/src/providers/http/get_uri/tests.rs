//! Source bytes and retained cache observations.

/// Emit a DEBUG event carrying a credential-shaped field, so a capture can prove it sees one.
pub(super) fn probe_credential(value: &str) {
    tracing::debug!(password = value, "credential probe");
}

#[test]
fn pac_data_sources_preserve_bytes_and_cache_identity() {
    crate::tests::runtime::run(async {
        let fetch = super::super::configured_fetch(super::super::Route::default());
        for (uri, bytes) in [
            ("data:,", b"".as_slice()),
            ("data:,hello+world", b"hello+world".as_slice()),
            ("data:,%EF%BB%BFplain", b"\xef\xbb\xbfplain".as_slice()),
            ("data:,%FF%EF%BB%BF", b"\xff\xef\xbb\xbf".as_slice()),
        ] {
            let url = url::Url::parse(uri).unwrap();
            let source = super::load(&url, None, &fetch).await.unwrap();
            assert_eq!(source.bytes, bytes);
            let unchanged = super::load(&url, Some(&source.cache), &fetch).await;
            assert!(
                unchanged
                    .unwrap_err()
                    .is::<super::notmodified::NotModified>()
            );
        }
    });
}

#[test]
fn pac_data_decoding_matches_escaped_and_base64_inputs() {
    crate::tests::runtime::run(async {
        let fetch = super::super::configured_fetch(super::super::Route::default());
        for (uri, bytes) in [
            ("data:;base64,YWJj", b"abc".as_slice()),
            ("data:;base64,YQ", b"a".as_slice()),
            (
                "data:;base64,Y!W_J-j===ignored",
                b"\x61\x6f\xc9\xfa".as_slice(),
            ),
            ("data:;base64,Y", b"".as_slice()),
            ("data:,%u0041%u00FF", b"A\xff".as_slice()),
            ("data:,%2%XX", b"%2%XX".as_slice()),
            ("data:,雪", "雪".as_bytes()),
            ("data:,a+b", b"a+b".as_slice()),
        ] {
            let source = super::load(&url::Url::parse(uri).unwrap(), None, &fetch)
                .await
                .unwrap();
            assert_eq!(source.bytes, bytes, "{uri}");
        }
        let error = super::load(&url::Url::parse("data:broken").unwrap(), None, &fetch)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "malformed data: URI");
    });
}

#[test]
fn pac_file_sources_compare_opened_file_modification_time() {
    crate::tests::runtime::run(async {
        let path = std::env::temp_dir().join(format!("maestro-pac-file-{}", std::process::id()));
        std::fs::write(&path, b"first script").unwrap();
        let uri = url::Url::from_file_path(&path).unwrap();
        let fetch = super::super::configured_fetch(super::super::Route::default());
        let source = super::load(&uri, None, &fetch).await.unwrap();
        assert_eq!(source.bytes, b"first script");
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let modified = file.metadata().unwrap().modified().unwrap();
        std::fs::write(&path, b"second script").unwrap();
        file.set_modified(modified).unwrap();
        let unchanged = super::load(&uri, Some(&source.cache), &fetch)
            .await
            .unwrap_err();
        assert!(unchanged.is::<super::notmodified::NotModified>());
        file.set_modified(modified + std::time::Duration::from_secs(1))
            .unwrap();
        let changed = super::load(&uri, Some(&source.cache), &fetch)
            .await
            .unwrap();
        assert_eq!(changed.bytes, b"second script");
        drop(file);
        std::fs::remove_file(path).unwrap();
    });
}

/// Serve a finite source exchange and retain the requests for validator assertions.
async fn source_server(responses: Vec<String>) -> (url::Url, tokio::task::JoinHandle<Vec<String>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let uri =
        url::Url::parse(&format!("http://{}/source", listener.local_addr().unwrap())).unwrap();
    let peer = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            requests.push(String::from_utf8(bytes).unwrap());
            socket.write_all(response.as_bytes()).await.unwrap();
        }
        requests
    });
    (uri, peer)
}

#[test]
fn pac_http_sources_revalidate_and_follow_redirects() {
    crate::tests::runtime::run(async {
        let (uri, peer) = source_server(vec![
            "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
            "HTTP/1.1 200 OK\r\nETag: \"one\"\r\nLast-Modified: Wed, 21 Oct 2015 07:28:00 GMT\r\nContent-Length: 3\r\nConnection: close\r\n\r\none".into(),
            "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
            "HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".into(),
        ]).await;
        let fetch = super::super::configured_fetch(super::super::Route::default());
        let source = super::load(&uri, None, &fetch).await.unwrap();
        assert_eq!(source.bytes, b"one");
        let error = super::load(&uri, Some(&source.cache), &fetch)
            .await
            .unwrap_err();
        assert!(error.is::<super::notmodified::NotModified>());
        let requests = peer.await.unwrap();
        assert!(requests[1].starts_with("GET /final HTTP/1.1"));
        assert!(!requests[2].contains("if-none-match"));
        assert!(requests[3].contains("if-none-match: \"one\"\r\n"));
        assert!(requests[3].contains("if-modified-since: Wed, 21 Oct 2015 07:28:00 GMT\r\n"));
    });
}

#[test]
fn pac_http_redirects_keep_limit_and_cached_targets() {
    crate::tests::runtime::run(async {
        let redirect = "HTTP/1.1 302 Found\r\nCache-Control: max-age=3600\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (uri, peer) = source_server(vec![
            redirect.into(),
            "HTTP/1.1 200 OK\r\nCache-Control: max-age=3600\r\nContent-Length: 3\r\nConnection: close\r\n\r\none".into(),
        ]).await;
        let fetch = super::super::configured_fetch(super::super::Route::default());
        let source = super::load(&uri, None, &fetch).await.unwrap();
        let requests = peer.await.unwrap();
        assert_eq!(requests.len(), 2);
        // The server is gone: both redirect and final source must come from the cache.
        assert!(
            super::load(&uri, Some(&source.cache), &fetch)
                .await
                .unwrap_err()
                .is::<super::notmodified::NotModified>()
        );
        let (uri, peer) = source_server(vec![redirect.into(); 6]).await;
        let error = super::load(&uri, None, &fetch).await.unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<super::http_error::HttpError>()
                .unwrap()
                .0,
            302
        );
        assert_eq!(peer.await.unwrap().len(), 6);
        let (uri, peer) = source_server(
            vec![redirect.into(); 5]
                .into_iter()
                .chain(std::iter::once(
                    "HTTP/1.1 201 Created\r\nContent-Length: 3\r\nConnection: close\r\n\r\nend"
                        .into(),
                ))
                .collect(),
        )
        .await;
        assert_eq!(super::load(&uri, None, &fetch).await.unwrap().bytes, b"end");
        assert_eq!(peer.await.unwrap().len(), 6);
    });
}

#[test]
fn pac_source_status_failures_keep_the_http_status() {
    crate::tests::runtime::run(async {
        let fetch = super::super::configured_fetch(super::super::Route::default());
        for status in [
            200, 201, 202, 204, 206, 250, 299, 301, 302, 304, 307, 308, 404, 407, 500, 599, 600,
        ] {
            let (uri, peer) = source_server(vec![format!(
                "HTTP/1.1 {status} Custom\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )])
            .await;
            let result = super::load(&uri, None, &fetch).await;
            match status {
                200..=299 => assert!(result.unwrap().bytes.is_empty()),
                304 => assert_eq!(
                    result.unwrap_err().to_string(),
                    "Source has not been modified since the provided \"cache\", re-use previous results"
                ),
                404 => assert_eq!(
                    result.unwrap_err().to_string(),
                    "File does not exist at the specified endpoint"
                ),
                _ => assert_eq!(
                    result
                        .unwrap_err()
                        .downcast_ref::<super::http_error::HttpError>()
                        .unwrap()
                        .0,
                    status
                ),
            }
            assert_eq!(peer.await.unwrap().len(), 1);
        }
    });
}

#[test]
fn pac_https_sources_share_the_http_cache_owner() {
    crate::tests::runtime::run(async {
        use crate::tests::{loopback, tls};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let config = tls::server_config(&[b"http/1.1"]).unwrap();
        let server = loopback::start_routed(Arc::new(move |socket| {
            use std::io::Write;
            let mut stream = rustls::StreamOwned::new(
                rustls::ServerConnection::new(config.clone()).unwrap(), socket);
            assert!(loopback::read_request(&mut stream)?.is_some());
            observed.fetch_add(1, Ordering::SeqCst);
            stream.write_all(b"HTTP/1.1 200 OK\r\nCache-Control: max-age=3600\r\nContent-Length: 3\r\nConnection: close\r\n\r\none")?;
            stream.conn.send_close_notify();
            stream.flush()
        })).unwrap();
        let uri = url::Url::parse(
            &server
                .url
                .replacen("http://127.0.0.1", "https://localhost", 1),
        )
        .unwrap();
        let fetch = super::super::supplied_fetch(
            reqwest::Client::builder()
                .no_proxy()
                .add_root_certificate(reqwest::Certificate::from_der(tls::CERT).unwrap())
                .build()
                .unwrap(),
        );
        let source = super::load(&uri, None, &fetch).await.unwrap();
        assert_eq!(source.bytes, b"one");
        assert!(
            super::load(&uri, Some(&source.cache), &fetch)
                .await
                .unwrap_err()
                .is::<super::notmodified::NotModified>()
        );
        server.finish().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        https_rejects_invalid_identity(false).await;
        https_rejects_invalid_identity(true).await;
    });
}

/// Observe a TLS alert and no source request for an untrusted chain or mismatched name.
async fn https_rejects_invalid_identity(trust: bool) {
    use crate::tests::{loopback, tls};
    let config = tls::server_config(&[b"http/1.1"]).unwrap();
    let server = loopback::start_routed(std::sync::Arc::new(move |socket| {
        use std::io::Read;
        let mut stream = rustls::StreamOwned::new(
            rustls::ServerConnection::new(config.clone()).unwrap(),
            socket,
        );
        let mut application = Vec::new();
        assert!(stream.read_to_end(&mut application).is_err());
        assert!(application.is_empty());
        Ok(())
    }))
    .unwrap();
    let uri = url::Url::parse(&server.url.replacen("http", "https", 1)).unwrap();
    let mut client = reqwest::Client::builder().no_proxy();
    if trust {
        client = client.add_root_certificate(reqwest::Certificate::from_der(tls::CERT).unwrap());
    }
    let fetch = super::super::supplied_fetch(client.build().unwrap());
    let error = super::load(&uri, None, &fetch).await.unwrap_err();
    assert!(error.to_string().contains("certificate"), "{error}");
    server.finish().unwrap();
}

mod files;
mod ftp;
mod listing;
mod redirects;
