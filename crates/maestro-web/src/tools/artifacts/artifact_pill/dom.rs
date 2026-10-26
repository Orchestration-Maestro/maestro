//! Page-content filename pills and the artifact caption that holds one.
use super::Pill;
use crate::{
    browser::{
        attachment_content::browser::{element, host_error},
        event_subscription::EventSubscription,
    },
    dom::{DomContent, DomRoot, DomView, MountedContent},
    tools::{
        renderer_registry::header::dom::{Click, glyph},
        types::ToolUiError,
    },
};
use maestro_app::browser::tools::artifacts::LocalArtifactsPanel;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Event};

/// The panel and authored filename a click currently opens.
type Target = Rc<RefCell<(Option<Rc<LocalArtifactsPanel>>, Option<String>)>>;
/// A mounted pill's name, its live click operands and, for a button, the click registration.
struct Mounted {
    /// Literal filename text.
    name: Element,
    /// Operands read when clicked, replaced on update.
    target: Target,
    /// Native click registration of a button, removed with the root.
    _click: Option<Click>,
}
impl DomContent for Pill {
    /// A pill with a panel is its own button; without one it is text whose clicks reach the
    /// containing header.
    fn key(&self) -> Option<&str> {
        Some(if self.panel.is_some() {
            "button"
        } else {
            "text"
        })
    }
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = if self.panel.is_some() {
            let button = element(
                &view.document,
                "button",
                "maestro-artifact-pill maestro-text-action",
            )?;
            button
                .set_attribute("type", "button")
                .map_err(|error| host_error(&error))?;
            button
        } else {
            element(&view.document, "span", "maestro-artifact-pill")?
        };
        let icon = crate::icons::icon_dom(&glyph("FileCode2")?, crate::icons::IconSize::Sm, None)
            .map_err(|error| host_error(&error))?;
        let name = element(&view.document, "span", "")?;
        root.append_with_node_2(&icon, &name)
            .map_err(|error| host_error(&error))?;
        let target = Target::default();
        let click = self
            .panel
            .as_ref()
            .map(|_| open_binding(&root, &target))
            .transpose()?;
        let root = DomRoot {
            node: root.into(),
            owner: Some(Box::new(Mounted {
                name,
                target,
                _click: click,
            })),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("Missing pill state".into()))?;
        mounted.name.set_text_content(Some(&self.text));
        let previous = mounted
            .target
            .replace((self.panel.clone(), self.filename.clone()));
        drop(previous);
        Ok(())
    }
}
/// Open the current file in the current panel, consuming the click so the header does not toggle.
fn open_binding(root: &Element, target: &Target) -> Result<Click, ToolUiError> {
    let current = Rc::clone(target);
    let callback = Closure::new(move |event: Event| {
        let (panel, filename) = current.borrow().clone();
        let Some(panel) = panel else {
            return Ok(());
        };
        event.prevent_default();
        event.stop_propagation();
        if let Some(filename) = filename {
            panel.open_artifact(&filename);
        }
        Ok(())
    });
    Ok(
        EventSubscription::new(root.clone().into(), "click", callback)
            .map_err(|error| host_error(&error))?,
    )
}
/// A localized label followed by its filename pill.
struct Caption {
    /// Already localized label.
    label: String,
    /// Captured filename and panel.
    pill: Pill,
}
/// Prepare the rich caption `label`, a space and `pill`.
pub(in crate::tools) fn caption(label: String, pill: &Pill) -> Box<dyn DomContent> {
    Box::new(Caption {
        label,
        pill: pill.clone(),
    })
}
impl DomContent for Caption {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = element(&view.document, "span", "")?;
        root.append_with_str_2("", " ")
            .map_err(|error| host_error(&error))?;
        let root = DomRoot {
            node: root.into(),
            owner: Some(Box::new(RefCell::new(MountedContent::default()))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let slot = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<MountedContent>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing caption state".into()))?;
        if let Some(label) = root.node.first_child() {
            label.set_text_content(Some(&self.label));
        }
        let mut pill = slot.take();
        let result = pill.show(&self.pill, &root.node, view);
        let old = slot.replace(pill);
        drop(old);
        result
    }
}
