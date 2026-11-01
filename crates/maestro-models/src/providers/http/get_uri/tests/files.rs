//! File URL and source-error categories through actual native loaders.

use super::super::{self as get_uri, notfound::NotFound, notmodified::NotModified};
use crate::providers::http::{Route, configured_fetch};

#[test]
fn pac_file_sources_validate_urls_and_keep_io_failures() {
    crate::tests::runtime::run(async {
        let path = std::env::temp_dir().join(format!("maestro-pac-urls-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        let file = path.join("space 雪.pac");
        std::fs::write(&file, b"script").unwrap();
        let fetch = configured_fetch(Route::default());
        let uri = url::Url::from_file_path(&file).unwrap();
        assert!(uri.as_str().contains("space%20%E9%9B%AA.pac"));
        assert_eq!(
            get_uri::load(&uri, None, &fetch).await.unwrap().bytes,
            b"script"
        );
        let local =
            url::Url::parse(&uri.as_str().replacen("file:///", "file://localhost/", 1)).unwrap();
        assert_eq!(
            get_uri::load(&local, None, &fetch).await.unwrap().bytes,
            b"script"
        );
        let directory = url::Url::from_directory_path(&path).unwrap();
        assert!(
            !get_uri::load(&directory, None, &fetch)
                .await
                .unwrap_err()
                .is::<NotFound>()
        );
        for text in [
            uri.as_str().replace("space", "bad%2fspace"),
            format!("file://foreign.invalid{}", uri.path()),
        ] {
            let url = url::Url::parse(&text).unwrap();
            // The host decides what a file URL names: Windows reads a foreign host as a UNC
            // server, where other hosts refuse it.
            let readable = url
                .to_file_path()
                .is_ok_and(|path| std::fs::read(path).is_ok());
            assert_eq!(
                get_uri::load(&url, None, &fetch).await.is_ok(),
                readable,
                "{text}"
            );
        }
        #[cfg(unix)]
        permission_failure(&file, &uri, &fetch).await;
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(path).unwrap();
    });
}

/// Permission failures stay native I/O errors, not a missing-source category.
#[cfg(unix)]
async fn permission_failure(path: &std::path::Path, uri: &url::Url, fetch: &crate::Fetch) {
    // A privileged run reads the file anyway, so there is no failure to check.
    let Some(restore) = maestro_test_directory::deny(path, 0o0).unwrap() else {
        return;
    };
    let result = get_uri::load(uri, None, fetch).await;
    drop(restore);
    let error = result.unwrap_err();
    assert!(!error.is::<NotFound>());
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

#[test]
fn pac_source_errors_keep_not_found_distinct() {
    crate::tests::runtime::run(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let path = scratch.path().join("missing.pac");
        assert!(!path.exists());
        let file = url::Url::from_file_path(&path).unwrap();
        let fetch = configured_fetch(Route::default());
        let (http, peer) = super::source_server(vec![
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        ])
        .await;
        // A link whose target is absent names a missing source too.
        let link = cfg!(unix).then(|| path.with_extension("link"));
        #[cfg(unix)]
        std::os::unix::fs::symlink(&path, link.as_ref().unwrap()).unwrap();
        let links = link
            .iter()
            .map(|link| url::Url::from_file_path(link).unwrap());
        for uri in [file, http].into_iter().chain(links) {
            let error = get_uri::load(&uri, None, &fetch).await.unwrap_err();
            assert!(error.is::<NotFound>());
            assert_eq!(
                error.to_string(),
                "File does not exist at the specified endpoint"
            );
        }
        assert_eq!(peer.await.unwrap().len(), 1);
    });
}

#[test]
fn pac_source_errors_keep_not_modified_distinct() {
    crate::tests::runtime::run(async {
        let fetch = configured_fetch(Route::default());
        let uri = url::Url::parse("data:,script").unwrap();
        let source = get_uri::load(&uri, None, &fetch).await.unwrap();
        let unchanged = get_uri::load(&uri, Some(&source.cache), &fetch)
            .await
            .unwrap_err();
        let (http, peer) = super::source_server(vec![
            "HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".into(),
        ])
        .await;
        let uncached = get_uri::load(&http, None, &fetch).await.unwrap_err();
        for error in [unchanged, uncached] {
            assert!(error.is::<NotModified>());
            assert!(!error.is::<NotFound>());
            assert_eq!(
                error.to_string(),
                "Source has not been modified since the provided \"cache\", re-use previous results"
            );
        }
        assert_eq!(peer.await.unwrap().len(), 1);
    });
}
