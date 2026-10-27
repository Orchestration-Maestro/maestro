//! Frame writer requests through the generated host bindings.

use super::{Generated, route};
use crate::bindings::maestro::extension::host;
use crate::component_adapter::{TerminalRequest, WriterAnswer, WriterRequest, WriterTarget};
use crate::types::ExtensionResult;

/// The wire form of a target.
fn target<'a>(target: &WriterTarget<'a, Generated>) -> host::WriterTarget<'a> {
    match *target {
        WriterTarget::Widget(widget) => host::WriterTarget::Widget(widget),
        WriterTarget::Component(component) => host::WriterTarget::Component(component),
        WriterTarget::Lent(id) => host::WriterTarget::Lent(id),
    }
}

/// The wire form of a terminal request.
fn terminal(request: TerminalRequest<'_>) -> host::TerminalRequest {
    match request {
        TerminalRequest::Stop => host::TerminalRequest::Stop,
        TerminalRequest::Write(data) => host::TerminalRequest::Write(data.to_owned()),
        TerminalRequest::Columns => host::TerminalRequest::Columns,
        TerminalRequest::Rows => host::TerminalRequest::Rows,
        TerminalRequest::KittyProtocolActive => host::TerminalRequest::KittyProtocolActive,
        TerminalRequest::MoveBy(lines) => host::TerminalRequest::MoveBy(lines),
        TerminalRequest::HideCursor => host::TerminalRequest::HideCursor,
        TerminalRequest::ShowCursor => host::TerminalRequest::ShowCursor,
        TerminalRequest::ClearLine => host::TerminalRequest::ClearLine,
        TerminalRequest::ClearFromCursor => host::TerminalRequest::ClearFromCursor,
        TerminalRequest::ClearScreen => host::TerminalRequest::ClearScreen,
        TerminalRequest::SetTitle(title) => host::TerminalRequest::SetTitle(title.to_owned()),
        TerminalRequest::SetProgress(active) => host::TerminalRequest::SetProgress(active),
    }
}

/// The wire form of a frame writer request.
fn wire<'a>(request: &WriterRequest<'a, Generated>) -> host::WriterRequest<'a> {
    match *request {
        WriterRequest::GetShowHardwareCursor => host::WriterRequest::GetShowHardwareCursor,
        WriterRequest::SetShowHardwareCursor(enabled) => {
            host::WriterRequest::SetShowHardwareCursor(enabled)
        }
        WriterRequest::GetClearOnShrink => host::WriterRequest::GetClearOnShrink,
        WriterRequest::SetClearOnShrink(enabled) => host::WriterRequest::SetClearOnShrink(enabled),
        WriterRequest::SetFocus(ref focus) => {
            host::WriterRequest::SetFocus(focus.as_ref().map(target))
        }
        WriterRequest::ShowOverlay(ref component, options) => {
            host::WriterRequest::ShowOverlay((target(component), options))
        }
        WriterRequest::HideOverlay => host::WriterRequest::HideOverlay,
        WriterRequest::HasOverlay => host::WriterRequest::HasOverlay,
        WriterRequest::Start => host::WriterRequest::Start,
        WriterRequest::Stop => host::WriterRequest::Stop,
        WriterRequest::RequestRender(force) => host::WriterRequest::RequestRender(force),
        WriterRequest::SetOnDebug(callback) => host::WriterRequest::SetOnDebug(callback),
        WriterRequest::Terminal(request) => host::WriterRequest::Terminal(terminal(request)),
        WriterRequest::TerminalStart(input, resize) => {
            host::WriterRequest::TerminalStart((input, resize))
        }
        WriterRequest::LentLive(id) => host::WriterRequest::LentLive(id),
    }
}

/// Runs one frame writer request and the nested calls the host makes first.
pub(super) fn request(
    widget: &host::Widget,
    request: &WriterRequest<'_, Generated>,
) -> ExtensionResult<WriterAnswer<Generated>> {
    match route::value(&widget.writer(&wire(request)))? {
        host::RouteValue::Unit => Ok(WriterAnswer::Done),
        host::RouteValue::Boolean(flag) => Ok(WriterAnswer::Flag(flag)),
        host::RouteValue::Count(count) => Ok(WriterAnswer::Count(count)),
        host::RouteValue::Overlay(handle) => Ok(WriterAnswer::Overlay(handle)),
        _ => Err("extension frame writer result kind mismatch".into()),
    }
}
