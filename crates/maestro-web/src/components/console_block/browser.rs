//! Native console nodes using the existing reading transaction and control slots.
use super::Prepared;
use crate::browser::attachment_content::ReadingContent;
#[cfg(target_arch = "wasm32")]
use crate::browser::attachment_content::{
    AttachmentHostError, ReadingRoot,
    browser::{append, element},
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

/// Mounted native console with its local scroller and canvas-control slot.
#[cfg(target_arch = "wasm32")]
pub(super) struct Mounted {
    /// Literal output body.
    pre: web_sys::Element,
    /// Localized header label.
    label: web_sys::Element,
    /// Feature-owned scrolling body, distinct from the host viewport.
    scroll: web_sys::Element,
    /// Empty canvas-control slot outside the scrolling body.
    slot: web_sys::Element,
}
/// Native console mounting is unavailable off the browser.
#[cfg(not(target_arch = "wasm32"))]
pub(super) enum Mounted {}
#[cfg(target_arch = "wasm32")]
impl Mounted {
    /// Bring the console's own updated scroller to its bottom.
    pub(super) fn scroll_bottom(&self) {
        self.scroll.set_scroll_top(self.scroll.scroll_height());
    }
    /// Reveal the native control after keyboard focus.
    pub(super) fn reveal(&self) {
        self.slot.scroll_into_view();
    }
    /// Reconcile literal nodes without replacing the published root.
    fn update(&self, next: &Prepared) {
        use crate::browser::attachment_content::browser::set_text_if_changed;
        set_text_if_changed(&self.pre, &next.content);
        set_text_if_changed(&self.label, &next.label);
        self.pre
            .set_class_name(if next.variant == super::ConsoleVariant::Error {
                "maestro-console-error"
            } else {
                ""
            });
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Mounted {
    /// Unreachable without a native mount.
    pub(super) fn scroll_bottom(&self) {
        match *self {}
    }
    /// Unreachable without a native control slot.
    pub(super) fn reveal(&self) {
        match *self {}
    }
}
/// Build an owned factory with only a weak route to its mounted owner.
pub(super) fn content(
    prepared: &Rc<Prepared>,
    target: &Rc<RefCell<Weak<Mounted>>>,
    id: egui::Id,
) -> Rc<ReadingContent> {
    #[cfg(target_arch = "wasm32")]
    {
        let (prepared, target) = (Rc::clone(prepared), Rc::clone(target));
        Rc::new(ReadingContent {
            mount: Rc::new(move |document| mount(document, &prepared, &target, id)),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (prepared, target, id);
        Rc::new(ReadingContent {})
    }
}
/// Mount literal output, header and its empty canvas-control slot.
#[cfg(target_arch = "wasm32")]
fn mount(
    document: &web_sys::Document,
    prepared: &Rc<Prepared>,
    target: &Rc<RefCell<Weak<Mounted>>>,
    id: egui::Id,
) -> Result<ReadingRoot, AttachmentHostError> {
    let root = element(document, "div", "maestro-console")?;
    let style = element(document, "style", "")?;
    style.set_text_content(Some(include_str!("console.css")));
    append(&root, &style)?;
    let header = element(document, "div", "maestro-console-header")?;
    let label = element(document, "span", "maestro-console-label")?;
    let slot = element(document, "span", "control-slot")?;
    append(&header, &label)?;
    append(&header, &slot)?;
    let scroll = element(document, "div", "console-scroll")?;
    let pre = element(document, "pre", "")?;
    append(&scroll, &pre)?;
    append(&root, &header)?;
    append(&root, &scroll)?;
    let mounted = Rc::new(Mounted {
        pre,
        label,
        scroll,
        slot: slot.clone(),
    });
    mounted.update(prepared);
    *target.borrow_mut() = Rc::downgrade(&mounted);
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        element: root,
        slots: vec![(id, slot)],
        owner: Some(Box::new(mounted)),
        commit: Some(Box::new({
            let (prepared, target) = (Rc::clone(prepared), Rc::clone(target));
            move |previous| {
                if let Some(mounted) = previous
                    .owner
                    .as_ref()
                    .and_then(|owner| owner.downcast_ref::<Rc<Mounted>>())
                {
                    mounted.update(&prepared);
                    *target.borrow_mut() = Rc::downgrade(mounted);
                }
            }
        })),
    })
}
