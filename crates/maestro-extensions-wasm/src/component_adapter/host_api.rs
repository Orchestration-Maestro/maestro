//! Registrations and actions forwarded to the host imports.

use super::callbacks::Identity;
use super::exports::encode_value;
use super::imports::Imports;
use crate::types::{CommandOptions, ExtensionHandler, ExtensionHost, ExtensionResult};
use crate::{FlagOptions, FlagValue, ShortcutOptions, ToolDefinition};

/// The host imports behind the facade.
pub(super) struct Host<I: Imports>(pub(super) I);

impl<I: Imports> ExtensionHost for Host<I> {
    fn register_message_renderer(
        &self,
        custom_type: &str,
        renderer: crate::MessageRenderer,
    ) -> ExtensionResult<()> {
        let identity = Identity::new(&self.0);
        self.0
            .register_message_renderer(custom_type, &identity.handle)?;
        identity.keep(renderer);
        Ok(())
    }

    fn send_message(
        &self,
        message: crate::CustomMessageInput,
        options: Option<crate::SendMessageOptions>,
    ) -> ExtensionResult<()> {
        self.0.send_message(
            &encode_value(&message)?,
            options.as_ref().map(encode_value).transpose()?.as_deref(),
        )
    }
    fn send_user_message(
        &self,
        content: crate::UserContent,
        options: Option<crate::SendUserMessageOptions>,
    ) -> ExtensionResult<()> {
        self.0.send_user_message(
            &encode_value(&content)?,
            options.as_ref().map(encode_value).transpose()?.as_deref(),
        )
    }
    fn set_model(&self, model: crate::Model) -> crate::ExtensionFuture<'_, bool> {
        Box::pin(async move { self.0.set_model(encode_value(&model)?).await })
    }
    fn get_all_tools(&self) -> ExtensionResult<Vec<crate::ToolInfo>> {
        let (text, bindings) = self.0.get_all_tools()?;
        let handlers = bindings
            .into_iter()
            .map(|(index, pointer, handle)| {
                (
                    index,
                    pointer,
                    super::callbacks::Scoped::from_handle(&self.0, handle),
                )
            })
            .collect::<Vec<_>>();
        let tools = super::tools::decode_tools(&text)?;
        let mut codes = std::collections::HashMap::<usize, crate::schema_code::SchemaCode>::new();
        for (index, pointer, identity) in &handlers {
            let index = *index as usize;
            if index >= tools.len() {
                return Err("schema binding has no matching tool".into());
            }
            let id = self.0.callback_id(&identity.handle);
            let handler = super::callbacks::find::<crate::schema_code::Handler>(id)?;
            codes
                .entry(index)
                .or_default()
                .insert(pointer.clone(), super::schema::node(&handler));
        }
        Ok(tools
            .into_iter()
            .enumerate()
            .map(|(index, mut tool)| {
                tool.parameters = tool
                    .parameters
                    .with_code(codes.remove(&index).unwrap_or_default());
                tool
            })
            .collect())
    }

    fn get_commands(&self) -> ExtensionResult<Vec<crate::SlashCommandInfo>> {
        super::session::decode(&self.0.get_commands()?)
    }
    fn get_thinking_level(&self) -> ExtensionResult<crate::ThinkingLevel> {
        super::session::decode(&self.0.get_thinking_level()?)
    }
    fn set_thinking_level(&self, level: crate::ThinkingLevel) -> ExtensionResult<()> {
        self.0.set_thinking_level(&encode_value(&level)?)
    }
    fn set_session_name(&self, name: &str) -> ExtensionResult<()> {
        self.0.set_session_name(name)
    }
    fn get_session_name(&self) -> ExtensionResult<Option<String>> {
        self.0.get_session_name()
    }
    fn set_label(&self, entry_id: &str, label: Option<&str>) -> ExtensionResult<()> {
        self.0.set_label(entry_id, label)
    }
    fn get_active_tools(&self) -> ExtensionResult<Vec<String>> {
        self.0.get_active_tools()
    }
    fn set_active_tools(&self, tool_names: &[String]) -> ExtensionResult<()> {
        self.0.set_active_tools(tool_names)
    }

    fn events(&self) -> crate::EventBus {
        super::event_bus::events(self.0.clone())
    }
    fn on(&self, event: &str, handler: ExtensionHandler) -> ExtensionResult<()> {
        let identity = Identity::new(&self.0);
        self.0.on(event, &identity.handle)?;
        identity.keep(handler);
        Ok(())
    }

    fn register_command(&self, name: &str, options: CommandOptions) -> ExtensionResult<()> {
        let CommandOptions {
            description,
            handler,
            get_argument_completions,
        } = options;
        let completions = get_argument_completions.map(|f| (Identity::new(&self.0), f));
        let identity = Identity::new(&self.0);
        self.0.register_command(
            name,
            description.as_deref(),
            completions.as_ref().map(|(id, _)| &id.handle),
            &identity.handle,
        )?;
        if let Some((id, f)) = completions {
            id.keep(f);
        }
        identity.keep(handler);
        Ok(())
    }
    fn register_shortcut(&self, shortcut: &str, options: ShortcutOptions) -> ExtensionResult<()> {
        let identity = Identity::new(&self.0);
        self.0
            .register_shortcut(shortcut, options.description.as_deref(), &identity.handle)?;
        identity.keep(options.handler);
        Ok(())
    }
    fn register_flag(&self, name: &str, options: FlagOptions) -> ExtensionResult<()> {
        self.0.register_flag(name, &options)
    }
    fn get_flag(&self, name: &str) -> ExtensionResult<Option<FlagValue>> {
        self.0.get_flag(name)
    }

    fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()> {
        let ToolDefinition {
            metadata,
            prepare_arguments,
            execute,
        } = tool;
        let code = metadata
            .parameters
            .code()
            .iter()
            .map(|(pointer, node)| {
                (
                    pointer.clone(),
                    Identity::new(&self.0),
                    super::schema::handler(node.clone()),
                )
            })
            .collect::<Vec<_>>();
        let metadata = super::tools::encode_metadata(&metadata)?;
        let prepare = prepare_arguments.map(|prepare| (Identity::new(&self.0), prepare));
        let identity = Identity::new(&self.0);
        self.0.register_tool(
            &metadata,
            prepare.as_ref().map(|(identity, _)| &identity.handle),
            &identity.handle,
            &code
                .iter()
                .map(|(pointer, id, _)| (pointer.clone(), &id.handle))
                .collect::<Vec<_>>(),
        )?;
        for (_, identity, handler) in code {
            identity.keep(handler);
        }
        if let Some((identity, prepare)) = prepare {
            identity.keep(prepare);
        }
        identity.keep(execute);
        Ok(())
    }
    fn append_entry(
        &self,
        custom_type: &str,
        data: Option<serde_json::Value>,
    ) -> ExtensionResult<()> {
        self.0
            .append_entry(custom_type, data.map(|data| data.to_string()).as_deref())
    }
    fn register_provider(&self, name: &str, config: crate::ProviderConfig) -> ExtensionResult<()> {
        self.0.register_provider(name, &encode_value(&config)?)
    }
    fn unregister_provider(&self, name: &str) -> ExtensionResult<()> {
        self.0.unregister_provider(name)
    }
}
