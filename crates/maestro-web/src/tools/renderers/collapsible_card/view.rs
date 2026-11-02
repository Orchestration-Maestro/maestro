//! Rendered values of a prepared disclosure card, projected for native tests.
use super::{Body, CollapsibleCard, Content, Download};
use crate::{
    dom::PreparedContent,
    tools::{
        artifacts::ArtifactsToolRenderer, extract_document::ExtractDocumentRenderer,
        javascript_repl::JavaScriptReplRenderer, renderers::card_content::view::variant,
    },
};
use serde_json::{Value, json};
use std::any::Any;

impl Download {
    /// The tile's file name and the text preview it opens with.
    fn view(&self) -> Value {
        let record = self.record();
        json!({"fileName": record.file_name, "extractedText": record.extracted_text})
    }
}
impl Body<PreparedContent> {
    /// Template identity and every subsection the disclosure shows.
    fn view(&self) -> Value {
        json!({
            "phase": format!("{:?}", self.phase),
            "url": self.url,
            "code": self.code.as_ref().map(|(code, language)| json!({"code": code, "language": language})),
            "console": self.console.as_ref().map(|(text, color)| json!({"text": text, "variant": variant(*color)})),
            "diff": self.diff.as_deref().and_then(crate::test_support::prepared_view),
            "error": self.error,
            "fallback": self.fallback,
        })
    }
}
impl CollapsibleCard<PreparedContent> {
    /// Header status, glyph and caption with the disclosure, file tiles and plain output.
    fn view(&self) -> Value {
        json!({
            "state": format!("{:?}", self.state),
            "glyph": self.glyph,
            "caption": self.caption.view(),
            "body": self.body.as_ref().map(Body::view),
            "attachments": self.attachments.iter().map(Download::view).collect::<Vec<_>>(),
            "plainPhase": self.plain_phase.map(|phase| format!("{phase:?}")),
            "error": self.error,
        })
    }
}
/// The disclosure card a built-in document or artifact renderer prepared.
pub(in crate::tools) fn prepared(content: &dyn Any) -> Option<Value> {
    content
        .downcast_ref::<Content<JavaScriptReplRenderer>>()
        .map(|content| &content.card)
        .or_else(|| {
            content
                .downcast_ref::<Content<ExtractDocumentRenderer>>()
                .map(|content| &content.card)
        })
        .or_else(|| {
            content
                .downcast_ref::<Content<ArtifactsToolRenderer>>()
                .map(|content| &content.card)
        })
        .map(CollapsibleCard::view)
}
