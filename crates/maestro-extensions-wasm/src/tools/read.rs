//! Supplied read tool data.
use super::truncate::TruncationResult;
use crate::Presence;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the read tool.
pub struct ReadToolInput {
    /// Path.
    pub path: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Offset.
    pub offset: Presence<f64>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Limit.
    pub limit: Presence<f64>,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
/// Observations supplied by the read tool with guest field presence.
pub type ReadToolDetails = maestro_tool::tools::read::ReadToolDetails<Presence<TruncationResult>>;

/// Fallible asynchronous work supplied by a tool operation.
pub type OperationFuture<'a, T> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<T, crate::DiagnosticErrorInfo>> + 'a>,
>;
/// Optional image classification performed before the bytes are read.
pub type DetectImageMimeType =
    std::rc::Rc<dyn Fn(String) -> OperationFuture<'static, Option<String>>>;
/// Replaceable file observations consumed by the native read owner.
#[derive(Clone)]
pub struct ReadOperations {
    /// Reads the original bytes without text conversion.
    pub read_file: std::rc::Rc<dyn Fn(String) -> OperationFuture<'static, Vec<u8>>>,
    /// Rejects a file that cannot be read.
    pub access: std::rc::Rc<dyn Fn(String) -> OperationFuture<'static, ()>>,
    /// Classifies images; absence means text.
    pub detect_image_mime_type: Option<DetectImageMimeType>,
}
/// Read construction choices; omitted operations use native file access.
#[derive(Default)]
pub struct ReadToolOptions {
    /// Whether images are fitted to inline limits; absence means yes.
    pub auto_resize_images: Option<bool>,
    /// Independently retained replacement file observations.
    pub operations: Option<ReadOperations>,
}
