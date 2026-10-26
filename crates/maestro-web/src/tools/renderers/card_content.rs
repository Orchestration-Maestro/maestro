//! Owned card captions and mutually exclusive retained bodies.
use super::collapsible_card::{CollapsibleCard, Content};
use crate::dom::PreparedContent;
use crate::tools::{
    renderer_registry::ToolHeaderState,
    types::{ToolRenderResult, ToolResultMessageView, ToolUiError},
};
use maestro_app::{JsonObject, display_truthy};
use std::borrow::Cow;
/// Private localized or authored caption representation.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
pub(in crate::tools) enum Caption {
    /// Caller-authored text.
    Literal(String),
    /// Localized artifact label followed by an optional captured filename.
    Artifact(
        &'static str,
        Option<crate::tools::artifacts::artifact_pill::Pill>,
    ),
    /// Translated key followed by a literal suffix.
    Localized(&'static str, String),
}
#[cfg(target_arch = "wasm32")]
impl Caption {
    /// Translate only the fixed key.
    pub(super) fn resolve(&self, i18n: &crate::I18n) -> Result<String, ToolUiError> {
        match self {
            Self::Literal(text) => Ok(text.clone()),
            Self::Artifact(key, _) => Ok(i18n.text(key)?),
            Self::Localized(key, suffix) => Ok(format!("{}{suffix}", i18n.text(key)?)),
        }
    }
}
/// One mutually exclusive below-header branch.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
pub(in crate::tools) enum Body {
    /// Header only.
    Empty,
    /// Literal command and output delegated to the console owner.
    Console(String, crate::components::console_block::ConsoleVariant),
    /// Plain destructive result text.
    Error(String),
}
/// One owned card recipe shared by the three stateless renderer modules.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
pub(in crate::tools) struct CardContent {
    /// Result status, independent of argument streaming.
    pub(in crate::tools) state: ToolHeaderState,
    /// Tool glyph catalog name.
    pub(in crate::tools) glyph: &'static str,
    /// Owned literal or localized caption.
    pub(in crate::tools) caption: Caption,
    /// Selected below-header branch.
    pub(in crate::tools) body: Body,
}
impl CardContent {
    /// Prepare an ordinary outer card with this owned recipe.
    pub(super) fn result<C: CardOutput>(self) -> ToolRenderResult<C> {
        ToolRenderResult {
            content: C::card(self),
            is_custom: false,
        }
    }
}
/// Status deliberately does not use the default renderer's streaming rule.
pub(super) fn state<D>(result: Option<&ToolResultMessageView<'_, D>>) -> ToolHeaderState {
    match result {
        Some(result) if result.is_error() => ToolHeaderState::Error,
        Some(_) => ToolHeaderState::Complete,
        None => ToolHeaderState::Inprogress,
    }
}
/// One fallible projection shared by all selected-field reads.
pub(in crate::tools) fn field<'a>(
    params: Option<&'a JsonObject>,
    key: &str,
) -> Result<Option<Cow<'a, str>>, ToolUiError> {
    display_truthy(params.and_then(|params| params.get(key)))
        .map_err(|error| ToolUiError::Presentation(error.to_string()))
}

/// Content construction shared by branch-selecting renderer implementations.
pub(in crate::tools) trait CardOutput: Sized {
    /// Retain the selected branch in this representation.
    fn card(card: CardContent) -> Self;
    /// Retain renderer `R`'s selected disclosure branch in this representation.
    fn collapsible<R: 'static>(card: CollapsibleCard<Self>) -> Self;
    /// Retain a selected line difference in this representation.
    fn diff(props: crate::components::diff::DiffProps) -> Self;
}
impl CardOutput for PreparedContent {
    fn card(card: CardContent) -> Self {
        Box::new(card)
    }
    fn collapsible<R: 'static>(card: CollapsibleCard<Self>) -> Self {
        Box::new(Content::<R, Self>::from(card))
    }
    fn diff(props: crate::components::diff::DiffProps) -> Self {
        crate::components::diff::diff(props)
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) mod dom;
