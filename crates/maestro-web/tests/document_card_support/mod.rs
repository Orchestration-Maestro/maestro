//! Recorded document operands and the shared native containing receiver.
#![cfg(test)]
use crate::{
    attachment_support::{Frame, Host, click},
    support::tool_input::Input,
};
use maestro_web::{
    Attachment,
    tools::types::{
        ToolContentOutput, ToolRenderResult, ToolRenderState, ToolRenderView, ToolUiError,
    },
};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub group: String,
    pub tool: String,
    pub test: Option<String>,
    pub input: Input,
    pub expected: Option<Expected>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub caption: String,
    pub status: String,
    pub spinner: usize,
    pub collapsible: bool,
    pub url: Option<String>,
    pub blocks: Vec<Block>,
    pub console: Vec<crate::document_card_support::Console>,
    pub attachments: Vec<DownloadExpectation>,
}
/// Expected downloadable fields; generated identity is intentionally not projected.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DownloadExpectation {
    pub r#type: maestro_app::browser::utils::attachment_utils::AttachmentType,
    pub file_name: String,
    pub mime_type: String,
    pub size: usize,
    pub content: String,
    pub preview: Option<String>,
    pub extracted_text: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub code: String,
    pub language: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Console {
    pub content: String,
    pub variant: String,
}
pub fn fixture() -> Vec<Row> {
    serde_json::from_str(include_str!("../fixtures/document_tool_cards.json")).unwrap()
}
pub fn prepare(row: &Row) -> Result<ToolRenderResult, ToolUiError> {
    maestro_web::render_tool(
        name(&row.tool),
        row.input
            .params
            .as_ref()
            .map(maestro_web::tools::types::ToolParams::object),
        row.input.result.as_ref(),
        row.input.is_streaming,
    )
}
pub fn name(tool: &str) -> &'static str {
    match tool {
        "repl" => "javascript_repl",
        "extract" => "extract_document",
        _ => panic!("unknown document tool"),
    }
}
pub fn frame_owner(
    frame: &mut Frame,
    host: &Rc<Host>,
    events: Vec<egui::Event>,
    draw: impl FnOnce(&mut egui::Ui, ToolRenderView<'_>) -> ToolContentOutput,
) -> ToolContentOutput {
    frame.run(events, |ctx, i18n, presentation| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("document-owner"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        let host: Rc<dyn maestro_web::browser::attachment_content::AttachmentHost> = host.clone();
        draw(
            &mut ui,
            ToolRenderView {
                host: &host,
                i18n,
                presentation,
                highlighter: &maestro_theme::SyntectHighlighter::new().unwrap(),
                modal: None,
            },
        )
    })
}
pub fn downloads(row: &Row) -> Vec<Attachment> {
    let expected = &row.expected.as_ref().unwrap().attachments;
    let host = Rc::new(Host::default());
    let mut frame = Frame::default();
    let recipe = prepare(row).unwrap();
    let mut retained = ToolRenderState::default();
    let first = frame_owner(&mut frame, &host, vec![], |ui, view| {
        recipe.render(ui, view, &mut retained)
    });
    first.response.unwrap();
    assert!(first.overlays.is_empty());
    for index in 0..expected.len() {
        let at = tile_position(&frame, &host, expected, index);
        assert!(
            frame.screen.contains(at),
            "file {index} pointer outside screen: {at:?}"
        );
        let mut output = frame_owner(&mut frame, &host, click(at), |ui, view| {
            recipe.render(ui, view, &mut retained)
        });
        output.response.unwrap();
        assert_eq!(output.overlays.len(), 1, "{} file {index}", row.id);
        let mut overlay = output.overlays.pop().unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        let download = frame.control(1);
        frame.overlay(&mut overlay, click(download)).unwrap();
        frame.overlay(&mut overlay, vec![escape()]).unwrap();
        frame_owner(&mut frame, &host, vec![], |ui, view| {
            recipe.render(ui, view, &mut retained)
        })
        .response
        .unwrap();
    }
    assert_eq!(host.downloads.borrow().len(), expected.len());
    host.downloads
        .borrow()
        .iter()
        .map(|a| a.borrow().clone())
        .collect()
}
pub fn escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
pub fn assert_records(actual: &[Attachment], expected: &[DownloadExpectation]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.r#type, expected.r#type);
        assert_eq!(actual.file_name, expected.file_name);
        assert_eq!(actual.mime_type, expected.mime_type);
        assert_eq!(actual.size, expected.size);
        assert_eq!(actual.content, expected.content);
        assert_eq!(actual.preview, expected.preview);
        assert_eq!(actual.extracted_text, expected.extracted_text);
    }
}

pub mod forwarding;

#[cfg(target_arch = "wasm32")]
pub mod browser;

/// Target each real published image or painted filename, preserving duplicates by occurrence.
fn tile_position(
    frame: &Frame,
    host: &Rc<Host>,
    records: &[DownloadExpectation],
    index: usize,
) -> egui::Pos2 {
    let record = &records[index];
    let preview = record.preview.as_ref().is_some_and(|p| !p.is_empty());
    let preceding = records[..index]
        .iter()
        .filter(|a| a.file_name == record.file_name)
        .count();
    if preview {
        return host
            .surfaces
            .borrow()
            .iter()
            .filter_map(|surface| match &surface.content {
                crate::attachment_support::Content::Image(_, name, _)
                    if name == &record.file_name =>
                {
                    Some(surface.rect.center())
                }
                _ => None,
            })
            .next_back()
            .unwrap();
    }
    let label = if record.file_name.encode_utf16().count() > 10 {
        format!("{}...", maestro_app::truncate_utf16(&record.file_name, 8))
    } else {
        record.file_name.clone()
    };
    frame
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .nth(preceding)
        .expect("file label was painted")
}

/// Select distinct file-list inputs while retaining each recorded file's independent oracle.
pub fn file_subset(indices: &[usize]) -> Row {
    let mut row = fixture()
        .into_iter()
        .find(|r| r.id == "repl-files")
        .unwrap();
    let result = row.input.result.as_mut().unwrap();
    let files = result.details.as_ref().unwrap()["files"]
        .as_array()
        .unwrap();
    let selected = indices.iter().map(|i| files[*i].clone()).collect();
    result.details.as_mut().unwrap()["files"] = serde_json::Value::Array(selected);
    let expected = &mut row.expected.as_mut().unwrap().attachments;
    *expected = indices.iter().map(|i| expected[*i].clone()).collect();
    row
}
#[cfg(target_arch = "wasm32")]
pub mod lifecycle;

#[cfg(target_arch = "wasm32")]
pub mod overlays;
#[cfg(target_arch = "wasm32")]
pub mod styles;

/// Observe an opened original while a retained tile is rebound to new returned bytes.
pub fn changed_file_keeps_opened_original() {
    let mut row = file_subset(&[1]);
    let host = Rc::new(Host::default());
    let mut frame = Frame::default();
    let mut state = ToolRenderState::default();
    let recipe = prepare(&row).unwrap();
    let output = frame_owner(&mut frame, &host, vec![], |ui, view| {
        recipe.render(ui, view, &mut state)
    });
    output.response.unwrap();
    assert!(output.overlays.is_empty());
    let at = tile_position(
        &frame,
        &host,
        &row.expected.as_ref().unwrap().attachments,
        0,
    );
    let output = frame_owner(&mut frame, &host, click(at), |ui, view| {
        recipe.render(ui, view, &mut state)
    });
    let mut receiver = output.overlays;
    output.response.unwrap();
    assert_eq!(receiver.len(), 1);
    row.input.result.as_mut().unwrap().details.as_mut().unwrap()["files"][0]["contentBase64"] =
        serde_json::json!("TmV3");
    let changed = prepare(&row).unwrap();
    let output = frame_owner(&mut frame, &host, vec![], |ui, view| {
        changed.render(ui, view, &mut state)
    });
    output.response.unwrap();
    assert!(output.overlays.is_empty());
    let output = frame_owner(&mut frame, &host, click(at), |ui, view| {
        changed.render(ui, view, &mut state)
    });
    receiver.extend(output.overlays);
    output.response.unwrap();
    assert_eq!(receiver.len(), 2);
    drop(state);
    for overlay in &mut receiver {
        frame.overlay(overlay, vec![]).unwrap();
        frame.overlay(overlay, vec![]).unwrap();
        let at = frame.control(1);
        frame.overlay(overlay, click(at)).unwrap();
        frame.overlay(overlay, vec![escape()]).unwrap();
    }
    assert_eq!(
        host.downloads.borrow()[0].borrow().content,
        "6Zuq",
        "opened original was rewritten by a later tool render"
    );
    assert_eq!(host.downloads.borrow()[1].borrow().content, "TmV3");
}
