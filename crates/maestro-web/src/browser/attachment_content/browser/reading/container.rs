//! Positional native slot lifetimes in the existing reading registry.
use super::{AttachmentHostError, BrowserAttachmentHost, NativeReading, host_error};
use crate::browser::attachment_content::{AttachmentSurface, PlacementScope, ReadingSlotMount};
use std::rc::Rc;
use web_sys::Element;

impl BrowserAttachmentHost {
    /// Mount standalone surfaces under the body and contained frames under their slot.
    pub(crate) fn placement_parent(&self) -> Result<Element, AttachmentHostError> {
        self.containers
            .borrow()
            .last()
            .map(|(parent, _)| parent.clone())
            .map_or_else(
                || {
                    self.document
                        .body()
                        .map(Into::into)
                        .ok_or_else(|| AttachmentHostError::Host("Document has no body".into()))
                },
                Ok,
            )
    }
    /// Resolve the native parent once and restore nesting when its lease closes.
    pub(crate) fn contain(
        &self,
        id: egui::Id,
        slot: Option<egui::Id>,
    ) -> Result<PlacementScope, AttachmentHostError> {
        let surfaces = self.surfaces.borrow();
        let reading = surfaces
            .get(&id)
            .and_then(|s| s.reading.as_ref())
            .ok_or_else(|| AttachmentHostError::Host("Missing reading container".into()))?;
        let scale = self.transform(reading.pixels)?.scale;
        let parent = slot
            .map(|id| {
                reading
                    .slot_index
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| AttachmentHostError::Host("Missing reading slot".into()))
            })
            .transpose()?
            .unwrap_or_else(|| reading.root.element.clone());
        let depth = self.containers.borrow().len();
        self.containers.borrow_mut().push((parent, scale));
        let active = Rc::clone(&self.containers);
        Ok(PlacementScope::new(move || {
            active.borrow_mut().truncate(depth);
        }))
    }
    /// Invert both canvas axes into the retained native parent's local coordinates.
    pub(crate) fn contained_style(
        &self,
        surface: &AttachmentSurface<'_>,
    ) -> Result<Option<String>, AttachmentHostError> {
        let active = self.containers.borrow();
        let Some((parent, scale)) = active.last() else {
            return Ok(None);
        };
        let bounds = super::bounds(parent)?;
        let [left, top] = self
            .transform(surface.pixels_per_point)?
            .position(surface.rect.min);
        let height = if surface.rect.height().is_finite() {
            format!("{}px", surface.rect.height())
        } else {
            "auto".into()
        };
        Ok(Some(format!(
            "position:absolute;left:{}px;top:{}px;width:{}px;height:{};opacity:{};overflow:visible;transform-origin:0 0;transform:scale(1,1);clip-path:none;",
            (left - bounds.left()) / scale.0,
            (top - bounds.top()) / scale.1,
            surface.rect.width(),
            height,
            surface.opacity,
        )))
    }
    /// Release the registry borrow before invoking a caller's native factory.
    pub(crate) fn resize_slots(
        &self,
        id: egui::Id,
        count: usize,
        mount: &ReadingSlotMount<'_>,
    ) -> Result<(), AttachmentHostError> {
        let mut surface = self
            .surfaces
            .borrow_mut()
            .remove(&id)
            .ok_or_else(|| AttachmentHostError::Host("Missing reading container".into()))?;
        let result = surface
            .reading
            .as_mut()
            .ok_or_else(|| AttachmentHostError::Host("Missing reading container".into()))
            .and_then(|reading| reading.resize_slots(&self.document, count, mount));
        let displaced = self.surfaces.borrow_mut().insert(id, surface);
        drop(displaced);
        result
    }
}
impl NativeReading {
    /// Prepare every addition before changing the retained native tree.
    fn resize_slots(
        &mut self,
        document: &web_sys::Document,
        count: usize,
        mount: &ReadingSlotMount<'_>,
    ) -> Result<(), AttachmentHostError> {
        let additions = (self.root.slots.len()..count)
            .map(|index| mount(document, index))
            .collect::<Result<Vec<_>, _>>()?;
        let appended = additions
            .iter()
            .try_for_each(|(_, slot)| self.root.element.append_child(slot).map(|_| ()));
        if let Err(error) = appended {
            for (_, slot) in &additions {
                slot.remove();
            }
            return Err(host_error(&error));
        }
        for (id, slot) in self.root.slots.drain(count.min(self.root.slots.len())..) {
            self.slot_index.remove(&id);
            slot.remove();
        }
        for (id, slot) in additions {
            self.slot_index.insert(id, slot.clone());
            self.root.slots.push((id, slot));
        }
        Ok(())
    }
}
