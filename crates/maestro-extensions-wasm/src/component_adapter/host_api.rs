//! Registrations and ordinary actions forwarded to the generated host imports.

use super::callbacks::Identity;
use super::session;
use crate::agent::ThinkingLevel;
use crate::bindings::maestro::extension::host::{
    self, FlagOptions, FlagValue, SendMessageOptions, SendUserMessageOptions,
};
use crate::bindings::maestro::extension::session::ToolInfo;
use crate::event_bus::EventBus;
use crate::messages::CustomMessageInput;
use crate::models::{Model, UserContent};
use crate::slash_commands::SlashCommandInfo;
use crate::types::{
    CommandOptions, ExtensionFuture, ExtensionHandler, ExtensionHost, ExtensionResult,
    MessageRenderer, ShortcutOptions, ToolDefinition,
};

/// Forwards requests to the generated imports.
pub(super) struct GeneratedHost;

impl ExtensionHost for GeneratedHost {
    fn on(&self, event: &str, handler: ExtensionHandler) -> ExtensionResult<()> {
        let identity = Identity::new();
        host::on(event, &identity.handle)?;
        identity.keep(handler);
        Ok(())
    }

    fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()> {
        let ToolDefinition {
            metadata,
            prepare_arguments,
            execute,
        } = tool;
        let prepare = prepare_arguments.map(|prepare| (Identity::new(), prepare));
        let execute_identity = Identity::new();
        host::register_tool(
            &metadata,
            prepare.as_ref().map(|(identity, _)| &identity.handle),
            &execute_identity.handle,
        )?;
        execute_identity.keep(execute);
        if let Some((identity, prepare)) = prepare {
            identity.keep(prepare);
        }
        Ok(())
    }

    fn register_command(&self, name: &str, options: CommandOptions) -> ExtensionResult<()> {
        let CommandOptions {
            description,
            get_argument_completions,
            handler,
        } = options;
        let completions = get_argument_completions.map(|closure| (Identity::new(), closure));
        let identity = Identity::new();
        host::register_command(
            name,
            description.as_deref(),
            completions.as_ref().map(|(identity, _)| &identity.handle),
            &identity.handle,
        )?;
        identity.keep(handler);
        if let Some((identity, closure)) = completions {
            identity.keep(closure);
        }
        Ok(())
    }

    fn register_shortcut(&self, shortcut: &str, options: ShortcutOptions) -> ExtensionResult<()> {
        let identity = Identity::new();
        host::register_shortcut(shortcut, options.description.as_deref(), &identity.handle)?;
        identity.keep(options.handler);
        Ok(())
    }

    fn register_flag(&self, name: &str, options: FlagOptions) -> ExtensionResult<()> {
        host::register_flag(name, &options)
    }

    fn get_flag(&self, name: &str) -> ExtensionResult<Option<FlagValue>> {
        host::get_flag(name)
    }

    fn register_message_renderer(
        &self,
        custom_type: &str,
        renderer: MessageRenderer,
    ) -> ExtensionResult<()> {
        let identity = Identity::new();
        host::register_message_renderer(custom_type, &identity.handle)?;
        identity.keep(renderer);
        Ok(())
    }

    fn send_message(
        &self,
        message: CustomMessageInput,
        options: Option<SendMessageOptions>,
    ) -> ExtensionResult<()> {
        host::send_message(&message, options)
    }

    fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> ExtensionResult<()> {
        host::send_user_message(&content, options)
    }

    fn append_entry(
        &self,
        custom_type: &str,
        data: Option<serde_json::Value>,
    ) -> ExtensionResult<()> {
        host::append_entry(custom_type, data.map(|data| data.to_string()).as_deref())
    }

    fn set_session_name(&self, name: &str) -> ExtensionResult<()> {
        host::set_session_name(name)
    }

    fn get_session_name(&self) -> ExtensionResult<Option<String>> {
        host::get_session_name()
    }

    fn set_label(&self, entry_id: &str, label: Option<&str>) -> ExtensionResult<()> {
        host::set_label(entry_id, label)
    }

    fn get_active_tools(&self) -> ExtensionResult<Vec<String>> {
        host::get_active_tools()
    }

    fn get_all_tools(&self) -> ExtensionResult<Vec<ToolInfo>> {
        host::get_all_tools()
    }

    fn set_active_tools(&self, names: &[String]) -> ExtensionResult<()> {
        host::set_active_tools(names)
    }

    fn get_commands(&self) -> ExtensionResult<Vec<SlashCommandInfo>> {
        host::get_commands()
    }

    fn get_thinking_level(&self) -> ExtensionResult<ThinkingLevel> {
        host::get_thinking_level()
    }

    fn set_thinking_level(&self, level: ThinkingLevel) -> ExtensionResult<()> {
        host::set_thinking_level(level)
    }

    fn set_model(&self, model: Model) -> ExtensionFuture<'_, bool> {
        Box::pin(host::set_model(model))
    }

    fn events(&self) -> EventBus {
        session::bus(host::events())
    }
}
