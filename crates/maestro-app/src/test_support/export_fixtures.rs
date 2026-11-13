//! HTML export facilities: recorded export assets and a rendering environment over a
//! published theme.
use maestro_export::{ExportAsset, ExportOperations};
use maestro_extensions::EventError;
use maestro_theme::{
    ColorMode, LiveTheme, SyntectHighlighter, Theme, ThemeError, ThemeExportColors, ThemeOptions,
};
use maestro_tools::ToolRenderEnvironment;
use maestro_tui::{
    LocalFuture, LogContext, RenderCallback, RenderTimer, TerminalImage, TuiRuntime,
};
use std::{cell::RefCell, io, rc::Rc, time::Duration};

/// The host an export renders through: no clock, deferred work, environment or log files.
struct ExportHost;

/// A scheduled callback this host never runs.
struct IdleTimer;

impl RenderTimer for IdleTimer {
    fn cancel(&mut self) {}
}

impl TuiRuntime for ExportHost {
    fn now(&self) -> Duration {
        Duration::ZERO
    }

    fn schedule(&self, _delay: Duration, _callback: RenderCallback) -> Box<dyn RenderTimer> {
        Box::new(IdleTimer)
    }

    fn spawn_local(&self, _future: LocalFuture) {}

    fn log_context(&self) -> LogContext {
        LogContext {
            agent_dir: String::new(),
            iso_time: String::new(),
            unix_ms: 0,
            nonce: String::new(),
        }
    }
}

/// Export assets whose template is the payload alone, with recorded theme lookups.
#[derive(Default)]
pub struct Recording {
    /// Theme names passed to color lookups.
    pub themes: RefCell<Vec<Option<String>>>,
}

impl ExportOperations for Recording {
    fn app_name(&self) -> &'static str {
        "maestro"
    }
    fn read_asset(&self, asset: ExportAsset) -> io::Result<String> {
        Ok(if asset == ExportAsset::Template {
            "{{SESSION_DATA}}".into()
        } else {
            String::new()
        })
    }
    fn get_resolved_theme_colors(
        &self,
        theme: Option<&str>,
    ) -> Result<Vec<(String, String)>, ThemeError> {
        self.themes.borrow_mut().push(theme.map(str::to_owned));
        Ok(vec![("userMessageBg".into(), "#343541".into())])
    }
    fn get_theme_export_colors(&self, _: Option<&str>) -> ThemeExportColors {
        ThemeExportColors::default()
    }
}

/// A plain theme published as the host publishes one.
///
/// # Errors
/// Returns the failure to build the plain theme.
pub fn published() -> Result<LiveTheme, ThemeError> {
    let theme = Theme::new([], [], ColorMode::Color256, ThemeOptions::default())?;
    Ok(LiveTheme::from_theme(Rc::new(theme)))
}

/// A rendering environment over `live_theme`.
///
/// # Errors
/// Returns the failure to build the syntax highlighter.
pub fn environment(live_theme: LiveTheme) -> Result<Rc<ToolRenderEnvironment>, EventError> {
    Ok(Rc::new(ToolRenderEnvironment {
        runtime: Rc::new(ExportHost),
        readme_path: String::new(),
        live_theme,
        highlighter: Rc::new(SyntectHighlighter::new()?),
        terminal_image: TerminalImage::default(),
    }))
}
