//! File reads through the application's executable tool factory.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::{run_io, session_harness::case};
use base64::{Engine, engine::general_purpose::STANDARD};
use maestro_models::DiagnosticErrorInfo;
use serde_json::{Value, json};

/// Reads a disposable file through the application export, preserving the complete result.
fn read(bytes: Option<&[u8]>, arguments: Value) -> Result<Value, DiagnosticErrorInfo> {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = arguments["path"].as_str().unwrap();
    if let Some(bytes) = bytes {
        std::fs::write(root.path().join(path), bytes).unwrap();
    }
    let tool = maestro_app::create_read_tool(
        root.path().display().to_string(),
        maestro_app::ReadToolOptions::default(),
    );
    run_io((tool.execute)("read".into(), arguments, None, None))
        .map(|result| serde_json::to_value(result).unwrap())
}

/// The complete recorded result for one read.
fn expected(name: &str) -> Value {
    case("tools_root", name)["expected"]["ok"].clone()
}

/// Numbered lines without a final newline.
fn lines(count: usize) -> String {
    (1..=count)
        .map(|n| format!("Line {n}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn read_returns_complete_text_without_details() {
    let result = read(
        Some(b"Hello, world!\nLine 2\nLine 3"),
        json!({"path":"test.txt"}),
    );
    assert_eq!(
        result.unwrap(),
        expected("read_returns_complete_text_without_details")
    );
}

#[test]
fn read_reports_missing_file() {
    let error = read(None, json!({"path":"nonexistent.txt"})).unwrap_err();
    assert_eq!(
        error.code,
        Some(maestro_models::DiagnosticCode::Number(2.0))
    );
    assert_eq!(
        error.message,
        std::io::Error::from_raw_os_error(2).to_string()
    );
}

#[test]
fn read_clips_after_two_thousand_lines() {
    let result = read(Some(lines(2500).as_bytes()), json!({"path":"large.txt"}));
    assert_eq!(
        result.unwrap()["content"],
        expected("read_clips_after_two_thousand_lines")["content"]
    );
}

#[test]
fn read_clips_at_fifty_kibibytes() {
    let content = (1..=500)
        .map(|n| format!("Line {n}: {}", "x".repeat(200)))
        .collect::<Vec<_>>()
        .join("\n");
    let result = read(Some(content.as_bytes()), json!({"path":"large-bytes.txt"}));
    assert_eq!(result.unwrap(), expected("read_clips_at_fifty_kibibytes"));
}

#[test]
fn read_offset_starts_at_requested_line() {
    let result = read(
        Some(lines(100).as_bytes()),
        json!({"path":"offset.txt","offset":51}),
    );
    assert_eq!(
        result.unwrap(),
        expected("read_offset_starts_at_requested_line")
    );
}

#[test]
fn read_limit_reports_remaining_lines() {
    let result = read(
        Some(lines(100).as_bytes()),
        json!({"path":"limit.txt","limit":10}),
    );
    assert_eq!(
        result.unwrap(),
        expected("read_limit_reports_remaining_lines")
    );
}

#[test]
fn read_combines_offset_and_limit() {
    let result = read(
        Some(lines(100).as_bytes()),
        json!({"path":"both.txt","offset":41,"limit":20}),
    );
    assert_eq!(result.unwrap(), expected("read_combines_offset_and_limit"));
}

#[test]
fn read_rejects_offset_past_eof() {
    let error = read(
        Some(lines(3).as_bytes()),
        json!({"path":"short.txt","offset":100}),
    )
    .unwrap_err();
    assert!(
        error
            .message
            .contains("Offset 100 is beyond end of file (3 lines total)"),
        "{error:?}"
    );
}

#[test]
fn read_returns_line_truncation_details() {
    let result = read(Some(lines(2500).as_bytes()), json!({"path":"large.txt"}));
    assert_eq!(
        result.unwrap()["details"],
        expected("read_returns_line_truncation_details")["details"]
    );
}

#[test]
fn read_recognizes_png_bytes_without_image_suffix() {
    let bytes = STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==").unwrap();
    let result = read(Some(&bytes), json!({"path":"image.txt"}));
    assert_eq!(
        result.unwrap(),
        expected("read_recognizes_png_bytes_without_image_suffix")
    );
}

#[test]
fn read_keeps_nonimage_png_text() {
    let result = read(
        Some(b"definitely not a png"),
        json!({"path":"not-an-image.png"}),
    );
    assert_eq!(result.unwrap(), expected("read_keeps_nonimage_png_text"));
}
