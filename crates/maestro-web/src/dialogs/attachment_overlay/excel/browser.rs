//! Native selectable table nodes and empty canvas-control slots.
use super::Sheets;
use crate::browser::attachment_content::{
    AttachmentHostError, ReadingRoot,
    browser::{element as create, host_error},
};
use web_sys::Document;
/// Mount every sheet once; only native visibility changes on tab selection.
pub(super) fn mount(
    document: &Document,
    sheets: &Sheets,
    surface: (egui::Id, egui::Rect, Option<egui::Id>, bool),
    selected: usize,
) -> Result<ReadingRoot, AttachmentHostError> {
    let root = root(document, sheets, surface.3, selected)?;
    let mut slots = Vec::new();
    if let Some(strip) = root
        .query_selector(".maestro-excel-tabs")
        .map_err(|e| host_error(&e))?
    {
        for (index, _) in sheets.values.iter().enumerate() {
            let slot = create(document, "span", "maestro-excel-tab")?;
            strip.append_child(&slot).map_err(|e| host_error(&e))?;
            slots.push((surface.0.with(index), slot));
        }
    }
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        commit: None,
        owner: None,
        element: root,
        slots,
    })
}
/// Build stable table bodies independently of the caller's tab controls.
pub(super) fn root(
    document: &Document,
    sheets: &Sheets,
    padded: bool,
    selected: usize,
) -> Result<web_sys::Element, AttachmentHostError> {
    let root = create(
        document,
        "section",
        if padded {
            "maestro-excel maestro-excel-artifact"
        } else {
            "maestro-excel"
        },
    )?;
    let style = create(document, "style", "")?;
    style.set_text_content(Some(include_str!("../excel.css")));
    root.append_child(&style).map_err(|e| host_error(&e))?;
    let multi = sheets.values.len() > 1;
    if multi {
        let strip = create(document, "div", "maestro-excel-tabs")?;
        root.append_child(&strip).map_err(|e| host_error(&e))?;
    }
    let mut nodes = Vec::new();
    for (_, content) in &sheets.values {
        let body = create(
            document,
            "div",
            if multi {
                "maestro-excel-sheet"
            } else {
                "maestro-excel-single"
            },
        )?;
        match content {
            Ok(html) => body.set_inner_html(html),
            Err(text) => {
                let error = super::super::error::root(document, text, padded)?;
                body.append_child(&error).map_err(|e| host_error(&e))?;
            }
        }
        root.append_child(&body).map_err(|e| host_error(&e))?;
        nodes.push(body);
    }
    root.set_attribute("style", &format!("height:{}px;", sheets.height.get()))
        .map_err(|e| host_error(&e))?;
    *sheets.root.borrow_mut() = Some(root.clone());
    *sheets.nodes.borrow_mut() = nodes;
    select(sheets, selected)?;
    Ok(root)
}
/// Hide rather than rebuild bodies, preserving native selection and both offsets.
pub(super) fn select(sheets: &Sheets, selected: usize) -> Result<(), AttachmentHostError> {
    for (index, node) in sheets.nodes.borrow().iter().enumerate() {
        let value = if index == selected {
            "display:block;"
        } else {
            "display:none;"
        };
        if node.get_attribute("style").as_deref() != Some(value) {
            node.set_attribute("style", value)
                .map_err(|e| host_error(&e))?;
        }
    }
    if let Some(root) = sheets.root.borrow().as_ref() {
        let buttons = root
            .query_selector_all(".maestro-excel-tabs > button")
            .map_err(|e| host_error(&e))?;
        for index in 0..buttons.length() {
            use wasm_bindgen::JsCast as _;
            if let Some(button) = buttons.item(index) {
                let button: web_sys::Element = button.unchecked_into();
                button
                    .set_attribute("aria-selected", &(index as usize == selected).to_string())
                    .map_err(|e| host_error(&e))?;
            }
        }
    }
    Ok(())
}

/// Keep inner scrolling bounded while the registry measures natural root geometry.
pub(super) fn bound(sheets: &Sheets, height: f32) -> Result<(), AttachmentHostError> {
    if sheets.height.get().partial_cmp(&height) == Some(std::cmp::Ordering::Equal) {
        return Ok(());
    }
    sheets.height.set(height);
    if let Some(root) = sheets.root.borrow().as_ref() {
        root.set_attribute("style", &format!("height:{height}px;"))
            .map_err(|e| host_error(&e))?;
    }
    Ok(())
}

/// Reveal the focused canvas tab through its retained native layout slot.
pub(super) fn reveal(sheets: &Sheets, index: usize) -> Result<(), AttachmentHostError> {
    if let Some(root) = sheets.root.borrow().as_ref()
        && let Some(slot) = root
            .query_selector(&format!(".maestro-excel-tab:nth-child({})", index + 1))
            .map_err(|e| host_error(&e))?
    {
        slot.scroll_into_view();
    }
    Ok(())
}

/// Mount native tab controls on the shared sheet bodies without a canvas placement owner.
pub(super) fn native(
    view: &crate::dom::DomView,
    sheets: &std::rc::Rc<Sheets>,
    selected: &std::rc::Rc<std::cell::Cell<usize>>,
    padded: bool,
) -> Result<crate::dom::DomRoot, crate::tools::types::ToolUiError> {
    use crate::tools::types::ToolUiError;
    let root = root(&view.document, sheets, padded, selected.get())?;
    root.remove_attribute("style")
        .map_err(|error| ToolUiError::native(&error))?;
    let mut events = Vec::new();
    if let Some(strip) = root
        .query_selector(".maestro-excel-tabs")
        .map_err(|error| ToolUiError::native(&error))?
    {
        for index in 0..sheets.values.len() {
            let (button, event) = tab(view, sheets, selected, index)?;
            strip
                .append_child(&button)
                .map_err(|error| ToolUiError::native(&error))?;
            events.push(event);
        }
    }
    select(sheets, selected.get())?;
    Ok(crate::dom::DomRoot {
        node: root.into(),
        owner: Some(Box::new(events)),
    })
}
/// Retained native worksheet selection callback.
type TabEvent = crate::browser::event_subscription::EventSubscription<
    wasm_bindgen::closure::Closure<dyn Fn() -> Result<(), wasm_bindgen::JsValue>>,
>;
/// Prepare one native tab and subscribe through the shared event owner.
fn tab(
    view: &crate::dom::DomView,
    sheets: &std::rc::Rc<Sheets>,
    selected: &std::rc::Rc<std::cell::Cell<usize>>,
    index: usize,
) -> Result<(web_sys::Element, TabEvent), crate::tools::types::ToolUiError> {
    use crate::{browser::event_subscription::EventSubscription, tools::types::ToolUiError};
    use std::rc::Rc;
    use wasm_bindgen::closure::Closure;
    let button = view
        .document
        .create_element("button")
        .map_err(|error| ToolUiError::native(&error))?;
    button.set_class_name("maestro-excel-tab");
    button.set_text_content(Some(&sheets.values[index].0));
    button
        .set_attribute("type", "button")
        .map_err(|error| ToolUiError::native(&error))?;
    let (sheets, selected) = (Rc::clone(sheets), Rc::clone(selected));
    let click = Closure::<dyn Fn() -> Result<(), wasm_bindgen::JsValue>>::new(move || {
        selected.set(index);
        select(&sheets, index).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
    });
    let event = EventSubscription::new(button.clone().into(), "click", click)
        .map_err(|error| ToolUiError::native(&error))?;
    Ok((button, event))
}
