//! Borrow the authoritative message; project only its details representation.
use maestro_app::ToolResultMessage;
use maestro_app::{ToolRenderResultLike, UserBlock};
use serde_json::Value;
/// Object-safe access to the authoritative message envelope.
trait MessageHeader {
    /// Message role.
    fn role(&self) -> &'static str;
    /// Original call identity.
    fn tool_call_id(&self) -> &str;
    /// Original tool name.
    fn tool_name(&self) -> &str;
    /// Original timestamp.
    fn timestamp(&self) -> f64;
}
impl<D> MessageHeader for ToolResultMessage<D> {
    fn role(&self) -> &'static str {
        "toolResult"
    }
    fn tool_call_id(&self) -> &str {
        &self.tool_call_id
    }
    fn tool_name(&self) -> &str {
        &self.tool_name
    }
    fn timestamp(&self) -> f64 {
        self.timestamp
    }
}
/// One borrowed message envelope, with borrowed native or operation-owned decoded details.
pub struct ToolResultMessageView<'a, D = Value> {
    /// Authoritative message header.
    head: &'a dyn MessageHeader,
    /// Existing borrowing result projection.
    payload: ToolRenderResultLike<'a, D>,
}
impl<'a, D> From<&'a ToolResultMessage<D>> for ToolResultMessageView<'a, D> {
    fn from(result: &'a ToolResultMessage<D>) -> Self {
        Self {
            head: result,
            payload: ToolRenderResultLike {
                content: &result.content,
                details: result.details.as_ref(),
                terminate: None,
                is_error: Some(result.is_error),
            },
        }
    }
}
impl<D> ToolResultMessageView<'_, D> {
    /// Message role.
    #[must_use]
    pub fn role(&self) -> &'static str {
        self.head.role()
    }
    /// Original call identity.
    #[must_use]
    pub fn tool_call_id(&self) -> &str {
        self.head.tool_call_id()
    }
    /// Original tool name.
    #[must_use]
    pub fn tool_name(&self) -> &str {
        self.head.tool_name()
    }
    /// Original timestamp.
    #[must_use]
    pub fn timestamp(&self) -> f64 {
        self.head.timestamp()
    }
    /// Ordered text and image blocks.
    #[must_use]
    pub fn content(&self) -> &[UserBlock] {
        self.payload.content
    }
    /// Native details, preserving absence.
    #[must_use]
    pub fn details(&self) -> Option<&D> {
        self.payload.details
    }
    /// Original error flag.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.payload.is_error.unwrap_or(false)
    }
    /// Borrow the same envelope with operation-owned decoded details.
    pub(crate) fn with_details<'b, T>(
        &'b self,
        details: Option<&'b T>,
    ) -> ToolResultMessageView<'b, T> {
        ToolResultMessageView {
            head: self.head,
            payload: self.payload.with_details(details),
        }
    }
}
