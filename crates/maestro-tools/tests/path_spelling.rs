//! Route witnesses that a real file keeps the spelling its author typed.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::edit_support;
use crate::read_support::{TempDir, call, content_json, input, only_text, text_json};
use crate::runtime::run;
#[cfg(unix)]
use crate::{find_support, ls_support};
use maestro_tools::{
    EditToolOptions, ReadToolOptions, WriteToolInput, WriteToolOptions,
    create_edit_tool_definition, create_read_tool_definition, create_write_tool_definition,
    edit_diff::{Edit, compute_edits_diff},
};
use serde_json::json;
#[cfg(unix)]
use {
    maestro_agent::AgentTool,
    maestro_tools::{
        FindToolOptions, GrepToolOptions, LsToolOptions, ToolsManager, create_find_tool,
        create_grep_tool, create_ls_tool,
    },
    std::sync::{Arc, atomic::AtomicUsize},
};

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

/// Rewriting a file named with a no-break space changes that file and creates no neighbour.
#[test]
fn write_rewrites_a_file_named_with_a_unicode_space() {
    let dir = TempDir::new();
    let written = dir.file("x\u{a0}y.txt", b"before");
    let tool = create_write_tool_definition::<()>(dir.text(), WriteToolOptions::default());
    let arguments = WriteToolInput {
        path: "x\u{a0}y.txt".into(),
        content: "after".into(),
    };
    run((tool.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(std::fs::read(&written).unwrap(), b"after");
    assert!(!dir.0.join("x y.txt").exists());
}

/// Run `tool` over a program that records its operands, and return them.
#[cfg(unix)]
fn recorded_operands(
    program: &str,
    tool: impl FnOnce(Arc<ToolsManager>) -> AgentTool,
    params: serde_json::Value,
) -> Vec<String> {
    let bin = TempDir::new();
    let operands = bin.0.join("operands.json");
    let body = format!(
        "#!/usr/bin/python3\nimport sys,json\njson.dump(sys.argv[1:],open({},'w'))\n",
        json!(operands.to_str().unwrap())
    );
    find_support::executable(&bin, program, &body);
    let manager = find_support::manager(bin.text(), &Arc::new(AtomicUsize::new(0)));
    let tool = tool(manager);
    run((tool.execute)("call".into(), params, None, None)).unwrap();
    serde_json::from_slice(&std::fs::read(&operands).unwrap()).unwrap()
}

/// Listing and searching reach a directory named with a no-break space, not its neighbour.
#[cfg(unix)]
#[test]
fn search_roots_reach_a_directory_named_with_a_unicode_space() {
    let dir = TempDir::new();
    std::fs::create_dir(dir.0.join("s\u{a0}dir")).unwrap();
    dir.file("s\u{a0}dir/written.txt", b"");
    std::fs::create_dir(dir.0.join("s dir")).unwrap();
    dir.file("s dir/plain.txt", b"");
    let written_root = format!("{}/s\u{a0}dir", dir.text());
    let ls = create_ls_tool(dir.text(), LsToolOptions::default());
    let listed = run((ls.execute)(
        "call".into(),
        json!({"path": "s\u{a0}dir"}),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(
        ls_support::result_json(&listed.content, listed.details)["content"][0]["text"],
        json!("written.txt")
    );
    let found = recorded_operands(
        "fd",
        |manager| create_find_tool(dir.text(), FindToolOptions::default(), manager),
        json!({"pattern": "*", "path": "s\u{a0}dir"}),
    );
    assert_eq!(
        found.last().map(String::as_str),
        Some(written_root.as_str())
    );
    let searched = recorded_operands(
        "rg",
        |manager| create_grep_tool(dir.text(), GrepToolOptions::default(), manager),
        json!({"pattern": "hit", "path": "s\u{a0}dir"}),
    );
    assert_eq!(
        searched.last().map(String::as_str),
        Some(written_root.as_str())
    );
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
