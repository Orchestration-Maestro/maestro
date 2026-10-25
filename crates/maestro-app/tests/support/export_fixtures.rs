//! HTML export facilities: recorded export assets and a rendering environment over a
//! published theme.
#![cfg(not(target_arch = "wasm32"))]
use maestro_export::{ExportAsset, ExportOperations};
use maestro_theme::{ColorMode, LiveTheme, Theme, ThemeError, ThemeExportColors, ThemeOptions};
use maestro_tools::ToolRenderEnvironment;
use maestro_tui::TerminalImage;
use maestro_tui::tui::{LocalFuture, LogContext, RenderCallback, RenderTimer, TuiRuntime};
use std::{cell::RefCell, io, path::Path, rc::Rc, time::Duration};

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

    fn environment(&self, _key: &str) -> Option<String> {
        None
    }

    fn log_context(&self) -> LogContext {
        LogContext {
            home: Path::new("").to_path_buf(),
            iso_time: String::new(),
            unix_ms: 0,
            nonce: String::new(),
        }
    }

    fn append_log(&self, path: &Path, _contents: &str) -> io::Result<()> {
        Err(no_log_file(path))
    }

    fn write_log(&self, path: &Path, _contents: &str) -> io::Result<()> {
        Err(no_log_file(path))
    }
}

/// The failure an export sees when the host keeps no log file at `path`.
fn no_log_file(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        format!("no log file at {}", path.display()),
    )
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
pub fn published() -> LiveTheme {
    let theme = Theme::new([], [], ColorMode::Color256, ThemeOptions::default()).unwrap();
    LiveTheme::from_theme(Rc::new(theme))
}

/// A rendering environment over `live_theme`.
pub fn environment(live_theme: LiveTheme) -> Rc<ToolRenderEnvironment> {
    Rc::new(ToolRenderEnvironment {
        runtime: Rc::new(ExportHost),
        readme_path: String::new(),
        live_theme,
        highlighter: Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        terminal_image: TerminalImage::default(),
    })
}
