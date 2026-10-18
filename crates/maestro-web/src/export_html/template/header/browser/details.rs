//! Native disclosure nodes with root-owned subscriptions and weak callbacks.
use super::super::{
    Prepared,
    details::{Parameter, Prompt, Tool},
};
use super::{append, element};
use crate::browser::{
    attachment_content::{AttachmentHostError, browser::host_error},
    event_subscription::EventSubscription,
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::{Document, Element};

/// One native container and its independent expansion decision.
struct Disclosure {
    /// Complete clickable container.
    element: Element,
    /// Base classes for collapsed state.
    base: &'static str,
    /// Expanded class to append.
    class: &'static str,
    /// Last successfully committed expansion.
    expanded: Cell<bool>,
}
/// Fallible native click callable retained through listener removal.
type Click = Closure<dyn FnMut() -> Result<(), JsValue>>;
/// Strong mounted state and exact subscription guards, never retained by a factory.
struct Mounted {
    /// Strong counterpart to each callback's weak capture.
    _state: Rc<Vec<Disclosure>>,
    /// Detachment precedes callable release through the shared owner.
    _listeners: Vec<EventSubscription<Click>>,
}
/// Compose disclosures and install callbacks before publishing their root.
pub(super) fn mount(
    document: &Document,
    root: &Element,
    prepared: &Prepared,
) -> Result<Option<Box<dyn std::any::Any>>, AttachmentHostError> {
    let mut state = Vec::new();
    if let Some(prompt) = &prepared.prompt {
        let node = prompt_node(document, prompt, &mut state)?;
        append(root, &node)?;
    }
    if !prepared.tools.is_empty() {
        let tools = element(document, "div", "tools-list", "")?;
        append(
            &tools,
            &element(document, "div", "tools-header", "Available Tools")?,
        )?;
        let content = element(document, "div", "tools-content", "")?;
        for tool in &prepared.tools {
            append(&content, &tool_node(document, tool, &mut state)?)?;
        }
        append(&tools, &content)?;
        append(root, &tools)?;
    }
    if state.is_empty() {
        return Ok(None);
    }
    let state = Rc::new(state);
    let mut listeners = Vec::new();
    for (ordinal, disclosure) in state.iter().enumerate() {
        let weak = Rc::downgrade(&state);
        let callback = Closure::new(move || {
            if let Some(state) = weak.upgrade() {
                toggle(&state[ordinal])?;
            }
            Ok(())
        });
        listeners.push(
            EventSubscription::new(disclosure.element.clone().into(), "click", callback)
                .map_err(|e| host_error(&e))?,
        );
    }
    Ok(Some(Box::new(Mounted {
        _state: state,
        _listeners: listeners,
    })))
}
/// Append the short or preview/full prompt branch using literal text nodes.
fn prompt_node(
    document: &Document,
    prompt: &Prompt,
    state: &mut Vec<Disclosure>,
) -> Result<Element, AttachmentHostError> {
    let node = element(document, "div", "system-prompt", "")?;
    append(
        &node,
        &element(document, "div", "system-prompt-header", "System Prompt")?,
    )?;
    let full = match prompt {
        Prompt::Full(full) => full,
        Prompt::Preview {
            full,
            preview,
            hint,
        } => {
            node.set_attribute("class", "system-prompt expandable")
                .map_err(|e| host_error(&e))?;
            append(
                &node,
                &element(document, "div", "system-prompt-preview", preview)?,
            )?;
            append(
                &node,
                &element(document, "div", "system-prompt-expand-hint", hint)?,
            )?;
            state.push(Disclosure {
                element: node.clone(),
                base: "system-prompt expandable",
                class: "expanded",
                expanded: Cell::new(false),
            });
            full
        }
    };
    let content = element(document, "div", "system-prompt-full", full)?;
    if matches!(prompt, Prompt::Full(_)) {
        content
            .set_attribute("style", "display:block")
            .map_err(|e| host_error(&e))?;
    }
    append(&node, &content)?;
    Ok(node)
}
/// Compose tool summary and eagerly prepared parameter details.
fn tool_node(
    document: &Document,
    tool: &Tool,
    state: &mut Vec<Disclosure>,
) -> Result<Element, AttachmentHostError> {
    let node = element(document, "div", "tool-item", "")?;
    append(
        &node,
        &element(document, "span", "tool-item-name", &tool.name)?,
    )?;
    append(&node, &element(document, "span", "", " - ")?)?;
    append(
        &node,
        &element(document, "span", "tool-item-desc", &tool.description)?,
    )?;
    if !tool.parameters.is_empty() {
        append(&node, &element(document, "span", "", " ")?)?;
        append(&node, &element(document, "span", "tool-params-hint", "")?)?;
        let content = element(document, "div", "tool-params-content", "")?;
        for parameter in &tool.parameters {
            append(&content, &parameter_node(document, parameter)?)?;
        }
        append(&node, &content)?;
        state.push(Disclosure {
            element: node.clone(),
            base: "tool-item",
            class: "params-expanded",
            expanded: Cell::new(false),
        });
    }
    Ok(node)
}
/// Use the actual owning window's live selection before changing one container.
fn toggle(disclosure: &Disclosure) -> Result<(), JsValue> {
    let window = disclosure
        .element
        .owner_document()
        .and_then(|d| d.default_view())
        .ok_or_else(|| JsValue::from_str("missing disclosure window"))?;
    let selection = super::invoke0(&window, "getSelection")?;
    if super::invoke0(&selection, "toString")?
        .as_string()
        .is_some_and(|s| !s.is_empty())
    {
        return Ok(());
    }
    let expanded = !disclosure.expanded.get();
    let class = if expanded {
        format!("{} {}", disclosure.base, disclosure.class)
    } else {
        disclosure.base.into()
    };
    disclosure.element.set_attribute("class", &class)?;
    disclosure.expanded.set(expanded);
    Ok(())
}

/// Compose one parameter with literal spacing and its optional description.
fn parameter_node(
    document: &Document,
    parameter: &Parameter,
) -> Result<Element, AttachmentHostError> {
    let row = element(document, "div", "tool-param", "")?;
    append(
        &row,
        &element(document, "span", "tool-param-name", &parameter.name)?,
    )?;
    append(&row, &element(document, "span", "", " ")?)?;
    append(
        &row,
        &element(document, "span", "tool-param-type", &parameter.kind)?,
    )?;
    append(&row, &element(document, "span", "", " ")?)?;
    let (class, label) = if parameter.required {
        ("tool-param-required", "required")
    } else {
        ("tool-param-optional", "optional")
    };
    append(&row, &element(document, "span", class, label)?)?;
    if let Some(description) = &parameter.description {
        append(
            &row,
            &element(document, "div", "tool-param-desc", description)?,
        )?;
    }
    Ok(row)
}
