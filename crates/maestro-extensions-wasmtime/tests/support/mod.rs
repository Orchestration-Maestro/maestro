//! Live domain bindings shared by component and typed-request witnesses.
#![cfg(test)]
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, types::ProviderActions,
};
use std::rc::Rc;

/// Explicit controlled effects for fixtures that do not resolve credentials.
pub fn credential_operations() -> Rc<dyn maestro_credentials::ConfigValueOperations> {
    Rc::new(NoCredentials)
}

/// A fixture must not accidentally invoke an ambient environment or shell.
struct NoCredentials;

impl maestro_credentials::ConfigValueOperations for NoCredentials {
    fn environment(&self, _: &str) -> Option<String> {
        panic!("fixture requests no credential environment");
    }
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        panic!("fixture requests no credential shell");
    }
}

/// Binds a prompt reader without replacing command-specific operations.
pub fn bind_prompt(runner: &ExtensionRunner<Local>, prompt: &'static str) {
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_system_prompt: Rc::new(move || Ok(prompt.into())),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}
