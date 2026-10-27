//! Thread-safe native read operations invoke the retained guest callbacks.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use maestro_extensions::DiagnosticErrorInfo;
use wasmtime::component::{ComponentType, Lift, Lower};

/// One original guest read callback operation.
#[derive(ComponentType, Lower, Clone, Copy)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Operation {
    /// Read unmodified file bytes.
    #[component(name = "read-file")]
    ReadFile,
    /// Check readability.
    #[component(name = "access")]
    Access,
    /// Optional image classification.
    #[component(name = "detect-image-mime-type")]
    DetectImageMimeType,
}
/// Actual guest operation output, without a JSON byte conversion.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Output {
    /// Unmodified bytes.
    #[component(name = "bytes")]
    Bytes(Vec<u8>),
    /// Successful access.
    #[component(name = "unit")]
    Unit,
    /// Optional image MIME type, including an empty string.
    #[component(name = "mime")]
    Mime(Option<String>),
}
/// One admitted invocation of an independently retained read operation owner.
pub(crate) struct Invocation<H = Handle> {
    /// Original guest operations.
    pub(crate) handler: H,
    /// The operation to run.
    pub(crate) operation: Operation,
    /// Exact absolute authored path.
    pub(crate) path: String,
    /// Original diagnostic fields survive the canonical result.
    pub(crate) reply: Reply<Result<Output, DiagnosticErrorInfo>>,
}
/// The native thread-safe observation interface retains no local registry borrow.
pub(crate) struct Operations(pub(crate) Handle);
impl Operations {
    /// Uses the original Store lifecycle, preserving diagnostic fields independently of faults.
    async fn invoke(
        &self,
        operation: Operation,
        path: String,
    ) -> Result<Output, DiagnosticErrorInfo> {
        self.0
            .endpoint()
            .submit(|reply| {
                Call::Read(Invocation {
                    handler: self.0.clone(),
                    operation,
                    path,
                    reply,
                })
            })
            .await
            .map_err(DiagnosticErrorInfo::error)?
    }
}
impl maestro_extensions::ReadOperations for Operations {
    fn read_file(&self, path: String) -> Work<Result<Vec<u8>, DiagnosticErrorInfo>> {
        let operations = Self(self.0.clone());
        Box::pin(async move {
            match operations.invoke(Operation::ReadFile, path).await? {
                Output::Bytes(bytes) => Ok(bytes),
                _ => Err(DiagnosticErrorInfo::error(
                    "read operation result kind mismatch",
                )),
            }
        })
    }
    fn access(&self, path: String) -> Work<Result<(), DiagnosticErrorInfo>> {
        let operations = Self(self.0.clone());
        Box::pin(async move {
            match operations.invoke(Operation::Access, path).await? {
                Output::Unit => Ok(()),
                _ => Err(DiagnosticErrorInfo::error(
                    "read operation result kind mismatch",
                )),
            }
        })
    }
    fn detect_image_mime_type(
        &self,
        path: String,
    ) -> Work<Result<Option<String>, DiagnosticErrorInfo>> {
        let operations = Self(self.0.clone());
        Box::pin(async move {
            match operations
                .invoke(Operation::DetectImageMimeType, path)
                .await?
            {
                Output::Mime(mime) => Ok(mime),
                _ => Err(DiagnosticErrorInfo::error(
                    "read operation result kind mismatch",
                )),
            }
        })
    }
}

/// Thread-safe future required by the existing native read operation owner.
type Work<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send>>;
