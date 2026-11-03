//! Readiness is an actual browser-output or EOF signal.

use super::*;
use crate::browser_driver::tests::block_on;

/// Diagnostics precede the exact endpoint line; absence retains the browser's failure text.
#[test]
fn browser_readiness_requires_an_advertised_endpoint() {
    block_on(async {
        let mut lines = BufReader::new(
            &b"diagnostic\nDevTools listening on ws://127.0.0.1/devtools/browser/id\r\n"[..],
        )
        .lines();
        assert_eq!(
            ready(&mut lines).await.unwrap(),
            "ws://127.0.0.1/devtools/browser/id"
        );
        let mut failed =
            BufReader::new(&b"chrome_crashpad_handler: --database is required\n"[..]).lines();
        let error = ready(&mut failed).await.unwrap_err();
        assert!(error.to_string().contains("--database is required"));
        let mut empty = BufReader::new(&b""[..]).lines();
        assert!(ready(&mut empty).await.is_err());
    });
}

/// Caller switches become the library's key and value, so Chromium sees one `--` and merged lists.
#[test]
fn launch_switches_split_into_key_and_value() {
    assert_eq!(
        switch("--disable-features=IsolateSandboxedIframes").unwrap(),
        ("disable-features", Some("IsolateSandboxedIframes"))
    );
    assert_eq!(
        switch("--js-flags=--expose-gc").unwrap(),
        ("js-flags", Some("--expose-gc"))
    );
    assert_eq!(
        switch("--no-startup-window").unwrap(),
        ("no-startup-window", None)
    );
}

/// A switch without exactly one leading `--` and a key is refused before launch.
#[test]
fn launch_switches_without_the_required_prefix_are_refused() {
    for argument in [
        "disable-features=x",
        "-disable-features=x",
        "----disable-features=x",
        "--",
        "--=x",
        "",
    ] {
        let error = switch(argument).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{argument}");
        assert!(error.to_string().contains("--key"), "{argument}: {error}");
    }
}
