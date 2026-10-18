//! A UI host without optional operations, shared by capability and registration witnesses.
#![cfg(test)]
use maestro_extensions::types::ExtensionUIContextPort;
use maestro_theme::LiveTheme;

/// A host installed without supporting any optional operation.
pub struct Limited;
impl ExtensionUIContextPort for Limited {
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}
