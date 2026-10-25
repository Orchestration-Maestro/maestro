//! Selector presentations the command line and the interactive frontend share.
//!
//! One record carries each caller operation, so both fronts hand the same
//! component the same fields and the settlement rules stay in one place.

use maestro_tui::KeybindingsManager;
use maestro_tui::tui::ComponentHandle;
#[cfg(not(target_arch = "wasm32"))]
use std::cell::RefCell;
use std::{error::Error, future::Future, io, pin::Pin, rc::Rc};

use crate::SessionInfo;
#[cfg(not(target_arch = "wasm32"))]
use crate::{ResolvedPaths, SettingsManager};

/// One presented component with the handle its input focuses.
pub struct PresentedSelector {
    /// The component added to the frame writer.
    pub component: ComponentHandle,
    /// The retained input target focused after adding.
    pub focus: ComponentHandle,
}

/// Supplied locations and managers of one configuration presentation.
#[cfg(not(target_arch = "wasm32"))]
pub struct ConfigSelectorOptions {
    /// Resolved resource paths the configuration edits.
    pub resolved_paths: ResolvedPaths,
    /// Settings manager the resource toggles write.
    pub settings_manager: Rc<RefCell<SettingsManager>>,
    /// Project directory used for project-relative settings.
    pub cwd: String,
    /// User resource directory.
    pub agent_dir: String,
}

/// Caller notifications of a configuration presentation.
pub struct ConfigSelectorCallbacks {
    /// Close notification.
    pub on_close: Rc<dyn Fn()>,
    /// Explicit exit notification.
    pub on_exit: Rc<dyn Fn()>,
    /// Redraw notification.
    pub request_render: Rc<dyn Fn()>,
}

/// Builds a presented configuration component over the supplied options.
#[cfg(not(target_arch = "wasm32"))]
pub type ConfigSelectorFactory =
    Rc<dyn Fn(ConfigSelectorOptions, ConfigSelectorCallbacks) -> io::Result<PresentedSelector>>;

/// Discovery progress notification of one scope.
pub type SessionListProgress = Rc<dyn Fn(usize, usize)>;

/// One scope's pending discovery.
pub type SessionsFuture =
    Pin<Box<dyn Future<Output = Result<Vec<SessionInfo>, Box<dyn Error>>> + 'static>>;

/// One scope's discovery factory, invoked before its future is scheduled.
pub type SessionsLoader = Rc<dyn Fn(Option<SessionListProgress>) -> SessionsFuture>;

/// Caller notifications of a session presentation.
pub struct SessionSelectorCallbacks {
    /// Selected session path.
    pub on_select: Rc<dyn Fn(&str)>,
    /// Cancellation notification.
    pub on_cancel: Rc<dyn Fn()>,
    /// Explicit exit notification.
    pub on_exit: Rc<dyn Fn()>,
}

/// Supplied discovery, bindings and redraw of one session presentation.
pub struct SessionSelectorRequest {
    /// Current-scope discovery factory.
    pub current_sessions_loader: SessionsLoader,
    /// All-scope discovery factory.
    pub all_sessions_loader: SessionsLoader,
    /// Bindings the selector reads and updates.
    pub keybindings: KeybindingsManager,
    /// Whether the rename hint is shown.
    pub show_rename_hint: bool,
    /// Redraw notification.
    pub request_render: Rc<dyn Fn()>,
}

/// Builds the presented session component over the supplied request.
pub type SessionSelectorFactory =
    Rc<dyn Fn(SessionSelectorRequest, SessionSelectorCallbacks) -> io::Result<PresentedSelector>>;

/// Supplied content, notifications and frame writer for an extension selector.
pub struct ExtensionSelectorRequest {
    /// Prompt displayed above the choices.
    pub title: String,
    /// Choices in presentation order.
    pub options: Vec<String>,
    /// Selected choice notification.
    pub on_select: Rc<dyn Fn(&str)>,
    /// Cancellation notification.
    pub on_cancel: Rc<dyn Fn()>,
    /// Supplied frame writer.
    pub tui: maestro_tui::TUI,
}

/// Builds an extension selector over the supplied content and writer.
pub type ExtensionSelectorFactory =
    Rc<dyn Fn(ExtensionSelectorRequest) -> io::Result<PresentedSelector>>;
