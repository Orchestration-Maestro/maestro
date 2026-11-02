//! UTF-8 writes and parent creation through the application's executable tool factory.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::{
    run_io,
    session_harness::{as_recorded, case},
};
use serde_json::json;

/// Writes through the root export and checks both the result and native file bytes.
fn check_write(name: &str, path: &str, content: &str) {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let file = root.path().join(path);
    let tool = maestro_app::create_write_tool(
        root.path().display().to_string(),
        maestro_app::WriteToolOptions::default(),
    );
    let result = run_io((tool.execute)(
        "write".into(),
        json!({"path":file.display().to_string(),"content":content}),
        None,
        None,
    ))
    .unwrap();
    let observed = json!({"ok":result,"after":{"bytes":std::fs::read(file).unwrap()}});
    assert_eq!(
        as_recorded(observed, &[(&root.path().display().to_string(), "$ROOT")]),
        case("tools_root", name)["expected"]
    );
}

#[test]
fn write_reports_utf8_bytes_and_preserves_content() {
    for (suffix, path, content) in [
        ("", "write-test.txt", "Test content"),
        ("/empty", "empty.txt", ""),
        ("/euro", "euro.txt", "€"),
        ("/emoji", "emoji.txt", "😀"),
    ] {
        check_write(
            &format!("write_reports_utf8_bytes_and_preserves_content{suffix}"),
            path,
            content,
        );
    }
}

#[test]
fn write_creates_missing_parent_directories() {
    check_write(
        "write_creates_missing_parent_directories",
        "nested/dir/test.txt",
        "Nested content",
    );
}
