//! Daemon operations, pane rendering, themes, supported agents and default settings exposed to frontends.

/// The daemon operations and records used by native frontends.
pub mod daemon {
    #[cfg(target_os = "linux")]
    #[doc(inline)]
    pub use maestro_daemon::app::App;
    #[cfg(target_os = "linux")]
    #[doc(inline)]
    pub use maestro_daemon::app::construction::{
        ManifestCache, PaneLaunchInputs, ProcessDetectionMode, PtySetupEntry, setup_entry,
    };
    #[cfg(target_os = "linux")]
    #[doc(inline)]
    pub use maestro_daemon::app::model::ThemeRuntimeConfig;
    #[doc(inline)]
    pub use maestro_daemon::locations::DaemonPaths;
    #[doc(inline)]
    pub use maestro_daemon::panes::display::{PaneRenderView, PaneTerminal};
    #[cfg(target_os = "linux")]
    #[doc(inline)]
    pub use maestro_daemon::panes::launch::STARTUP_CWD_ENV;
    #[cfg(unix)]
    #[doc(inline)]
    pub use maestro_daemon::platform::current_process_is_detached_server_daemon;
    #[doc(inline)]
    pub use maestro_daemon::render_signal::RenderSignal;
    #[doc(inline)]
    pub use maestro_daemon::service::RequestMessage;
    #[cfg(unix)]
    #[doc(inline)]
    pub use maestro_daemon::service::RuntimeStatus;
    #[doc(inline)]
    pub use maestro_daemon::service::event_hub::EventHub;
    #[cfg(unix)]
    #[doc(inline)]
    pub use maestro_daemon::service::socket::{
        AsyncLocalListener, AsyncLocalListenerExt, AsyncLocalStream, SocketClaim,
        SocketFileIdentity, bind_async_local_listener, is_connection_closed_error,
        prepare_socket_path, remove_socket_file_if_owned, restrict_socket_permissions,
        socket_file_identity, split_async_stream,
    };
    #[cfg(unix)]
    #[doc(inline)]
    pub use maestro_daemon::service::transport::{ApiClientError, ConnectionTarget, DaemonClient};
    #[doc(inline)]
    pub use maestro_daemon::settings::{
        DEFAULTS_TEXT,
        appearance::{CustomThemeColors, THEME_NAMES, canonical_theme_name},
        model::{Config, NewTerminalCwdConfig},
    };
    #[doc(inline)]
    pub use maestro_daemon::terminal_theme::{HostAppearance, RgbColor, TerminalTheme};
    #[doc(inline)]
    pub use maestro_daemon::{Agent, agent_label};

    /// File logging setup and lifecycle events for native daemon roles.
    pub mod logging {
        #[doc(inline)]
        pub use maestro_daemon::diagnostics::{init_file_logging, shutdown};
    }

    /// Opt-in measurements emitted by pane rendering and ANSI encoding.
    pub mod render_prof {
        #[doc(inline)]
        pub use maestro_daemon::render_prof::{
            counter, duration, duration_since, enabled, event, timer,
        };
    }
}
