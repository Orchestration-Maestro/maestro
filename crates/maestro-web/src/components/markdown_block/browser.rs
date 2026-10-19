//! Native Markdown nodes, code listing owners and their empty canvas-control slots.
use super::Document;
#[cfg(target_arch = "wasm32")]
use crate::browser::attachment_content::AttachmentHostError;
use crate::browser::attachment_content::ReadingContent;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

#[cfg(target_arch = "wasm32")]
use crate::{
    browser::attachment_content::{
        ReadingRoot,
        browser::{append, element, host_error},
    },
    components::code_block::browser as code,
};
#[cfg(target_arch = "wasm32")]
use web_sys::Element;

/// Mounted document root, retained while the surface lives.
#[cfg(target_arch = "wasm32")]
pub(crate) struct Mounted {
    /// Document node carrying the presentation class.
    root: Element,
    /// Listing owners, kept for the root's lifetime.
    listings: Vec<code::Mounted>,
}
/// Off the browser no document is ever mounted, so no owner exists.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum Mounted {}

#[cfg(target_arch = "wasm32")]
impl Mounted {
    /// Bring the focused listing control's slot into the visible native viewport.
    pub(crate) fn reveal(&self, listing: usize) {
        if let Some(listing) = self.listings.get(listing) {
            listing.reveal();
        }
    }
    /// Switch reasoning presentation without replacing content.
    pub(crate) fn set_thinking(&self, thinking: bool) {
        self.root.set_class_name(if thinking {
            "maestro-markdown maestro-thinking"
        } else {
            "maestro-markdown"
        });
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Mounted {
    /// Unreachable: there is no native slot to reveal.
    pub(crate) fn reveal(&self, _: usize) {
        match *self {}
    }
}

/// Whether publication replaces content or reconciles only its presentation.
#[derive(Clone, Copy)]
pub(super) enum Update {
    /// Replace the complete document.
    Replace,
    /// Retain native text, focus and control owners.
    Presentation,
}

/// Native factory owning the prepared document, never mounted state.
pub(super) fn content(
    document: &Rc<Document>,
    ids: Vec<egui::Id>,
    (thinking, padded, slot): (bool, bool, &Rc<RefCell<Weak<Mounted>>>),
    update: Update,
) -> Rc<ReadingContent> {
    #[cfg(target_arch = "wasm32")]
    {
        let (document, slot) = (Rc::clone(document), Rc::clone(slot));
        Rc::new(ReadingContent {
            mount: Rc::new(move |dom| {
                mount(dom, &document, &ids, (thinking, padded, &slot), update)
            }),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (document, ids, thinking, padded, slot, update);
        Rc::new(ReadingContent {})
    }
}
/// Mount the document, replacing each listing slot with a native listing.
#[cfg(target_arch = "wasm32")]
fn mount(
    dom: &web_sys::Document,
    document: &Document,
    ids: &[egui::Id],
    (thinking, padded, owner): (bool, bool, &Rc<RefCell<Weak<Mounted>>>),
    update: Update,
) -> Result<ReadingRoot, AttachmentHostError> {
    crate::browser::math_assets::install(dom)?;
    let flow = element(
        dom,
        "div",
        if padded {
            "maestro-markdown-root maestro-artifact-padding"
        } else {
            "maestro-markdown-root"
        },
    )?;
    append(&flow, &styles(dom)?)?;
    let root = element(dom, "div", "maestro-markdown")?;
    root.set_inner_html(&document.html);
    append(&flow, &root)?;
    let (mut slots, mut listings) = (Vec::new(), Vec::new());
    for (index, (code_block, id)) in document.code.iter().zip(ids).enumerate() {
        let built = code::build(dom, code_block)?;
        let placeholder = root
            .query_selector(&format!("[{}=\"{index}\"]", document.slot_attribute))
            .map_err(|e| host_error(&e))?
            .ok_or_else(|| AttachmentHostError::Host("Missing code slot".into()))?;
        placeholder
            .set_attribute("class", "maestro-code-wrap")
            .map_err(|e| host_error(&e))?;
        append(&placeholder, &built.root)?;
        slots.push((*id, built.slot));
        listings.push(built.mounted);
    }
    let mounted = Rc::new(Mounted { root, listings });
    mounted.set_thinking(thinking);
    *owner.borrow_mut() = Rc::downgrade(&mounted);
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        element: flow,
        slots,
        commit: match update {
            Update::Replace => None,
            Update::Presentation => Some(Box::new({
                let owner = Rc::clone(owner);
                move |previous| {
                    if let Some(mounted) = previous
                        .owner
                        .as_ref()
                        .and_then(|owner| owner.downcast_ref::<Rc<Mounted>>())
                    {
                        mounted.set_thinking(thinking);
                        *owner.borrow_mut() = Rc::downgrade(mounted);
                    }
                    Ok(())
                }
            })),
        },
        owner: Some(Box::new(mounted)),
    })
}

/// Combine Markdown presentation with the shared native listing stylesheet.
#[cfg(target_arch = "wasm32")]
fn styles(dom: &web_sys::Document) -> Result<Element, AttachmentHostError> {
    let style = code::styles(dom)?;
    style.set_text_content(Some(&format!(
        "{}{}",
        style.text_content().unwrap_or_default(),
        include_str!("markdown.css")
    )));
    Ok(style)
}
