//! Native disclosure button over the captured children's retained roots.
use super::ExpandableSection;
use crate::{
    browser::{
        attachment_content::browser::{element, host_error, set_text_if_changed},
        event_subscription::EventSubscription,
    },
    dom::{self, DomContent, DomRoot, DomView, MountedContent},
    icons::IconSize,
    tools::{
        renderer_registry::header::dom::{Click, glyph},
        types::ToolUiError,
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::{Element, Event};

/// The expansion and the elements it changes.
struct Disclosure {
    /// Section root, parent of the details while expanded.
    root: Element,
    /// Chevron container.
    chevron: Element,
    /// Detail element holding every captured child's root, detached while collapsed.
    details: Element,
    /// Current expansion.
    expanded: Cell<bool>,
}
impl Disclosure {
    /// Show the chevron for `expanded` and attach or detach the details.
    fn set(&self, expanded: bool) -> Result<(), ToolUiError> {
        self.expanded.set(expanded);
        let name = if expanded {
            "ChevronDown"
        } else {
            "ChevronRight"
        };
        let icon = crate::icons::icon_dom(&glyph(name)?, IconSize::Sm, None)
            .map_err(|error| host_error(&error))?;
        self.chevron.replace_children_with_node_1(&icon);
        if expanded {
            self.root
                .append_child(&self.details)
                .map_err(|error| host_error(&error))?;
        } else {
            self.details.remove();
        }
        Ok(())
    }
}
/// The summary, disclosure and children's roots one mounted section owns.
struct Mounted {
    /// Literal summary text.
    summary: Element,
    /// Shared with the click callback, which holds it weakly.
    disclosure: Rc<Disclosure>,
    /// Captured children's roots, each mounted once.
    children: RefCell<Vec<MountedContent>>,
    /// Native click registration, removed with the root.
    _click: Click,
}
impl DomContent for ExpandableSection {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let document = &view.document;
        let root = element(document, "div", "maestro-expandable")?;
        let button = element(document, "button", "maestro-expandable-toggle")?;
        button
            .set_attribute("type", "button")
            .map_err(|error| host_error(&error))?;
        let chevron = element(document, "span", "maestro-expandable-chevron")?;
        let summary = element(document, "span", "")?;
        button
            .append_with_node_2(&chevron, &summary)
            .map_err(|error| host_error(&error))?;
        root.append_child(&button)
            .map_err(|error| host_error(&error))?;
        let details = element(document, "div", "maestro-expandable-details")?;
        let mut children = Vec::with_capacity(self.children.len());
        for child in &self.children {
            let mut mounted = MountedContent::default();
            mounted.show(child.as_ref(), &details, view)?;
            children.push(mounted);
        }
        let disclosure = Rc::new(Disclosure {
            root: root.clone(),
            chevron,
            details,
            expanded: Cell::new(false),
        });
        disclosure.set(self.default_expanded)?;
        let weak = Rc::downgrade(&disclosure);
        let callback = Closure::new(move |_: Event| {
            if let Some(disclosure) = weak.upgrade() {
                disclosure
                    .set(!disclosure.expanded.get())
                    .map_err(|error| JsValue::from_str(&error.to_string()))?;
            }
            Ok(())
        });
        let click = EventSubscription::new(button.into(), "click", callback)
            .map_err(|error| host_error(&error))?;
        let root = DomRoot {
            node: root.into(),
            owner: Some(Box::new(Mounted {
                summary,
                disclosure,
                children: RefCell::new(children),
                _click: click,
            })),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Change the summary and update each captured child in its retained root.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = mounted(root)?;
        set_text_if_changed(&mounted.summary, &self.summary);
        let mut children = mounted.children.take();
        let result = self
            .children
            .iter()
            .zip(&mut children)
            .try_for_each(|(child, placed)| {
                placed.show(child.as_ref(), &mounted.disclosure.details, view)
            });
        let old = mounted.children.replace(children);
        drop(old);
        result
    }
}
impl ExpandableSection {
    /// Reapply the current default to a mounted root, as reconnection does; the captured
    /// children stay in place.
    /// # Errors
    /// Returns a root this section did not mount, or a native failure.
    pub fn connected_callback(&self, root: &DomRoot) -> Result<(), ToolUiError> {
        mounted(root)?.disclosure.set(self.default_expanded)
    }
}
/// The state a section's mount left on `root`.
fn mounted(root: &DomRoot) -> Result<&Mounted, ToolUiError> {
    root.owner
        .as_ref()
        .and_then(|owner| owner.downcast_ref::<Mounted>())
        .ok_or_else(|| ToolUiError::Presentation("Missing section state".into()))
}
