//! Registrations and actions forwarded to the host imports.

use std::rc::Rc;

use super::callbacks::Identity;
use super::exports::encode_value;
use super::imports::{Imports, ToolCallbacks};
use super::signals::Binding;
use crate::types::{CommandOptions, ExtensionHandler, ExtensionHost, ExtensionResult};
use crate::{FlagOptions, FlagValue, ShortcutOptions, ToolDefinition};

/// The host imports behind the facade.
pub(super) struct Host<I: Imports>(pub(super) I);

impl<I: Imports> ExtensionHost for Host<I> {
    fn call_host_operation(
        &self,
        operation: crate::bindings::maestro::extension::host::HostOperation,
    ) -> crate::ExtensionFuture<'_, serde_json::Value> {
        self.0.call_host_operation(operation)
    }

    fn create_coding_tools(
        &self,
        cwd: String,
        options: crate::ToolsOptions,
    ) -> ExtensionResult<Vec<crate::Tool>> {
        self.0.create_coding_tools(cwd, options)
    }
    fn create_coding_tool_definitions(
        &self,
        cwd: String,
        options: crate::ToolsOptions,
    ) -> ExtensionResult<Vec<crate::ToolDef>> {
        self.0.create_coding_tool_definitions(cwd, options)
    }
    fn create_read_only_tools(
        &self,
        cwd: String,
        options: crate::ToolsOptions,
    ) -> ExtensionResult<Vec<crate::Tool>> {
        self.0.create_read_only_tools(cwd, options)
    }
    fn create_read_only_tool_definitions(
        &self,
        cwd: String,
        options: crate::ToolsOptions,
    ) -> ExtensionResult<Vec<crate::ToolDef>> {
        self.0.create_read_only_tool_definitions(cwd, options)
    }
    fn create_all_tools(
        &self,
        cwd: String,
        options: crate::ToolsOptions,
    ) -> ExtensionResult<indexmap::IndexMap<crate::ToolName, crate::Tool>> {
        self.0.create_all_tools(cwd, options)
    }
    fn create_all_tool_definitions(
        &self,
        cwd: String,
        options: crate::ToolsOptions,
    ) -> ExtensionResult<indexmap::IndexMap<crate::ToolName, crate::ToolDef>> {
        self.0.create_all_tool_definitions(cwd, options)
    }

    fn create_tool(
        &self,
        tool_name: &str,
        cwd: String,
        options: crate::ToolsOptions,
    ) -> ExtensionResult<crate::Tool> {
        self.0.create_tool(tool_name, cwd, options)
    }
    fn create_tool_definition(
        &self,
        tool_name: &str,
        cwd: String,
        options: crate::ToolsOptions,
    ) -> ExtensionResult<crate::ToolDef> {
        self.0.create_tool_definition(tool_name, cwd, options)
    }
    fn create_local_bash_operations(
        &self,
        shell_path: Option<String>,
    ) -> ExtensionResult<crate::BashOperations> {
        self.0.create_local_bash_operations(shell_path)
    }
    fn create_bash_tool(
        &self,
        cwd: String,
        options: crate::BashToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        self.0.create_bash_tool(cwd, options)
    }
    fn create_bash_tool_definition(
        &self,
        cwd: String,
        options: crate::BashToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        self.0.create_bash_tool_definition(cwd, options)
    }
    fn with_file_mutation_queue(
        &self,
        file_path: String,
        operation: Box<dyn FnOnce() -> crate::ExtensionFuture<'static, ()>>,
    ) -> crate::ExtensionFuture<'static, ()> {
        self.0.with_file_mutation_queue(file_path, operation)
    }

    fn create_grep_tool(
        &self,
        cwd: String,
        options: crate::GrepToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        self.0.create_grep_tool(cwd, options)
    }

    fn create_grep_tool_definition(
        &self,
        cwd: String,
        options: crate::GrepToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        self.0.create_grep_tool_definition(cwd, options)
    }

    fn create_ls_tool(
        &self,
        cwd: String,
        options: crate::LsToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        self.0.create_ls_tool(cwd, options)
    }

    fn create_ls_tool_definition(
        &self,
        cwd: String,
        options: crate::LsToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        self.0.create_ls_tool_definition(cwd, options)
    }

    fn create_find_tool(
        &self,
        cwd: String,
        options: crate::FindToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        self.0.create_find_tool(cwd, options)
    }

    fn create_find_tool_definition(
        &self,
        cwd: String,
        options: crate::FindToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        self.0.create_find_tool_definition(cwd, options)
    }

    fn create_edit_tool(
        &self,
        cwd: String,
        options: crate::EditToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        self.0.create_edit_tool(cwd, options)
    }
    fn create_edit_tool_definition(
        &self,
        cwd: String,
        options: crate::EditToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        self.0.create_edit_tool_definition(cwd, options)
    }
    fn create_write_tool(
        &self,
        cwd: String,
        options: crate::WriteToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        self.0.create_write_tool(cwd, options)
    }
    fn create_write_tool_definition(
        &self,
        cwd: String,
        options: crate::WriteToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        self.0.create_write_tool_definition(cwd, options)
    }
    fn create_read_tool(
        &self,
        cwd: String,
        options: crate::ReadToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        self.0.create_read_tool(cwd, options)
    }
    fn create_read_tool_definition(
        &self,
        cwd: String,
        options: crate::ReadToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        self.0.create_read_tool_definition(cwd, options)
    }

    fn native_io(&self) -> std::rc::Rc<dyn crate::io::NativeIo> {
        std::rc::Rc::new(super::io::Io(self.0.clone()))
    }
    fn native_format_size(&self, bytes: f64) -> ExtensionResult<String> {
        self.0.native_format_size(bytes)
    }
    fn native_frontmatter(
        &self,
        content: &str,
    ) -> ExtensionResult<crate::bindings::maestro::extension::host::FrontmatterDocument> {
        self.0.native_frontmatter(content)
    }
    fn native_yaml(
        &self,
        content: &str,
    ) -> ExtensionResult<Vec<crate::bindings::maestro::extension::host::FrontmatterToken>> {
        self.0.native_yaml(content)
    }
    fn native_helper(&self, operation: &str, data: &str) -> ExtensionResult<String> {
        self.0.native_helper(operation, data)
    }
    fn auth_storage_from_backend(
        &self,
        backend: Rc<dyn crate::AuthStorageBackend>,
    ) -> ExtensionResult<crate::AuthStorage> {
        let identity = Identity::new(&self.0);
        identity.stage(backend);
        let result = self.0.auth_storage_from_backend(&identity.handle);
        identity.settle(result.is_ok());
        result.map(|resource| super::auth_storage::storage(self.0.clone(), resource))
    }
    fn create_auth_storage(&self, path: Option<&str>) -> ExtensionResult<crate::AuthStorage> {
        self.0
            .create_auth_storage(path)
            .map(|resource| super::auth_storage::storage(self.0.clone(), resource))
    }
    fn auth_storage_in_memory(
        &self,
        data: crate::AuthStorageData,
    ) -> ExtensionResult<crate::AuthStorage> {
        self.0
            .auth_storage_in_memory(&encode_value(&data)?)
            .map(|resource| super::auth_storage::storage(self.0.clone(), resource))
    }
    fn create_model_registry(
        &self,
        auth: crate::AuthStorage,
        path: Option<&str>,
    ) -> ExtensionResult<crate::ModelRegistry> {
        self.0
            .create_model_registry(super::auth_storage::resource::<I>(&auth)?, path)
            .map(|resource| super::model_registry::registry(self.0.clone(), resource))
    }
    fn in_memory_model_registry(
        &self,
        auth: crate::AuthStorage,
    ) -> ExtensionResult<crate::ModelRegistry> {
        self.0
            .in_memory_model_registry(super::auth_storage::resource::<I>(&auth)?)
            .map(|resource| super::model_registry::registry(self.0.clone(), resource))
    }
    fn exec<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: Option<crate::ExecOptions>,
    ) -> crate::ExtensionFuture<'a, crate::ExecResult> {
        Box::pin(async move {
            let data = encode_value(&(command, args, options.as_ref()))?;
            let caller = options.and_then(|options| options.signal);
            let binding = Binding::new(&self.0, caller.as_ref())?;
            let result = self.0.call("exec", data, binding.signal()).await;
            binding
                .settle(result)
                .and_then(|data| super::session::decode(&data))
        })
    }

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
            .map(|(index, pointer, handle, shape, local)| {
                (
                    index,
                    pointer,
                    super::callbacks::Scoped::from_handle(&self.0, handle),
                    shape,
                    local,
                )
            })
            .collect::<Vec<_>>();
        let tools = super::tools::decode_tools(&text)?;
        let mut codes = std::collections::HashMap::<usize, crate::schema_code::SchemaCode>::new();
        for (index, pointer, identity, shape, local) in handlers {
            let index = index as usize;
            if index >= tools.len() {
                return Err("schema binding has no matching tool".into());
            }
            let node = if local {
                let id = self.0.callback_id(&identity.handle);
                let handler = super::callbacks::find::<crate::schema_code::Handler>(id)?;
                super::schema::node(&handler)
            } else {
                super::schema::foreign(&self.0, identity, shape)
            };
            codes.entry(index).or_default().insert(pointer, node);
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
    fn contribute_load_time(&self, contributions: serde_json::Value) -> ExtensionResult<()> {
        self.0.contribute_load_time(&encode_value(&contributions)?)
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
            render_call,
            render_result,
        } = tool;
        let encoded = super::tools::encode_metadata(&metadata)?;
        let code = metadata
            .parameters
            .code()
            .iter()
            .map(|(pointer, node)| {
                let identity = Identity::new(&self.0);
                let handler = super::schema::handler(node.clone());
                identity.stage(handler);
                (pointer.clone(), identity)
            })
            .collect::<Vec<_>>();
        let prepare = prepare_arguments.map(|prepare| (Identity::new(&self.0), prepare));
        let render_call = render_call.map(|render| (Identity::new(&self.0), render));
        let render_result = render_result.map(|render| (Identity::new(&self.0), render));
        let identity = Identity::new(&self.0);
        let callbacks = ToolCallbacks {
            prepare: prepare.as_ref().map(|(identity, _)| &identity.handle),
            execute: &identity.handle,
            render_call: render_call.as_ref().map(|(identity, _)| &identity.handle),
            render_result: render_result.as_ref().map(|(identity, _)| &identity.handle),
        };
        let schemas = code
            .iter()
            .map(|(pointer, id)| (pointer.clone(), &id.handle))
            .collect::<Vec<_>>();
        let result = self.0.register_tool(&encoded, callbacks, &schemas);
        for (_, identity) in code {
            identity.settle(result.is_ok());
        }
        result?;
        if let Some((identity, prepare)) = prepare {
            identity.keep(prepare);
        }
        if let Some((identity, render)) = render_call {
            identity.keep(render);
        }
        if let Some((identity, render)) = render_result {
            identity.keep(render);
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
        super::provider_registration::register(&self.0, config, |text, callbacks| {
            self.0.register_provider(name, text, callbacks)
        })
    }
    fn unregister_provider(&self, name: &str) -> ExtensionResult<()> {
        self.0.unregister_provider(name)
    }
}
