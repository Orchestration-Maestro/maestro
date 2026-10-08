//! Author seam for the native-async extension component contract.
//!
//! Generated records are re-exported under their domain names. Callbacks and
//! contexts are facades over two adapters that share this seam: the component
//! adapter forwarding to the generated imports and exports, and the controlled
//! adapter used by native tests.
#![forbid(unsafe_code)]

mod api;
#[doc(hidden)]
pub mod bindings;
mod component;
mod context;
pub mod controlled;
mod dispatch;
mod generated;
mod tool;

pub use api::{
    Extension, ExtensionAPI, ExtensionFuture, ExtensionHandler, ExtensionHost, ExtensionResult,
    InputHandler,
};
pub use bindings::maestro::extension::types::{
    CustomMessage, InputEvent, InputOutcome, InputReplacement, InputResult, InputSource,
    NewSessionOptions, SessionChangeResult, ToolMetadata,
};
pub use component::{Component, ComponentView, MessageRenderer};
pub use context::{
    AbortSignal, CommandContextPort, CommandHandler, ContextPort, ExtensionCommandContext,
    ExtensionContext, NewSessionCommandOptions, ReplacedSessionContext, ReplacedSessionContextPort,
    SignalPort, WithSession,
};
pub use dispatch::{ToolInvocation, prepare_arguments, run_input, run_tool};
#[doc(hidden)]
pub use generated::Glue;
pub use tool::{
    AgentToolResult, AgentToolUpdateCallback, PrepareArguments, ToolDefinition, ToolExecute,
};

/// Exports an extension factory as the component's entry point.
#[macro_export]
macro_rules! export_extension {
    ($factory:path) => {
        struct MaestroExtension;
        impl $crate::Extension for MaestroExtension {
            fn load(api: $crate::ExtensionAPI) -> $crate::ExtensionFuture<'static, ()> {
                $factory(api)
            }
        }
        type MaestroGlue = $crate::Glue<MaestroExtension>;
        $crate::bindings::export!(MaestroGlue with_types_in $crate::bindings);
    };
}
