//! Native editor factories enter their authored guest factory, and native editor operations
//! reach the authored editor it built, through the original frame.
use super::{Call, Callback, Handle};
use crate::keys_reads::KeysEndpoint;
use crate::lifecycle::replies::Reply;
use crate::local_resources::{
    Registry,
    effects::{EmitterEndpoint, TuiEndpoint},
};
use crate::widget_reads::{StyleEndpoint, WidgetEndpoint};
use maestro_extensions::{
    EventError, ExtensionAPI, Local,
    types::{EditorFactory, EditorFactoryOutcome, UIErrorReporter},
    widgets::{EditorTheme, ExtensionKeybindings},
};
use std::rc::Rc;
use wasmtime::component::{ComponentType, Lift, Lower, Resource, ResourceTable};

/// An editor a factory built, as the guest transfers it.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct Built {
    /// The editor seen as a component.
    pub(crate) component: Resource<Callback>,
    /// The editor operations.
    pub(crate) editor: Resource<Callback>,
    /// The application's custom editor the editor extends.
    pub(crate) custom: Option<Resource<WidgetEndpoint>>,
}

/// The native owners of a built editor.
pub(crate) struct RetainedEditor {
    /// The component's owner.
    pub(crate) component: Handle,
    /// The editor operations' owner.
    pub(crate) editor: Handle,
    /// The custom editor the editor extends.
    pub(crate) custom: Option<WidgetEndpoint>,
}

/// Keeps a built editor's identities as native owners.
pub(crate) fn retained<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    built: Built,
) -> wasmtime::Result<RetainedEditor> {
    let component = crate::callback_imports::transferred(store, built.component)?;
    let editor = crate::callback_imports::transferred(store, built.editor)?;
    let custom = built
        .custom
        .map(|custom| store.ctx().table.delete(custom))
        .transpose()?;
    Ok(RetainedEditor {
        component,
        editor,
        custom,
    })
}

/// One editor factory invocation with what the native owner lent it.
pub(crate) struct FactoryCall<H = Handle> {
    /// Retains the authored factory and its admission.
    pub(crate) handler: H,
    /// Original terminal.
    pub(crate) tui: TuiEndpoint,
    /// The border style, then the completion list styles in field order.
    pub(crate) theme: (StyleEndpoint, Vec<StyleEndpoint>),
    /// Original keybindings manager.
    pub(crate) keybindings: KeysEndpoint,
    /// Original native callback frame.
    pub(crate) emitter: EmitterEndpoint,
    /// The built editor's identities.
    pub(crate) reply: Reply<RetainedEditor>,
}

/// One native request on an authored editor.
pub(crate) enum Request {
    /// The text.
    GetText,
    /// Replaces the text.
    SetText(String),
    /// Replaces or removes the submission callback.
    SetOnSubmit(Option<TextCallbackEndpoint>),
    /// The change callback.
    OnChange,
    /// Replaces or removes the change callback.
    SetOnChange(Option<TextCallbackEndpoint>),
    /// Adds text to the history.
    AddToHistory(String),
    /// Inserts text at the cursor.
    InsertTextAtCursor(String),
    /// The text with markers expanded.
    GetExpandedText,
    /// The border style.
    BorderColor,
    /// Replaces the border style.
    SetBorderColor(StyleEndpoint),
    /// Replaces the side padding.
    SetPaddingX(f64),
    /// Replaces the completion row count.
    SetAutocompleteMaxVisible(f64),
    /// Replaces the completion provider with a lent one.
    SetAutocompleteProvider(crate::guest_calls::providers::ProviderEndpoint),
}

/// The wire form of a request, with its lent callbacks and styles transferred.
#[derive(ComponentType, Lower)]
#[component(variant)]
pub(crate) enum WireRequest {
    /// The text.
    #[component(name = "get-text")]
    GetText,
    /// Replaces the text.
    #[component(name = "set-text")]
    SetText(String),
    /// Replaces or removes the submission callback.
    #[component(name = "set-on-submit")]
    SetOnSubmit(Option<Resource<TextCallbackEndpoint>>),
    /// The change callback.
    #[component(name = "on-change")]
    OnChange,
    /// Replaces or removes the change callback.
    #[component(name = "set-on-change")]
    SetOnChange(Option<Resource<TextCallbackEndpoint>>),
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
    SetBorderColor(Resource<StyleEndpoint>),
    /// Replaces the side padding.
    #[component(name = "set-padding-x")]
    SetPaddingX(f64),
    /// Replaces the completion row count.
    #[component(name = "set-autocomplete-max-visible")]
    SetAutocompleteMaxVisible(f64),
    /// Replaces the completion provider with a lent one.
    #[component(name = "set-autocomplete-provider")]
    SetAutocompleteProvider(Resource<crate::guest_calls::providers::ProviderEndpoint>),
}

impl Request {
    /// The wire form, transferring lent callbacks and styles into `table`.
    pub(crate) fn lower(&self, table: &mut ResourceTable) -> wasmtime::Result<WireRequest> {
        let mut lend = |callback: &Option<TextCallbackEndpoint>| {
            callback
                .clone()
                .map(|callback| table.push(callback))
                .transpose()
        };
        Ok(match self {
            Self::GetText => WireRequest::GetText,
            Self::SetText(text) => WireRequest::SetText(text.clone()),
            Self::SetOnSubmit(callback) => WireRequest::SetOnSubmit(lend(callback)?),
            Self::OnChange => WireRequest::OnChange,
            Self::SetOnChange(callback) => WireRequest::SetOnChange(lend(callback)?),
            Self::AddToHistory(text) => WireRequest::AddToHistory(text.clone()),
            Self::InsertTextAtCursor(text) => WireRequest::InsertTextAtCursor(text.clone()),
            Self::GetExpandedText => WireRequest::GetExpandedText,
            Self::BorderColor => WireRequest::BorderColor,
            Self::SetBorderColor(style) => WireRequest::SetBorderColor(table.push(style.clone())?),
            Self::SetPaddingX(padding) => WireRequest::SetPaddingX(*padding),
            Self::SetAutocompleteMaxVisible(max) => WireRequest::SetAutocompleteMaxVisible(*max),
            Self::SetAutocompleteProvider(provider) => {
                WireRequest::SetAutocompleteProvider(table.push(provider.clone())?)
            }
        })
    }
}

/// What an authored editor answered, as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Answer {
    /// The operation ran.
    #[component(name = "done")]
    Done,
    /// The editor lacks the operation.
    #[component(name = "absent")]
    Absent,
    /// Text.
    #[component(name = "text")]
    Text(String),
    /// The change callback's transferred identity, when one is installed.
    #[component(name = "handler")]
    Handler(Option<Resource<Callback>>),
    /// The border style's transferred identity.
    #[component(name = "style")]
    Style(Resource<Callback>),
}

/// What an authored editor answered, with transferred callbacks kept as native owners.
pub(crate) enum Answered {
    /// The operation ran.
    Done,
    /// The editor lacks the operation.
    Absent,
    /// Text.
    Text(String),
    /// The change callback, when one is installed.
    Handler(Option<Handle>),
    /// The border style.
    Style(Handle),
}

/// Keeps the transferred callbacks of an answer as native owners.
pub(crate) fn answered<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    answer: Answer,
) -> wasmtime::Result<Answered> {
    Ok(match answer {
        Answer::Done => Answered::Done,
        Answer::Absent => Answered::Absent,
        Answer::Text(text) => Answered::Text(text),
        Answer::Handler(handler) => Answered::Handler(
            handler
                .map(|handler| crate::callback_imports::transferred(store, handler))
                .transpose()?,
        ),
        Answer::Style(style) => {
            Answered::Style(crate::callback_imports::transferred(store, style)?)
        }
    })
}

/// One native request on an authored editor.
pub(crate) struct EditorCall<H = Handle> {
    /// Retains the authored editor and its admission.
    pub(crate) handler: H,
    /// The request.
    pub(crate) request: Request,
    /// Original native callback frame.
    pub(crate) emitter: EmitterEndpoint,
    /// Returns its answer.
    pub(crate) reply: Reply<Answered>,
}

/// Store-safe identity of a text callback lent to an authored editor.
#[derive(Clone)]
pub(crate) struct TextCallbackEndpoint(pub(crate) std::sync::Arc<crate::local_resources::Lease>);

impl TextCallbackEndpoint {
    /// Runs the callback; its native code may call back into this instance.
    pub(crate) fn call(&self, text: String) -> crate::import_route::Route {
        self.0.route(|id, reply| {
            crate::local_resources::Message::Widgets(crate::widget_reads::WidgetMessage::Lent(
                id,
                crate::widget_reads::Lent::Text(text, reply),
            ))
        })
    }
}

impl Handle {
    /// A native editor factory bound to the original extension's callback frame.
    pub(crate) fn editor_factory(
        self,
        registry: Registry,
        api: &ExtensionAPI<Local>,
        reporter: Option<UIErrorReporter>,
    ) -> EditorFactory {
        let events = api.events.clone();
        api.ui_callback(move |(tui, theme, keybindings), emitter| {
            let tui = registry.tui(tui, reporter.clone())?;
            let frame = registry.emitter(emitter.frame())?;
            let built = self.build_editor(&registry, (tui, theme, keybindings), frame)?;
            let custom = built
                .custom
                .as_ref()
                .map(|custom| registry.custom_editor(custom))
                .transpose()?;
            let custom_built = custom.is_some();
            let editor = Rc::new(crate::guest_editor::GuestEditor::new(
                built,
                registry.clone(),
                events.clone(),
                custom,
            ));
            Ok(if custom_built {
                EditorFactoryOutcome::CustomEditor(editor)
            } else {
                EditorFactoryOutcome::Editor(editor)
            })
        })
    }

    /// Runs the authored factory with the lent terminal, styles and keybindings.
    fn build_editor(
        &self,
        registry: &Registry,
        (tui, theme, keybindings): (TuiEndpoint, EditorTheme, ExtensionKeybindings),
        emitter: EmitterEndpoint,
    ) -> Result<RetainedEditor, EventError> {
        self.0
            .endpoint
            .synchronous(registry, |reply| {
                Ok(Call::EditorFactory(FactoryCall {
                    handler: self.clone(),
                    tui,
                    theme: registry.lend_editor_theme(theme)?,
                    keybindings: registry.keybindings(keybindings)?,
                    emitter,
                    reply,
                }))
            })
            .map_err(EventError::from)
    }
}
