//! Registrations and actions forwarded to the host imports.

use super::callbacks::Identity;
use super::exports::encode_value;
use super::imports::Imports;
use crate::ToolDefinition;
use crate::types::{CommandOptions, ExtensionHandler, ExtensionHost, ExtensionResult};

/// The host imports behind the facade.
pub(super) struct Host<I: Imports>(pub(super) I);

impl<I: Imports> ExtensionHost for Host<I> {
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
        } = options;
        let identity = Identity::new(&self.0);
        self.0
            .register_command(name, description.as_deref(), &identity.handle)?;
        identity.keep(handler);
        Ok(())
    }

    fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()> {
        let ToolDefinition {
            metadata,
            prepare_arguments,
            execute,
        } = tool;
        let metadata = encode_value(&metadata)?;
        let prepare = prepare_arguments.map(|prepare| (Identity::new(&self.0), prepare));
        let identity = Identity::new(&self.0);
        self.0.register_tool(
            &metadata,
            prepare.as_ref().map(|(identity, _)| &identity.handle),
            &identity.handle,
        )?;
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
}
