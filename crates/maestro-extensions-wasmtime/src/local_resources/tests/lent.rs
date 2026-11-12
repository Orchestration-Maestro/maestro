//! Lent host styles, highlighting and callbacks end with their capabilities.
use super::ended_endpoint;
use crate::{
    guest_calls::editors::TextCallbackEndpoint,
    local_resources::{Capability, Lease},
    widget_lists::{RowStyleEndpoint, SubmenuDoneEndpoint},
    widget_reads::{HighlighterEndpoint, StyleEndpoint},
};
use std::{rc::Rc, sync::Arc};

/// Releases the capability `endpoint` was lent for and returns what `operation` settles with.
fn ended<E: Send + 'static>(
    capability: Capability,
    endpoint: fn(Arc<Lease>) -> E,
    operation: impl FnOnce(E) -> String + Send + 'static,
) -> String {
    let [error] = ended_endpoint(capability, endpoint, move |endpoint| [operation(endpoint)]);
    error
}

#[test]
fn released_style_rejects_with_its_ended_text() {
    let error = ended(
        Capability::Style(Rc::new(str::to_owned)),
        StyleEndpoint,
        |style| style.apply("text".into()).unwrap_err(),
    );
    assert_eq!(error, "extension style resource ended");
}

#[test]
fn released_highlighter_rejects_with_its_ended_text() {
    let error = ended(
        Capability::Highlighter(Rc::new(|_, _| Vec::new())),
        HighlighterEndpoint,
        |highlighter| highlighter.highlight("code".into(), None).unwrap_err(),
    );
    assert_eq!(error, "extension style resource ended");
}

#[test]
fn released_row_style_rejects_with_its_ended_text() {
    let error = ended(
        Capability::RowStyle(Rc::new(|text, _| text.to_owned())),
        RowStyleEndpoint,
        |style| style.apply("text".into(), true).unwrap_err(),
    );
    assert_eq!(error, "extension style resource ended");
}

#[test]
fn released_submenu_closer_rejects_with_its_ended_text() {
    let error = ended(
        Capability::SubmenuDone(Rc::new(|_| {})),
        SubmenuDoneEndpoint,
        |done| done.done(None).result().unwrap_err(),
    );
    assert_eq!(error, "extension style resource ended");
}

#[test]
fn released_text_callback_rejects_with_its_ended_text() {
    let error = ended(
        Capability::Text(Rc::new(|_| {})),
        TextCallbackEndpoint,
        |callback| callback.call("text".into()).result().unwrap_err(),
    );
    assert_eq!(error, "extension style resource ended");
}
