//! Retained literal or highlighted source artifact with original-text controls.
use super::artifact_element::{ArtifactElement, ArtifactView};
use crate::tools::artifacts::ArtifactUiError;
use crate::{
    browser::attachment_content::{self, AttachmentHost, ReadingContent},
    components::{copy_button::CopyButton, download_button},
};
use std::{cell::RefCell, rc::Rc};

/// Fixed source policy of an artifact family.
#[derive(Clone, Copy, Default)]
pub(super) enum SourceProfile {
    /// Filename-classified text.
    #[default]
    Text,
    /// Markdown source.
    Markdown,
    /// XML source.
    Svg,
}
/// Prepared native body, recomputed only after a field edit.
#[derive(Default)]
struct Body {
    /// Requested source language, absent for plain text.
    language: Option<&'static str>,
    /// Original literal text.
    text: String,
    /// Escaped highlighted fragment when classification succeeded.
    html: Option<String>,
    /// Markdown source wraps without changing its literal text.
    #[cfg(target_arch = "wasm32")]
    wrap: bool,
    /// Stable code node retained across edits.
    #[cfg(target_arch = "wasm32")]
    code: Option<web_sys::Element>,
}
/// Editable source artifact retaining its native reading root and copy feedback.
pub struct TextArtifact {
    /// Authored filename.
    filename: String,
    /// Fixed source family.
    profile: SourceProfile,
    /// Original content.
    content: String,
    /// Native effects owner.
    host: Option<Rc<dyn AttachmentHost>>,
    /// Independent surface identity.
    id: egui::Id,
    /// Prepared body and mounted node.
    body: Rc<RefCell<Body>>,
    /// Published reading factory.
    reading: Option<Rc<ReadingContent>>,
    /// Retained header feedback.
    copy: Option<CopyButton>,
    /// Whether body operands need preparation.
    changed: bool,
    /// Whether the copy operand needs updating.
    copy_changed: bool,
    /// Last drawing context for setters.
    context: Option<egui::Context>,
}
impl TextArtifact {
    /// Retain one source owner under its fixed family policy.
    pub(super) fn with_profile(host: Rc<dyn AttachmentHost>, profile: SourceProfile) -> Self {
        let mut artifact = Self::new(host);
        artifact.profile = profile;
        #[cfg(target_arch = "wasm32")]
        {
            artifact.body.borrow_mut().wrap = matches!(profile, SourceProfile::Markdown);
        }
        artifact
    }

    /// Whether retained effects were supplied at construction.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn is_retained(&self) -> bool {
        self.host.is_some()
    }
    /// Create a host-free source listing under its fixed family policy.
    pub(super) fn page_profile(profile: SourceProfile) -> Self {
        let mut source = Self::default();
        source.profile = profile;
        source
    }
    /// Start with empty filename and content using the supplied native host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self::with_host(Some(host))
    }
    /// Create source storage without requiring retained surface effects.
    fn with_host(host: Option<Rc<dyn AttachmentHost>>) -> Self {
        Self {
            filename: String::new(),
            profile: SourceProfile::Text,
            content: String::new(),
            host,
            id: attachment_content::identity(),
            body: Rc::default(),
            reading: None,
            copy: None,
            changed: true,
            copy_changed: true,
            context: None,
        }
    }
    /// Wake the last frame after edits.
    fn repaint(&self) {
        if let Some(context) = &self.context {
            context.request_repaint();
        }
    }
    /// Classify the last literal filename segment.
    fn language(&self) -> Option<&'static str> {
        match self.profile {
            SourceProfile::Markdown => return Some("markdown"),
            SourceProfile::Svg => return Some("xml"),
            SourceProfile::Text => {}
        }
        Some(match super::extension(&self.filename).as_str() {
            "js" | "javascript" => "javascript",
            "ts" | "typescript" => "typescript",
            "py" | "python" => "python",
            "rb" | "ruby" => "ruby",
            "yml" | "yaml" => "yaml",
            "ps1" => "powershell",
            "bat" => "batch",
            "jsx" => "jsx",
            "tsx" => "tsx",
            "java" => "java",
            "c" => "c",
            "cpp" => "cpp",
            "cs" => "cs",
            "php" => "php",
            "go" => "go",
            "rust" => "rust",
            "swift" => "swift",
            "kotlin" => "kotlin",
            "scala" => "scala",
            "dart" => "dart",
            "html" => "html",
            "css" => "css",
            "scss" => "scss",
            "sass" => "sass",
            "less" => "less",
            "json" => "json",
            "xml" => "xml",
            "toml" => "toml",
            "sql" => "sql",
            "sh" => "sh",
            "bash" => "bash",
            "r" => "r",
            "matlab" => "matlab",
            "julia" => "julia",
            "lua" => "lua",
            "perl" => "perl",
            "vue" => "vue",
            "svelte" => "svelte",
            _ => return None,
        })
    }
    /// Select text download media type without inferring it from content.
    fn mime(&self) -> &'static str {
        match self.profile {
            SourceProfile::Markdown => return "text/markdown",
            SourceProfile::Svg => return "image/svg+xml",
            SourceProfile::Text => {}
        }
        match super::extension(&self.filename).as_str() {
            "svg" => "image/svg+xml",
            "md" | "markdown" => "text/markdown",
            _ => "text/plain",
        }
    }
    /// Localized title keys owned by this source family.
    fn titles(&self) -> (&'static str, &'static str) {
        match self.profile {
            SourceProfile::Text => ("Copy", "Download"),
            SourceProfile::Markdown => ("Copy Markdown", "Download Markdown"),
            SourceProfile::Svg => ("Copy SVG", "Download SVG"),
        }
    }
    /// Prepare highlighting before borrowing any mounted node state.
    fn highlighted(
        &self,
        highlighter: &dyn maestro_theme::SyntaxHighlighter,
        language: Option<&str>,
    ) -> Option<String> {
        language.and_then(|language| {
            maestro_theme::highlight_html(highlighter, &self.content, Some(language))
                .ok()
                .flatten()
        })
    }
    /// Prepare outside the state borrow so the caller's engine may call out safely.
    fn prepare(&mut self, view: ArtifactView<'_>) {
        if !self.changed {
            return;
        }
        let language = self.language();
        let html = self.highlighted(view.highlighter, language);
        let mut body = self.body.borrow_mut();
        body.language = language;
        body.text.clone_from(&self.content);
        #[cfg(target_arch = "wasm32")]
        let html_changed = body.html != html;
        body.html = html;
        #[cfg(target_arch = "wasm32")]
        browser::sync(&body, html_changed);
        self.changed = false;
    }
    super::retained::artifact_reading!(body);
}
impl Default for TextArtifact {
    /// Empty host-free source content.
    fn default() -> Self {
        Self::with_host(None)
    }
}
impl ArtifactElement for TextArtifact {
    fn filename(&self) -> &str {
        &self.filename
    }
    fn content(&self) -> &str {
        &self.content
    }
    fn set_filename(&mut self, filename: String) {
        self.changed |= self.filename != filename;
        self.filename = filename;
        self.repaint();
    }
    fn set_content(&mut self, content: String) {
        self.changed |= self.content != content;
        self.copy_changed |= self.content != content;
        self.content = content;
        self.repaint();
    }
    super::retained::artifact_visibility!();
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        if self.copy.is_none() {
            self.copy = Some(CopyButton::new(
                Rc::clone(super::retained::host(self.host.as_ref())?),
                view.i18n,
            )?);
        }
        let titles = self.titles();
        if let Some(copy) = &mut self.copy {
            if self.copy_changed {
                copy.text.clone_from(&self.content);
                self.copy_changed = false;
            }
            copy.title = view.i18n.text(titles.0)?;
            copy.render(ui, view.i18n, view.presentation)
                .map_err(ArtifactUiError::localized_control)?;
        }
        download_button::draw_titled(
            ui,
            view,
            super::retained::host(self.host.as_ref())?.as_ref(),
            &self.filename,
            (self.mime(), self.content.as_bytes(), titles.1),
        )
    }
    fn render(&mut self, ui: &mut egui::Ui, view: ArtifactView<'_>) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        self.prepare(view);
        let reading = self.reading();
        let rect = ui
            .allocate_space(ui.available_size().max(egui::Vec2::ZERO))
            .1;
        attachment_content::reading::present(
            super::retained::host(self.host.as_ref())?.as_ref(),
            ui,
            (self.id, rect, None),
            &reading,
            view.presentation,
        )?;
        self.reading = Some(reading);
        Ok(())
    }
}
impl Drop for TextArtifact {
    fn drop(&mut self) {
        if let Some(host) = &self.host {
            host.remove(self.id);
        }
    }
}
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub(crate) use browser::source_nodes;

#[cfg(target_arch = "wasm32")]
mod dom;
