//! Route witnesses that a real file keeps the spelling its author typed.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::edit_support;
use crate::read_support::{
    Fixture, Scratch, call, content_json, input, isolated, only_text, text_json,
};
use crate::runtime::run;
#[cfg(unix)]
use crate::{find_support, grep_support, ls_support};
use maestro_tools::{
    EditToolOptions, ReadToolOptions, WriteToolInput, WriteToolOptions,
    create_edit_tool_definition, create_read_tool_definition, create_write_tool_definition,
    edit_diff::{Edit, compute_edits_diff},
    path_utils::{resolve_existing_path, resolve_existing_spelling},
};
use serde_json::json;
#[cfg(unix)]
use {
    maestro_tools::{
        FindToolOptions, GrepToolOptions, LsToolOptions, create_find_tool, create_grep_tool,
        create_ls_tool,
    },
    std::sync::{Arc, atomic::AtomicUsize},
};

/// A file named with a no-break space is read under that name, never under its neighbour.
#[test]
fn read_opens_a_file_named_with_a_unicode_space() {
    let dir = Scratch::new().unwrap();
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
    let dir = Scratch::new().unwrap();
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
    let dir = Scratch::new().unwrap();
    let written = dir.file("x\u{a0}y.txt", b"before");
    let tool = create_write_tool_definition::<()>(dir.text(), WriteToolOptions::default());
    let arguments = WriteToolInput {
        path: "x\u{a0}y.txt".into(),
        content: "after".into(),
    };
    run((tool.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(std::fs::read(&written).unwrap(), b"after");
    assert!(!dir.path().join("x y.txt").exists());
}

/// Run `tool` over a recorded program and return the operands it was given.
#[cfg(unix)]
fn recorded(path: &std::path::Path) -> Vec<String> {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// A directory named with a no-break space beside its plain-space twin.
#[cfg(unix)]
fn spaced_directories() -> (Scratch, String) {
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("s\u{a0}dir")).unwrap();
    dir.file("s\u{a0}dir/written.txt", b"");
    std::fs::create_dir(dir.path().join("s dir")).unwrap();
    dir.file("s dir/plain.txt", b"");
    let written = format!("{}/s\u{a0}dir", dir.text());
    (dir, written)
}

/// Listing reaches a directory named with a no-break space, not its neighbour.
#[cfg(unix)]
#[test]
fn ls_reaches_a_directory_named_with_a_unicode_space() {
    let (dir, _) = spaced_directories();
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
}

/// Searching with `find` reaches a directory named with a no-break space, not its neighbour.
#[cfg(unix)]
#[test]
fn find_reaches_a_directory_named_with_a_unicode_space() {
    let (dir, written) = spaced_directories();
    let bin = Scratch::new().unwrap();
    let operands = find_support::recorder(&bin);
    let manager = find_support::manager(bin.text(), &Arc::new(AtomicUsize::new(0)));
    let tool = create_find_tool(dir.text(), FindToolOptions::default(), manager);
    run((tool.execute)(
        "call".into(),
        json!({"pattern": "*", "path": "s\u{a0}dir"}),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(
        recorded(&operands).last().map(String::as_str),
        Some(written.as_str())
    );
}

/// Searching with `grep` reaches a directory named with a no-break space, not its neighbour.
#[cfg(unix)]
#[test]
fn grep_reaches_a_directory_named_with_a_unicode_space() {
    let (dir, written) = spaced_directories();
    let bin = Scratch::new().unwrap();
    grep_support::producer(&bin, &grep_support::Input::default());
    let manager = find_support::manager(bin.text(), &Arc::new(AtomicUsize::new(0)));
    let tool = create_grep_tool(dir.text(), GrepToolOptions::default(), manager);
    run((tool.execute)(
        "call".into(),
        json!({"pattern": "hit", "path": "s\u{a0}dir"}),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(
        recorded(&bin.path().join("args.json"))
            .last()
            .map(String::as_str),
        Some(written.as_str())
    );
}

/// A new file lands inside the directory the file system holds under its written spelling.
#[test]
fn write_creates_a_new_file_inside_a_directory_named_with_a_unicode_space() {
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("s\u{a0}dir")).unwrap();
    let tool = create_write_tool_definition::<()>(dir.text(), WriteToolOptions::default());
    let arguments = WriteToolInput {
        path: "s\u{a0}dir/new.txt".into(),
        content: "body".into(),
    };
    run((tool.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("s\u{a0}dir/new.txt")).unwrap(),
        b"body"
    );
    assert!(!dir.path().join("s dir").exists());
}

/// A written directory beside its plain-space twin is the one that receives the new file.
#[test]
fn write_leaves_the_plain_neighbour_directory_untouched() {
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("s\u{a0}dir")).unwrap();
    std::fs::create_dir(dir.path().join("s dir")).unwrap();
    let tool = create_write_tool_definition::<()>(dir.text(), WriteToolOptions::default());
    let arguments = WriteToolInput {
        path: "s\u{a0}dir/new.txt".into(),
        content: "body".into(),
    };
    run((tool.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("s\u{a0}dir/new.txt")).unwrap(),
        b"body"
    );
    assert!(!dir.path().join("s dir/new.txt").exists());
}

/// Every directory on disk keeps the spelling the path was written with.
#[test]
fn path_resolution_keeps_every_existing_directory_spelling() {
    let test = "path_resolution_keeps_every_existing_directory_spelling";
    let dir = Scratch::new().unwrap();
    std::fs::create_dir_all(dir.path().join("a\u{a0}b/c\u{a0}d")).unwrap();
    std::fs::create_dir(dir.path().join("s\u{a0}dir")).unwrap();
    dir.file("s\u{a0}dir/existing.txt", b"written");
    let cwd = dir.text();
    let written = format!("{cwd}/s\u{a0}dir");
    let nested = format!("{cwd}/a\u{a0}b/c\u{a0}d/new.txt");
    for (ask, expected) in [
        ("a\u{a0}b/c\u{a0}d/new.txt".to_owned(), nested),
        (
            "@s\u{a0}dir/new.txt".to_owned(),
            format!("{written}/new.txt"),
        ),
        (format!("{written}/new.txt"), format!("{written}/new.txt")),
        ("s\u{a0}dir/".to_owned(), written.clone()),
        (
            "s\u{a0}dir/existing.txt".to_owned(),
            format!("{written}/existing.txt"),
        ),
    ] {
        assert_eq!(
            resolve_existing_path(&ask, &cwd).unwrap(),
            expected,
            "{ask:?}"
        );
    }
    let home = Scratch::new().unwrap();
    std::fs::create_dir(home.path().join("h\u{a0}d")).unwrap();
    let home_text = home.text();
    let envs = [
        ("HOME", Some(home_text.as_str())),
        ("USERPROFILE", Some(home_text.as_str())),
    ];
    if isolated(test, "home-written", &envs) {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(
            resolve_existing_path("~/h\u{a0}d/new.txt", &cwd).unwrap(),
            format!("{home}/h\u{a0}d/new.txt")
        );
    }
}

/// A part of the path nothing on disk holds still takes the space-replaced spelling.
#[test]
fn path_resolution_normalizes_only_the_part_without_a_counterpart() {
    let test = "path_resolution_normalizes_only_the_part_without_a_counterpart";
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("p q")).unwrap();
    std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
    std::fs::create_dir(dir.path().join("a\u{a0}b")).unwrap();
    dir.file("s\u{a0}dir", b"a file holds the written spelling");
    let cwd = dir.text();
    for (ask, expected) in [
        ("s\u{a0}dir/new.txt", "s dir/new.txt"),
        ("p\u{a0}q/new.txt", "p q/new.txt"),
        ("a/b/c\u{a0}d/new.txt", "a/b/c d/new.txt"),
        ("nothing\u{a0}here/new.txt", "nothing here/new.txt"),
        ("d/x y.txt", "d/x y.txt"),
    ] {
        assert_eq!(
            resolve_existing_path(ask, &cwd).unwrap(),
            format!("{cwd}/{expected}"),
            "{ask:?}"
        );
    }
    assert_eq!(
        resolve_existing_path("a\u{a0}b/c\u{a0}d/new.txt", &cwd).unwrap(),
        format!("{cwd}/a\u{a0}b/c d/new.txt")
    );
    #[cfg(unix)]
    if isolated(test, "no-existing-directory", &[]) {
        let parent = Scratch::new().unwrap();
        parent.enter_removed("gone").unwrap();
        assert_eq!(
            resolve_existing_spelling("up\u{a0}down/new.txt", |path| Ok(path.to_owned())).unwrap(),
            "up down/new.txt"
        );
    }
}

/// A new file still takes the space-replaced name its author typed.
#[test]
fn write_creates_the_space_replaced_name() {
    let dir = Scratch::new().unwrap();
    let tool = create_write_tool_definition::<()>(dir.text(), WriteToolOptions::default());
    let arguments = WriteToolInput {
        path: "new\u{a0}file.txt".into(),
        content: "body".into(),
    };
    run((tool.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("new file.txt")).unwrap(),
        b"body"
    );
    assert!(!dir.path().join("new\u{a0}file.txt").exists());
}
