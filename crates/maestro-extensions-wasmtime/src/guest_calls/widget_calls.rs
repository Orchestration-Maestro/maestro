//! Authored callbacks a host list or loader calls: handlers, styles, truncation and submenus.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use crate::local_resources::{Registry, effects::EmitterEndpoint};
use crate::widget_lists::SubmenuDoneEndpoint;
use maestro_extensions::{EventBus, EventError, Local};
use wasmtime::component::{ComponentType, Lift, Lower, Resource, ResourceTable};

/// One choice of a selection list.
#[derive(Clone, ComponentType, Lift, Lower)]
#[component(record)]
pub(crate) struct Item {
    /// The value.
    pub(crate) value: String,
    /// The label.
    pub(crate) label: String,
    /// The description, when there is one.
    pub(crate) description: Option<String>,
}

impl Item {
    /// A choice from its value, label and description.
    pub(crate) fn new(value: &str, label: &str, description: Option<&String>) -> Self {
        Self {
            value: value.to_owned(),
            label: label.to_owned(),
            description: description.cloned(),
        }
    }
}

/// What an authored primary-column truncation receives.
#[derive(Clone, ComponentType, Lower)]
#[component(record)]
pub(crate) struct Truncate {
    /// The primary text.
    pub(crate) text: String,
    /// The width the text must fit.
    #[component(name = "max-width")]
    pub(crate) max_width: i64,
    /// The primary column's width.
    #[component(name = "column-width")]
    pub(crate) column_width: i64,
    /// The row's choice.
    pub(crate) item: Item,
    /// Whether the row is selected.
    #[component(name = "is-selected")]
    pub(crate) is_selected: bool,
}

/// What a widget tells an authored callback, Store-safe.
pub(crate) enum Event {
    /// A confirmed choice.
    Select(Item),
    /// A cancellation.
    Cancel,
    /// A moved selection.
    SelectionChange(Item),
    /// A setting's id and new value.
    Change(String, String),
    /// A loader's cancellation.
    Abort,
    /// A row to truncate.
    Truncate(Truncate),
    /// A settings row's text and whether it is selected.
    Row(String, bool),
    /// A submenu's current value and closer.
    Submenu(String, SubmenuDoneEndpoint),
    /// An editor's submitted or changed text.
    Text(String),
    /// An editor key or action.
    Notice,
    /// Input offered to an editor's interceptor.
    Shortcut(String),
    /// An overlay's visibility check at these terminal columns and rows.
    Visible(u32, u32),
    /// A custom overlay's handle after it is shown.
    OverlayHandle(crate::overlay_reads::HandleEndpoint),
    /// A custom overlay asks for its placement.
    OverlayOptions,
}

/// The wire form of an event.
#[derive(ComponentType, Lower)]
#[component(variant)]
pub(crate) enum WireEvent {
    /// A confirmed choice.
    #[component(name = "select")]
    Select(Item),
    /// A cancellation.
    #[component(name = "cancel")]
    Cancel,
    /// A moved selection.
    #[component(name = "selection-change")]
    SelectionChange(Item),
    /// A setting's id and new value.
    #[component(name = "change")]
    Change((String, String)),
    /// A loader's cancellation.
    #[component(name = "abort")]
    Abort,
    /// A row to truncate.
    #[component(name = "truncate")]
    Truncate(Truncate),
    /// A settings row's text and whether it is selected.
    #[component(name = "row")]
    Row((String, bool)),
    /// A submenu's current value and transferred closer.
    #[component(name = "submenu")]
    Submenu((String, Resource<SubmenuDoneEndpoint>)),
    /// An editor's submitted or changed text.
    #[component(name = "text")]
    Text(String),
    /// An editor key or action.
    #[component(name = "notice")]
    Notice,
    /// Input offered to an editor's interceptor.
    #[component(name = "shortcut")]
    Shortcut(String),
    /// An overlay's visibility check.
    #[component(name = "visible")]
    Visible((u32, u32)),
    /// A custom overlay's transferred handle.
    #[component(name = "overlay-handle")]
    OverlayHandle(Resource<crate::overlay_reads::HandleEndpoint>),
    /// A custom overlay asks for its placement.
    #[component(name = "overlay-options")]
    OverlayOptions,
}

impl Event {
    /// The wire form, transferring a submenu's closer into `table`.
    pub(crate) fn lower(&self, table: &mut ResourceTable) -> wasmtime::Result<WireEvent> {
        Ok(match self {
            Self::Select(item) => WireEvent::Select(item.clone()),
            Self::Cancel => WireEvent::Cancel,
            Self::SelectionChange(item) => WireEvent::SelectionChange(item.clone()),
            Self::Change(id, value) => WireEvent::Change((id.clone(), value.clone())),
            Self::Abort => WireEvent::Abort,
            Self::Truncate(context) => WireEvent::Truncate(context.clone()),
            Self::Row(text, selected) => WireEvent::Row((text.clone(), *selected)),
            Self::Submenu(value, done) => {
                WireEvent::Submenu((value.clone(), table.push(done.clone())?))
            }
            Self::Text(text) => WireEvent::Text(text.clone()),
            Self::Notice => WireEvent::Notice,
            Self::Shortcut(data) => WireEvent::Shortcut(data.clone()),
            Self::Visible(columns, rows) => WireEvent::Visible((*columns, *rows)),
            Self::OverlayHandle(handle) => WireEvent::OverlayHandle(table.push(handle.clone())?),
            Self::OverlayOptions => WireEvent::OverlayOptions,
        })
    }
}

/// What an authored widget callback answered, as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Answer {
    /// Nothing.
    #[component(name = "done")]
    Done,
    /// Text.
    #[component(name = "text")]
    Text(String),
    /// A built component's transferred identity.
    #[component(name = "component")]
    Component(Resource<super::Callback>),
    /// Whether an interceptor consumed its input.
    #[component(name = "consumed")]
    Consumed(bool),
    /// Whether an overlay is visible.
    #[component(name = "visible")]
    Visible(bool),
    /// An overlay's transferred placement owner.
    #[component(name = "overlay-options")]
    OverlayOptions(Resource<crate::overlay_reads::OptionsEndpoint>),
}

/// What an authored widget callback answered, with a component kept as a native owner.
pub(crate) enum Answered {
    /// Nothing.
    Done,
    /// Text.
    Text(String),
    /// A built component.
    Component(Handle),
    /// Whether an interceptor consumed its input.
    Consumed(bool),
    /// Whether an overlay is visible.
    Visible(bool),
    /// An overlay's placement owner.
    OverlayOptions(crate::overlay_reads::OptionsEndpoint),
}

/// Keeps a transferred component of an answer as a native owner.
pub(crate) fn answered<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    answer: Answer,
) -> wasmtime::Result<Answered> {
    Ok(match answer {
        Answer::Done => Answered::Done,
        Answer::Text(text) => Answered::Text(text),
        Answer::Consumed(consumed) => Answered::Consumed(consumed),
        Answer::Visible(visible) => Answered::Visible(visible),
        Answer::OverlayOptions(options) => {
            Answered::OverlayOptions(store.ctx().table.delete(options)?)
        }
        Answer::Component(component) => {
            Answered::Component(crate::callback_imports::transferred(store, component)?)
        }
    })
}

/// One call of an authored widget callback.
pub(crate) struct WidgetCall<H = Handle> {
    /// Retains the authored closure and its admission.
    pub(crate) handler: H,
    /// What the widget tells it.
    pub(crate) event: Event,
    /// Original native bus and deferred deliveries.
    pub(crate) emitter: EmitterEndpoint,
    /// Returns its answer.
    pub(crate) reply: Reply<Answered>,
}

impl Handle {
    /// Calls the authored widget callback with a fresh frame of the extension's bus.
    pub(crate) fn widget(
        &self,
        registry: &Registry,
        events: &EventBus<Local>,
        event: Event,
    ) -> Result<Answered, String> {
        events
            .invoke_callback(|emitter| -> Result<Answered, EventError> {
                let emitter = registry.emitter(emitter.frame())?;
                self.widget_in(registry, emitter, event)
                    .map_err(EventError::from)
            })
            .map_err(|error| error.to_string())
    }

    /// Calls the authored widget callback within a native callback's own frame.
    pub(crate) fn widget_in(
        &self,
        registry: &Registry,
        emitter: EmitterEndpoint,
        event: Event,
    ) -> Result<Answered, String> {
        self.0.endpoint.synchronous(registry, |reply| {
            Ok(Call::Widget(WidgetCall {
                handler: self.clone(),
                event,
                emitter,
                reply,
            }))
        })
    }
}
