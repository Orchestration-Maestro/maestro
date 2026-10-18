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
) -> Result<ReadingRoot, AttachmentHostError> {
    let root = create(
        document,
        "section",
        if surface.3 {
            "maestro-excel maestro-excel-artifact"
        } else {
            "maestro-excel"
        },
    )?;
    let style = create(document, "style", "")?;
    style.set_text_content(Some(include_str!("../excel.css")));
    root.append_child(&style).map_err(|e| host_error(&e))?;
    let multi = sheets.values.len() > 1;
    let mut slots = Vec::new();
    if multi {
        let strip = create(document, "div", "maestro-excel-tabs")?;
        for (index, _) in sheets.values.iter().enumerate() {
            let slot = create(document, "span", "maestro-excel-tab")?;
            strip.append_child(&slot).map_err(|e| host_error(&e))?;
            slots.push((surface.0.with(index), slot));
        }
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
                let error = super::super::error::mount(document, text, surface.3)?;
                body.append_child(&error.element)
                    .map_err(|e| host_error(&e))?;
            }
        }
        root.append_child(&body).map_err(|e| host_error(&e))?;
        nodes.push(body);
    }
    root.set_attribute("style", &format!("height:{}px;", sheets.height.get()))
        .map_err(|e| host_error(&e))?;
    *sheets.root.borrow_mut() = Some(root.clone());
    *sheets.nodes.borrow_mut() = nodes;
    select(sheets, 0)?;
    Ok(ReadingRoot {
        commit: None,
        owner: None,
        element: root,
        slots,
    })
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
