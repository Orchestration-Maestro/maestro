//! Re-reading unchanged code preserves globals even without cache validators.

use super::{Route, configured_fetch, request};
use crate::tests::ftp_server;
use crate::tests::loopback;
use std::sync::{Arc, mpsc};

#[test]
fn reloaded_identical_source_keeps_globals_without_sharing_them_between_bridges() {
    crate::tests::runtime::run(async {
        let (sent, received) = mpsc::channel();
        let server = loopback::serve_routed(move |request| {
            assert!(request.header("if-none-match").is_none());
            assert!(request.header("if-modified-since").is_none());
            sent.send(()).unwrap();
            loopback::Reply {
                status: 200,
                content_type: "text/plain",
                headers: &[],
                body: b"let n=0;function FindProxyForURL(){return 'COUNT '+(++n)}".to_vec(),
                close: false,
            }
        })
        .unwrap();
        let source = url::Url::parse(&format!("pac+{}", server.url)).unwrap();
        let route = || {
            let source = source.clone();
            configured_fetch(Route {
                proxy: Some(Arc::new(move |_| Ok(Some(source.clone())))),
                ..Route::default()
            })
        };
        let fetch = route();
        for (sender, count) in [(fetch.clone(), 1), (fetch, 2), (route(), 1)] {
            let error = sender(request("http://unused.invalid/".into()))
                .await
                .err()
                .unwrap();
            assert!(
                error.to_string().contains(&format!("COUNT {count}")),
                "{error}"
            );
        }
        server.finish().unwrap();
        assert_eq!(received.try_iter().count(), 3);
    });
}

#[test]
fn file_observation_and_identical_reread_preserve_the_compiled_globals() {
    crate::tests::runtime::run(async {
        let path =
            std::env::temp_dir().join(format!("maestro-pac-observation-{}", std::process::id()));
        let code = "let n=0;function FindProxyForURL(){return 'FIRST '+(++n)}";
        std::fs::write(&path, code).unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        let modified = file.metadata().unwrap().modified().unwrap();
        let source =
            url::Url::parse(&format!("pac+{}", url::Url::from_file_path(&path).unwrap())).unwrap();
        let fetch = configured_fetch(Route {
            proxy: Some(Arc::new(move |_| Ok(Some(source.clone())))),
            ..Route::default()
        });
        for count in 1..=3 {
            if count == 2 {
                std::fs::write(&path, code.replace("FIRST", "HIDDEN")).unwrap();
                file.set_modified(modified).unwrap();
            } else if count == 3 {
                std::fs::write(&path, code).unwrap();
                file.set_modified(modified + std::time::Duration::from_secs(1))
                    .unwrap();
            }
            let error = fetch(request("http://unused.invalid/".into()))
                .await
                .err()
                .unwrap();
            assert!(
                error.to_string().contains(&format!("FIRST {count}")),
                "{error}"
            );
        }
        drop(file);
        std::fs::remove_file(path).unwrap();
    });
}

#[test]
fn source_bytes_use_lossy_utf8_and_preserve_byte_order_marks() {
    crate::tests::runtime::run(async {
        let source = url::Url::parse(
            "pac+data:,%EF%BB%BFfunction%20FindProxyForURL(){return%20'VALUE%20%FF%EF%BB%BFEND'}",
        )
        .unwrap();
        let fetch = configured_fetch(Route {
            proxy: Some(Arc::new(move |_| Ok(Some(source.clone())))),
            ..Route::default()
        });
        let error = fetch(request("http://unused.invalid/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("VALUE �\u{feff}END"), "{error}");
    });
}

#[test]
fn pac_ftp_unchanged_source_keeps_compiled_globals() {
    crate::tests::runtime::run(async {
        let code = |marker: &str| {
            format!("let n=0;function FindProxyForURL(){{return '{marker} '+(++n)}}")
        };
        let scenario = |modification, marker: &str| ftp_server::Scenario {
            modification,
            transfer: ftp_server::Transfer::body(code(marker).into_bytes()),
            ..ftp_server::Scenario::default()
        };
        let peer = ftp_server::serve(vec![
            scenario(ftp_server::Modification::Instant("20240101120000"), "FIRST"),
            scenario(ftp_server::Modification::Instant("20240101120000"), "FIRST"),
            scenario(
                ftp_server::Modification::Instant("20240101120001"),
                "SECOND",
            ),
        ])
        .unwrap();
        let source = url::Url::parse(&format!("pac+ftp://{}/script.pac", peer.address())).unwrap();
        let fetch = configured_fetch(Route {
            proxy: Some(Arc::new(move |_| Ok(Some(source.clone())))),
            ..Route::default()
        });
        for expected in ["FIRST 1", "FIRST 2", "SECOND 1"] {
            let error = fetch(request("http://unused.invalid/".into()))
                .await
                .err()
                .unwrap();
            assert!(error.to_string().contains(expected), "{error}");
        }
        peer.finish().unwrap();
    });
}
