//! Route witnesses that a real file keeps the spelling its author typed.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::edit_support;
use crate::read_support::{TempDir, call, content_json, input, only_text, text_json};
use crate::runtime::run;
use maestro_tools::{
    EditToolOptions, ReadToolOptions, WriteToolInput, WriteToolOptions,
    create_edit_tool_definition, create_read_tool_definition, create_write_tool_definition,
    edit_diff::{Edit, compute_edits_diff},
};
use serde_json::json;

/// A file named with a no-break space is read under that name, never under its neighbour.
#[test]
fn read_opens_a_file_named_with_a_unicode_space() {
    let dir = TempDir::new();
    dir.file("x\u{a0}y.txt", b"written name");
    dir.file("x y.txt", b"plain neighbour");
    let tool = create_read_tool_definition::<()>(dir.text(), ReadToolOptions::default());
    let written = call(&tool, input("x\u{a0}y.txt", None, None), None, None).unwrap();
    assert_eq!(only_text(&written), "written name");
    let neighbour = call(&tool, input("x y.txt", None, None), None, None).unwrap();
    assert_eq!(only_text(&neighbour), "plain neighbour");
}

/// The edit and preview routes reach the same written name and leave its neighbour.
#[test]
fn edit_and_preview_reach_a_file_named_with_a_unicode_space() {
    let dir = TempDir::new();
    let written = dir.file("x\u{a0}y.txt", b"alpha\nbeta\n");
    let neighbour = dir.file("x y.txt", b"alpha\nneighbour\n");
    let tool = create_edit_tool_definition::<()>(dir.text(), EditToolOptions::default());
    let arguments = edit_support::input("x\u{a0}y.txt", "alpha", "gamma");
    let result = run((tool.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(
        content_json(&result),
        json!([text_json(
            "Successfully replaced 1 block(s) in x\u{a0}y.txt."
        )])
    );
    assert_eq!(std::fs::read(&written).unwrap(), b"gamma\nbeta\n");
    assert_eq!(std::fs::read(&neighbour).unwrap(), b"alpha\nneighbour\n");
    let edits = vec![Edit {
        old_text: "beta".into(),
        new_text: "delta".into(),
    }];
    let preview = compute_edits_diff("x\u{a0}y.txt", edits, &dir.text()).unwrap();
    let preview = run(preview).unwrap();
    assert!(preview.diff.contains("delta"), "{}", preview.diff);
    assert_eq!(std::fs::read(&written).unwrap(), b"gamma\nbeta\n");
}

/// A new file still takes the space-replaced name its author typed.
#[test]
fn write_creates_the_space_replaced_name() {
    let dir = TempDir::new();
    let tool = create_write_tool_definition::<()>(dir.text(), WriteToolOptions::default());
    let arguments = WriteToolInput {
        path: "new\u{a0}file.txt".into(),
        content: "body".into(),
    };
    run((tool.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(std::fs::read(dir.0.join("new file.txt")).unwrap(), b"body");
    assert!(!dir.0.join("new\u{a0}file.txt").exists());
}
