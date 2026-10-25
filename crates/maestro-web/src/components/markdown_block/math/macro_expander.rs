//! Error retention for delegated native macro callbacks.
use katex::macros::{MacroDefinition, MacroExpansionResult};
use katex::{ParseError, Settings};
use std::sync::{Arc, Mutex};

/// Retain the first builtin error in settings owned by one formula invocation.
pub(super) fn settings(
    settings: &Settings,
    first_error: &Arc<Mutex<Option<ParseError>>>,
) -> Settings {
    let mut patched = settings.clone();
    for (name, definition) in katex::macros::builtins::BUILTIN_MACROS.entries() {
        let MacroDefinition::StaticFunction(function) = definition else {
            continue;
        };
        let function = *function;
        let captured = Arc::clone(first_error);
        patched.macros.get_mut().insert(
            (*name).to_owned(),
            MacroDefinition::Function(Arc::new(move |ctx| match function(ctx) {
                Ok(value) => Ok(value),
                Err(error) => {
                    let mut first = captured
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if first.is_none() {
                        *first = Some(error);
                    }
                    Ok(MacroExpansionResult::Empty)
                }
            })),
        );
    }
    patched
}
