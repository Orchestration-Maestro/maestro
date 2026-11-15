//! The component host's requests on the authored editor it shows.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::Value;
use wasmtime::component::Resource;

use super::editor_host::{self as described, EditorOp};
use super::host::{Harness, borrow};
use super::widget_component::{StyleResource, TextCallbackResource};
use crate::bindings::host_side::maestro::extension::host;

/// The wire form of a request, lending its named callbacks and styles.
fn wire(harness: &mut Harness, op: EditorOp) -> wasmtime::Result<host::AuthoredEditorRequest> {
    use host::AuthoredEditorRequest as Wire;
    let state = harness.store.data_mut();
    let mut lend = |name: Option<String>| {
        name.map(|name| state.push(TextCallbackResource(name)))
            .transpose()
    };
    Ok(match op {
        EditorOp::GetText => Wire::GetText,
        EditorOp::SetText { text } => Wire::SetText(text),
        EditorOp::SetOnSubmit { callback } => Wire::SetOnSubmit(lend(callback)?),
        EditorOp::OnChange => Wire::OnChange,
        EditorOp::SetOnChange { callback } => Wire::SetOnChange(lend(callback)?),
        EditorOp::AddToHistory { text } => Wire::AddToHistory(text),
        EditorOp::InsertTextAtCursor { text } => Wire::InsertTextAtCursor(text),
        EditorOp::GetExpandedText => Wire::GetExpandedText,
        EditorOp::BorderColor => Wire::BorderColor,
        EditorOp::SetBorderColor { role } => {
            Wire::SetBorderColor(harness.store.data_mut().push(StyleResource(role))?)
        }
        EditorOp::SetPaddingX { padding } => Wire::SetPaddingX(padding),
        EditorOp::SetAutocompleteMaxVisible { max } => Wire::SetAutocompleteMaxVisible(max),
        EditorOp::SetAutocompleteProvider => {
            let base = super::provider_host::Lent::Base;
            Wire::SetAutocompleteProvider(harness.store.data_mut().push(base)?)
        }
    })
}

/// Runs one request on the authored editor `editor`; an answered handler is called with the
/// probe text and an answered style styles the border text, each through its own export.
pub async fn request(
    harness: &mut Harness,
    editor: u32,
    op: EditorOp,
) -> wasmtime::Result<Result<Value, String>> {
    let request = wire(harness, op)?;
    let emitter = harness
        .store
        .data_mut()
        .table
        .push(super::bus_host::Emitter { key: 0 })?;
    let exports = harness.exports.clone();
    let rep = emitter.rep();
    let described = harness
        .store
        .run_concurrent(
            async |accessor| -> wasmtime::Result<Result<Value, String>> {
                let answer = exports
                    .call_invoke_editor(
                        accessor,
                        borrow(editor),
                        request,
                        Resource::new_borrow(rep),
                    )
                    .await?;
                let answer = match answer {
                    Ok(answer) => answer,
                    Err(error) => return Ok(Err(error)),
                };
                Ok(match answer {
                    host::AuthoredEditorAnswer::Done => Ok(described::done()),
                    host::AuthoredEditorAnswer::Absent => Ok(described::absent()),
                    host::AuthoredEditorAnswer::Text(text) => Ok(described::text(&text)),
                    host::AuthoredEditorAnswer::Handler(None) => Ok(described::handler(false)),
                    host::AuthoredEditorAnswer::Handler(Some(handler)) => {
                        let event = host::WidgetEvent::Text(described::PROBE.to_owned());
                        let called = exports
                            .call_invoke_widget(
                                accessor,
                                borrow(handler.rep()),
                                event,
                                Resource::new_borrow(rep),
                            )
                            .await?;
                        called.map(|_| described::handler(true))
                    }
                    host::AuthoredEditorAnswer::Style(style) => exports
                        .call_invoke_style(
                            accessor,
                            borrow(style.rep()),
                            described::BORDER.to_owned(),
                        )
                        .await?
                        .map(|styled| described::style(&styled)),
                })
            },
        )
        .await??;
    harness.store.data_mut().table.delete(emitter)?;
    Ok(described)
}

/// Runs one request on the authored provider `provider`, transferring the lent flag `signal`
/// to a suggestion request.
pub async fn provider(
    harness: &mut Harness,
    provider: u32,
    op: super::provider_host::ProviderOp,
    signal: Option<u32>,
) -> wasmtime::Result<Result<Value, String>> {
    use super::provider_host::{self as hosted, ProviderOp};
    let request = match op {
        ProviderOp::Suggestions {
            lines,
            cursor,
            force,
            ..
        } => host::ProviderRequest::Suggestions((lines, cursor, force)),
        ProviderOp::Apply {
            lines,
            cursor,
            item: (value, label, description),
            prefix,
        } => {
            let item = host::AutocompleteItem {
                value,
                label,
                description,
            };
            host::ProviderRequest::Apply((lines, cursor, item, prefix))
        }
        ProviderOp::Trigger { lines, cursor } => host::ProviderRequest::Trigger((lines, cursor)),
    };
    let exports = harness.exports.clone();
    let signal = signal.map(Resource::new_own);
    let answer = harness
        .store
        .run_concurrent(async |accessor| {
            exports
                .call_invoke_autocomplete(accessor, borrow(provider), request, signal)
                .await
        })
        .await??;
    Ok(answer.map(|answer| match answer {
        host::ProviderAnswer::Suggestions(found) => hosted::described(found.map(|found| {
            let items = found.items.into_iter();
            let items = items.map(|item| (item.value, item.label, item.description));
            (items.collect(), found.prefix)
        })),
        host::ProviderAnswer::Applied(result) => {
            hosted::applied(&result.lines, result.cursor_line, result.cursor_col)
        }
        host::ProviderAnswer::Trigger(decision) => hosted::decided(decision),
    }))
}
