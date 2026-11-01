//! Canonical imports of the editor factories other extensions set, and of the editors they
//! build.
use crate::callback_imports::{CallbackView, optional, retain};
use crate::guest_calls::Callback;
use crate::import_route::push;
use crate::keys_reads::KeysEndpoint;
use crate::local_resources::Message;
use crate::local_resources::effects::TuiEndpoint;
use crate::ui_reads::{Endpoint, FactoryRequest, Request};
use crate::widget_editor_imports::{EditorStyles, editor_build};
use crate::widget_editors::EditorRequest;
use crate::widget_foreign::{FactoryEndpoint, ForeignBuild, ForeignRequest};
use crate::widget_imports::{StyleSource, paint, widget};
use crate::widget_reads::{Request as WidgetRequest, WidgetEndpoint, WidgetMessage};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Lower, Resource},
};

/// The current editor factory, as the guest receives it.
#[derive(ComponentType, Lower)]
#[component(variant)]
enum Current {
    /// One this extension set, by identity.
    #[component(name = "authored")]
    Authored(u32),
    /// Another extension's, lent.
    #[component(name = "host")]
    Host(Resource<FactoryEndpoint>),
}

/// One editor operation as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
enum WireRequest {
    /// The text.
    #[component(name = "get-text")]
    GetText,
    /// Replaces the text.
    #[component(name = "set-text")]
    SetText(String),
    /// Replaces or removes the submission handler.
    #[component(name = "set-on-submit")]
    SetOnSubmit(Option<Resource<Callback>>),
    /// The change handler.
    #[component(name = "on-change")]
    OnChange,
    /// Replaces or removes the change handler.
    #[component(name = "set-on-change")]
    SetOnChange(Option<Resource<Callback>>),
    /// Adds text to the history.
    #[component(name = "add-to-history")]
    AddToHistory(String),
    /// Inserts text at the cursor.
    #[component(name = "insert-text-at-cursor")]
    InsertTextAtCursor(String),
    /// The text with markers expanded.
    #[component(name = "get-expanded-text")]
    GetExpandedText,
    /// The border style.
    #[component(name = "border-color")]
    BorderColor,
    /// Replaces the border style.
    #[component(name = "set-border-color")]
    SetBorderColor(StyleSource),
    /// Replaces the side padding.
    #[component(name = "set-padding-x")]
    SetPaddingX(f64),
    /// Replaces the completion row count.
    #[component(name = "set-autocomplete-max-visible")]
    SetAutocompleteMaxVisible(f64),
    /// Replaces the completion provider.
    #[component(name = "set-autocomplete-provider")]
    SetAutocompleteProvider(Resource<Callback>),
}

/// The operation a wire request names, retaining its authored handlers and styles.
fn request<T: CallbackView>(store: &mut T, wire: WireRequest) -> wasmtime::Result<ForeignRequest> {
    Ok(match wire {
        WireRequest::GetText => ForeignRequest::GetText,
        WireRequest::SetText(text) => ForeignRequest::SetText(text),
        WireRequest::SetOnSubmit(handler) => {
            ForeignRequest::SetOnSubmit(optional(store, handler.as_ref())?)
        }
        WireRequest::OnChange => ForeignRequest::OnChange,
        WireRequest::SetOnChange(handler) => {
            ForeignRequest::SetOnChange(optional(store, handler.as_ref())?)
        }
        WireRequest::AddToHistory(text) => ForeignRequest::AddToHistory(text),
        WireRequest::InsertTextAtCursor(text) => ForeignRequest::InsertTextAtCursor(text),
        WireRequest::GetExpandedText => ForeignRequest::GetExpandedText,
        WireRequest::BorderColor => ForeignRequest::BorderColor,
        WireRequest::SetBorderColor(style) => ForeignRequest::SetBorderColor(paint(store, &style)?),
        WireRequest::SetPaddingX(padding) => ForeignRequest::SetPaddingX(padding),
        WireRequest::SetAutocompleteMaxVisible(max) => {
            ForeignRequest::SetAutocompleteMaxVisible(max)
        }
        WireRequest::SetAutocompleteProvider(provider) => {
            ForeignRequest::SetAutocompleteProvider(retain(store, &provider)?)
        }
    })
}

/// The operands of a build with another extension's factory.
type BuildOperands = (
    Resource<FactoryEndpoint>,
    Resource<TuiEndpoint>,
    EditorStyles,
    Resource<KeysEndpoint>,
);

/// Installs the current editor factory read, the lent factory and the editor operations.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, FactoryEndpoint>(host, "editor-factory")?;
    host.func_wrap(
        "[method]extension-ui-context.get-editor-component",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Endpoint>,)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let current = crate::read_imports::ReadEndpoint::read(&endpoint, |reply| {
                Request::Factory(FactoryRequest::GetEditorComponent(reply))
            });
            let current = match current {
                Ok(Some(Ok(id))) => Ok(Some(Current::Authored(id))),
                Ok(Some(Err(factory))) => Ok(Some(Current::Host(
                    store.data_mut().ctx().table.push(factory)?,
                ))),
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            };
            Ok((current,))
        },
    )?;
    host.func_wrap(
        "[method]editor-factory.build",
        |mut store: StoreContextMut<'_, T>, (this, tui, theme, keys): BuildOperands| {
            let factory = store.data_mut().ctx().table.get(&this)?.clone();
            let editor = editor_build(store.data_mut(), (&tui, &theme), Some(&keys), (None, None))?;
            let route = store.data().api().0.route(|api, reply| {
                let build = ForeignBuild {
                    factory,
                    editor,
                    reply,
                };
                Message::Widgets(WidgetMessage::Foreign(api, build))
            });
            Ok((push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "[method]widget.foreign-editor",
        |mut store: StoreContextMut<'_, T>,
         (this, wire): (Resource<WidgetEndpoint>, WireRequest)| {
            let request = request(store.data_mut(), wire)?;
            let route = widget(store.data_mut(), &this)?
                .route(|reply| WidgetRequest::Editor(EditorRequest::Foreign(request, reply)));
            Ok((push(store.data_mut(), route)?,))
        },
    )
}
