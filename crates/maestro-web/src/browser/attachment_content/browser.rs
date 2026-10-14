//! Retained native image and selectable text elements above a supplied canvas.
use super::{
    AttachmentContent, AttachmentHost, AttachmentSurface, AttachmentUiError, ImageFit, TextView,
};
use crate::Attachment;
use maestro_theme::BrandPresentation;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::{cell::RefCell, rc::Rc};
use web_sys::{Document, Element, HtmlCanvasElement};
/// Browser objects scoped to the supplied canvas's document.
pub struct BrowserAttachmentHost {
    /// Canvas coordinate frame.
    canvas: HtmlCanvasElement,
    /// Owning document.
    document: Document,
    /// Independently retained content elements.
    surfaces: RefCell<HashMap<egui::Id, DomSurface>>,
    /// Open modal identities in mounting order.
    overlays: RefCell<Vec<egui::Id>>,
}
/// One retained content element with its unclipped layout.
struct DomSurface {
    /// Owned placement wrapper.
    wrapper: Element,
    /// Native image, preformatted text or document container.
    child: Element,
    /// Current element tag.
    tag: &'static str,
    /// Complete layout excluding modal visibility.
    style: String,
}
impl Drop for DomSurface {
    fn drop(&mut self) {
        self.wrapper.remove();
    }
}
/// Preserve the browser exception's category at the effect boundary.
pub(crate) fn host_error(error: &wasm_bindgen::JsValue) -> AttachmentUiError {
    AttachmentUiError::Host(format!("{error:?}"))
}
impl BrowserAttachmentHost {
    /// Retain the caller's canvas and its owning document.
    /// # Errors
    /// Returns an unavailable-document failure.
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, AttachmentUiError> {
        let document = canvas
            .owner_document()
            .ok_or_else(|| AttachmentUiError::Host("Canvas has no document".into()))?;
        Ok(Self {
            canvas,
            document,
            surfaces: RefCell::new(HashMap::new()),
            overlays: RefCell::new(Vec::new()),
        })
    }
    /// Create a detached element owner, publishing it only after setup.
    fn create(&self, tag: &'static str) -> Result<DomSurface, AttachmentUiError> {
        let wrapper = self
            .document
            .create_element("div")
            .map_err(|error| host_error(&error))?;
        let child = self
            .document
            .create_element(tag)
            .map_err(|error| host_error(&error))?;
        wrapper
            .append_child(&child)
            .map_err(|error| host_error(&error))?;
        Ok(DomSurface {
            wrapper,
            child,
            tag,
            style: String::new(),
        })
    }
    /// Map canvas points to the full viewport rectangle, clipping without resizing.
    fn placement(&self, s: &AttachmentSurface<'_>) -> Result<String, AttachmentUiError> {
        let bounds = self.canvas.get_bounding_client_rect();
        if self.canvas.width() == 0 || self.canvas.height() == 0 {
            return Err(AttachmentUiError::Host(
                "Canvas has no drawable size".into(),
            ));
        }
        let x = bounds.width() * f64::from(s.pixels_per_point) / f64::from(self.canvas.width());
        let y = bounds.height() * f64::from(s.pixels_per_point) / f64::from(self.canvas.height());
        let r = s.rect;
        let clip = super::placement::clip_path(s, (x, y));
        Ok(format!(
            "position:fixed;left:{}px;top:{}px;width:{}px;height:{}px;clip-path:{};opacity:{};",
            bounds.left() + f64::from(r.left()) * x,
            bounds.top() + f64::from(r.top()) * y,
            f64::from(r.width()) * x,
            f64::from(r.height()) * y,
            clip,
            s.opacity
        ))
    }
    /// Apply modal visibility without replacing retained image/text objects.
    fn visibility(
        &self,
        placed: Option<egui::Id>,
        previous: Option<egui::Id>,
    ) -> Result<(), AttachmentUiError> {
        let top = self.overlays.borrow().last().copied();
        let surfaces = self.surfaces.borrow();
        for id in super::style_targets(surfaces.keys().copied(), placed, previous, top) {
            let Some(s) = surfaces.get(&id) else { continue };
            let visibility = if top.is_none_or(|top| top == id) {
                "visible"
            } else {
                "hidden"
            };
            let z = if top == Some(id) { 51 } else { 1 };
            let style = format!("{}visibility:{visibility};z-index:{z};", s.style);
            s.wrapper
                .set_attribute("style", &style)
                .map_err(|error| host_error(&error))?;
        }
        Ok(())
    }
    /// Update copied DOM properties while retaining unchanged native content.
    fn content(
        element: &Element,
        content: &AttachmentContent<'_>,
    ) -> Result<(), AttachmentUiError> {
        let css=match content {
            AttachmentContent::Image{source,alt,fit}=>{
                if element.get_attribute("src").as_deref()!=Some(*source){element.set_attribute("src",source).map_err(|error|host_error(&error))?;}
                element.set_attribute("alt",alt).map_err(|error|host_error(&error))?;element.set_attribute("title",alt).map_err(|error|host_error(&error))?;
                format!("border-radius:var(--maestro-radii-panel);pointer-events:none;{}",if *fit==ImageFit::Cover{"object-fit:cover;"}else{"object-fit:contain;width:auto;height:auto;max-width:100%;max-height:100%;"})
            },
            AttachmentContent::Text{text,view}=>{
                if element.text_content().as_deref()!=Some(text){element.set_text_content(Some(text));}
                format!("white-space:pre-wrap;overflow:auto;margin:0;padding:var(--maestro-spacing-lg);font-family:var(--maestro-type-mono-font);font-size:{};line-height:var(--maestro-type-mono-line-height);background:var(--maestro-color-panel);color:var(--maestro-color-text);border:1px solid var(--maestro-color-border);",if *view==TextView::Extracted{"calc(var(--maestro-type-mono-size) * 6 / 7)"}else{"var(--maestro-type-mono-size)"})
            },
            AttachmentContent::Preview{..}=>"background:var(--maestro-color-panel);color:var(--maestro-color-text);overflow:auto;".into(),
        };
        element
            .set_attribute(
                "style",
                &format!(
                    "width:100%;height:100%;box-sizing:border-box;{css}{}",
                    super::content_border(content)
                ),
            )
            .map_err(|error| host_error(&error))
    }
}
impl AttachmentHost for BrowserAttachmentHost {
    fn present(
        &self,
        s: AttachmentSurface<'_>,
        p: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        let id = s.id;
        let tag = match &s.content {
            AttachmentContent::Image { .. } => "img",
            AttachmentContent::Text { .. } => "pre",
            AttachmentContent::Preview { .. } => "div",
        };
        let retained = self.surfaces.borrow_mut().remove(&s.id);
        let mut surface = retained.map_or_else(|| self.create(tag), Ok)?;
        let result: Result<(), AttachmentUiError> = (|| {
            if surface.tag != tag {
                let child = self
                    .document
                    .create_element(tag)
                    .map_err(|error| host_error(&error))?;
                surface
                    .wrapper
                    .replace_child(&child, &surface.child)
                    .map_err(|error| host_error(&error))?;
                surface.child = child;
                surface.tag = tag;
            }
            surface.style = self.placement(&s)?;
            for (key, value) in p.css_properties() {
                let _ = write!(surface.style, "{key}:{value};");
            }
            if tag == "img" {
                surface.style.push_str(
                    "pointer-events:none;display:flex;align-items:center;justify-content:center;",
                );
            }
            let presentation = matches!(
                &s.content,
                AttachmentContent::Text {
                    view: TextView::Presentation,
                    ..
                }
            );
            Self::content(&surface.child, &s.content).map_err(|error| match error {
                AttachmentUiError::Host(message) if presentation => {
                    AttachmentUiError::Presentation(message)
                }
                other => other,
            })?;
            let body = self
                .document
                .body()
                .ok_or_else(|| AttachmentUiError::Host("Document has no body".into()))?;
            if !surface.wrapper.is_connected() {
                body.append_child(&surface.wrapper)
                    .map_err(|error| host_error(&error))?;
            }
            Ok(())
        })();
        self.surfaces.borrow_mut().insert(id, surface);
        result?;
        self.visibility(Some(id), self.overlays.borrow().last().copied())
    }
    fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentUiError> {
        let previous = self.overlays.borrow().last().copied();
        {
            let mut overlays = self.overlays.borrow_mut();
            if !visible {
                overlays.retain(|i| *i != id);
            } else if !overlays.contains(&id) {
                overlays.push(id);
            }
        }
        self.visibility(None, previous)
    }
    fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
        self.document
            .default_view()
            .ok_or_else(|| AttachmentUiError::Host("Document has no window".into()))?
            .match_media("(hover: hover)")
            .map_err(|error| host_error(&error))?
            .map(|m| m.matches())
            .ok_or_else(|| AttachmentUiError::Host("Hover media query unavailable".into()))
    }
    fn remove(&self, id: egui::Id) {
        let removed = self.surfaces.borrow_mut().remove(&id);
        drop(removed);
    }
    fn download(&self, attachment: Rc<RefCell<Attachment>>) -> Result<(), AttachmentUiError> {
        crate::utils::download::download(&self.document, &attachment)
    }
}
