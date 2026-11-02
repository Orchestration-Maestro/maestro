//! Retained artifact panels using application-owned filename policy.
pub mod artifact_pill;
pub mod artifacts_tool_renderer;
#[cfg(target_arch = "wasm32")]
pub mod console;
mod presentation_cause;
pub use presentation_cause::PresentationCause;
mod ui_error;
pub use ui_error::ArtifactUiError;
pub mod artifact_element;
#[cfg(target_arch = "wasm32")]
pub mod docx_artifact;
pub mod excel_artifact;
pub mod generic_artifact;
#[cfg(target_arch = "wasm32")]
pub mod html_artifact;
pub mod image_artifact;
pub mod markdown_artifact;
pub mod pdf_artifact;
pub mod svg_artifact;
pub mod text_artifact;
use crate::browser::attachment_content::AttachmentHost;
pub use artifact_element::ArtifactElement;
use artifact_element::ArtifactView;
use maestro_app::browser::tools::artifacts::ArtifactPresentation;
use maestro_app::browser::tools::artifacts::{FileType, get_file_type};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
mod render;
/// Owned view handle shared across drawing and reentrant callbacks.
type Element = Rc<RefCell<Option<Box<dyn ArtifactElement>>>>;
/// Temporarily own a view so callbacks run without a live slot borrow.
struct TakenElement<'a> {
    /// Original slot, which may have been detached from the panel during a callback.
    slot: &'a Element,
    /// View restored even when a host callback unwinds.
    value: Option<Box<dyn ArtifactElement>>,
}
impl Drop for TakenElement<'_> {
    fn drop(&mut self) {
        let previous = self.slot.replace(self.value.take());
        drop(previous);
    }
}
/// Run a view operation with exclusive ownership, returning conflicts on reentry.
fn with_element(
    slot: &Element,
    operation: impl FnOnce(&mut dyn ArtifactElement) -> Result<(), ArtifactUiError>,
) -> Result<(), ArtifactUiError> {
    let value = slot
        .try_borrow_mut()
        .map_err(|_| ArtifactUiError::BorrowConflict)?
        .take()
        .ok_or(ArtifactUiError::BorrowConflict)?;
    let mut taken = TakenElement {
        slot,
        value: Some(value),
    };
    operation(
        taken
            .value
            .as_deref_mut()
            .ok_or(ArtifactUiError::BorrowConflict)?,
    )
}
/// Replaceable close or caller-notification operation.
type Callback = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// Retained views and layout using the application's single selected-file slot.
pub struct ArtifactsPanel {
    /// Native effect owner.
    host: Option<Rc<dyn AttachmentHost>>,
    /// Per-instance layout and modal identity.
    id: egui::Id,
    /// Installed views keyed by literal filename.
    elements: RefCell<indexmap::IndexMap<String, Element>>,
    /// Whether the panel is attached to its caller's surface.
    connected: Cell<bool>,
    /// Requested collapsed property.
    collapsed: Rc<Cell<bool>>,
    /// Requested full-screen property.
    overlay: Cell<bool>,
    /// Owned optional notification.
    on_close: Callback,
    /// Stable native placement identity for this panel instance.
    #[cfg(target_arch = "wasm32")]
    native_key: String,
    /// Native application adapter initialized by page content.
    #[cfg(target_arch = "wasm32")]
    native: RefCell<Option<Rc<panel::State>>>,
    /// Native callbacks and sandbox address, independently replaceable.
    #[cfg(target_arch = "wasm32")]
    native_options: Rc<panel::Options>,
    /// Last owned repaint target.
    context: RefCell<Option<egui::Context>>,
    /// One-shot literal tab-scroll intent, not selected state.
    scroll: RefCell<Option<String>>,
}
impl ArtifactsPanel {
    /// Create a disconnected, expanded inline panel with no views.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self::with_host(Some(host))
    }
    /// Create panel storage independently of retained effects.
    fn with_host(host: Option<Rc<dyn AttachmentHost>>) -> Self {
        Self {
            host,
            id: crate::browser::attachment_content::identity(),
            elements: RefCell::default(),
            connected: Cell::new(false),
            collapsed: Rc::new(Cell::new(false)),
            overlay: Cell::new(false),
            on_close: Rc::default(),
            #[cfg(target_arch = "wasm32")]
            native: RefCell::default(),
            #[cfg(target_arch = "wasm32")]
            native_key: panel::new_key(),
            #[cfg(target_arch = "wasm32")]
            native_options: Rc::default(),
            context: RefCell::default(),
            scroll: RefCell::default(),
        }
    }
    /// Read the collapsed property.
    #[must_use]
    pub fn collapsed(&self) -> bool {
        self.collapsed.get()
    }
    /// Change the collapsed property and request repaint.
    pub fn set_collapsed(&self, collapsed: bool) {
        self.collapsed.set(collapsed);
        self.repaint();
    }
    /// Read the full-screen property.
    #[must_use]
    pub fn overlay(&self) -> bool {
        self.overlay.get()
    }
    /// Change the full-screen property and request repaint.
    pub fn set_overlay(&self, overlay: bool) {
        self.overlay.set(overlay);
        self.repaint();
    }
    /// Replace the optional close notification without changing panel state.
    pub fn set_on_close(&self, on_close: Option<Rc<dyn Fn()>>) {
        let previous = self.on_close.replace(on_close);
        drop(previous);
        self.repaint();
    }
    /// Wake the owned context outside its container borrow.
    fn repaint(&self) {
        let context = self.context.borrow().clone();
        #[cfg(target_arch = "wasm32")]
        if let Some(state) = self.native.borrow().clone() {
            state.refresh();
        }
        if let Some(context) = context {
            context.request_repaint();
        }
    }
    /// Capture owned handles before calling view or host operations.
    fn elements(&self) -> Vec<(String, Element)> {
        self.elements
            .borrow()
            .iter()
            .map(|(name, element)| (name.clone(), Rc::clone(element)))
            .collect()
    }
    /// Find a view without borrowing the map through view operations.
    fn element(&self, filename: &str) -> Option<Element> {
        self.elements.borrow().get(filename).cloned()
    }
    /// Create an installed view or update its original content in place.
    /// # Errors
    /// Returns factory, retained-visibility or incompatible view-borrow failures.
    pub fn get_or_create_artifact_element(
        &self,
        filename: &str,
        content: &str,
    ) -> Result<(), ArtifactUiError> {
        if let Some(element) = self.element(filename) {
            with_element(&element, |element| {
                element.set_content(content.into());
                Ok(())
            })?;
        } else {
            let mut element = create_artifact_element(
                filename,
                content,
                Rc::clone(retained::host(self.host.as_ref())?),
            )?;
            element.set_visibility(false, None)?;
            if self.connected.get() {
                element.connected_callback()?;
            }
            self.elements
                .borrow_mut()
                .insert(filename.into(), Rc::new(RefCell::new(Some(element))));
        }
        self.repaint();
        Ok(())
    }
    /// Apply literal view selection and request scrolling after the next draw.
    /// # Errors
    /// Returns retained-visibility or incompatible view-borrow failures.
    pub fn show_artifact(&self, filename: &str) -> Result<(), ArtifactUiError> {
        self.visibility(Some(filename), self.connected.get() && !self.collapsed())?;
        *self.scroll.borrow_mut() = Some(filename.into());
        self.repaint();
        Ok(())
    }
    /// Release only the named view; the application decides selection fallback.
    pub fn remove_artifact_element(&self, filename: &str) {
        let removed = self.elements.borrow_mut().shift_remove(filename);
        drop(removed);
        self.repaint();
    }
    /// Restore retained views, defaulting only absent or empty selection to the first key.
    /// # Errors
    /// Returns retained-visibility or incompatible view-borrow failures.
    pub fn connected_callback<P>(
        &self,
        caller: &maestro_app::browser::tools::artifacts::ArtifactsPanel<P>,
    ) -> Result<(), ArtifactUiError>
    where
        P: Clone + std::ops::Deref,
        P::Target: ArtifactPresentation,
    {
        if !self.connected.replace(true) {
            for (_, element) in self.elements() {
                with_element(&element, |element| element.connected_callback())?;
            }
        }
        if caller.active_filename().is_none_or(|name| name.is_empty()) {
            let first = caller
                .artifacts()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .keys()
                .next()
                .cloned();
            if let Some(first) = first {
                let scroll = self.scroll.borrow_mut().take();
                caller.show_artifact(&first);
                *self.scroll.borrow_mut() = scroll;
            }
        }
        self.visibility(caller.active_filename().as_deref(), !self.collapsed())?;
        self.repaint();
        Ok(())
    }
    /// Hide native content without disposing views or their roots.
    /// # Errors
    /// Returns retained-visibility or incompatible view-borrow failures.
    pub fn disconnected_callback(&self) -> Result<(), ArtifactUiError> {
        if self.connected.replace(false) {
            for (_, element) in self.elements() {
                with_element(&element, |element| element.disconnected_callback())?;
            }
        }
        self.visibility(None, false)?;
        self.repaint();
        Ok(())
    }
    /// Reconcile all retained policies before changing containing modal admission.
    fn visibility(&self, active: Option<&str>, shown: bool) -> Result<(), ArtifactUiError> {
        let modal = (shown && self.overlay()).then_some(self.id);
        for (name, element) in self.elements() {
            with_element(&element, |element| {
                element.set_visibility(shown && active == Some(name.as_str()), modal)
            })?;
        }
        retained::host(self.host.as_ref())?
            .set_overlay(self.id, modal.is_some())
            .map_err(ArtifactUiError::Host)
    }
}
impl Default for ArtifactsPanel {
    /// Empty host-free native panel.
    fn default() -> Self {
        Self::with_host(None)
    }
}
impl Drop for ArtifactsPanel {
    fn drop(&mut self) {
        self.elements.get_mut().clear();
        if let Some(host) = &self.host {
            let _ = host.set_overlay(self.id, false);
        }
    }
}
/// Construct the installed view without disguising recognized specialized files.
/// # Errors
/// Returns a presentation error when the selected specialized view is unavailable.
pub fn create_artifact_element(
    filename: &str,
    content: &str,
    host: Rc<dyn AttachmentHost>,
) -> Result<Box<dyn ArtifactElement>, ArtifactUiError> {
    let mut element: Box<dyn ArtifactElement> = match get_file_type(filename) {
        FileType::Markdown => Box::new(markdown_artifact::MarkdownArtifact::new(host)),
        FileType::Svg => Box::new(svg_artifact::SvgArtifact::new(host)),
        FileType::Text => Box::new(text_artifact::TextArtifact::new(host)),
        FileType::Image => Box::new(image_artifact::ImageArtifact::new(host)),
        FileType::Excel => Box::new(excel_artifact::ExcelArtifact::new(host)),
        FileType::Pdf => Box::new(pdf_artifact::PdfArtifact::new(host)),
        FileType::Generic => Box::new(generic_artifact::GenericArtifact::new(host)),
        _ => {
            return Err(ArtifactUiError::Presentation(
                "Artifact view is not installed".to_owned().into(),
            ));
        }
    };
    element.set_filename(filename.into());
    element.set_content(content.into());
    Ok(element)
}

/// Select the Unicode-lowercased last literal dot-delimited filename segment.
fn extension(filename: &str) -> String {
    filename
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod native;
mod retained;

#[cfg(target_arch = "wasm32")]
pub(crate) mod panel;
pub use maestro_app::browser::tools::artifacts::{Artifact, ArtifactsParams};

pub use artifacts_tool_renderer::ArtifactsToolRenderer;
#[cfg(target_arch = "wasm32")]
pub use html_artifact::HtmlArtifact;
pub use markdown_artifact::MarkdownArtifact;
pub use svg_artifact::SvgArtifact;
pub use text_artifact::TextArtifact;

#[cfg(target_arch = "wasm32")]
pub use native::NativeArtifact;
