//! Controlled access failures close the observation set before any read.
use super::resolved_preview;
use crate::read::ReadOperations;
use maestro_models::{BoxFuture, DiagnosticCode, DiagnosticErrorInfo};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fail access and count any forbidden reads.
struct FailedAccess {
    failure: DiagnosticErrorInfo,
    reads: AtomicUsize,
}
impl ReadOperations for FailedAccess {
    fn access(&self, _: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(Err(self.failure.clone())))
    }
    fn read_file(&self, _: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(b"hello".to_vec())))
    }
}
#[test]
fn preview_access_failure_stops_before_read() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for (code, detail) in [
        (Some(DiagnosticCode::Number(5.0)), "Error code: 5"),
        (
            Some(DiagnosticCode::Text("ENOENT".into())),
            "Error code: ENOENT",
        ),
        (None, "disk offline"),
    ] {
        let operations = FailedAccess {
            failure: DiagnosticErrorInfo {
                code,
                ..DiagnosticErrorInfo::error("disk offline")
            },
            reads: AtomicUsize::new(0),
        };
        let result = runtime.block_on(resolved_preview(
            "@authored",
            "resolved".into(),
            &[],
            &operations,
        ));
        assert_eq!(
            result.unwrap_err().error,
            format!("Could not edit file: @authored. {detail}.")
        );
        assert_eq!(operations.reads.load(Ordering::SeqCst), 0);
    }
}
