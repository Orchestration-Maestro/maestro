//! Native mounting and event ownership.
use super::AttachmentTile;
use crate::{
    AttachmentOverlay,
    browser::{
        attachment_content::browser::{append, element},
        event_subscription::EventSubscription,
    },
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    dom::{self, DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Element, Event};
/// Native listeners retained by the mounted tile.
type Binding = EventSubscription<Closure<dyn Fn(Event) -> Result<(), JsValue>>>;
/// Subscriptions retained by one tile root.
type Events = RefCell<Vec<Binding>>;
impl DomContent for AttachmentTile {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-attachment-tile")?.into(),
            owner: Some(Box::new(Events::default())),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let events = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Events>())
            .ok_or_else(|| ToolUiError::Presentation("Missing tile state".into()))?;
        let (candidate, bindings) = self.prepare(view)?;
        let node: &Element = root.node.unchecked_ref();
        node.replace_children_with_node_1(&candidate);
        let old = events.replace(bindings);
        drop(old);
        Ok(())
    }
}
impl AttachmentTile {
    /// Prepare a complete candidate without changing the published tile.
    fn prepare(&self, view: &DomView) -> Result<(Element, Vec<Binding>), ToolUiError> {
        let (name, preview, pdf, excel) =
            super::thumbnail_data(&self.attachment).map_err(ToolUiError::Attachment)?;
        let shell = element(&view.document, "div", "maestro-tile-shell")?;
        let target = element(&view.document, "div", "maestro-tile-open")?;
        target
            .set_attribute("data-open", "")
            .map_err(|error| ToolUiError::native(&error))?;
        target
            .set_attribute("title", &name)
            .map_err(|error| ToolUiError::native(&error))?;
        if let Some(source) = preview {
            let image = element(&view.document, "img", "maestro-tile-image")?;
            image
                .set_attribute("src", &source)
                .map_err(|error| ToolUiError::native(&error))?;
            image
                .set_attribute("alt", &name)
                .map_err(|error| ToolUiError::native(&error))?;
            image
                .set_attribute("title", &name)
                .map_err(|error| ToolUiError::native(&error))?;
            append(&target, &image)?;
            if pdf {
                let badge = element(&view.document, "div", "maestro-tile-badge")?;
                badge.set_text_content(Some(&view.i18n.text("PDF")?));
                append(&shell, &badge)?;
            }
        } else {
            target.set_class_name("maestro-tile-open maestro-tile-fallback");
            append(
                &target,
                &glyph(
                    if excel { "FileSpreadsheet" } else { "FileText" },
                    crate::icons::IconSize::Md,
                )?,
            )?;
            let label = element(&view.document, "div", "maestro-tile-label")?;
            label.set_text_content(Some(&super::filename_label(&name)));
            append(&target, &label)?;
        }
        shell
            .prepend_with_node_1(&target)
            .map_err(|error| ToolUiError::native(&error))?;
        let mut events = vec![self.open_binding(&target, view)?];
        if self.show_delete {
            events.push(self.remove(&shell, view)?);
        }
        Ok((shell, events))
    }
    /// Open the original record independently of this tile's lifetime.
    fn open_binding(&self, target: &Element, view: &DomView) -> Result<Binding, ToolUiError> {
        let attachment = Arc::clone(&self.attachment);
        let view = Rc::new(view.clone());
        let callback = Closure::new(move |_: Event| {
            AttachmentOverlay::open(&view, Arc::clone(&attachment), None)
                .map(|_| ())
                .map_err(|error| JsValue::from_str(&error.to_string()))
        });
        EventSubscription::new(target.clone().into(), "click", callback)
            .map_err(|error| ToolUiError::native(&error))
    }
    /// Removal is an independent native button and never bubbles into opening.
    fn remove(&self, shell: &Element, view: &DomView) -> Result<Binding, ToolUiError> {
        let title = view.i18n.text("Remove")?;
        let control = button(
            &view.document,
            &ButtonProps {
                variant: ButtonVariant::Ghost,
                size: ButtonSize::Icon,
                class_name: "maestro-tile-remove",
                title: Some(&title),
                ..ButtonProps::default()
            },
            &[glyph("X", crate::icons::IconSize::Xs)?.into()],
        )
        .map_err(|error| ToolUiError::native(&error))?;
        control
            .set_attribute("data-remove", "")
            .map_err(|error| ToolUiError::native(&error))?;
        append(shell, &control)?;
        let on_delete = self.on_delete.clone();
        let callback = Closure::new(move |event: Event| {
            event.stop_propagation();
            if let Some(callback) = &on_delete {
                callback();
            }
            Ok(())
        });
        EventSubscription::new(control.into(), "click", callback)
            .map_err(|error| ToolUiError::native(&error))
    }
}
/// Resolve native glyphs through the shared catalog and SVG renderer.
fn glyph(name: &str, size: crate::icons::IconSize) -> Result<Element, ToolUiError> {
    let icon = crate::icons::icon_node(name)
        .map_err(|error| ToolUiError::Presentation(error.to_string()))?
        .ok_or_else(|| ToolUiError::Presentation(format!("Missing glyph: {name}")))?;
    crate::icons::icon_dom(&icon, size, None)
        .map(Into::into)
        .map_err(|error| ToolUiError::native(&error))
}
