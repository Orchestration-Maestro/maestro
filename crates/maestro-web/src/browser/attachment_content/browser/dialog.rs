//! Native dialog focus effects on the existing document owner.
use super::{BrowserAttachmentHost, host_error};
use crate::browser::attachment_content::{AttachmentHostError, FocusTarget};
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
impl BrowserAttachmentHost {
    /// Retain the native focus capability, including focusable non-HTML elements.
    pub(super) fn capture_native_focus(
        &self,
    ) -> Result<Option<Rc<FocusTarget>>, AttachmentHostError> {
        let Some(target) = self.document.active_element() else {
            return Ok(None);
        };
        let focus = js_sys::Reflect::get(&target, &"focus".into()).map_err(|e| host_error(&e))?;
        let Ok(focus) = focus.dyn_into::<js_sys::Function>() else {
            return Ok(None);
        };
        Ok(Some(Rc::new(move || {
            if target.is_connected() {
                focus.call0(&target).map_err(|e| host_error(&e))?;
            }
            Ok(())
        })))
    }
}
