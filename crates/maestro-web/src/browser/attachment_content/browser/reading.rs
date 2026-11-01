//! Reading transactions in the existing native surface registry.
use super::super::placement::Transform;
use super::super::{ReadingContent, ReadingLayout, ReadingPhase, ReadingPlacement, ReadingRoot};
use crate::browser::{
    attachment_content::{
        AttachmentContent, AttachmentHostError, AttachmentSurface,
        browser::{BrowserAttachmentHost, DomSurface, bounds, host_error},
    },
    event_subscription::EventSubscription,
};
use maestro_theme::BrandPresentation;
use std::{cell::RefCell, fmt::Write as _, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, ResizeObserver};
mod transaction;
use transaction::{Scroll, decoration_border, size_slots, transaction};
/// Retained root/viewport lifetime with staged frame and viewport placements.
pub(super) struct NativeReading {
    /// Stable feature identity.
    content: Rc<ReadingContent>,
    /// Whole feature-owned root.
    root: ReadingRoot,
    /// Host-created bounded scroll owner beneath the placement frame.
    viewport: Element,
    /// Staged viewport geometry for publication.
    viewport_style: String,
    /// Hidden auto-height viewport geometry for readback.
    viewport_measurement: String,
    /// Current repaint owner, without a root backedge.
    context: Rc<RefCell<egui::Context>>,
    /// Native resize registration.
    observer: ResizeObserver,
    /// Native scrolling registration.
    _scroll: EventSubscription<Closure<dyn FnMut()>>,
    /// Hidden frame measurement placement, distinct from published placement.
    measurement: String,
    /// Current physical density for live canvas observation.
    pixels: f32,
    /// Owned dimensions staged for the next read and publication.
    sizes: Vec<(egui::Id, egui::Vec2)>,
    /// Viewport offsets clamped during the last successful measurement.
    scroll: Option<Scroll>,
}
/// Preserve the native failure category when installing placement.
fn attr(element: &Element, value: &str) -> Result<(), AttachmentHostError> {
    element
        .set_attribute("style", value)
        .map_err(|e| host_error(&e))
}
impl NativeReading {
    /// Install repaint notifications with no strong backedge to this owner.
    fn new(
        root: ReadingRoot,
        content: Rc<ReadingContent>,
        viewport: Element,
        host: &BrowserAttachmentHost,
        repaint: &egui::Context,
    ) -> Result<Self, AttachmentHostError> {
        let repaint = Rc::new(RefCell::new(repaint.clone()));
        let target = Rc::clone(&repaint);
        let callback = Closure::wrap(Box::new(move || {
            let repaint = target.borrow().clone();
            repaint.request_repaint();
        }) as Box<dyn FnMut()>);
        let observer =
            ResizeObserver::new(callback.as_ref().unchecked_ref()).map_err(|e| host_error(&e))?;
        let scroll = EventSubscription::new(viewport.clone().into(), "scroll", callback)
            .map_err(|e| host_error(&e))?;
        observer.observe(&root.element);
        observer.observe(&host.canvas);
        Ok(Self {
            content,
            root,
            viewport,
            viewport_style: String::new(),
            viewport_measurement: String::new(),
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
        self.viewport_style =
            "width:100%;height:100%;box-sizing:border-box;overflow:auto;clip-path:inset(0);".into();
        self.viewport_measurement = self.viewport_style.clone() + "height:auto;";
        self.measurement = style + "visibility:hidden;pointer-events:none;height:auto;";
        if placement.intrinsic_width {
            let _ = write!(
                self.measurement,
                "width:fit-content;max-width:{}px;",
                s.rect.width()
            );
        }
        if s.rect.height().is_finite() {
            let _ = write!(self.measurement, "max-height:{}px;", s.rect.height());
            let _ = write!(
                self.viewport_measurement,
                "max-height:{}px;",
                s.rect.height()
            );
        }
        self.sizes = placement.sizes.to_vec();
        self.pixels = s.pixels_per_point;
        *self.context.borrow_mut() = placement.context.clone();
    }
    /// Apply final geometry with the same rollback boundary used by measurement.
    fn publish(&self, frame: &Element, style: &str) -> Result<(), AttachmentHostError> {
        transaction(
            (frame, &self.viewport),
            &self.root.slots,
            ReadingPhase::Publish,
            |_| {
                size_slots(&self.root.slots, &self.sizes)?;
                attr(frame, style)?;
                attr(&self.viewport, &self.viewport_style)?;
                if let Some(scroll) = &self.scroll {
                    scroll.apply(&self.viewport)?;
                }
                Ok(())
            },
        )
    }
    /// Read a complete slot snapshot through the existing inverse projection.
    fn controls(
        &self,
        frame: &Element,
        transform: &Transform,
    ) -> Result<Vec<(egui::Id, egui::Rect)>, AttachmentHostError> {
        let origin = bounds(frame)?;
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
        frame: &Element,
        transform: &Transform,
        retained: Option<&Scroll>,
    ) -> Result<ReadingLayout, AttachmentHostError> {
        let (layout, scroll) = transaction(
            (frame, &self.viewport),
            &self.root.slots,
            ReadingPhase::Measure,
            |saved| {
                size_slots(&self.root.slots, &self.sizes)?;
                attr(frame, &self.measurement)?;
                attr(&self.viewport, &self.viewport_measurement)?;
                retained.unwrap_or(saved).apply(&self.viewport)?;
                let layout = ReadingLayout {
                    border: decoration_border(&self.root)?,
                    width: transform
                        .relative([0., 0., bounds(frame)?.width(), 0.], [0., 0.])
                        .width(),
                    height: transform.height(bounds(&self.root.element)?.height()),
                    controls: self.controls(frame, transform)?,
                };
                Ok((layout, Scroll::read(&self.viewport)?))
            },
        )?;
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
    /// Project the fixed frame once, including inherited typography and external clip.
    fn reading_style(
        &self,
        s: &AttachmentSurface<'_>,
        p: &BrandPresentation<'_>,
    ) -> Result<String, AttachmentHostError> {
        let transform = self.transform(s.pixels_per_point)?;
        let [left, top] = transform.position(s.rect.min);
        let mut style = format!(
            "position:fixed;left:{left}px;top:{top}px;width:{}px;height:{};overflow:visible;transform-origin:0 0;transform:scale({},{});clip-path:{};opacity:{};",
            s.rect.width(),
            if s.rect.height().is_finite() {
                s.rect.height().to_string() + "px"
            } else {
                "auto".into()
            },
            transform.scale.0,
            transform.scale.1,
            super::super::placement::frame_clip_path(
                s,
                egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(
                        super::super::points(
                            f64::from(self.canvas.width()) / f64::from(s.pixels_per_point)
                        ),
                        super::super::points(
                            f64::from(self.canvas.height()) / f64::from(s.pixels_per_point)
                        ),
                    ),
                )
            ),
            s.opacity
        );
        crate::browser::append_brand_properties(&mut style, p);
        Ok(style)
    }
    /// Mount a hidden candidate with its complete registration lifetime.
    fn create_reading(
        &self,
        placement: &ReadingPlacement<'_>,
        style: &str,
    ) -> Result<DomSurface, AttachmentHostError> {
        let mut candidate = self.create("div")?;
        let root = (placement.content.mount)(&self.document)?;
        candidate
            .wrapper
            .set_attribute("class", "maestro-reading-frame")
            .map_err(|e| host_error(&e))?;
        candidate
            .child
            .set_attribute("class", "maestro-reading-viewport")
            .map_err(|e| host_error(&e))?;
        if let Some(decoration) = &root.decoration {
            candidate
                .wrapper
                .insert_before(decoration, Some(&candidate.child))
                .map_err(|e| host_error(&e))?;
        }
        candidate
            .child
            .append_child(&root.element)
            .map_err(|e| host_error(&e))?;
        candidate.reading = Some(NativeReading::new(
            root,
            Rc::clone(&placement.content),
            candidate.child.clone(),
            self,
            placement.context,
        )?);
        attr(
            &candidate.wrapper,
            &(style.to_owned() + "visibility:hidden;pointer-events:none;"),
        )?;
        self.document
            .body()
            .ok_or_else(|| AttachmentHostError::Host("Document has no body".into()))?
            .append_child(&candidate.wrapper)
            .map_err(|e| host_error(&e))?;
        Ok(candidate)
    }
    /// Run one explicit phase without publishing a partial replacement.
    pub(super) fn present_reading(
        &self,
        s: &AttachmentSurface<'_>,
        p: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError> {
        let retained = self.surfaces.borrow_mut().remove(&s.id);
        let mut surface = retained.map_or_else(|| self.create("div"), Ok)?;
        let result = self.update_reading(s, p, &mut surface);
        if result.is_ok()
            && matches!(&s.content, AttachmentContent::Reading(placement) if placement.phase == ReadingPhase::Publish)
        {
            surface.ancestors.clone_from(&self.scopes.borrow());
        }
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
    ) -> Result<(), AttachmentHostError> {
        let AttachmentContent::Reading(placement) = &s.content else {
            return Ok(());
        };

        let style = self.reading_style(s, p)?;
        let retained = surface.retained;
        let same = surface
            .reading
            .as_ref()
            .is_some_and(|r| Rc::ptr_eq(&r.content, &placement.content));
        if placement.phase == ReadingPhase::Measure {
            surface.pending = if same {
                None
            } else {
                Some(Box::new(self.create_reading(placement, &style)?))
            };
        }
        let target = match surface.pending.as_deref_mut() {
            Some(pending) => pending,
            None => &mut *surface,
        };
        let reading = target
            .reading
            .as_mut()
            .filter(|r| Rc::ptr_eq(&r.content, &placement.content))
            .ok_or_else(|| AttachmentHostError::Host("Reading content was not measured".into()))?;
        if placement.phase == ReadingPhase::Measure {
            reading.stage(s, style, placement);
            return Ok(());
        }
        let style = style + reading.root.frame_style;
        let top = self.overlays.borrow().last().copied();
        let previous = std::mem::replace(&mut target.style, style);
        let previous_modal = target.modal;
        target.modal = placement.modal;
        target.retained = retained;
        let projected = target.projected_style(s.id, top);
        let result = target
            .reading
            .as_ref()
            .ok_or_else(|| AttachmentHostError::Host("Missing reading owner".into()))?
            .publish(&target.wrapper, &projected);
        if let Err(error) = result {
            target.style = previous;
            target.modal = previous_modal;
            return Err(error);
        }
        publish_pending(surface, &projected)
    }
    /// Read natural height now, restoring placement and native scroll afterwards.
    pub(super) fn measure_reading(
        &self,
        id: egui::Id,
    ) -> Result<ReadingLayout, AttachmentHostError> {
        let mut surface = self
            .surfaces
            .borrow_mut()
            .remove(&id)
            .ok_or_else(|| AttachmentHostError::Host("Missing reading surface".into()))?;
        let result = (|| {
            let retained = if surface.pending.as_ref().is_some_and(|candidate| {
                candidate
                    .reading
                    .as_ref()
                    .is_some_and(|r| r.root.commit.is_some())
            }) {
                surface
                    .reading
                    .as_ref()
                    .map(|r| Scroll::read(&r.viewport))
                    .transpose()?
            } else {
                None
            };
            let target = match surface.pending.as_deref_mut() {
                Some(pending) => pending,
                None => &mut surface,
            };
            let r = target.reading.as_mut().ok_or_else(|| {
                AttachmentHostError::Host("Surface is not reading content".into())
            })?;
            let transform = self.transform(r.pixels)?;
            r.measure(&target.wrapper, &transform, retained.as_ref())
        })();
        if result.is_err() {
            drop(surface.pending.take());
        }
        let displaced = self.surfaces.borrow_mut().insert(id, surface);
        drop(displaced);
        result
    }
}

impl BrowserAttachmentHost {}
/// Publish staged geometry with a fallible feature commit into retained native nodes.
fn reconcile(
    surface: &mut DomSurface,
    candidate: &DomSurface,
    projected: &str,
) -> Result<bool, AttachmentHostError> {
    let Some((previous, next)) = surface.reading.as_mut().zip(candidate.reading.as_ref()) else {
        return Ok(false);
    };
    let Some(commit) = &next.root.commit else {
        return Ok(false);
    };
    transaction(
        (&surface.wrapper, &previous.viewport),
        &previous.root.slots,
        ReadingPhase::Publish,
        |_| {
            size_slots(&previous.root.slots, &next.sizes)?;
            attr(&surface.wrapper, projected)?;
            attr(&previous.viewport, &next.viewport_style)?;
            commit(&previous.root)
        },
    )?;
    if let Some(scroll) = &next.scroll {
        scroll.commit(&previous.viewport);
    }
    previous.content = Rc::clone(&next.content);
    surface.style.clone_from(&candidate.style);
    surface.modal = candidate.modal;
    surface.retained = candidate.retained;
    Ok(true)
}

/// Release staged callbacks after committing or replacing the published root.
fn publish_pending(surface: &mut DomSurface, projected: &str) -> Result<(), AttachmentHostError> {
    if let Some(candidate) = surface.pending.take()
        && !reconcile(surface, &candidate, projected)?
    {
        *surface = *candidate;
    }
    if let Some(reading) = surface.reading.as_mut() {
        reading.root.commit = None;
    }
    Ok(())
}
