//! Registration observations shared by the controlled and component hosts.
use serde_json::{Value, json};
use std::collections::HashMap;

/// Operations observed at the host boundary and explicit supplied replies.
#[derive(Default)]
pub(crate) struct Bank {
    /// Explicit host rejection applied before registration state changes.
    pub error: Option<String>,
    /// Registration number to refuse, counted from zero, with the host's message.
    pub refuse_at: Option<(usize, String)>,
    /// Registrations admitted while a refusal was armed.
    admitted: usize,
    /// Command names, descriptions and independent completion presence.
    pub commands: Vec<(String, Option<String>, bool)>,
    /// Literal shortcut keys and descriptions.
    pub shortcuts: Vec<(String, Option<String>)>,
    /// Literal flag names and selected option fields.
    pub flags: Vec<(String, Value)>,
    /// Current host-supplied flag replies; absence is a valid reply.
    pub values: HashMap<String, Result<Option<Value>, String>>,
}

impl Bank {
    /// The message refusing this registration, after admitting the configured prefix.
    pub fn refusal(&mut self) -> Option<String> {
        if let Some(error) = &self.error {
            return Some(error.clone());
        }
        match &self.refuse_at {
            Some((position, message)) if *position == self.admitted => Some(message.clone()),
            _ => {
                self.admitted += 1;
                None
            }
        }
    }

    /// Returns the currently configured host reply without resolving defaults.
    pub fn value(&self, name: &str) -> Result<Option<crate::FlagValue>, String> {
        self.values
            .get(name)
            .cloned()
            .unwrap_or(Ok(None))?
            .map(|value| match value {
                Value::Bool(value) => Ok(crate::FlagValue::Boolean(value)),
                Value::String(value) => Ok(crate::FlagValue::String(value)),
                _ => Err("invalid controlled flag reply".into()),
            })
            .transpose()
    }
}

/// Captures the option fields independently of default resolution.
pub(super) fn options(options: &crate::FlagOptions) -> Value {
    let default = options.default.as_ref().map(|value| match value {
        crate::FlagValue::Boolean(value) => json!(value),
        crate::FlagValue::String(value) => json!(value),
    });
    json!({"description": options.description, "type": match options.type_ {
        crate::FlagType::Boolean => "boolean", crate::FlagType::String => "string",
    }, "default": default})
}
