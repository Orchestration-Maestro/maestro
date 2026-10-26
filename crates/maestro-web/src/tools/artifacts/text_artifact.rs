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
    host: Rc<dyn AttachmentHost>,
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

    /// Start with empty filename and content using the supplied native host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
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
    /// Prepare outside the state borrow so the caller's engine may call out safely.
    fn prepare(&mut self, view: ArtifactView<'_>) {
        if !self.changed {
            return;
        }
        let language = self.language();
        let html = language.and_then(|language| {
            maestro_theme::highlight_html(view.highlighter, &self.content, Some(language))
                .ok()
                .flatten()
        });
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
    fn set_visibility(
        &mut self,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), ArtifactUiError> {
        self.host
            .set_retained_visibility(self.id, visible, modal)
            .map_err(ArtifactUiError::Host)
    }
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        if self.copy.is_none() {
            self.copy = Some(CopyButton::new(Rc::clone(&self.host), view.i18n)?);
        }
        if let Some(copy) = &mut self.copy {
            if self.copy_changed {
                copy.text.clone_from(&self.content);
                self.copy_changed = false;
            }
            copy.title = view.i18n.text(match self.profile {
                SourceProfile::Text => "Copy",
                SourceProfile::Markdown => "Copy Markdown",
                SourceProfile::Svg => "Copy SVG",
            })?;
            copy.render(ui, view.i18n, view.presentation)
                .map_err(ArtifactUiError::localized_control)?;
        }
        download_button::draw_titled(
            ui,
            view,
            self.host.as_ref(),
            &self.filename,
            (
                self.mime(),
                self.content.as_bytes(),
                match self.profile {
                    SourceProfile::Text => "Download",
                    SourceProfile::Markdown => "Download Markdown",
                    SourceProfile::Svg => "Download SVG",
                },
            ),
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
            self.host.as_ref(),
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
        self.host.remove(self.id);
    }
}
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub(crate) use browser::source_nodes;
