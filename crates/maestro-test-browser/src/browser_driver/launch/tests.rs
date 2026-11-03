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
