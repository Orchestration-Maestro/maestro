//! Native supplied-bundle and theme adapter.
use super::{ExportAsset, ExportOperations};
use maestro_theme::{ThemeError, ThemeExportColors, ThemeState};
use std::{fs, io};

/// Native bundle paths with a supplied theme owner.
pub struct NativeExportOperations<'a> {
    /// Application configuration name for default output names.
    pub app_name: &'a str,
    /// Directory containing the five prebuilt bundle assets.
    pub template_dir: &'a str,
    /// Theme owner used for separate resolved-color observations.
    pub theme: &'a ThemeState,
}
impl ExportOperations for NativeExportOperations<'_> {
    fn app_name(&self) -> &str {
        self.app_name
    }
    fn read_asset(&self, asset: ExportAsset) -> io::Result<String> {
        let name = match asset {
            ExportAsset::Template => "template.html",
            ExportAsset::Css => "template.css",
            ExportAsset::JavaScript => "template.js",
            ExportAsset::Markdown => "vendor/marked.min.js",
            ExportAsset::Highlight => "vendor/highlight.min.js",
        };
        Ok(
            String::from_utf8_lossy(&fs::read(maestro_path::join(&[self.template_dir, name]))?)
                .into_owned(),
        )
    }
    fn get_resolved_theme_colors(
        &self,
        theme_name: Option<&str>,
    ) -> Result<Vec<(String, String)>, ThemeError> {
        self.theme.get_resolved_theme_colors(theme_name)
    }
    fn get_theme_export_colors(&self, theme_name: Option<&str>) -> ThemeExportColors {
        self.theme.get_theme_export_colors(theme_name)
    }
}
