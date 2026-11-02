//! Native format dispatch for registered source elements and the application panel adapter.
use super::{
    artifact_element::ArtifactElement as _, docx_artifact::DocxArtifact,
    excel_artifact::ExcelArtifact, generic_artifact::GenericArtifact, html_artifact::HtmlArtifact,
    image_artifact::ImageArtifact, markdown_artifact::MarkdownArtifact, pdf_artifact::PdfArtifact,
    svg_artifact::SvgArtifact, text_artifact::TextArtifact,
};
use crate::{
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use maestro_app::browser::tools::artifacts::{FileType, get_file_type};
mod element;
pub(crate) use element::define;
pub(super) use element::{mount, update};

/// One installed native format, never a replacement application file policy.
pub enum NativeArtifact {
    /// Literal or highlighted source.
    Text(TextArtifact),
    /// Original binary file guidance.
    Generic(GenericArtifact),
    /// Native image.
    Image(ImageArtifact),
    /// Markdown document.
    Markdown(MarkdownArtifact),
    /// Inert SVG resource.
    Svg(SvgArtifact),
    /// Progressive PDF pages.
    Pdf(PdfArtifact),
    /// Workbook tables.
    Excel(ExcelArtifact),
    /// Word document.
    Docx(DocxArtifact),
    /// Isolated executable HTML.
    Html(HtmlArtifact),
}
/// Apply the same native operation to each concrete owner without a second artifact trait.
macro_rules! visit {
    ($value:expr, $item:ident => $body:expr) => {
        match $value {
            NativeArtifact::Text($item) => $body,
            NativeArtifact::Generic($item) => $body,
            NativeArtifact::Image($item) => $body,
            NativeArtifact::Markdown($item) => $body,
            NativeArtifact::Svg($item) => $body,
            NativeArtifact::Pdf($item) => $body,
            NativeArtifact::Excel($item) => $body,
            NativeArtifact::Docx($item) => $body,
            NativeArtifact::Html($item) => $body,
        }
    };
}
impl NativeArtifact {
    /// Construct the application-selected native format with no platform host.
    pub(super) fn new(kind: FileType) -> Self {
        match kind {
            FileType::Html => Self::Html(HtmlArtifact::default()),
            FileType::Svg => Self::Svg(SvgArtifact::default()),
            FileType::Markdown => Self::Markdown(MarkdownArtifact::default()),
            FileType::Image => Self::Image(ImageArtifact::default()),
            FileType::Pdf => Self::Pdf(PdfArtifact::default()),
            FileType::Excel => Self::Excel(ExcelArtifact::default()),
            FileType::Docx => Self::Docx(DocxArtifact::default()),
            FileType::Text => Self::Text(TextArtifact::default()),
            FileType::Generic => Self::Generic(GenericArtifact::default()),
        }
    }
    /// Select the file format through the existing application classifier.
    pub(super) fn for_filename(filename: &str) -> Self {
        Self::new(get_file_type(filename))
    }
    /// Source-owned tag of this installed format.
    pub(super) fn tag(&self) -> &'static str {
        match self {
            Self::Text(_) => "text-artifact",
            Self::Generic(_) => "generic-artifact",
            Self::Image(_) => "image-artifact",
            Self::Markdown(_) => "markdown-artifact",
            Self::Svg(_) => "svg-artifact",
            Self::Pdf(_) => "pdf-artifact",
            Self::Excel(_) => "excel-artifact",
            Self::Docx(_) => "docx-artifact",
            Self::Html(_) => "html-artifact",
        }
    }
    /// Read the authored filename shared with a source tag's property.
    #[must_use]
    pub fn filename(&self) -> &str {
        match self {
            Self::Html(item) => &item.filename,
            Self::Docx(item) => &item.filename,
            Self::Text(item) => item.filename(),
            Self::Generic(item) => item.filename(),
            Self::Image(item) => item.filename(),
            Self::Markdown(item) => item.filename(),
            Self::Svg(item) => item.filename(),
            Self::Pdf(item) => item.filename(),
            Self::Excel(item) => item.filename(),
        }
    }
    /// Read the complete original source.
    #[must_use]
    pub fn content(&self) -> &str {
        visit!(self, item => item.content())
    }
    /// Assign the literal filename without retrying a failed document assignment.
    pub(super) fn set_filename(&mut self, filename: String) {
        match self {
            Self::Docx(item) => item.filename = filename,
            Self::Html(item) => item.filename = filename,
            Self::Text(item) => item.set_filename(filename),
            Self::Generic(item) => item.set_filename(filename),
            Self::Image(item) => item.set_filename(filename),
            Self::Markdown(item) => item.set_filename(filename),
            Self::Svg(item) => item.set_filename(filename),
            Self::Pdf(item) => item.set_filename(filename),
            Self::Excel(item) => item.set_filename(filename),
        }
    }
    /// Assign original content through its actual format owner.
    pub(super) fn set_content(&mut self, content: String) -> Result<(), ToolUiError> {
        match self {
            Self::Html(item) => return item.set_content(content),
            Self::Docx(item) => item.set_content(content),
            Self::Text(item) => item.set_content(content),
            Self::Generic(item) => item.set_content(content),
            Self::Image(item) => item.set_content(content),
            Self::Markdown(item) => item.set_content(content),
            Self::Svg(item) => item.set_content(content),
            Self::Pdf(item) => item.set_content(content),
            Self::Excel(item) => item.set_content(content),
        }
        Ok(())
    }
    /// Build this live format owner's independent native header.
    /// # Errors
    /// Returns native construction, localization or original-byte decoding failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        visit!(self, item => item.get_header_buttons(view))
    }
}
impl DomContent for NativeArtifact {
    fn key(&self) -> Option<&str> {
        Some(self.tag())
    }
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        visit!(self, item => item.mount(view))
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        visit!(self, item => item.update(root, view))
    }
}
