//! Native completion provider factories enter their authored factory, and native completion
//! requests reach the authored providers, through the original Store.
use super::{Call, Handle};
use crate::guest_calls::widget_calls::Item;
use crate::lifecycle::replies::Reply;
use crate::local_resources::{Lease, Message, Registry, SignalEndpoint, effects::EmitterEndpoint};
use crate::widget_reads::WidgetMessage;
use maestro_extensions::widgets::{AutocompleteProvider, CursorPosition};
use maestro_extensions::{
    Cancellation, EventError, ExtensionAPI, Local, types::AutocompleteProviderFactory,
};
use std::{rc::Rc, sync::Arc};
use wasmtime::component::{ComponentType, Lift, Lower};

/// A native completion provider lent to the guest.
pub(crate) type Native = Rc<dyn AutocompleteProvider<Signal = Cancellation>>;

/// A line index and a UTF-8 byte offset, as the canonical boundary carries them.
pub(crate) type Cursor = (u32, u32);

/// Candidates together with the text they replace.
#[derive(Clone, ComponentType, Lift, Lower)]
#[component(record)]
pub(crate) struct Suggestions {
    /// The candidates.
    pub(crate) items: Vec<Item>,
    /// The text they replace.
    pub(crate) prefix: String,
}

/// The lines and cursor after a completion was applied.
#[derive(Clone, ComponentType, Lift, Lower)]
#[component(record)]
pub(crate) struct Completed {
    /// The edited lines.
    pub(crate) lines: Vec<String>,
    /// The cursor's line.
    #[component(name = "cursor-line")]
    pub(crate) cursor_line: u32,
    /// The cursor's UTF-8 byte offset.
    #[component(name = "cursor-col")]
    pub(crate) cursor_col: u32,
}

/// One request on an authored completion provider.
#[derive(Clone, ComponentType, Lower)]
#[component(variant)]
pub(crate) enum Request {
    /// Suggestions for the lines and cursor, and whether completion was forced.
    #[component(name = "suggestions")]
    Suggestions((Vec<String>, Cursor, Option<bool>)),
    /// Applies a chosen item in place of its prefix.
    #[component(name = "apply")]
    Apply((Vec<String>, Cursor, Item, String)),
    /// Whether explicit completion should offer files.
    #[component(name = "trigger")]
    Trigger((Vec<String>, Cursor)),
}

/// What an authored completion provider answered.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Answer {
    /// Suggestions, absent when there are none.
    #[component(name = "suggestions")]
    Suggestions(Option<Suggestions>),
    /// The edited lines and cursor.
    #[component(name = "applied")]
    Applied(Completed),
    /// The provider's decision, absent without one.
    #[component(name = "trigger")]
    Trigger(Option<bool>),
}

/// One request on an authored provider, with the native signal of a suggestion request.
pub(crate) struct ProviderCall<H = Handle> {
    /// Retains the authored provider and its admission.
    pub(crate) handler: H,
    /// The request.
    pub(crate) request: Request,
    /// The caller's signal, lent for a suggestion request.
    pub(crate) signal: Option<SignalEndpoint>,
    /// Returns its answer.
    pub(crate) reply: Reply<Answer>,
}

/// One provider factory invocation on the lent current provider.
pub(crate) struct FactoryCall<H = Handle> {
    /// Retains the authored factory and its admission.
    pub(crate) handler: H,
    /// The current provider, lent.
    pub(crate) current: ProviderEndpoint,
    /// Original native callback frame.
    pub(crate) emitter: EmitterEndpoint,
    /// The returned provider's identity.
    pub(crate) reply: Reply<Handle>,
}

/// Store-safe identity of a native completion provider lent to the guest.
#[derive(Clone)]
pub(crate) struct ProviderEndpoint(pub(crate) Arc<Lease>);

/// One request on a lent native completion provider.
pub(crate) enum Lent {
    /// Applies a chosen item in place of its prefix.
    Apply(
        Vec<String>,
        Cursor,
        Item,
        String,
        std::sync::mpsc::Sender<Result<Completed, String>>,
    ),
    /// Whether explicit completion should offer files.
    Trigger(
        Vec<String>,
        Cursor,
        std::sync::mpsc::Sender<Result<Option<bool>, String>>,
    ),
}

/// One suggestion request on a lent native completion provider.
pub(crate) struct SuggestionRequest {
    /// The edited lines.
    pub(crate) lines: Vec<String>,
    /// The cursor.
    pub(crate) cursor: Cursor,
    /// The caller's signal.
    pub(crate) signal: SignalEndpoint,
    /// Whether completion was forced.
    pub(crate) force: Option<bool>,
    /// Returns the suggestions.
    pub(crate) reply: tokio::sync::oneshot::Sender<Result<Option<Suggestions>, String>>,
}

impl ProviderEndpoint {
    /// Requests suggestions; the native provider may suspend.
    pub(crate) async fn suggestions(
        &self,
        lines: Vec<String>,
        cursor: Cursor,
        signal: SignalEndpoint,
        force: Option<bool>,
    ) -> Result<Option<Suggestions>, String> {
        self.0
            .request_async(|id, reply, admission| {
                let request = SuggestionRequest {
                    lines,
                    cursor,
                    signal,
                    force,
                    reply,
                };
                Message::Widgets(WidgetMessage::Suggestions(id, request, admission))
            })
            .await
    }

    /// Sends one request whose native code may call back into this instance.
    pub(crate) fn route<T: Into<crate::import_route::Value> + Send + 'static>(
        &self,
        request: impl FnOnce(std::sync::mpsc::Sender<Result<T, String>>) -> Lent,
    ) -> crate::import_route::Route {
        self.0
            .route(|id, reply| Message::Widgets(WidgetMessage::Provider(id, request(reply))))
    }
}

/// The native cursor of a canonical cursor.
pub(crate) fn native_cursor((line, col): Cursor) -> Result<CursorPosition, String> {
    let convert = |value: u32| usize::try_from(value).map_err(|error| error.to_string());
    Ok(CursorPosition {
        line: convert(line)?,
        col: convert(col)?,
    })
}

/// The canonical cursor of a native cursor.
pub(crate) fn cursor(cursor: CursorPosition) -> Result<Cursor, String> {
    let convert = |value: usize| u32::try_from(value).map_err(|error| error.to_string());
    Ok((convert(cursor.line)?, convert(cursor.col)?))
}

impl Handle {
    /// A native provider factory bound to the original extension's callback frame.
    pub(crate) fn provider_factory(
        self,
        registry: Registry,
        api: &ExtensionAPI<Local>,
    ) -> AutocompleteProviderFactory {
        api.ui_callback(move |current: Native, emitter| {
            let frame = registry.emitter(emitter.frame())?;
            let provider = self
                .0
                .endpoint
                .synchronous(&registry, |reply| {
                    Ok(Call::AutocompleteFactory(FactoryCall {
                        handler: self.clone(),
                        current: registry.lend_provider(current)?,
                        emitter: frame,
                        reply,
                    }))
                })
                .map_err(EventError::from)?;
            let provider = crate::guest_provider::GuestProvider::new(provider, registry.clone());
            Ok(Rc::new(provider) as Native)
        })
    }
}
