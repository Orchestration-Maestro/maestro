//! Spreadsheet artifacts through their public native platform boundary.
#![cfg(test)]
use crate::artifact_support;
use crate::attachment_support;
use crate::session_header_support;
use maestro_web::{
    ArtifactElement,
    tools::artifacts::{create_artifact_element, excel_artifact::ExcelArtifact},
};
use std::rc::Rc;

#[test]
fn excel_factory_and_fields_follow_current_artifact() {
    let host = Rc::new(session_header_support::Host::default());
    for name in [
        "a.xlsx",
        "a.XLSX",
        "a.xls",
        "a.XLS",
        "a.pdf",
        "a.unknown",
        "a.txt",
        "a.png",
    ] {
        let view = create_artifact_element(name, "", host.clone()).unwrap();
        assert_eq!(view.filename(), name);
        assert_eq!(view.content(), "");
    }
    for name in ["a.html", "a.docx"] {
        assert!(create_artifact_element(name, "", host.clone()).is_err());
    }
    let mut view = ExcelArtifact::new(host);
    assert_eq!((view.filename(), view.content()), ("", ""));
    let frame = artifact_support::Frame::default();
    frame.draw(&mut view, false, vec![]).unwrap();
    view.set_filename("literal 雪.xls".into());
    view.set_content(encoded("two-sheets"));
    assert!(frame.context.has_requested_repaint());
    assert_eq!(
        (view.filename(), view.content()),
        ("literal 雪.xls", encoded("two-sheets").as_str())
    );
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../maestro-app/tests/fixtures/spreadsheet_html/{name}.xlsx",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn encoded(name: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes(name))
}
#[test]
fn excel_preview_replaces_only_changed_successful_content() {
    let host = Rc::new(session_header_support::Host::default());
    let frame = artifact_support::Frame::default();
    let mut view = create_artifact_element("a.xlsx", &encoded("two-sheets"), host.clone()).unwrap();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let first = host.published.borrow().values().next().unwrap().0.clone();
    for content in [encoded("two-sheets"), String::new()] {
        view.set_content(content);
        view.set_filename("literal name.xls".into());
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert!(Rc::ptr_eq(
            &first,
            &host.published.borrow().values().next().unwrap().0
        ));
    }
    view.set_content(encoded("one-sheet"));
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    assert!(!Rc::ptr_eq(
        &first,
        &host.published.borrow().values().next().unwrap().0
    ));
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ByteCase {
    input: String,
    expected: ByteOutcome,
}
#[derive(serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum ByteOutcome {
    Bytes { bytes: Vec<u8> },
    Error { error: String },
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MimeCase {
    filename: String,
    expected: String,
}
#[test]
fn excel_original_download_uses_complete_bytes_and_selected_mime() {
    let frame = artifact_support::Frame::default();
    let rows: Vec<ByteCase> =
        serde_json::from_str(include_str!("fixtures/excel/bytes.json")).unwrap();
    for row in rows {
        let host = Rc::new(session_header_support::Host::default());
        let mut view = create_artifact_element("original.xlsx", &row.input, host.clone()).unwrap();
        match row.expected {
            ByteOutcome::Bytes { bytes } => {
                frame.click(view.as_mut()).unwrap();
                assert_eq!(host.downloads.borrow().last().unwrap().2, bytes);
                if bytes == self::bytes("two-sheets") {
                    frame.draw(view.as_mut(), false, vec![]).unwrap();
                    assert_eq!(host.sizes.borrow().len(), 2);
                }
            }
            ByteOutcome::Error { error } => {
                assert_eq!(error, "base64");
                assert!(frame.click(view.as_mut()).is_err());
                assert!(host.downloads.borrow().is_empty());
            }
        }
    }
    let rows: Vec<MimeCase> =
        serde_json::from_str(include_str!("fixtures/excel/mime.json")).unwrap();
    for row in rows {
        let host = Rc::new(session_header_support::Host::default());
        let mut view = ExcelArtifact::new(host.clone());
        view.set_filename(row.filename.clone());
        view.set_content(encoded("one-sheet"));
        frame.click(&mut view).unwrap();
        assert_eq!(
            host.downloads.borrow()[0],
            (row.filename, row.expected, bytes("one-sheet"))
        );
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureCase {
    file: String,
    expected: String,
}
#[test]
fn excel_preview_error_recovery_keeps_download_independent() {
    let rows: Vec<FailureCase> = serde_json::from_str(include_str!(
        "../../maestro-app/tests/fixtures/spreadsheet_html/failures.json"
    ))
    .unwrap();
    let host = Rc::new(session_header_support::Host::default());
    let frame = artifact_support::Frame::default();
    let mut view = create_artifact_element("a.xlsx", "hello!", host.clone()).unwrap();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    assert!(frame.click(view.as_mut()).is_err());
    for row in rows {
        assert!(matches!(row.expected.as_str(), "error" | "load_error"));
        let name = row.file.strip_suffix(".xlsx").unwrap();
        view.set_content(encoded(name));
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        frame.click(view.as_mut()).unwrap();
        assert_eq!(host.downloads.borrow().last().unwrap().2, bytes(name));
        let failed = host.published.borrow().values().next().unwrap().0.clone();
        view.set_content(String::new());
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert!(!Rc::ptr_eq(
            &failed,
            &host.published.borrow().values().next().unwrap().0
        ));
        view.set_content(encoded("two-sheets"));
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert_eq!(host.sizes.borrow().len(), 2);
        host.fail_download.set(true);
        assert!(frame.click(view.as_mut()).is_err());
        host.fail_download.set(false);
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert_eq!(host.sizes.borrow().len(), 2);
    }
}
#[test]
fn excel_reading_transactions_keep_previous_publication() {
    use session_header_support::Failure;
    let host = Rc::new(session_header_support::Host::default());
    let frame = artifact_support::Frame::default();
    let mut view = create_artifact_element("a.xlsx", &encoded("two-sheets"), host.clone()).unwrap();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let (id, (factory, rect)) = host
        .published
        .borrow()
        .iter()
        .map(|(id, value)| (*id, value.clone()))
        .next()
        .unwrap();
    for phase in [Failure::Measure, Failure::Publish] {
        view.set_content(encoded("one-sheet"));
        host.failure.set(Some(phase));
        assert!(frame.draw(view.as_mut(), false, vec![]).is_err());
        assert_eq!(host.published.borrow()[&id].1, rect);
        assert!(Rc::ptr_eq(&factory, &host.published.borrow()[&id].0));
        host.failure.set(None);
    }
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    assert!(!Rc::ptr_eq(&factory, &host.published.borrow()[&id].0));
    drop(view);
    assert!(host.published.borrow().is_empty());
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn excel_tool_commands_retain_and_update_live_view() {
    use artifact_support::{
        panel::{Harness, Input},
        presentation::ready,
    };
    use serde_json::json;
    let harness = Harness::new(&Input::default());
    let tool = harness.caller.tool();
    let mut id = None;
    for (command, name) in [("create", "two-sheets"), ("rewrite", "one-sheet")] {
        ready((tool.execute)(
            "call".into(),
            json!({"command":command,"filename":"a.xlsx","content":encoded(name)}),
            None,
            None,
        ))
        .unwrap();
        harness.draw().unwrap();
        let current = *harness.host.published.borrow().keys().next().unwrap();
        assert_eq!(*id.get_or_insert(current), current);
        assert_eq!(
            harness.host.sizes.borrow().len(),
            if name == "two-sheets" { 2 } else { 0 }
        );
        assert_eq!(harness.caller.active_filename().as_deref(), Some("a.xlsx"));
    }
    ready((tool.execute)("append".into(),json!({"command":"update","filename":"a.xlsx","old_str":encoded("one-sheet"),"new_str":format!("{}\n",encoded("one-sheet"))}),None,None)).unwrap();
    harness.draw().unwrap();
    assert_eq!(
        *harness.host.published.borrow().keys().next().unwrap(),
        id.unwrap()
    );
    let output = harness.draw().unwrap();
    harness.click(artifact_support::panel::control(&output, 1));
    assert_eq!(
        harness.host.downloads.borrow().last().unwrap().2,
        bytes("one-sheet")
    );
    ready((tool.execute)(
        "delete".into(),
        json!({"command":"delete","filename":"a.xlsx"}),
        None,
        None,
    ))
    .unwrap();
    assert!(harness.host.published.borrow().is_empty());
}

#[test]
fn excel_mixed_workbooks_do_not_latch_workbook_failure() {
    use base64::Engine as _;
    let bytes = include_bytes!("../../maestro-app/tests/fixtures/workbooks/mixed-bad-first.xlsx");
    let content = base64::engine::general_purpose::STANDARD.encode(bytes);
    let host = Rc::new(session_header_support::Host::default());
    host.controls.set(true);
    let frame = artifact_support::Frame::default();
    let mut view = create_artifact_element("mixed.xlsx", &content, host.clone()).unwrap();
    let output = frame.draw(view.as_mut(), false, vec![]).unwrap();
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.job.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, ["Data", "Good"]);
    let center = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == "Good" => {
                Some(t.pos + t.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .unwrap();
    frame
        .draw(view.as_mut(), false, attachment_support::click(center))
        .unwrap();
    assert_eq!(host.sizes.borrow().len(), 2);
    frame.click(view.as_mut()).unwrap();
    assert_eq!(host.downloads.borrow().last().unwrap().2, bytes);
}
