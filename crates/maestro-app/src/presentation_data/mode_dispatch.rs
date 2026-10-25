//! Supplied presentations for the final startup handoff.
use crate::AgentSessionRuntime;
use maestro_tui::tui::LocalFuture;
use std::{error::Error, io, rc::Rc};
/// How the interactive mode starts.
#[derive(Default)]
pub struct InteractiveModeOptions {
    /// Forces the expanded startup help, even when startup is quiet.
    pub verbose: bool,
    /// Providers whose credentials were migrated at startup, warned about in order.
    pub migrated_providers: Vec<String>,
    /// The warning shown when the session model could not be restored.
    pub model_fallback_message: Option<String>,
    /// The first prompt sent at startup.
    pub initial_message: Option<String>,
    /// Images attached to the first startup prompt.
    pub initial_images: Option<Vec<crate::ImageContent>>,
    /// Prompts sent after the first startup prompt, in order.
    pub initial_messages: Vec<String>,
    /// Hosted service origin; absent selects the compiled default.
    pub service_origin: Option<String>,
    /// Supplied HTTP transport for startup requests.
    pub fetch: Option<crate::Fetch>,
}

/// A retained interactive presentation, with its existing lifecycle.
pub trait InteractiveModeRunner {
    /// Initialize the existing presentation.
    ///
    /// # Errors
    /// Returns the existing initialization error.
    fn init(self: Rc<Self>) -> LocalFuture;
    /// Run the existing presentation.
    fn run(self: Rc<Self>) -> LocalFuture;
    /// Stop the existing presentation without disposing the session runtime.
    ///
    /// # Errors
    /// Returns the existing stop error.
    fn stop(&self) -> io::Result<()>;
}
/// Constructs one retained interactive presentation.
pub type InteractiveModeFactory = Rc<
    dyn Fn(
        Rc<AgentSessionRuntime>,
        InteractiveModeOptions,
    ) -> Result<Rc<dyn InteractiveModeRunner>, Box<dyn Error>>,
>;
/// Runs the supplied standard-stream protocol presentation.
pub type RpcModeRunner = Rc<dyn Fn(Rc<AgentSessionRuntime>) -> LocalFuture>;
/// Presentations selected only by the final startup branch.
pub struct ModeLaunchers {
    /// Builds the interactive presentation only on its selected branch.
    pub interactive: InteractiveModeFactory,
    /// Runs the protocol presentation only on its selected branch.
    pub rpc: RpcModeRunner,
}
