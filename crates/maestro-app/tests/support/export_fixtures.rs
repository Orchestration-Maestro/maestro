//! HTML export facilities: recorded export assets and a rendering environment over a
//! published theme.
#![cfg(not(target_arch = "wasm32"))]
use maestro_export::{ExportAsset, ExportOperations};
use maestro_theme::{ColorMode, LiveTheme, Theme, ThemeError, ThemeExportColors, ThemeOptions};
use maestro_tools::ToolRenderEnvironment;
use maestro_tui::TerminalImage;
use maestro_tui::tui::{LocalFuture, LogContext, RenderCallback, RenderTimer, TuiRuntime};
use std::{
    cell::RefCell,
    io,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

/// A host whose deferred work never runs: exports render synchronously, so nothing here
/// is needed, and every operation the export path may reach is answered plainly.
#[derive(Default)]
pub struct ExportHost;

impl TuiRuntime for ExportHost {
    fn now(&self) -> Duration {
        Duration::ZERO
    }

    fn schedule(&self, _: Duration, _: RenderCallback) -> Box<dyn RenderTimer> {
        Box::new(ExportTimer)
    }

    fn spawn_local(&self, _: LocalFuture) {}

    fn environment(&self, _: &str) -> Option<String> {
        None
    }

    fn log_context(&self) -> LogContext {
        LogContext {
            home: PathBuf::new(),
            iso_time: String::new(),
            unix_ms: 0,
            nonce: String::new(),
        }
    }

    fn append_log(&self, path: &Path, _: &str) -> io::Result<()> {
        Err(io::Error::other(format!(
            "exports write no log: {}",
            path.display()
        )))
    }

    fn write_log(&self, path: &Path, _: &str) -> io::Result<()> {
        Err(io::Error::other(format!(
            "exports write no log: {}",
            path.display()
        )))
    }
}

/// A timer of a host that never runs deferred work.
struct ExportTimer;

impl RenderTimer for ExportTimer {
    fn cancel(&mut self) {}
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
