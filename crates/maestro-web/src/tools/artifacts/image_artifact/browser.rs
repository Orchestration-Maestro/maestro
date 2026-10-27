//! Retained native image and weak load-failure subscription.
use super::Body;
use crate::browser::{
    attachment_content::{
        AttachmentHostError, ReadingRoot,
        browser::{append, element, host_error},
    },
    event_subscription::EventSubscription,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure};
/// Assign only differing attributes, keeping the failure source across frames.
pub(super) fn sync(body: &Body) -> Result<(), wasm_bindgen::JsValue> {
    if let Some(image) = &body.image {
        if body.svg && body.source.is_empty() {
            image.remove_attribute("src")?;
            image.set_attribute("hidden", "")?;
        } else {
            image.remove_attribute("hidden")?;
        }
        let source = if body.failed {
            &body.fallback
        } else {
            &body.source
        };
        if !(body.svg && source.is_empty()) && image.get_attribute("src").as_deref() != Some(source)
        {
            image.set_attribute("src", source)?;
        }
        if image.get_attribute("alt").as_deref() != Some(&body.alt) {
            image.set_attribute("alt", &body.alt)?;
        }
    }
    Ok(())
}
/// Mount a centred image with a listener owned by the reading root.
pub(super) fn mount(
    document: &web_sys::Document,
    body: &Rc<RefCell<Body>>,
) -> Result<ReadingRoot, AttachmentHostError> {
    let root = nodes(document, body)?;
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        element: root.node.unchecked_into(),
        slots: vec![],
        owner: root.owner,
        commit: None,
    })
}
/// Build the native image independently of a retained placement.
pub(super) fn nodes(
    document: &web_sys::Document,
    body: &Rc<RefCell<Body>>,
) -> Result<crate::dom::DomRoot, AttachmentHostError> {
    let root = element(document, "div", "maestro-image-artifact")?;
    if body.borrow().svg {
        root.set_class_name("maestro-image-artifact maestro-svg-artifact");
    }
    let style = element(document, "style", "")?;
    style.set_text_content(Some(include_str!("../image_artifact.css")));
    append(&root, &style)?;
    let image = element(document, "img", "")?;
    append(&root, &image)?;
    body.borrow_mut().image = Some(image.clone());
    let weak = Rc::downgrade(body);
    let callback = Closure::<dyn FnMut()>::new(move || {
        let Some(body) = weak.upgrade() else {
            return;
        };
        let mut body = body.borrow_mut();
        if body.failed {
            return;
        }
        body.failed = true;
        if let Err(error) = sync(&body) {
            wasm_bindgen::throw_val(error);
        }
    });
    let listener = if body.borrow().svg {
        None
    } else {
        Some(
            EventSubscription::new(image.unchecked_into(), "error", callback)
                .map_err(|error| host_error(&error))?,
        )
    };
    sync(&body.borrow()).map_err(|error| host_error(&error))?;
    Ok(crate::dom::DomRoot {
        node: root.into(),
        owner: listener.map(|listener| Box::new(listener) as Box<dyn std::any::Any>),
    })
}
