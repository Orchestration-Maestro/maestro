//! Daemon operations, pane rendering, themes, supported agents and default settings exposed to frontends.

/// The daemon operations and records used by native frontends.
pub mod daemon {
    #[doc(inline)]
    pub use maestro_daemon::api::RequestMessage;
    #[cfg(unix)]
    #[doc(inline)]
    pub use maestro_daemon::api::RuntimeStatus;
    #[cfg(unix)]
    #[doc(inline)]
    pub use maestro_daemon::api::client::{ApiClientError, ConnectionTarget, DaemonClient};
    #[doc(inline)]
    pub use maestro_daemon::api::event_hub::EventHub;
    #[cfg(unix)]
    #[doc(inline)]
    pub use maestro_daemon::api::socket::{
        AsyncLocalListener, AsyncLocalListenerExt, AsyncLocalStream, SocketFileIdentity,
        bind_async_local_listener, is_connection_closed_error, prepare_socket_path,
        remove_socket_file_if_owned, restrict_socket_permissions, socket_file_identity,
        split_async_stream,
    };
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
    pub use maestro_daemon::app::state::ThemeRuntimeConfig;
    #[cfg(target_os = "linux")]
    #[doc(inline)]
    pub use maestro_daemon::pane::launch_env::STARTUP_CWD_ENV;
    #[doc(inline)]
    pub use maestro_daemon::pane::terminal::{PaneRenderView, PaneTerminal};
    #[doc(inline)]
    pub use maestro_daemon::paths::DaemonPaths;
    #[cfg(unix)]
    #[doc(inline)]
    pub use maestro_daemon::platform::current_process_is_detached_server_daemon;
    #[doc(inline)]
    pub use maestro_daemon::render_signal::RenderSignal;
    #[doc(inline)]
    pub use maestro_daemon::settings::{
        DEFAULTS_TEXT,
        model::{Config, NewTerminalCwdConfig},
        theme::{CustomThemeColors, THEME_NAMES, canonical_theme_name},
    };
    #[doc(inline)]
    pub use maestro_daemon::terminal_theme::{HostAppearance, RgbColor, TerminalTheme};
    #[doc(inline)]
    pub use maestro_daemon::{Agent, agent_label};

    /// File logging setup and lifecycle events for native daemon roles.
    pub mod logging {
        #[doc(inline)]
        pub use maestro_daemon::logging::{init_file_logging, shutdown};
    }

    /// Opt-in measurements emitted by pane rendering and ANSI encoding.
    pub mod render_prof {
        #[doc(inline)]
        pub use maestro_daemon::render_prof::{
            counter, duration, duration_since, enabled, event, timer,
        };
    }
}
