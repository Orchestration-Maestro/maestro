//! The extension API facade and the port both adapters implement.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use serde_json::Value;

use super::context::{ExtensionCommandContext, ExtensionContext};
use super::events::ExtensionHandler;
use super::extension_result::{ExtensionFuture, ExtensionResult};
use super::tools::ToolDefinition;
pub use crate::bindings::maestro::extension::host::{FlagOptions, FlagType, FlagValue};
use crate::{
    AutocompleteItem, CustomMessageInput, Model, SendMessageOptions, SendUserMessageOptions,
    SlashCommandInfo, ThinkingLevel, ToolInfo, UserContent,
};

/// Handler of a registered command: receives the argument text and a command context.
pub type CommandHandler =
    Rc<dyn Fn(String, ExtensionCommandContext) -> ExtensionFuture<'static, ()>>;

/// Supplies optional completion candidates for literal argument text.
pub type ArgumentCompletions =
    Rc<dyn Fn(String) -> ExtensionFuture<'static, Option<Vec<AutocompleteItem>>>>;
/// Runs a shortcut with an ordinary context.
pub type ShortcutHandler = Rc<dyn Fn(ExtensionContext) -> ExtensionFuture<'static, ()>>;
/// Shortcut registration options.
pub struct ShortcutOptions {
    /// Optional description.
    pub description: Option<String>,
    /// The shortcut callback.
    pub handler: ShortcutHandler,
}
/// Registration data of a command.
pub struct CommandOptions {
    /// Description shown to users.
    pub description: Option<String>,
    /// Runs when the command is invoked.
    pub handler: CommandHandler,
    /// Optional argument-completion callback.
    pub get_argument_completions: Option<ArgumentCompletions>,
}

port! {
    /// Registrations and ordinary actions a host provides; each method forwards one request.
    ExtensionHost for ExtensionAPI via 0 {
        /// Formats a native byte count without narrowing its floating-point domain.
        #[doc(hidden)]
        fn native_format_size(bytes: f64) -> String;
        /// Offers package sources, base prompts, model files and scoped base layers during base loading.
        fn contribute_load_time(contributions: Value) -> ();
        /// Reads complete typed metadata through the installed host.
        #[doc(hidden)]
        fn native_frontmatter(content: &str) -> crate::bindings::maestro::extension::host::FrontmatterDocument;
        /// Reads one raw YAML document through the installed host.
        #[doc(hidden)]
        fn native_yaml(content: &str) -> Vec<crate::bindings::maestro::extension::host::FrontmatterToken>;
        /// Forwards ordinary helper operands to their native owners.
        #[doc(hidden)]
        fn native_helper(operation: &str, data: &str) -> String;
        /// Constructs file-backed credentials with the application's optional default path.
        fn create_auth_storage(path: Option<&str>) -> crate::AuthStorage;
        /// Constructs credentials over a retained raw-text backend.
        fn auth_storage_from_backend(backend: Rc<dyn crate::AuthStorageBackend>) -> crate::AuthStorage;
        /// Constructs credentials without file access.
        fn auth_storage_in_memory(data: crate::AuthStorageData) -> crate::AuthStorage;
        /// Constructs a file catalog over the supplied live credential owner.
        fn create_model_registry(auth_storage: crate::AuthStorage, path: Option<&str>) -> crate::ModelRegistry;
        /// Constructs a catalog without file access.
        fn in_memory_model_registry(auth_storage: crate::AuthStorage) -> crate::ModelRegistry;
        /// Registers a handler for an event name. Names the host does not define are
        /// forwarded unchanged, and nothing is retained when the host rejects the handler.
        fn on(event: &str, handler: ExtensionHandler) -> ();
        /// Registers a command; rejection adds neither callback to the table.
        fn register_command(name: &str, options: CommandOptions) -> ();
        /// Registers a shortcut without parsing its key.
        fn register_shortcut(shortcut: &str, options: ShortcutOptions) -> ();
        /// Registers a flag without resolving its value.
        fn register_flag(name: &str, options: FlagOptions) -> ();
        /// Reads the host-resolved value of a flag.
        fn get_flag(name: &str) -> Option<FlagValue>;
        /// Retains the native local shell backend without starting a process.
        fn create_local_bash_operations(shell_path: Option<String>) -> crate::BashOperations;
        /// Constructs a context-free executable from the original native shell factory.
        fn create_bash_tool(cwd: String, options: crate::BashToolOptions) -> crate::Tool;
        /// Constructs a native shell definition with owned operations and interception.
        fn create_bash_tool_definition(cwd: String, options: crate::BashToolOptions) -> ToolDefinition;
        /// Constructs a retained native read executable that needs no extension context.
        fn create_read_tool(cwd: String, options: crate::ReadToolOptions) -> crate::Tool;
        /// Constructs a retained native read definition without performing file work.
        fn create_read_tool_definition(cwd: String, options: crate::ReadToolOptions) -> ToolDefinition;
        /// Constructs a retained native write executable without requiring a context.
        fn create_write_tool(cwd: String, options: crate::WriteToolOptions) -> crate::Tool;
        /// Constructs a native write definition without performing file work.
        fn create_write_tool_definition(cwd: String, options: crate::WriteToolOptions) -> ToolDefinition;
        /// Constructs a retained native edit executable with argument preparation.
        fn create_edit_tool(cwd: String, options: crate::EditToolOptions) -> crate::Tool;
        /// Constructs a native edit definition with retained effects and renderers.
        fn create_edit_tool_definition(cwd: String, options: crate::EditToolOptions) -> ToolDefinition;
        /// Constructs a context-free native directory lister.
        fn create_ls_tool(cwd: String, options: crate::LsToolOptions) -> crate::Tool;
        /// Constructs a native listing definition without observing the filesystem.
        fn create_ls_tool_definition(cwd: String, options: crate::LsToolOptions) -> ToolDefinition;
        /// Constructs a context-free native finder over retained observations.
        fn create_find_tool(cwd: String, options: crate::FindToolOptions) -> crate::Tool;
        /// Constructs a native find definition without acquiring programs or observing files.
        fn create_find_tool_definition(cwd: String, options: crate::FindToolOptions) -> ToolDefinition;
        /// Constructs a native grep executable without a caller context requirement.
        fn create_grep_tool(cwd: String, options: crate::GrepToolOptions) -> crate::Tool;
        /// Constructs a native grep definition over the application's search manager.
        fn create_grep_tool_definition(cwd: String, options: crate::GrepToolOptions) -> ToolDefinition;
        /// Selects an exact built-in executable name through the native factory.
        fn create_tool(tool_name: &str, cwd: String, options: crate::ToolsOptions) -> crate::Tool;
        /// Selects an exact built-in definition name through the native factory.
        fn create_tool_definition(tool_name: &str, cwd: String, options: crate::ToolsOptions) -> crate::ToolDef;
        /// Returns the native coding preset in its original insertion order.
        fn create_coding_tools(cwd: String, options: crate::ToolsOptions) -> Vec<crate::Tool>;
        /// Returns the native coding preset in its original insertion order.
        fn create_coding_tool_definitions(cwd: String, options: crate::ToolsOptions) -> Vec<crate::ToolDef>;
        /// Returns the native read-only preset in its original insertion order.
        fn create_read_only_tools(cwd: String, options: crate::ToolsOptions) -> Vec<crate::Tool>;
        /// Returns the native read-only preset in its original insertion order.
        fn create_read_only_tool_definitions(cwd: String, options: crate::ToolsOptions) -> Vec<crate::ToolDef>;
        /// Returns the native all preset in its original insertion order.
        fn create_all_tools(cwd: String, options: crate::ToolsOptions) -> indexmap::IndexMap<crate::ToolName, crate::Tool>;
        /// Returns the native all preset in its original insertion order.
        fn create_all_tool_definitions(cwd: String, options: crate::ToolsOptions) -> indexmap::IndexMap<crate::ToolName, crate::ToolDef>;
        /// Registers one tool; rejected registration retains no callbacks.
        fn register_tool(tool: ToolDefinition) -> ();
        /// Registers a custom-message renderer.
        fn register_message_renderer(custom_type: &str, renderer: crate::MessageRenderer) -> ();
        /// Forwards a custom entry to the host's session.
        fn append_entry(custom_type: &str, data: Option<Value>) -> ();
        /// Sends the supplied custom message and optional delivery choices.
        fn send_message(message: CustomMessageInput, options: Option<SendMessageOptions>) -> ();
        /// Sends the supplied user content and optional delivery choice.
        fn send_user_message(content: UserContent, options: Option<SendUserMessageOptions>) -> ();
        /// Sets the literal session name through the host.
        fn set_session_name(name: &str) -> ();
        /// Reads the host's current optional session name.
        fn get_session_name() -> Option<String>;
        /// Sets or clears a label through the host.
        fn set_label(entry_id: &str, label: Option<&str>) -> ();
        /// Reads the host's ordered active tool names.
        fn get_active_tools() -> Vec<String>;
        /// Reads the host's ordered tool inventory.
        fn get_all_tools() -> Vec<ToolInfo>;
        /// Selects the supplied tool names through the host.
        fn set_active_tools(tool_names: &[String]) -> ();
        /// Reads the host's ordered command inventory.
        fn get_commands() -> Vec<SlashCommandInfo>;
        /// Registers a provider; the host queues it until binding.
        fn register_provider(name: &str, config: super::ProviderConfig) -> ();
        /// Unregisters a provider together with its queued registrations.
        fn unregister_provider(name: &str) -> ();
        /// Reads the host's current thinking level.
        fn get_thinking_level() -> ThinkingLevel;
        /// Sets the requested thinking level through the host.
        fn set_thinking_level(level: ThinkingLevel) -> ();

    }
    asynchronous {
        /// Calls a factory-time operation on the original settings or package owner.
        fn call_host_operation<'a>(operation: crate::bindings::maestro::extension::host::HostOperation) -> Value;
        /// Selects the supplied model through the host.
        fn set_model<'a>(model: Model) -> bool;
        /// Executes a literal program and argument vector through the host.
        fn exec<'a>(command: &'a str, args: &'a [String], options: Option<crate::ExecOptions>) -> crate::ExecResult;
    }
    host_owned {
        /// Takes a native file queue turn immediately and retains it through the callback.
        fn with_file_mutation_queue(file_path: String, operation: Box<dyn FnOnce() -> ExtensionFuture<'static, ()>>) -> ();
    }
    infallible {
        /// Returns independently retained native operations.
        fn native_io() -> Rc<dyn crate::io::NativeIo>;
        /// Returns the supplied host bus.
        fn events() -> crate::EventBus;
    }
}

/// The handle an extension factory registers through.
#[derive(Clone)]
pub struct ExtensionAPI(Rc<dyn ExtensionHost>);

impl ExtensionAPI {
    /// Wraps a host adapter.
    #[must_use]
    pub fn new(host: Rc<dyn ExtensionHost>) -> Self {
        Self(host)
    }
}

impl ExtensionAPI {
    /// Executes a literal program and argument vector through the host.
    ///
    /// # Errors
    /// Returns encoding, host or result-decoding errors unchanged.
    #[must_use]
    pub fn exec<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: Option<crate::ExecOptions>,
    ) -> ExtensionFuture<'a, crate::ExecResult> {
        self.0.exec(command, args, options)
    }

    /// Selects the supplied model through the host and awaits its boolean reply.
    #[must_use]
    pub fn set_model(&self, model: Model) -> ExtensionFuture<'_, bool> {
        self.0.set_model(model)
    }
}

impl ExtensionAPI {
    /// Runs an operation in the native file queue, preserving its guest-owned result.
    ///
    /// Admission occurs before the returned future is polled. Dropping that future
    /// releases its native queue turn without starting a waiting operation.
    ///
    /// # Errors
    /// Returns native admission or callback errors unchanged.
    #[must_use]
    pub fn with_file_mutation_queue<T: 'static>(
        &self,
        file_path: String,
        operation: impl FnOnce() -> ExtensionFuture<'static, T> + 'static,
    ) -> ExtensionFuture<'static, T> {
        crate::tools::file_mutation_queue::run(&*self.0, file_path, operation)
    }

    /// Encodes ordinary operands without introducing another native algorithm.
    pub(crate) fn helper<A: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        operation: &str,
        args: &A,
    ) -> ExtensionResult<R> {
        let data = serde_json::to_string(args).map_err(|error| error.to_string())?;
        let value = self.0.native_helper(operation, &data)?;
        serde_json::from_str(&value).map_err(|error| error.to_string())
    }
}

impl ExtensionAPI {
    /// Calls the host without requiring a session or command context.
    pub(crate) fn call_host_operation(
        &self,
        operation: crate::bindings::maestro::extension::host::HostOperation,
    ) -> ExtensionFuture<'_, Value> {
        self.0.call_host_operation(operation)
    }
}
