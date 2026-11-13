//! Independently owned capabilities the local owner retains for each resource.
use crate::context_reads::Owner;
use maestro_extensions::{
    Cancellation, ExtensionUIContext, ModelRegistry, ReadonlySessionManager, SharedHandle,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Original credentials with the effects their configured commands run through.
pub(crate) struct CredentialOwner {
    /// The credential owner.
    pub(crate) owner: Arc<maestro_extensions::AuthStorage>,
    /// Request-time command effects of the creating catalog.
    pub(crate) effects: maestro_extensions::ConfigValues,
}

/// Independently owned capabilities, never sent to the Store.
pub(crate) enum Capability {
    /// Independently retained OAuth implementation.
    OAuthProvider(maestro_extensions::OAuthProviderHandle),
    /// Original native options and arbitrary selected author signal bindings.
    ProviderControls(
        Box<super::provider_controls::Owner>,
        crate::model_calls::Scopes,
    ),
    /// Original native notification registration, closed on its final release.
    NativeWatcher(Rc<dyn maestro_extensions::native_io::FsWatcher>),
    /// Original native child, independently retained after its creating callback.
    NativeChild(Rc<dyn maestro_extensions::native_io::ChildProcess>),
    /// Original native response, never a body snapshot.
    NativeResponse(Rc<dyn maestro_extensions::native_io::Response>),
    /// Original native consuming output.
    NativeReader(Rc<dyn maestro_extensions::native_io::Readable>),
    /// Original native ordered input.
    NativeWriter(Rc<dyn maestro_extensions::native_io::Writable>),
    /// An independently retained enumerated schema node.
    Schema(maestro_extensions::SchemaNode),
    /// Original native model invocation buffer.
    AssistantStream(maestro_extensions::AssistantMessageEventStream),
    /// Native event buffer with caller-local completion callbacks.
    Stream(super::stream_reads::Owner),
    /// Original typed native definition behind the component's tool resource.
    Builtin(Rc<dyn crate::builtin_tools::Definition>),
    /// Original native local backend, retained independently of a returned guest operation.
    LocalBash(Rc<dyn maestro_extensions::BashOperations<maestro_extensions::Local>>),
    /// Original row context, including its environment and shared state.
    NativeRow(crate::native_renders::Row),
    /// Original native component, not a guest proxy.
    NativeComponent(maestro_extensions::ComponentHandle),
    /// Original guarded base or command context.
    Context(Owner),
    /// Original registration facade; its domain owner remains weakly referenced.
    Api(maestro_extensions::ExtensionAPI<maestro_extensions::Local>),
    /// Original session's continuation capability, including its own message routes.
    Replaced(maestro_extensions::ReplacedSessionContext),
    /// Original cancellation, unaffected by context retirement or rebinding.
    Signal(Cancellation),
    /// Original read-only session, never a frozen serialized snapshot.
    History(Rc<RefCell<dyn ReadonlySessionManager>>),
    /// Original writable setup capability, not a copied or reconstructed history.
    Writer(Rc<RefCell<maestro_extensions::SessionManager>>),
    /// Original credentials and their request-time effects.
    Credentials(CredentialOwner),
    /// Original independently retained catalog.
    Catalog(SharedHandle<ModelRegistry>),
    /// A captured node, independent of the reader that produced its tree.
    Node(crate::tree_reads::Owner),
    /// Original observer supplied by native tool invocation.
    ToolUpdate(
        maestro_extensions::AgentToolUpdateCallback<serde_json::Value, maestro_extensions::Local>,
    ),
    /// Original callback invocation's bus and deferred queue, with native retirement.
    CallbackFrame(super::effects::Frame),
    /// Owned data stream for one execution; no borrowed sink is retained.
    BashOutput(Rc<super::bash_output::Frame>),
    /// Original explicit remover from the existing event bus.
    Subscription(maestro_extensions::Subscription<maestro_extensions::Local>),
    /// Removes an abort observer explicitly; dropping it keeps the observer.
    SignalSubscription(Rc<dyn Fn()>),
    /// Original live theme supplied by native rendering.
    Theme(maestro_extensions::LiveTheme),
    /// Removes a terminal input listener from the UI it was added to; dropping it keeps the
    /// listener.
    UiSubscription(Rc<dyn Fn() -> Result<(), maestro_extensions::EventError>>),
    /// Original raw UI capability, independent of later bindings.
    Ui(ExtensionUIContext),
    /// Original footer metadata lent to a footer factory.
    Footer(Rc<dyn maestro_extensions::footer::ReadonlyFooterDataProvider>),
    /// Original redraw capability of one tool row.
    Invalidate(Rc<dyn Fn()>),
    /// A host widget the extension built.
    Widget(Rc<crate::widget_reads::Widget>),
    /// A lent host style.
    Style(maestro_extensions::presentation::BorderColor),
    /// The lent host highlighting.
    Highlighter(Rc<crate::guest_calls::paints::HighlightFn>),
    /// Shared native overlay placement and its bound error reporter.
    OverlayOptions(super::overlays::Placement),
    /// A shown overlay's native handle, shared with the host.
    OverlayHandle(maestro_extensions::presentation::OverlayHandle),
    /// Original custom-component close operation.
    CustomClose(maestro_extensions::types::CustomClose),
    /// Original terminal lent to a component factory, with the reporter of failures of the
    /// callbacks it runs.
    Tui(
        maestro_extensions::TUI,
        Option<maestro_extensions::types::UIErrorReporter>,
    ),
    /// A lent settings row style.
    RowStyle(Rc<crate::widget_lists::RowStyleFn>),
    /// Closes an open settings submenu.
    SubmenuDone(Rc<dyn Fn(Option<String>)>),
    /// A keybindings manager.
    Keybindings(maestro_extensions::presentation::ExtensionKeybindings),
    /// A native text callback lent to an authored editor.
    Text(maestro_extensions::presentation::TextCallback),
    /// A native completion provider lent to the guest.
    Provider(crate::guest_calls::providers::Native),
    /// An editor factory another extension set.
    EditorFactory(maestro_extensions::types::EditorFactory),
}
