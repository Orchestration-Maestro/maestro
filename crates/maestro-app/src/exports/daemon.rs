//! Pane rendering and host theme records exposed to frontends.

/// The daemon records and profiling operations used by the multiplexer.
pub mod daemon {
    #[cfg(target_os = "linux")]
    #[doc(inline)]
    pub use maestro_daemon::app::model::ThemeRuntimeConfig;
    #[doc(inline)]
    pub use maestro_daemon::panes::display::{PaneRenderView, PaneTerminal};
    #[doc(inline)]
    pub use maestro_daemon::settings::{
        appearance::{CustomThemeColors, THEME_NAMES, canonical_theme_name},
        model::Config,
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
