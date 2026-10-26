//! Page-content status header.
use crate::{
    browser::attachment_content::browser::{element, host_error},
    dom::icon,
    tools::{renderer_registry::ToolHeaderState, types::ToolUiError},
};
use web_sys::{Document, Element};

/// Build a status header: the tool icon and caption, with a trailing loader while pending.
/// # Errors
/// Returns an icon or native creation failure.
pub(crate) fn render_header_element(
    view: &DomView,
    state: ToolHeaderState,
    tool_icon: &str,
    text: &str,
) -> Result<Element, ToolUiError> {
    let root =
        render_header(state, glyph(tool_icon)?, ToolHeaderText::Text(text.into())).mount(view)?;
    Ok(root.node.unchecked_into())
}
/// The catalog icon named `name`.
/// # Errors
/// Returns a catalog failure or a missing icon.
pub(crate) fn glyph(name: &str) -> Result<IconNode, ToolUiError> {
    crate::icons::icon_node(name)
        .map_err(|error| ToolUiError::Presentation(error.to_string()))?
        .ok_or_else(|| ToolUiError::Presentation(format!("Missing icon {name}")))
}

use super::{Header, ToolHeaderRefs, ToolHeaderText};
use crate::{
    browser::event_subscription::EventSubscription,
    dom::{DomContent, DomRoot, DomView, MountedContent},
    icons::IconNode,
    tools::renderer_registry::render_header,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::Event;

/// One retained click subscription.
pub(crate) type Click = EventSubscription<Closure<dyn Fn(Event) -> Result<(), JsValue>>>;
/// A stable caption placement and the header's installed event.
struct MountedHeader {
    /// Leading glyph and caption parent.
    lead: Element,
    /// Tool glyph span.
    tool: Element,
    /// Caption slot.
    caption: Element,
    /// Optional pending glyph.
    spinner: RefCell<Option<Element>>,
    /// Independent rich caption lifetime.
    rich: RefCell<MountedContent>,
    /// Current activation references, updated without rebuilding the header.
    refs: Rc<RefCell<Option<ToolHeaderRefs>>>,
    /// Native click registration, removed with the root.
    _click: Option<Click>,
}
impl DomContent for Header {
    /// A plain header has one template per state, a toggle one for all states.
    fn key(&self) -> Option<&str> {
        Some(match (&self.disclosure, self.state) {
            (Some(_), _) => "toggle",
            (None, ToolHeaderState::Inprogress) => "inprogress",
            (None, ToolHeaderState::Complete) => "complete",
            (None, ToolHeaderState::Error) => "error",
        })
    }
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        crate::dom::install_styles(view).map_err(|error| host_error(&error))?;
        let node = element(&view.document, "div", "maestro-tool-header")?;
        let lead = element(&view.document, "div", "maestro-tool-header-lead")?;
        let tool = element(&view.document, "span", "maestro-tool-icon")?;
        let caption = element(&view.document, "span", "maestro-tool-caption")?;
        lead.append_with_node_2(&tool, &caption)
            .map_err(|error| host_error(&error))?;
        node.append_child(&lead)
            .map_err(|error| host_error(&error))?;
        let refs = Rc::new(RefCell::new(None::<ToolHeaderRefs>));
        // The whole row toggles on a pointer click; its chevron is the keyboard disclosure, so a
        // caption's own controls never sit inside another control.
        let click = if let Some((targets, expanded)) = &self.disclosure {
            node.class_list()
                .add_1("maestro-tool-toggle")
                .map_err(|error| host_error(&error))?;
            let chevron = chevron(&view.document, *expanded, &caption)?;
            node.append_child(&chevron)
                .map_err(|error| host_error(&error))?;
            targets.chevron.replace(Some(chevron.unchecked_into()));
            let current = Rc::clone(&refs);
            let callback = Closure::new(move |event: Event| {
                event.prevent_default();
                let targets = current.borrow().clone();
                toggle(targets).map_err(|error| JsValue::from_str(&error.to_string()))
            });
            Some(
                EventSubscription::new(node.clone().into(), "click", callback)
                    .map_err(|error| host_error(&error))?,
            )
        } else {
            None
        };
        let root = DomRoot {
            node: node.into(),
            owner: Some(Box::new(MountedHeader {
                lead,
                tool,
                caption,
                spinner: RefCell::default(),
                rich: RefCell::default(),
                refs,
                _click: click,
            })),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<MountedHeader>())
            .ok_or_else(|| ToolUiError::Presentation("Missing header state".into()))?;
        let glyph = crate::icons::icon_dom(&self.tool_icon, crate::icons::IconSize::Sm, None)
            .map_err(|error| host_error(&error))?;
        self.caption(mounted, view)?;
        mounted.tool.replace_children_with_node_1(&glyph);
        mounted.tool.set_class_name(status_class(self.state));
        let previous = mounted
            .refs
            .replace(self.disclosure.as_ref().map(|(refs, _)| refs.clone()));
        drop(previous);
        let spinner = mounted.spinner.borrow_mut().take();
        match (spinner, self.state == ToolHeaderState::Inprogress) {
            (Some(spinner), false) => spinner.remove(),
            (None, true) => {
                let spinner = icon(
                    &view.document,
                    "Loader",
                    "maestro-tool-icon maestro-tool-spinner",
                )?;
                if self.disclosure.is_some() {
                    mounted.lead.prepend_with_node_1(&spinner)
                } else {
                    root.node.append_child(&spinner).map(|_| ())
                }
                .map_err(|error| host_error(&error))?;
                mounted.spinner.replace(Some(spinner));
            }
            (spinner, true) => {
                mounted.spinner.replace(spinner);
            }
            (None, false) => {}
        }
        Ok(())
    }
}
impl Header {
    /// Update native children without holding a borrow across caller content.
    fn caption(&self, mounted: &MountedHeader, view: &DomView) -> Result<(), ToolUiError> {
        let mut rich = mounted.rich.take();
        let result = match &self.text {
            ToolHeaderText::Text(text) => {
                mounted.caption.set_text_content(Some(text));
                rich = MountedContent::default();
                Ok(())
            }
            ToolHeaderText::Content(content) => {
                if rich.root().is_none() {
                    mounted.caption.set_text_content(None);
                }
                rich.show(content.as_ref(), mounted.caption.as_ref(), view)
            }
        };
        let old = mounted.rich.replace(rich);
        drop(old);
        result
    }
}
/// The disclosure button; both icon children stay mounted so activation can change only visibility.
fn chevron(document: &Document, expanded: bool, caption: &Element) -> Result<Element, ToolUiError> {
    /// Caption identities independent of authored text and other mounted headers.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = format!(
        "maestro-tool-caption-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    caption.set_id(&id);
    let root = element(document, "button", "maestro-tool-chevron")?;
    for (name, value) in [("type", "button"), ("aria-labelledby", id.as_str())] {
        root.set_attribute(name, value)
            .map_err(|error| host_error(&error))?;
    }
    for (name, class, hidden) in [
        ("ChevronUp", "chevron-up", !expanded),
        ("ChevronsUpDown", "chevrons-up-down", expanded),
    ] {
        let child = icon(document, name, class)?;
        if hidden {
            child
                .class_list()
                .add_1("hidden")
                .map_err(|error| host_error(&error))?;
        }
        root.append_child(&child)
            .map_err(|error| host_error(&error))?;
    }
    Ok(root)
}
/// Read live references before any native mutation; missing either target is a no-op.
fn toggle(refs: Option<ToolHeaderRefs>) -> Result<(), ToolUiError> {
    let Some(refs) = refs else { return Ok(()) };
    let content = refs.content.borrow().clone();
    let chevron = refs.chevron.borrow().clone();
    let (Some(content), Some(chevron)) = (content, chevron) else {
        return Ok(());
    };
    let collapsed = content.class_list().contains("max-h-0");
    for (class, present) in [
        ("max-h-0", !collapsed),
        ("max-h-[2000px]", collapsed),
        ("mt-3", collapsed),
    ] {
        content
            .class_list()
            .toggle_with_force(class, present)
            .map_err(|error| host_error(&error))?;
    }
    let up = chevron
        .query_selector(".chevron-up")
        .map_err(|error| host_error(&error))?;
    let down = chevron
        .query_selector(".chevrons-up-down")
        .map_err(|error| host_error(&error))?;
    if let (Some(up), Some(down)) = (up, down) {
        up.class_list()
            .toggle_with_force("hidden", !collapsed)
            .map_err(|error| host_error(&error))?;
        down.class_list()
            .toggle_with_force("hidden", collapsed)
            .map_err(|error| host_error(&error))?;
    }
    Ok(())
}

/// Semantic tool-glyph color shared by plain and collapsible headers.
fn status_class(state: ToolHeaderState) -> &'static str {
    match state {
        ToolHeaderState::Inprogress => "maestro-tool-icon",
        ToolHeaderState::Complete => "maestro-tool-icon maestro-tool-complete",
        ToolHeaderState::Error => "maestro-tool-icon maestro-tool-error",
    }
}
