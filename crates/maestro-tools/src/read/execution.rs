//! Cancellable dispatch of one read to its text or image route.

use super::{ReadContext, ReadOperations, ReadToolDetails, ReadToolInput, text::read_text};
use crate::utils::{
    diagnostics::{aborted, blocking, guarded},
    image_resize::{ImageResizeOptions, format_dimension_note, resize_image},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use maestro_agent::AgentToolResult;
use maestro_models::{
    Cancellation, DiagnosticErrorInfo, ImageContent, Model, ModelInput, TextContent, UserBlock,
};
use std::sync::Arc;

/// Sent in place of an image that cannot be fitted.
const OMITTED: &str = "[Image omitted: could not be resized below the inline image size limit.]";
/// Sent beside an image when the active model cannot accept images.
const NON_VISION: &str =
    "[Current model does not support images. The image will be omitted from this request.]";

/// Build the result for an image: header, optional notes and the attachment.
///
/// Decoding, resizing and encoding are CPU-bound; callers run this on the blocking pool.
/// A `model` without image input adds a note.
fn image_result(
    bytes: &[u8],
    mime_type: String,
    auto_resize: bool,
    model: Option<&Model>,
) -> AgentToolResult<ReadToolDetails> {
    let image = ImageContent {
        data: STANDARD.encode(bytes),
        mime_type,
    };
    let header = |mime: &str| format!("Read image file [{mime}]");
    let fitted = auto_resize.then(|| resize_image(&image, ImageResizeOptions::default()));
    let (mut lines, attachment) = match fitted {
        None => (vec![header(&image.mime_type)], Some(image)),
        Some(None) => (vec![header(&image.mime_type), OMITTED.into()], None),
        Some(Some(fit)) => (
            std::iter::once(header(&fit.mime_type))
                .chain(format_dimension_note(&fit))
                .collect(),
            Some(ImageContent {
                data: fit.data,
                mime_type: fit.mime_type,
            }),
        ),
    };
    if model.is_some_and(|model| !model.input.contains(&ModelInput::Image)) {
        lines.push(NON_VISION.into());
    }
    let content = std::iter::once(UserBlock::Text(TextContent::new(lines.join("\n"))))
        .chain(attachment.map(UserBlock::Image))
        .collect();
    AgentToolResult {
        content,
        details: None,
        terminate: None,
    }
}

/// The settings every read shares.
pub(super) struct Reader {
    /// File access, native or supplied.
    pub(super) operations: Arc<dyn ReadOperations>,
    /// Whether images are fitted to the inline limits.
    pub(super) auto_resize: bool,
}

impl Reader {
    /// Read `absolute_path`, honouring `signal` before every operation.
    pub(super) async fn read<C: ReadContext>(
        &self,
        absolute_path: String,
        input: ReadToolInput,
        signal: Option<Cancellation>,
        context: Option<C>,
    ) -> Result<AgentToolResult<ReadToolDetails>, DiagnosticErrorInfo> {
        let (signal, operations) = (signal.as_ref(), &self.operations);
        guarded(signal, || operations.access(absolute_path.clone())).await?;
        let mime = guarded(signal, || {
            operations.detect_image_mime_type(absolute_path.clone())
        })
        .await?;
        let model = context.and_then(|context| context.model());
        let bytes = guarded(signal, || operations.read_file(absolute_path)).await?;
        let result = match mime.filter(|mime| !mime.is_empty()) {
            Some(mime) => {
                let auto_resize = self.auto_resize;
                blocking(move || Ok(image_result(&bytes, mime, auto_resize, model.as_deref())))
                    .await
            }
            None => read_text(&bytes, &input.path, input.offset, input.limit)
                .map(|(text, details)| AgentToolResult::text_result(text, details)),
        };
        if signal.is_some_and(Cancellation::is_aborted) {
            return Err(aborted());
        }
        result
    }
}
