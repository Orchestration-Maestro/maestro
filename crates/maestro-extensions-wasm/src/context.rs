use crate::{
    AuthStatus, CompactionResult, ContextUsage, CustomMessage, Error, ExtensionFuture,
    ForkPosition, Model, NavigateTreeOptions, NewSessionOptions, ResolvedRequestAuth,
    SendMessageOptions, SendUserMessageOptions, SessionContext, SessionOutcome, SessionTreeNode,
    UserContent,
};
use serde_json::Value;
use std::rc::Rc;

/// Abort Signal.
pub trait AbortSignal {
    /// Aborted.
    fn aborted(&self) -> bool;
    /// Wait.
    fn wait(&self) -> ExtensionFuture<'_, ()>;
}
/// Ordinary event and tool capabilities retained independently of callback completion.
///
/// Ownership does not make a retired context valid. Implementations report the
/// supplied stale-context error rather than retargeting a captured context.
/// Extracted session and UI capabilities retain their original raw-object lifetime.
/// Session replacement, reload, navigation and waiting are command-only operations.
///
/// ```compile_fail
/// use maestro_extensions_wasm::ExtensionContext;
/// fn ordinary(context: &dyn ExtensionContext) {
///     let _ = context.wait_for_idle();
/// }
/// ```
///
/// ```compile_fail
/// use maestro_extensions_wasm::ExtensionContext;
/// fn ordinary(context: &dyn ExtensionContext) {
///     let _ = context.new_session(None);
/// }
/// ```
///
/// ```compile_fail
/// use maestro_extensions_wasm::ExtensionContext;
/// fn ordinary(context: &dyn ExtensionContext) {
///     let _ = context.fork(String::new(), None);
/// }
/// ```
///
/// ```compile_fail
/// use maestro_extensions_wasm::ExtensionContext;
/// fn ordinary(context: &dyn ExtensionContext) {
///     let _ = context.navigate_tree(String::new(), None);
/// }
/// ```
///
/// ```compile_fail
/// use maestro_extensions_wasm::ExtensionContext;
/// fn ordinary(context: &dyn ExtensionContext) {
///     let _ = context.switch_session(String::new(), None);
/// }
/// ```
///
/// ```compile_fail
/// use maestro_extensions_wasm::ExtensionContext;
/// fn ordinary(context: &dyn ExtensionContext) {
///     let _ = context.reload();
/// }
/// ```
pub trait ExtensionContext {
    /// Ui.
    fn ui(&self) -> Result<Rc<ExtensionUIContext>, Error>;
    /// Has ui.
    fn has_ui(&self) -> Result<bool, Error>;
    /// Cwd.
    fn cwd(&self) -> Result<String, Error>;
    /// Session manager.
    fn session_manager(&self) -> Result<Rc<dyn ReadonlySessionManager>, Error>;
    /// Model registry.
    fn model_registry(&self) -> Result<Rc<dyn ModelRegistry>, Error>;
    /// Model.
    fn model(&self) -> Result<Option<Model>, Error>;
    /// Is idle.
    fn is_idle(&self) -> Result<bool, Error>;
    /// Signal.
    fn signal(&self) -> Result<Option<Rc<dyn AbortSignal>>, Error>;
    /// Abort.
    fn abort(&self) -> Result<(), Error>;
    /// Has pending messages.
    fn has_pending_messages(&self) -> Result<bool, Error>;
    /// Shutdown.
    fn shutdown(&self) -> Result<(), Error>;
    /// Get context usage.
    fn get_context_usage(&self) -> Result<Option<ContextUsage>, Error>;
    /// Request compaction without awaiting its completion; completion uses the supplied callbacks.
    fn compact(&self, options: Option<CompactOptions>) -> Result<(), Error>;
    /// Get system prompt.
    fn get_system_prompt(&self) -> Result<String, Error>;
}
type CompactionComplete = Rc<dyn Fn(CompactionResult) -> Result<(), Error>>;
type CompactionError = Rc<dyn Fn(Error) -> Result<(), Error>>;
type SessionSetup = Rc<dyn Fn(Rc<dyn SessionManager>) -> ExtensionFuture<'static, ()>>;

/// Compact Options.
pub struct CompactOptions {
    /// Custom instructions.
    pub custom_instructions: Option<String>,
    /// On complete.
    pub on_complete: Option<CompactionComplete>,
    /// On error.
    pub on_error: Option<CompactionError>,
}
/// Extension Command Context.
pub trait ExtensionCommandContext: ExtensionContext {
    /// Wait for idle.
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()>;
    /// New session.
    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionOutcome>;
    /// Fork.
    fn fork(
        &self,
        entry_id: String,
        options: Option<ForkOptions>,
    ) -> ExtensionFuture<'_, SessionOutcome>;
    /// Navigate tree.
    fn navigate_tree(
        &self,
        target_id: String,
        options: Option<NavigateTreeOptions>,
    ) -> ExtensionFuture<'_, SessionOutcome>;
    /// Switch session.
    fn switch_session(
        &self,
        session_path: String,
        options: Option<SwitchSessionOptions>,
    ) -> ExtensionFuture<'_, SessionOutcome>;
    /// Reload.
    fn reload(&self) -> ExtensionFuture<'_, ()>;
}
/// Replaced Session Context.
pub trait ReplacedSessionContext: ExtensionCommandContext {
    /// Send message.
    fn send_message(
        &self,
        message: CustomMessage,
        options: Option<SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()>;
    /// Send user message.
    fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()>;
}
/// A callback awaited with the fresh context belonging to its replacement operation.
pub type WithSession = Rc<dyn Fn(Rc<dyn ReplacedSessionContext>) -> ExtensionFuture<'static, ()>>;
/// New Session Command Options.
pub struct NewSessionCommandOptions {
    /// Parent session.
    pub parent_session: Option<String>,
    /// Setup of the new writable session, awaited before continuing its replacement.
    pub setup: Option<SessionSetup>,
    /// With session.
    pub with_session: Option<WithSession>,
}
/// Fork Options.
pub struct ForkOptions {
    /// Position.
    pub position: Option<ForkPosition>,
    /// With session.
    pub with_session: Option<WithSession>,
}
/// Switch Session Options.
pub struct SwitchSessionOptions {
    /// With session.
    pub with_session: Option<WithSession>,
}
/// Readonly Session Manager.
pub trait ReadonlySessionManager {
    /// Get cwd.
    fn get_cwd(&self) -> Result<String, Error>;
    /// Get session dir.
    fn get_session_dir(&self) -> Result<String, Error>;
    /// Get session id.
    fn get_session_id(&self) -> Result<String, Error>;
    /// Get session file.
    fn get_session_file(&self) -> Result<Option<String>, Error>;
    /// Get leaf id.
    fn get_leaf_id(&self) -> Result<Option<String>, Error>;
    /// Get leaf entry.
    fn get_leaf_entry(&self) -> Result<Option<Value>, Error>;
    /// Get entry.
    fn get_entry(&self, id: &str) -> Result<Option<Value>, Error>;
    /// Get label.
    fn get_label(&self, id: &str) -> Result<Option<String>, Error>;
    /// Get branch.
    fn get_branch(&self, from_id: Option<&str>) -> Result<Vec<Value>, Error>;
    /// Get header.
    fn get_header(&self) -> Result<Option<Value>, Error>;
    /// Get entries.
    fn get_entries(&self) -> Result<Vec<Value>, Error>;
    /// Get tree.
    fn get_tree(&self) -> Result<Vec<SessionTreeNode>, Error>;
    /// Get session name.
    fn get_session_name(&self) -> Result<Option<String>, Error>;
}
/// Session Manager.
pub trait SessionManager: ReadonlySessionManager {
    /// Set session file.
    fn set_session_file(&self, path: &str) -> Result<(), Error>;
    /// New session.
    fn new_session(&self, options: Option<NewSessionOptions>) -> Result<Option<String>, Error>;
    /// Is persisted.
    fn is_persisted(&self) -> Result<bool, Error>;
    ///  persist.
    fn _persist(&self, entry: Value) -> Result<(), Error>;
    /// Append message.
    fn append_message(&self, message: Value) -> Result<String, Error>;
    /// Append thinking level change.
    fn append_thinking_level_change(&self, level: &str) -> Result<String, Error>;
    /// Append model change.
    fn append_model_change(&self, provider: &str, model_id: &str) -> Result<String, Error>;
    /// Append compaction.
    fn append_compaction(
        &self,
        summary: String,
        first_kept_entry_id: String,
        tokens_before: f64,
        details: Option<Value>,
        from_hook: Option<bool>,
    ) -> Result<String, Error>;
    /// Append custom entry.
    fn append_custom_entry(&self, custom_type: &str, data: Option<Value>) -> Result<String, Error>;
    /// Append session info.
    fn append_session_info(&self, name: &str) -> Result<String, Error>;
    /// Append custom message entry.
    fn append_custom_message_entry(
        &self,
        custom_type: String,
        content: UserContent,
        display: bool,
        details: Option<Value>,
    ) -> Result<String, Error>;
    /// Append label change.
    fn append_label_change(&self, target_id: &str, label: Option<&str>) -> Result<String, Error>;
    /// Get children.
    fn get_children(&self, parent_id: &str) -> Result<Vec<Value>, Error>;
    /// Build session context.
    fn build_session_context(&self) -> Result<SessionContext, Error>;
    /// Branch.
    fn branch(&self, branch_from_id: &str) -> Result<(), Error>;
    /// Reset leaf.
    fn reset_leaf(&self) -> Result<(), Error>;
    /// Branch with summary.
    fn branch_with_summary(
        &self,
        branch_from_id: Option<&str>,
        summary: String,
        details: Option<Value>,
        from_hook: Option<bool>,
    ) -> Result<String, Error>;
    /// Create branched session.
    fn create_branched_session(&self, leaf_id: &str) -> Result<Option<String>, Error>;
}
/// Model Registry.
pub trait ModelRegistry {
    /// Get error.
    fn get_error(&self) -> Result<Option<String>, Error>;
    /// Get all.
    fn get_all(&self) -> Result<Vec<Model>, Error>;
    /// Get available.
    fn get_available(&self) -> Result<Vec<Model>, Error>;
    /// Find.
    fn find(&self, provider: &str, model_id: &str) -> Result<Option<Model>, Error>;
    /// Has configured auth.
    fn has_configured_auth(&self, model: &Model) -> Result<bool, Error>;
    /// Get api key and headers.
    fn get_api_key_and_headers(&self, model: Model) -> ExtensionFuture<'_, ResolvedRequestAuth>;
    /// Get provider auth status.
    fn get_provider_auth_status(&self, provider: &str) -> Result<AuthStatus, Error>;
    /// Get provider display name.
    fn get_provider_display_name(&self, provider: &str) -> Result<String, Error>;
    /// Get api key for provider.
    fn get_api_key_for_provider(&self, provider: String) -> ExtensionFuture<'_, Option<String>>;
    /// Is using oauth.
    fn is_using_oauth(&self, model: &Model) -> Result<bool, Error>;
}
/// An independently owned host capability.
pub struct ExtensionUIContext {
    _resource: crate::bindings::maestro::extension::types::ExtensionUiContext,
}
/// An independently owned host capability.
pub struct BashOperations {
    _resource: crate::bindings::maestro::extension::types::BashOperations,
}
