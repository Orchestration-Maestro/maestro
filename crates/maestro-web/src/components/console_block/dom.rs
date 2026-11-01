//! Console output mounted as native page content with its own scroller and copy control.
use super::{
    ConsoleBlock, ConsoleVariant,
    nodes::{self, Nodes},
};
use crate::{
    browser::{attachment_content::browser::host_error, timer::Timer},
    components::copy_button::dom::{self as copy_button, Localization},
    dom::{DomContent, DomRoot, DomView, live},
    tools::types::ToolUiError,
};
use std::{cell::RefCell, rc::Rc};
use web_sys::{Element, HtmlElement};

/// Success feedback duration of the console's copy control, in milliseconds.
const FEEDBACK_MS: i32 = 1500;

/// Console state a mounted root owns.
struct Console {
    /// Shared console frame.
    nodes: Nodes,
    /// Copy control receiving the literal output.
    copy: Element,
    /// Localization the copy control's labels follow.
    i18n: Localization,
    /// Successfully published literal output and semantic color.
    published: RefCell<(String, ConsoleVariant)>,
    /// The next-frame scroll of a root that is not on the page yet.
    pending: RefCell<Option<Timer>>,
}

impl DomContent for ConsoleBlock {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let nodes = Nodes::build(document)?;
        let i18n = Rc::new(RefCell::new(Rc::clone(&view.i18n)));
        let scroller = js_sys::WeakRef::new(&nodes.scroll);
        let copy = copy_button::create(
            document,
            &i18n,
            FEEDBACK_MS,
            Some(Rc::new(move || {
                if let Some(scroll) = live(&scroller) {
                    nodes::scroll_bottom(&scroll);
                }
            })),
            None,
        )
        .map_err(|error| host_error(&error))?;
        for (name, value) in [("show-text", ""), ("compact", "")] {
            copy.set_attribute(name, value)
                .map_err(|error| host_error(&error))?;
        }
        nodes
            .header
            .append_child(&copy)
            .map_err(|error| host_error(&error))?;
        nodes.place()?;
        let mounted = DomRoot {
            node: nodes.root.clone().into(),
            owner: Some(Box::new(Console {
                nodes,
                copy,
                i18n,
                published: RefCell::new((String::new(), ConsoleVariant::default())),
                pending: RefCell::default(),
            })),
        };
        self.update(&mounted, view)?;
        Ok(mounted)
    }

    /// Change only the label, body, color and copy operands that differ, then follow the update.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let state = console(root)?;
        state.i18n.replace(Rc::clone(&view.i18n));
        let label = view.i18n.text("console")?;
        let title = view.i18n.text("Copy output")?;
        let changed = {
            let mut published = state.published.borrow_mut();
            let changed = published.0 != self.content || published.1 != self.variant;
            if changed {
                published.0.clone_from(&self.content);
                published.1 = self.variant;
            }
            changed
        };
        state
            .nodes
            .update((&label, self.content.as_str(), self.variant));
        for (name, value) in [("text", self.content.as_str()), ("title", title.as_str())] {
            if state.copy.get_attribute(name).as_deref() != Some(value) {
                state
                    .copy
                    .set_attribute(name, value)
                    .map_err(|error| host_error(&error))?;
            }
        }
        if changed {
            follow(&state.nodes.scroll, root, &state.pending);
        }
        Ok(())
    }
}

/// The console state a mounted root owns.
fn console(root: &DomRoot) -> Result<&Console, ToolUiError> {
    root.owner
        .as_ref()
        .and_then(|owner| owner.downcast_ref::<Console>())
        .ok_or_else(|| ToolUiError::Presentation("Missing console state".into()))
}

/// Follow an update at once on the page, or on the next frame while the root is still detached.
fn follow(scroll: &HtmlElement, root: &DomRoot, pending: &RefCell<Option<Timer>>) {
    let replaced = pending.borrow_mut().take();
    drop(replaced);
    if root.node.is_connected() {
        nodes::scroll_bottom(scroll);
        return;
    }
    let scroller = js_sys::WeakRef::new(scroll);
    let admitted = web_sys::window().and_then(|window| {
        crate::browser::timer::frame(
            window,
            Rc::new(move || {
                if let Some(scroll) = live(&scroller) {
                    nodes::scroll_bottom(&scroll);
                }
            }),
        )
        .ok()
    });
    *pending.borrow_mut() = admitted;
}
