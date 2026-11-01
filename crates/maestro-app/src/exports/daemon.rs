//! Pane rendering and host theme records exposed to frontends.

/// The daemon records and profiling operations used by the multiplexer.
pub mod daemon {
    #[cfg(target_os = "linux")]
    #[doc(inline)]
    pub use maestro_daemon::app::state::ThemeRuntimeConfig;
    #[doc(inline)]
    pub use maestro_daemon::pane::terminal::{PaneRenderView, PaneTerminal};
    #[doc(inline)]
    pub use maestro_daemon::settings::{
        model::Config,
        theme::{CustomThemeColors, THEME_NAMES, canonical_theme_name},
    };
    #[doc(inline)]
    pub use maestro_daemon::terminal_theme::{HostAppearance, RgbColor, TerminalTheme};

    /// Opt-in measurements emitted by pane rendering and ANSI encoding.
    pub mod render_prof {
        #[doc(inline)]
        pub use maestro_daemon::render_prof::{
            counter, duration, duration_since, enabled, event, timer,
        };
    }
}
