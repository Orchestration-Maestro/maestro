//! Custom-message renderer callbacks.
use crate::ExtensionExecution;
use maestro_models::{Execution, Threaded};
pub use maestro_session::messages::MessageRenderOptions;
use serde_json::Value;

/// Renders a custom message, or returns no component to keep the host's default.
pub type MessageRenderer<T = Value, E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Renderer<T>>;
