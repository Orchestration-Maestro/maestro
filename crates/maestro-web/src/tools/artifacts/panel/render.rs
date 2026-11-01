//! Native panel flow, stable artifact placements and application-selected tabs.
use super::State;
use crate::{
    browser::{
        attachment_content::browser::{append, element},
        event_subscription::EventSubscription,
    },
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    dom::{self, DomRoot},
    tools::types::ToolUiError,
};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, Event, HtmlElement};

/// Root-delegated native click subscription.
pub(super) type Click = EventSubscription<Closure<dyn Fn(Event)>>;
/// Current independent header and the operands that keep it valid.
pub(super) struct Header {
    /// Literal filename.
    name: String,
    /// Content revision, not another source snapshot.
    revision: Rc<()>,
    /// Language used by these controls.
    language: String,
    /// Control lifetimes owned by the header.
    _root: DomRoot,
}
impl State {
    /// Build plain page chrome; responsive overlays belong to the containing screen.
    fn build(self: &Rc<Self>, root: &HtmlElement) -> Result<(), ToolUiError> {
        let view = self.view.borrow().clone();
        let toolbar = element(&view.document, "div", "maestro-artifact-toolbar")?;
        let tabs = element(&view.document, "div", "maestro-artifact-tabs")?;
        let actions = element(&view.document, "div", "maestro-artifact-actions")?;
        let header = element(&view.document, "div", "maestro-artifact-active-header")?;
        let glyph = crate::browser::attachment_content::glyph_node("X")?;
        let icon = crate::icons::icon_dom(&glyph, crate::icons::IconSize::Sm, None)
            .map_err(|e| ToolUiError::native(&e))?;
        let title = view.i18n.text("Close artifacts")?;
        let close = button(
            &view.document,
            &ButtonProps {
                variant: ButtonVariant::Ghost,
                size: ButtonSize::Sm,
                title: Some(&title),
                ..ButtonProps::default()
            },
            &[icon.into()],
        )
        .map_err(|e| ToolUiError::native(&e))?;
        close
            .set_attribute("data-close-artifacts", "")
            .map_err(|e| ToolUiError::native(&e))?;
        append(&actions, &header)?;
        append(&actions, &close)?;
        append(&toolbar, &tabs)?;
        append(&toolbar, &actions)?;
        append(root, &toolbar)?;
        append(
            root,
            &element(&view.document, "div", "maestro-artifact-content")?,
        )?;
        let error = element(&view.document, "div", "maestro-artifact-panel-error")?;
        error
            .set_attribute("role", "status")
            .map_err(|e| ToolUiError::native(&e))?;
        append(root, &error)?;
        let weak = Rc::downgrade(self);
        let click = Closure::<dyn Fn(Event)>::new(move |event: Event| {
            if let Some(state) = weak.upgrade() {
                state.click(&event);
            }
        });
        self.events.borrow_mut().push(
            EventSubscription::new(root.clone().into(), "click", click)
                .map_err(|e| ToolUiError::native(&e))?,
        );
        Ok(())
    }
    /// Dispatch only this panel's own delegated tab and close activations.
    fn click(&self, event: &Event) {
        let Some(target) = event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
        else {
            return;
        };
        let owner = self.root.borrow().as_ref().and_then(dom::live);
        let belongs = target
            .closest("artifacts-panel")
            .ok()
            .flatten()
            .zip(owner)
            .is_some_and(|(scope, owner)| scope.is_same_node(Some(owner.as_ref())));
        if !belongs {
            return;
        }
        if let Ok(Some(tab)) = target.closest("button[data-filename]")
            && let Some(name) = tab.get_attribute("data-filename")
        {
            self.caller.show_artifact(&name);
        } else if target
            .closest("button[data-close-artifacts]")
            .ok()
            .flatten()
            .is_some()
        {
            let callback = self.close.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        }
    }
    /// Mount missing file views and update visibility without reconstructing unchanged roots.
    pub(super) fn paint(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let Some(root) = self.root.borrow().as_ref().and_then(dom::connected) else {
            return Ok(());
        };
        if root
            .query_selector(":scope > .maestro-artifact-content")
            .map_err(|e| ToolUiError::native(&e))?
            .is_none()
        {
            self.build(&root)?;
        }
        let entries = self.entries();
        root.toggle_attribute_with_force("hidden", self.collapsed.get() || entries.is_empty())
            .map_err(|e| ToolUiError::native(&e))?;
        let content = required(&root, ".maestro-artifact-content")?;
        let active = self.caller.active_filename();
        let view = self.view.borrow().clone();
        let close = required(
            &root,
            ".maestro-artifact-toolbar > .maestro-artifact-actions > button[data-close-artifacts]",
        )?;
        let title = view.i18n.text("Close artifacts")?;
        if close.get_attribute("title").as_deref() != Some(&title) {
            close
                .set_attribute("title", &title)
                .map_err(|error| ToolUiError::native(&error))?;
        }
        for (name, entry) in &entries {
            let (controller, existing) = {
                let entry = entry.borrow();
                (Rc::clone(&entry.controller), entry.root.clone())
            };
            let mounted = if let Some(root) = existing {
                super::super::native::update(&controller, &root, &view)?;
                root
            } else {
                let mounted = Rc::new(super::super::native::mount(&controller, &view)?);
                entry.borrow_mut().root = Some(Rc::clone(&mounted));
                mounted
            };
            let node: &Element = mounted.node.unchecked_ref();
            node.toggle_attribute_with_force("hidden", active.as_deref() != Some(name))
                .map_err(|e| ToolUiError::native(&e))?;
            if !mounted
                .node
                .parent_node()
                .is_some_and(|parent| parent.is_same_node(Some(content.as_ref())))
            {
                content
                    .append_child(&mounted.node)
                    .map_err(|e| ToolUiError::native(&e))?;
            }
        }
        let names: Vec<_> = entries.iter().map(|(name, _)| name.clone()).collect();
        self.tabs(&root, &names, active.as_deref())?;
        self.header(&root, active.as_deref())
    }
    /// Rebuild a changed tab inventory; selection alone preserves buttons and keyboard focus.
    fn tabs(
        &self,
        root: &HtmlElement,
        names: &[String],
        active: Option<&str>,
    ) -> Result<(), ToolUiError> {
        let strip = required(root, ".maestro-artifact-toolbar > .maestro-artifact-tabs")?;
        if *self.tabs.borrow() != names {
            strip.replace_children_with_node_0();
            let view = self.view.borrow();
            for name in names {
                let tab = element(&view.document, "button", "maestro-artifact-tab")?;
                tab.set_attribute("type", "button")
                    .map_err(|e| ToolUiError::native(&e))?;
                tab.set_attribute("data-filename", name)
                    .map_err(|e| ToolUiError::native(&e))?;
                tab.set_text_content(Some(name));
                append(&strip, &tab)?;
            }
            *self.tabs.borrow_mut() = names.to_vec();
        }
        let reveal = self.scroll.borrow_mut().take();
        let buttons = strip
            .query_selector_all("button")
            .map_err(|e| ToolUiError::native(&e))?;
        for (index, name) in names.iter().enumerate() {
            let Some(button) = u32::try_from(index).ok().and_then(|index| {
                buttons
                    .item(index)
                    .map(wasm_bindgen::JsCast::unchecked_into::<Element>)
            }) else {
                continue;
            };
            button
                .set_attribute("aria-pressed", &(active == Some(name)).to_string())
                .map_err(|e| ToolUiError::native(&e))?;
            if reveal.as_deref() == Some(name) {
                button.scroll_into_view();
            }
        }
        Ok(())
    }
    /// Reuse an unchanged header, including its feedback and focused native controls.
    fn header(&self, root: &HtmlElement, active: Option<&str>) -> Result<(), ToolUiError> {
        let entry = active.and_then(|name| self.entries.borrow().get(name).cloned());
        let slot = required(
            root,
            ".maestro-artifact-toolbar > .maestro-artifact-actions > .maestro-artifact-active-header",
        )?;
        let view = self.view.borrow().clone();
        let language = view.i18n.get_current_language()?;
        if let (Some(name), Some(entry)) = (active, entry) {
            let (revision, controller) = {
                let entry = entry.borrow();
                (Rc::clone(&entry.revision), Rc::clone(&entry.controller))
            };
            if self.header.borrow().as_ref().is_some_and(|old| {
                old.name == name && Rc::ptr_eq(&old.revision, &revision) && old.language == language
            }) {
                return Ok(());
            }
            self.header.take();
            slot.replace_children_with_node_0();
            let header = controller.borrow().get_header_buttons(&view)?;
            slot.append_child(&header.node)
                .map_err(|e| ToolUiError::native(&e))?;
            *self.header.borrow_mut() = Some(Header {
                name: name.into(),
                revision,
                language,
                _root: header,
            });
        } else {
            self.header.take();
            slot.replace_children_with_node_0();
        }
        Ok(())
    }
}
/// Find only this panel's fixed chrome slot, never interpolate a supplied filename.
fn required(root: &HtmlElement, selector: &str) -> Result<Element, ToolUiError> {
    root.query_selector(&format!(":scope > {selector}"))
        .map_err(|e| ToolUiError::native(&e))?
        .ok_or_else(|| ToolUiError::Presentation("Missing artifact panel chrome".into()))
}
