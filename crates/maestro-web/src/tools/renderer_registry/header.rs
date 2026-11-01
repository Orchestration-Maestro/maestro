//! Shared status icons, rich captions and collapsible header controls.
use super::ToolHeaderState;
use crate::{dom::PreparedContent, icons::IconNode};
use std::{cell::RefCell, rc::Rc};

#[cfg(target_arch = "wasm32")]
pub(crate) mod dom;

/// Literal text or independently executable rich caption content.
pub enum ToolHeaderText<C = PreparedContent> {
    /// Literal caption, never localized by the header.
    Text(String),
    /// Owned rich child with its own retained state.
    Content(C),
}
/// Live body and chevron references read together at activation.
#[derive(Clone, Default)]
pub struct ToolHeaderRefs {
    /// Current disclosure body, which may be rebound independently of the header.
    pub content: Rc<RefCell<Option<web_sys::HtmlElement>>>,
    /// Current chevron container.
    pub chevron: Rc<RefCell<Option<web_sys::HtmlElement>>>,
}
/// Prepare a plain status header with literal or native child content.
#[must_use]
pub fn render_header(
    state: ToolHeaderState,
    tool_icon: IconNode,
    text: ToolHeaderText,
) -> PreparedContent {
    Box::new(Header {
        state,
        tool_icon,
        text,
        disclosure: None,
    })
}
/// Prepare a native toggle; initial expansion selects icons, never rewrites the body.
#[must_use]
pub fn render_collapsible_header(
    state: ToolHeaderState,
    tool_icon: IconNode,
    text: ToolHeaderText,
    refs: ToolHeaderRefs,
    default_expanded: bool,
) -> PreparedContent {
    Box::new(Header {
        state,
        tool_icon,
        text,
        disclosure: Some((refs, default_expanded)),
    })
}
/// Immutable header recipe.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
struct Header {
    /// Current result state.
    state: ToolHeaderState,
    /// Caller-selected tool geometry.
    tool_icon: IconNode,
    /// Authored caption or independently owned native child.
    text: ToolHeaderText,
    /// Live targets and initial chevron selection.
    disclosure: Option<(ToolHeaderRefs, bool)>,
}
