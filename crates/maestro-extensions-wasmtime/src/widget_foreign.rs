//! Editors built with another extension's editor factory, served on the local owner through
//! their editor operations.
use crate::guest_calls::Handle;
use crate::guest_calls::editors::TextCallbackEndpoint;
use crate::local_resources::Lease;
use crate::widget_editors::{EditorBuild, EditorKind};
use crate::widget_lists::Binding;
use crate::widget_reads::{Paint, StyleEndpoint, WidgetEndpoint};
use maestro_extensions::widgets::EditorError;
use std::{rc::Rc, sync::Arc};

/// One editor operation on an editor another extension's factory built.
pub(crate) enum ForeignRequest {
    /// The text.
    GetText,
    /// Replaces the text.
    SetText(String),
    /// Replaces or removes the submission handler with an authored one.
    SetOnSubmit(Option<Handle>),
    /// The change handler.
    OnChange,
    /// Replaces or removes the change handler with an authored one.
    SetOnChange(Option<Handle>),
    /// Adds text to the history.
    AddToHistory(String),
    /// Inserts text at the cursor.
    InsertTextAtCursor(String),
    /// The text with markers expanded.
    GetExpandedText,
    /// The border style.
    BorderColor,
    /// Replaces the border style.
    SetBorderColor(Paint),
    /// Replaces the side padding.
    SetPaddingX(f64),
    /// Replaces the completion row count.
    SetAutocompleteMaxVisible(f64),
    /// Replaces the completion provider with an authored one.
    SetAutocompleteProvider(Handle),
}

/// What the editor answered, with lent callbacks and styles.
pub(crate) enum ForeignAnswered {
    /// The operation ran.
    Done,
    /// The editor lacks the operation.
    Absent,
    /// Text.
    Text(String),
    /// The change handler, lent, when one is installed.
    Handler(Option<TextCallbackEndpoint>),
    /// The border style, lent.
    Style(StyleEndpoint),
}

impl ForeignAnswered {
    /// `Done` when an optional operation ran, `Absent` when the editor lacks it.
    fn ran(ran: Result<Option<()>, EditorError>) -> Result<Self, String> {
        match ran.map_err(|error| error.to_string())? {
            Some(()) => Ok(Self::Done),
            None => Ok(Self::Absent),
        }
    }
}

impl EditorKind {
    /// Answers one editor operation, keeping the editor's absence of an optional one.
    pub(crate) fn foreign(
        &self,
        request: ForeignRequest,
        binding: &Binding<'_>,
    ) -> Result<ForeignAnswered, String> {
        let editor = self.operations();
        let registry = binding.registry;
        let failed = |error: EditorError| error.to_string();
        Ok(match request {
            ForeignRequest::GetText => {
                ForeignAnswered::Text(editor.try_get_text().map_err(failed)?)
            }
            ForeignRequest::SetText(text) => {
                editor.try_set_text(&text).map_err(failed)?;
                ForeignAnswered::Done
            }
            ForeignRequest::SetOnSubmit(handler) => {
                editor.set_on_submit(handler.map(|handle| binding.text(handle)));
                ForeignAnswered::Done
            }
            ForeignRequest::SetOnChange(handler) => {
                editor.set_on_change(handler.map(|handle| binding.text(handle)));
                ForeignAnswered::Done
            }
            ForeignRequest::OnChange => ForeignAnswered::Handler(
                editor
                    .on_change()
                    .map(|callback| registry.lend_text(callback))
                    .transpose()?,
            ),
            ForeignRequest::GetExpandedText => {
                match editor.try_get_expanded_text().map_err(failed)? {
                    Some(text) => ForeignAnswered::Text(text),
                    None => ForeignAnswered::Absent,
                }
            }
            ForeignRequest::BorderColor => match editor.border_color() {
                Some(style) => ForeignAnswered::Style(registry.lend_style(style)?),
                None => ForeignAnswered::Absent,
            },
            request => optional(editor.as_ref(), request, binding)?,
        })
    }
}

/// Runs one optional operation, answering `Absent` when the editor lacks it.
fn optional(
    editor: &dyn maestro_extensions::widgets::EditorComponent<Signal = maestro_extensions::Cancellation>,
    request: ForeignRequest,
    binding: &Binding<'_>,
) -> Result<ForeignAnswered, String> {
    match request {
        ForeignRequest::AddToHistory(text) => {
            ForeignAnswered::ran(editor.try_add_to_history(&text))
        }
        ForeignRequest::InsertTextAtCursor(text) => {
            ForeignAnswered::ran(editor.try_insert_text_at_cursor(&text))
        }
        ForeignRequest::SetBorderColor(paint) => {
            let style = binding.registry.paint(paint, binding.failure)?;
            ForeignAnswered::ran(Ok(editor.set_border_color(style)))
        }
        ForeignRequest::SetPaddingX(padding) => {
            ForeignAnswered::ran(editor.try_set_padding_x(padding))
        }
        ForeignRequest::SetAutocompleteMaxVisible(max) => {
            ForeignAnswered::ran(editor.try_set_autocomplete_max_visible(max))
        }
        ForeignRequest::SetAutocompleteProvider(handle) => {
            let provider =
                crate::guest_provider::GuestProvider::new(handle, binding.registry.clone());
            ForeignAnswered::ran(editor.try_set_autocomplete_provider(Rc::new(provider)))
        }
        _ => Err("expected an optional editor operation".to_owned()),
    }
}

/// One build with another extension's editor factory.
pub(crate) struct ForeignBuild {
    /// The lent factory.
    pub(crate) factory: FactoryEndpoint,
    /// The terminal, styles and keybindings.
    pub(crate) editor: EditorBuild,
    /// Returns the built editor's widget and whether it extends the custom editor.
    pub(crate) reply: std::sync::mpsc::Sender<Result<(WidgetEndpoint, bool), String>>,
}

/// Store-safe identity of an editor factory another extension set.
#[derive(Clone)]
pub(crate) struct FactoryEndpoint(pub(crate) Arc<Lease>);
