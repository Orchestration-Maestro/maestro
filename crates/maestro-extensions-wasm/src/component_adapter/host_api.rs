//! Registrations and actions forwarded to the generated host imports.

use super::callbacks::Identity;
use crate::bindings::maestro::extension::host;
use crate::types::{CommandOptions, ExtensionHandler, ExtensionHost, ExtensionResult};

/// Generated imports behind the facade.
pub(super) struct GeneratedHost;

impl ExtensionHost for GeneratedHost {
    fn on(&self, event: &str, handler: ExtensionHandler) -> ExtensionResult<()> {
        let identity = Identity::new();
        host::on(event, &identity.handle)?;
        identity.keep(handler);
        Ok(())
    }

    fn register_command(&self, name: &str, options: CommandOptions) -> ExtensionResult<()> {
        let CommandOptions {
            description,
            handler,
        } = options;
        let identity = Identity::new();
        host::register_command(name, description.as_deref(), &identity.handle)?;
        identity.keep(handler);
        Ok(())
    }

    fn append_entry(
        &self,
        custom_type: &str,
        data: Option<serde_json::Value>,
    ) -> ExtensionResult<()> {
        host::append_entry(custom_type, data.map(|data| data.to_string()).as_deref())
    }
}
