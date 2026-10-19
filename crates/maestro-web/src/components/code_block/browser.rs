//! Native code listing nodes and the empty canvas-control slot.
use super::Prepared;
#[cfg(target_arch = "wasm32")]
use crate::browser::attachment_content::AttachmentHostError;
use crate::browser::attachment_content::ReadingContent;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

#[cfg(target_arch = "wasm32")]
use crate::browser::attachment_content::{
    ReadingRoot,
    browser::{append, element, host_error},
};
#[cfg(target_arch = "wasm32")]
use web_sys::{Document, Element};

/// Mounted label and body of one listing.
#[cfg(target_arch = "wasm32")]
pub(crate) struct Mounted {
    /// Language label node.
    label: Element,
    /// Highlighted code node.
    code: Element,
    /// Empty canvas-control slot in the header.
    slot: Element,
    /// Content currently shown.
    shown: RefCell<(String, String)>,
}
/// Off the browser no listing is ever mounted, so no owner exists.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum Mounted {}

#[cfg(target_arch = "wasm32")]
impl Mounted {
    /// Bring the focused control's slot into the visible native viewport.
    pub(crate) fn reveal(&self) {
        self.slot.scroll_into_view();
    }
    /// Change only the label and body that differ, retaining native text selection.
    pub(crate) fn update(&self, next: &Prepared) {
        let mut shown = self.shown.borrow_mut();
        if shown.0 != next.label() {
            self.label.set_text_content(Some(next.label()));
            self.code
                .set_class_name(&format!("hljs language-{}", next.label()));
            next.label().clone_into(&mut shown.0);
        }
        if shown.1 != next.html {
            self.code.set_inner_html(&next.html);
            shown.1.clone_from(&next.html);
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Mounted {
    /// Unreachable: there is no native slot to reveal.
    pub(crate) fn reveal(&self) {
        match *self {}
    }
}

/// A listing built into the document, with its copy slot.
#[cfg(target_arch = "wasm32")]
pub(crate) struct Built {
    /// Bordered listing.
    pub(crate) root: Element,
    /// Empty canvas-control slot in the header.
    pub(crate) slot: Element,
    /// Updatable nodes.
    pub(crate) mounted: Mounted,
}
/// Build one listing: header outside the local scroller, body inside it.
#[cfg(target_arch = "wasm32")]
pub(crate) fn build(
    document: &Document,
    prepared: &Prepared,
) -> Result<Built, AttachmentHostError> {
    let root = element(document, "div", "maestro-code")?;
    let header = element(document, "div", "maestro-code-header")?;
    let label = element(document, "span", "maestro-code-label")?;
    label.set_text_content(Some(prepared.label()));
    let slot = element(document, "span", "control-slot")?;
    append(&header, &label)?;
    append(&header, &slot)?;
    let scroll = element(document, "div", "maestro-code-scroll")?;
    let pre = element(document, "pre", "")?;
    let code = element(
        document,
        "code",
        &format!("hljs language-{}", prepared.label()),
    )?;
    code.set_inner_html(&prepared.html);
    append(&pre, &code)?;
    append(&scroll, &pre)?;
    append(&root, &header)?;
    append(&root, &scroll)?;
    let shown = RefCell::new((prepared.label().to_owned(), prepared.html.clone()));
    Ok(Built {
        root,
        slot: slot.clone(),
        mounted: Mounted {
            label,
            code,
            slot,
            shown,
        },
    })
}
/// Stylesheet shared by every listing owner.
#[cfg(target_arch = "wasm32")]
pub(crate) fn styles(document: &Document) -> Result<Element, AttachmentHostError> {
    let style = element(document, "style", "")?;
    let syntax = maestro_theme::highlight_css()
        .map_err(|e| AttachmentHostError::Presentation(e.to_string()))?;
    style.set_text_content(Some(&format!("{}{syntax}", include_str!("code.css"))));
    Ok(style)
}

/// Native factory for a standalone listing; the mounted owner is published through `slot`.
pub(super) fn content(
    prepared: &Rc<Prepared>,
    slot: &Rc<RefCell<Weak<Mounted>>>,
    id: egui::Id,
) -> Rc<ReadingContent> {
    #[cfg(target_arch = "wasm32")]
    {
        let (prepared, slot) = (Rc::clone(prepared), Rc::clone(slot));
        Rc::new(ReadingContent {
            mount: Rc::new(move |document| mount(document, &prepared, id, &slot)),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (prepared, slot, id);
        Rc::new(ReadingContent {})
    }
}
/// Mount the listing with its styles in a flow root.
#[cfg(target_arch = "wasm32")]
fn mount(
    document: &Document,
    prepared: &Rc<Prepared>,
    id: egui::Id,
    slot: &Rc<RefCell<Weak<Mounted>>>,
) -> Result<ReadingRoot, AttachmentHostError> {
    let flow = element(document, "div", "maestro-code-root")?;
    flow.set_attribute("style", "display:flow-root")
        .map_err(|e| host_error(&e))?;
    append(&flow, &styles(document)?)?;
    let built = build(document, prepared)?;
    append(&flow, &built.root)?;
    let mounted = Rc::new(built.mounted);
    *slot.borrow_mut() = Rc::downgrade(&mounted);
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        element: flow,
        slots: vec![(id, built.slot)],
        commit: Some(Box::new({
            let prepared = Rc::clone(prepared);
            let slot = Rc::clone(slot);
            move |previous| {
                if let Some(mounted) = previous
                    .owner
                    .as_ref()
                    .and_then(|owner| owner.downcast_ref::<Rc<Mounted>>())
                {
                    mounted.update(&prepared);
                    *slot.borrow_mut() = Rc::downgrade(mounted);
                }
            }
        })),
        owner: Some(Box::new(mounted)),
    })
}
