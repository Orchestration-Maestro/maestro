//! Reading transactions in the existing native surface registry.
use super::super::placement::Transform;
use super::super::{ReadingContent, ReadingLayout, ReadingPhase, ReadingPlacement, ReadingRoot};
use crate::browser::{
    attachment_content::{
        AttachmentContent, AttachmentSurface, AttachmentUiError,
        browser::{BrowserAttachmentHost, DomSurface, bounds, host_error},
    },
    event_subscription::EventSubscription,
};
use maestro_theme::BrandPresentation;
use std::{cell::RefCell, fmt::Write as _, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, ResizeObserver};
mod transaction;
use transaction::{Scroll, size_slots, transaction};
/// One root's registration lifetime and staged measurement operands.
pub(super) struct NativeReading {
    /// Stable feature identity.
    content: Rc<ReadingContent>,
    /// Whole feature-owned root.
    root: ReadingRoot,
    /// Current repaint owner, without a root backedge.
    context: Rc<RefCell<egui::Context>>,
    /// Native resize registration.
    observer: ResizeObserver,
    /// Native scrolling registration.
    _scroll: EventSubscription<Closure<dyn FnMut()>>,
    /// Hidden measurement placement, distinct from published placement.
    measurement: String,
    /// Current physical density for live canvas observation.
    pixels: f32,
    /// Owned dimensions staged for the next read and publication.
    sizes: Vec<(egui::Id, egui::Vec2)>,
    /// Clamped offsets observed during the last successful measurement.
    scroll: Option<Scroll>,
}
/// Preserve the native failure category when installing placement.
fn attr(element: &Element, value: &str) -> Result<(), AttachmentUiError> {
    element
        .set_attribute("style", value)
        .map_err(|e| host_error(&e))
}
impl NativeReading {
    /// Install repaint notifications with no strong backedge to this owner.
    fn new(
        root: ReadingRoot,
        content: Rc<ReadingContent>,
        wrapper: &Element,
        host: &BrowserAttachmentHost,
        repaint: &egui::Context,
    ) -> Result<Self, AttachmentUiError> {
        let repaint = Rc::new(RefCell::new(repaint.clone()));
        let target = Rc::clone(&repaint);
        let callback = Closure::wrap(Box::new(move || {
            let repaint = target.borrow().clone();
            repaint.request_repaint();
        }) as Box<dyn FnMut()>);
        let observer =
            ResizeObserver::new(callback.as_ref().unchecked_ref()).map_err(|e| host_error(&e))?;
        let scroll = EventSubscription::new(wrapper.clone().into(), "scroll", callback)
            .map_err(|e| host_error(&e))?;
        observer.observe(&root.element);
        observer.observe(&host.canvas);
        Ok(Self {
            content,
            root,
            context: repaint,
            observer,
            _scroll: scroll,
            measurement: String::new(),
            pixels: 1.,
            sizes: vec![],
            scroll: None,
        })
    }
    /// Retain call-scoped dimensions and bounded hidden placement for readback.
    fn stage(
        &mut self,
        s: &AttachmentSurface<'_>,
        style: String,
        placement: &ReadingPlacement<'_>,
    ) {
        self.measurement = style + "visibility:hidden;pointer-events:none;height:auto;";
        if s.rect.height().is_finite() {
            let _ = write!(self.measurement, "max-height:{}px;", s.rect.height());
        }
        self.sizes = placement.sizes.to_vec();
        self.pixels = s.pixels_per_point;
        *self.context.borrow_mut() = placement.context.clone();
    }
    /// Apply final geometry with the same rollback boundary used by measurement.
    fn publish(&self, wrapper: &Element, style: &str) -> Result<(), AttachmentUiError> {
        transaction(wrapper, &self.root.slots, ReadingPhase::Publish, |_| {
            size_slots(&self.root.slots, &self.sizes)?;
            attr(wrapper, style)?;
            if let Some(scroll) = &self.scroll {
                scroll.apply(wrapper)?;
            }
            Ok(())
        })
    }
    /// Read a complete slot snapshot through the existing inverse projection.
    fn controls(
        &self,
        wrapper: &Element,
        transform: &Transform,
    ) -> Result<Vec<(egui::Id, egui::Rect)>, AttachmentUiError> {
        let origin = bounds(wrapper)?;
        self.root
            .slots
            .iter()
            .map(|(id, element)| {
                let rect = bounds(element)?;
                Ok((
                    *id,
                    transform.relative(
                        [rect.x(), rect.y(), rect.width(), rect.height()],
                        [origin.x(), origin.y()],
                    ),
                ))
            })
            .collect()
    }
    /// Restore native state before committing only the successful staged offsets.
    fn measure(
        &mut self,
        wrapper: &Element,
        transform: &Transform,
    ) -> Result<ReadingLayout, AttachmentUiError> {
        let (layout, scroll) =
            transaction(wrapper, &self.root.slots, ReadingPhase::Measure, |saved| {
                size_slots(&self.root.slots, &self.sizes)?;
                attr(wrapper, &self.measurement)?;
                saved.apply(wrapper)?;
                let layout = ReadingLayout {
                    height: transform.height(bounds(&self.root.element)?.height()),
                    controls: self.controls(wrapper, transform)?,
                };
                Ok((layout, Scroll::read(wrapper)?))
            })?;
        self.scroll = Some(scroll);
        Ok(layout)
    }
}
impl Drop for NativeReading {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}
impl BrowserAttachmentHost {
    /// Project the entire native viewport once, including its typography.
    fn reading_style(
        &self,
        s: &AttachmentSurface<'_>,
        p: &BrandPresentation<'_>,
    ) -> Result<String, AttachmentUiError> {
        let transform = self.transform(s.pixels_per_point)?;
        let [left, top] = transform.position(s.rect.min);
        let mut style = format!(
            "position:fixed;left:{left}px;top:{top}px;width:{}px;height:{};overflow:auto;transform-origin:0 0;transform:scale({},{});clip-path:{};opacity:{};",
            s.rect.width(),
            if s.rect.height().is_finite() {
                s.rect.height().to_string() + "px"
            } else {
                "auto".into()
            },
            transform.scale.0,
            transform.scale.1,
            super::super::placement::clip_path(s, (1., 1.)),
            s.opacity
        );
        for (key, value) in p.css_properties() {
            let _ = write!(style, "{key}:{value};");
        }
        Ok(style)
    }
    /// Mount a hidden candidate with its complete registration lifetime.
    fn create_reading(
        &self,
        placement: &ReadingPlacement<'_>,
    ) -> Result<DomSurface, AttachmentUiError> {
        let mut candidate = self.create("div")?;
        let root = (placement.content.mount)(&self.document)?;
        candidate
            .wrapper
            .replace_child(&root.element, &candidate.child)
            .map_err(|e| host_error(&e))?;
        candidate.child = root.element.clone();
        candidate.reading = Some(NativeReading::new(
            root,
            Rc::clone(&placement.content),
            &candidate.wrapper,
            self,
            placement.context,
        )?);
        attr(
            &candidate.wrapper,
            "position:fixed;visibility:hidden;pointer-events:none;",
        )?;
        self.document
            .body()
            .ok_or_else(|| AttachmentUiError::Host("Document has no body".into()))?
            .append_child(&candidate.wrapper)
            .map_err(|e| host_error(&e))?;
        Ok(candidate)
    }
    /// Run one explicit phase without publishing a partial replacement.
    pub(super) fn present_reading(
        &self,
        s: &AttachmentSurface<'_>,
        p: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        let retained = self.surfaces.borrow_mut().remove(&s.id);
        let mut surface = retained.map_or_else(|| self.create("div"), Ok)?;
        let result = self.update_reading(s, p, &mut surface);
        if result.is_err() {
            drop(surface.pending.take());
        }
        let displaced = self.surfaces.borrow_mut().insert(s.id, surface);
        drop(displaced);
        result
    }
    /// Update the reached native owner without committing partial placement.
    fn update_reading(
        &self,
        s: &AttachmentSurface<'_>,
        p: &BrandPresentation<'_>,
        surface: &mut DomSurface,
    ) -> Result<(), AttachmentUiError> {
        let AttachmentContent::Reading(placement) = &s.content else {
            return Ok(());
        };

        let mut style = self.reading_style(s, p)?;
        let same = surface
            .reading
            .as_ref()
            .is_some_and(|r| Rc::ptr_eq(&r.content, &placement.content));
        if placement.phase == ReadingPhase::Measure && !same {
            surface.pending = Some(Box::new(self.create_reading(placement)?));
        }
        let target = match surface.pending.as_deref_mut() {
            Some(pending) => pending,
            None => &mut *surface,
        };
        let reading = target
            .reading
            .as_mut()
            .filter(|r| Rc::ptr_eq(&r.content, &placement.content))
            .ok_or_else(|| AttachmentUiError::Host("Reading content was not measured".into()))?;
        if placement.phase == ReadingPhase::Measure {
            reading.stage(s, style, placement);
            return Ok(());
        }
        let top = self.overlays.borrow().last().copied();
        style.push_str(super::super::visibility_style(s.id, placement.modal, top));
        reading.publish(&target.wrapper, &style)?;
        target.style = style;
        target.modal = placement.modal;
        if let Some(candidate) = surface.pending.take() {
            *surface = *candidate;
        }
        Ok(())
    }
    /// Read natural height now, restoring placement and native scroll afterwards.
    pub(super) fn measure_reading(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentUiError> {
        let mut surface = self
            .surfaces
            .borrow_mut()
            .remove(&id)
            .ok_or_else(|| AttachmentUiError::Host("Missing reading surface".into()))?;
        let result = (|| {
            let target = match surface.pending.as_deref_mut() {
                Some(pending) => pending,
                None => &mut surface,
            };
            let r = target
                .reading
                .as_mut()
                .ok_or_else(|| AttachmentUiError::Host("Surface is not reading content".into()))?;
            let transform = self.transform(r.pixels)?;
            r.measure(&target.wrapper, &transform)
        })();
        if result.is_err() {
            drop(surface.pending.take());
        }
        let displaced = self.surfaces.borrow_mut().insert(id, surface);
        drop(displaced);
        result
    }
}
