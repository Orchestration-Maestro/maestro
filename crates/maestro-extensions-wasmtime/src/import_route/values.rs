//! Typed native operation results share the existing routed continuation.
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Native results remain their original owners until the guest accepts the route's result.
pub(crate) enum Value {
    /// A completed mutation.
    Unit,
    /// Original native component.
    Component(crate::native_renders::ComponentEndpoint),
    /// Native key-release preference.
    Key(bool),
    /// A configured-auth observation.
    Boolean(bool),
    /// An encoded shared record.
    Text(String),
    /// An encoded shared record, absent when the owner has none.
    OptionalText(Option<String>),
    /// A list of strings: rendered rows or provider names.
    Lines(Vec<String>),
    /// The lines and cursor after a completion was applied.
    Completion(crate::guest_calls::providers::Completed),
    /// A file-completion decision, absent without one.
    Trigger(Option<bool>),
    /// An editor another extension's factory built, and whether it extends the custom editor.
    Editor((crate::widget_reads::WidgetEndpoint, bool)),
    /// What such an editor answered.
    EditorAnswer(crate::widget_foreign::ForeignAnswered),
    /// Independently retained credentials.
    Credentials(crate::credential_reads::Endpoint),
    /// Independently retained catalog.
    Catalog(crate::catalog_reads::Endpoint),
    /// A terminal dimension.
    Count(u32),
    /// A shown overlay's handle.
    Overlay(crate::overlay_reads::HandleEndpoint),
}
/// A result transferred to the guest through the same resource table.
#[derive(ComponentType, Lower)]
#[component(variant)]
pub(super) enum Transferred {
    /// A completed mutation.
    #[component(name = "unit")]
    Unit,
    /// A configured-auth observation.
    #[component(name = "boolean")]
    Boolean(bool),
    /// An encoded shared record.
    #[component(name = "text")]
    Text(String),
    /// An encoded shared record, absent when the owner has none.
    #[component(name = "optional-text")]
    OptionalText(Option<String>),
    /// A list of strings: rendered rows or provider names.
    #[component(name = "lines")]
    Lines(Vec<String>),
    /// The lines and cursor after a completion was applied.
    #[component(name = "completion")]
    Completion(crate::guest_calls::providers::Completed),
    /// A file-completion decision, absent without one.
    #[component(name = "trigger")]
    Trigger(Option<bool>),
    /// An editor another extension's factory built.
    #[component(name = "editor")]
    Editor(Built),
    /// What such an editor answered.
    #[component(name = "editor-answer")]
    EditorAnswer(Answer),
    /// Independently retained credentials.
    #[component(name = "credentials")]
    Credentials(Resource<crate::credential_reads::Endpoint>),
    /// Independently retained catalog.
    #[component(name = "catalog")]
    Catalog(Resource<crate::catalog_reads::Endpoint>),
    /// Original native component.
    #[component(name = "component")]
    Component(Resource<crate::native_renders::ComponentEndpoint>),
    /// Native key-release preference.
    #[component(name = "key")]
    Key(bool),
    /// A terminal dimension.
    #[component(name = "count")]
    Count(u32),
    /// A shown overlay's handle.
    #[component(name = "overlay")]
    Overlay(Resource<crate::overlay_reads::HandleEndpoint>),
}
impl Value {
    /// Transfers one actual result, without reducing non-unit success to unit.
    pub(super) fn transfer(self, table: &mut ResourceTable) -> wasmtime::Result<Transferred> {
        Ok(match self {
            Self::Unit => Transferred::Unit,
            Self::Component(value) => Transferred::Component(table.push(value)?),
            Self::Key(value) => Transferred::Key(value),
            Self::Boolean(value) => Transferred::Boolean(value),
            Self::Text(value) => Transferred::Text(value),
            Self::OptionalText(value) => Transferred::OptionalText(value),
            Self::Lines(value) => Transferred::Lines(value),
            Self::Completion(value) => Transferred::Completion(value),
            Self::Trigger(value) => Transferred::Trigger(value),
            Self::Editor((widget, custom)) => Transferred::Editor(Built {
                widget: table.push(widget)?,
                custom,
            }),
            Self::EditorAnswer(answer) => Transferred::EditorAnswer(answer.transfer(table)?),
            Self::Credentials(value) => Transferred::Credentials(table.push(value)?),
            Self::Catalog(value) => Transferred::Catalog(table.push(value)?),
            Self::Count(value) => Transferred::Count(value),
            Self::Overlay(value) => Transferred::Overlay(table.push(value)?),
        })
    }
}
/// Declares direct type-preserving native result admissions.
macro_rules! returns {
    ($($ty:ty => $kind:ident),* $(,)?) => {$(
        impl From<$ty> for Value {
            fn from(value: $ty) -> Self { Self::$kind(value) }
        }
    )*};
}
returns! { bool => Boolean, String => Text, Option<String> => OptionalText, Vec<String> => Lines, crate::guest_calls::providers::Completed => Completion, Option<bool> => Trigger, (crate::widget_reads::WidgetEndpoint, bool) => Editor, crate::widget_foreign::ForeignAnswered => EditorAnswer, crate::credential_reads::Endpoint => Credentials, crate::catalog_reads::Endpoint => Catalog }
impl From<()> for Value {
    fn from((): ()) -> Self {
        Self::Unit
    }
}

/// An editor another extension's factory built, transferred.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct Built {
    /// The editor's widget.
    widget: Resource<crate::widget_reads::WidgetEndpoint>,
    /// Whether the editor extends the custom editor.
    custom: bool,
}

/// What an editor another extension's factory built answered, with lent resources transferred.
#[derive(ComponentType, Lower)]
#[component(variant)]
pub(super) enum Answer {
    /// The operation ran.
    #[component(name = "done")]
    Done,
    /// The editor lacks the operation.
    #[component(name = "absent")]
    Absent,
    /// Text.
    #[component(name = "text")]
    Text(String),
    /// The change handler, lent, when one is installed.
    #[component(name = "handler")]
    Handler(Option<Resource<crate::guest_calls::editors::TextCallbackEndpoint>>),
    /// The border style, lent.
    #[component(name = "style")]
    Style(Resource<crate::widget_reads::StyleEndpoint>),
}

impl crate::widget_foreign::ForeignAnswered {
    /// Transfers the lent resources into `table`.
    fn transfer(self, table: &mut ResourceTable) -> wasmtime::Result<Answer> {
        Ok(match self {
            Self::Done => Answer::Done,
            Self::Absent => Answer::Absent,
            Self::Text(text) => Answer::Text(text),
            Self::Handler(handler) => {
                Answer::Handler(handler.map(|handler| table.push(handler)).transpose()?)
            }
            Self::Style(style) => Answer::Style(table.push(style)?),
        })
    }
}
