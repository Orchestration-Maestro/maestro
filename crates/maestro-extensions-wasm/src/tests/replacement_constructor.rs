//! Independently selected author ports exercised by the replacement constructor.
use maestro_extensions_wasm::{
    AbortSignal, CommandContextPort, CompactOptions, ContextPort, ContextUsage, CustomMessageInput,
    ExtensionAPI, ExtensionFuture, ExtensionResult, ExtensionUIContext, ForkOptions, Model,
    ModelRegistry, NavigateTreeOptions, NewSessionCommandOptions, ReadonlySessionManager,
    ReplacedSessionContext, ReplacedSessionContextPort, SendMessageOptions, SendUserMessageOptions,
    SessionChangeResult, SwitchSessionOptions, UserContent,
};
use std::rc::Rc;
/// A command projection deliberately different from the send target.
struct Projection(ReplacedSessionContext);
impl ContextPort for Projection {
    fn cwd(&self) -> ExtensionResult<String> {
        self.0.cwd()
    }
    fn compact(&self, options: Option<CompactOptions>) -> ExtensionResult<()> {
        self.0.compact(options)
    }
    fn ui(&self) -> ExtensionResult<ExtensionUIContext> {
        self.0.ui()
    }
    fn has_ui(&self) -> ExtensionResult<bool> {
        self.0.has_ui()
    }
    fn model_registry(&self) -> ExtensionResult<ModelRegistry> {
        self.0.model_registry()
    }
    fn model(&self) -> ExtensionResult<Option<Model>> {
        self.0.model()
    }
    fn is_idle(&self) -> ExtensionResult<bool> {
        self.0.is_idle()
    }
    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        self.0.signal()
    }
    fn abort(&self) -> ExtensionResult<()> {
        self.0.abort()
    }
    fn has_pending_messages(&self) -> ExtensionResult<bool> {
        self.0.has_pending_messages()
    }
    fn shutdown(&self) -> ExtensionResult<()> {
        self.0.shutdown()
    }
    fn get_context_usage(&self) -> ExtensionResult<Option<ContextUsage>> {
        self.0.get_context_usage()
    }
    fn get_system_prompt(&self) -> ExtensionResult<String> {
        self.0.get_system_prompt()
    }
    fn session_manager(&self) -> ExtensionResult<ReadonlySessionManager> {
        self.0.session_manager()
    }
}
impl CommandContextPort for Projection {
    fn fork<'a>(
        &'a self,
        id: &'a str,
        options: Option<ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.0.fork(id, options)
    }
    fn navigate_tree<'a>(
        &'a self,
        id: &'a str,
        options: Option<NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.0.navigate_tree(id, options)
    }
    fn switch_session<'a>(
        &'a self,
        id: &'a str,
        options: Option<SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.0.switch_session(id, options)
    }
    fn reload(&self) -> ExtensionFuture<'_, ()> {
        self.0.reload()
    }
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        self.0.wait_for_idle()
    }
    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        self.0.new_session(options)
    }
}
/// Owned custom replacement port with separate command and send targets.
struct Port {
    /// Observable owner teardown.
    log: ExtensionAPI,
    /// Send target.
    sends: ReplacedSessionContext,
    /// Independent command projection.
    projection: Rc<Projection>,
}
impl ReplacedSessionContextPort for Port {
    fn command(&self) -> Rc<dyn CommandContextPort> {
        self.projection.clone()
    }
    fn send_message(
        &self,
        message: CustomMessageInput,
        options: Option<SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        self.sends.send_message(message, options)
    }
    fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        self.sends.send_user_message(content, options)
    }
}
impl Drop for Port {
    fn drop(&mut self) {
        self.log
            .append_entry("constructor-drop", Some(serde_json::json!("owner")))
            .unwrap();
    }
}
/// Construct the facade from explicit ports rather than the adapter's internal constructor.
pub(super) fn construct(
    log: ExtensionAPI,
    sends: ReplacedSessionContext,
    command: ReplacedSessionContext,
) -> ReplacedSessionContext {
    ReplacedSessionContext::new(Rc::new(Port {
        log,
        sends,
        projection: Rc::new(Projection(command)),
    }))
}
