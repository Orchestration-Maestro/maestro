//! Console output mounted as native page content with its own scroller and copy control.
use super::{ConsoleBlock, ConsoleVariant};
use crate::{
    browser::{
        attachment_content::browser::{element, host_error, set_text_if_changed},
        timer::Timer,
    },
    components::copy_button::dom::{self as copy_button, Localization},
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::{cell::RefCell, rc::Rc};
use web_sys::Element;

/// Success feedback duration of the console's copy control, in milliseconds.
const FEEDBACK_MS: i32 = 1500;

/// Console state a mounted root owns.
struct Console {
    /// Localized header label.
    label: Element,
    /// Copy control receiving the literal output.
    copy: Element,
    /// Feature-owned scrolling body, distinct from the host viewport.
    scroll: Element,
    /// Literal output body.
    pre: Element,
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
        let root = element(document, "div", "maestro-console")?;
        let style = element(document, "style", "")?;
        style.set_text_content(Some(include_str!("console.css")));
        root.append_child(&style)
            .map_err(|error| host_error(&error))?;
        let header = element(document, "div", "maestro-console-header")?;
        let label = element(document, "span", "maestro-console-label")?;
        header
            .append_child(&label)
            .map_err(|error| host_error(&error))?;
        let i18n = Rc::new(RefCell::new(Rc::clone(&view.i18n)));
        let scroll = element(document, "div", "console-scroll")?;
        let scroller = scroll.clone();
        let copy = copy_button::create(
            document,
            &i18n,
            FEEDBACK_MS,
            Some(Rc::new(move || scroll_bottom(&scroller))),
        )
        .map_err(|error| host_error(&error))?;
        copy.set_attribute("show-text", "")
            .map_err(|error| host_error(&error))?;
        header
            .append_child(&copy)
            .map_err(|error| host_error(&error))?;
        let pre = element(document, "pre", "")?;
        scroll
            .append_child(&pre)
            .map_err(|error| host_error(&error))?;
        for child in [&header, &scroll] {
            root.append_child(child)
                .map_err(|error| host_error(&error))?;
        }
        let mounted = DomRoot {
            node: root.into(),
            owner: Some(Box::new(Console {
                label,
                copy,
                scroll,
                pre,
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
            let next = (self.content.clone(), self.variant);
            if *published == next {
                false
            } else {
                *published = next;
                true
            }
        };
        set_text_if_changed(&state.label, &label);
        set_text_if_changed(&state.pre, &self.content);
        state.pre.set_class_name(match self.variant {
            ConsoleVariant::Error => "maestro-console-error",
            ConsoleVariant::Default => "",
        });
        for (name, value) in [("text", self.content.as_str()), ("title", title.as_str())] {
            if state.copy.get_attribute(name).as_deref() != Some(value) {
                state
                    .copy
                    .set_attribute(name, value)
                    .map_err(|error| host_error(&error))?;
            }
        }
        if changed {
            follow(&state.scroll, root, &state.pending);
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

/// Bring the console's own scroller to its bottom.
fn scroll_bottom(scroll: &Element) {
    scroll.set_scroll_top(scroll.scroll_height());
}

/// Follow an update at once on the page, or on the next frame while the root is still detached.
fn follow(scroll: &Element, root: &DomRoot, pending: &RefCell<Option<Timer>>) {
    let replaced = pending.borrow_mut().take();
    drop(replaced);
    if root.node.is_connected() {
        scroll_bottom(scroll);
        return;
    }
    let scroller = scroll.clone();
    let admitted = web_sys::window().and_then(|window| {
        crate::browser::timer::frame(window, Rc::new(move || scroll_bottom(&scroller))).ok()
    });
    *pending.borrow_mut() = admitted;
}
