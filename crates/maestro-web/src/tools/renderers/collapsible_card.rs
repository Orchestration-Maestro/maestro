//! Retained document-card disclosures and independently owned file tiles.
use super::card_content::{Caption, CardOutput};
use crate::{
    Attachment,
    components::console_block::ConsoleVariant,
    dom::PreparedContent,
    tools::{renderer_registry::ToolHeaderState, types::ToolRenderResult},
};

/// Selected subtree identity: result arrival replaces the pending body.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::tools) enum Phase {
    /// Streamed parameters without a result.
    Pending,
    /// Completed execution, successful or failed.
    Result,
    /// Artifact's selected template, independent of its status animation.
    Artifact(ArtifactTemplate),
}
/// Artifact templates which retain the same header and disclosure subtree.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::tools) enum ArtifactTemplate {
    /// No selected command.
    Preparing,
    /// Pending create or rewrite.
    PendingCode,
    /// Pending update.
    PendingDiff,
    /// Pending get or logs.
    PendingEmpty,
    /// Pending delete or unknown command.
    PendingPlain,
    /// Successful create or rewrite.
    ResultCode,
    /// Successful update.
    ResultDiff,
    /// Successful get.
    ResultGet,
    /// Successful logs.
    ResultLogs,
    /// Successful delete or unknown command.
    ResultPlain,
    /// Failed create, update or rewrite.
    ErrorEdit,
    /// All other failures.
    ErrorPlain,
}
/// Owned body operands, independent of the preparation call; `C` is the retained child representation.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
pub(in crate::tools) struct Body<C = PreparedContent> {
    /// Pending or completed subtree.
    pub(in crate::tools) phase: Phase,
    /// Literal URL value, including a present empty pending URL.
    pub(in crate::tools) url: Option<String>,
    /// Literal listing with its explicit language.
    pub(in crate::tools) code: Option<(String, &'static str)>,
    /// Optional nonempty console output.
    pub(in crate::tools) console: Option<(String, ConsoleVariant)>,
    /// Selected line difference recipe.
    pub(in crate::tools) diff: Option<C>,
    /// Plain destructive text inside the disclosure.
    pub(in crate::tools) error: Option<String>,
    /// Localized replacement for an empty selected output.
    pub(in crate::tools) fallback: Option<&'static str>,
}
/// One immutable card recipe shared by document tools.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
pub(in crate::tools) struct CollapsibleCard<C = PreparedContent> {
    /// Result status takes precedence over streaming.
    pub(in crate::tools) state: ToolHeaderState,
    /// Catalog glyph.
    pub(in crate::tools) glyph: &'static str,
    /// Literal or localized caption.
    pub(in crate::tools) caption: Caption,
    /// Absent until parameters arrive.
    pub(in crate::tools) body: Option<Body<C>>,
    /// Ordered downloadable file records.
    pub(in crate::tools) attachments: Vec<Download>,
    /// Template identity when no disclosure exists.
    pub(in crate::tools) plain_phase: Option<Phase>,
    /// Plain destructive output outside any disclosure.
    pub(in crate::tools) error: Option<String>,
}
/// Renderer-specific erased identity for one shared implementation per representation.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
pub(in crate::tools) struct Content<R, C = PreparedContent> {
    /// Owned selected card operands.
    card: CollapsibleCard<C>,
    /// Nominal renderer identity used by retained row erasure.
    renderer: std::marker::PhantomData<fn() -> R>,
}
impl<R, C> From<CollapsibleCard<C>> for Content<R, C> {
    fn from(card: CollapsibleCard<C>) -> Self {
        Self {
            card,
            renderer: std::marker::PhantomData,
        }
    }
}
/// Original download with its tool-selected text-preview policy.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
pub(in crate::tools) struct Download {
    /// Original encoded record, before decoding a preview.
    pub(in crate::tools) attachment: Attachment,
    /// Whether this MIME type supports extracted text.
    pub(in crate::tools) text: bool,
}
#[cfg(any(target_arch = "wasm32", test))]
impl Download {
    /// Decode only when the retained tile first observes changed original bytes.
    pub(in crate::tools) fn record(&self) -> Attachment {
        let mut record = self.attachment.clone();
        if self.text && !record.content.is_empty() {
            record.extracted_text = crate::utils::base64::decode(&record.content)
                .ok()
                .map(|bytes| maestro_app::decode_utf8(&bytes).into_owned());
        }
        record
    }
}
impl<C: CardOutput> CollapsibleCard<C> {
    /// Prepare renderer `R`'s ordinary outer tool card.
    pub(in crate::tools) fn result<R: 'static>(self) -> ToolRenderResult<C> {
        ToolRenderResult {
            content: C::collapsible::<R>(self),
            is_custom: false,
        }
    }
}
#[cfg(target_arch = "wasm32")]
mod dom;
