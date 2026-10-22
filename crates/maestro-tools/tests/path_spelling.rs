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
    path_utils::{resolve_existing_path, resolve_to_cwd},
};
use serde_json::json;
#[cfg(unix)]
use {
    maestro_tools::{
        FindToolOptions, GrepToolOptions, LsToolOptions, create_find_tool, create_grep_tool,
        create_ls_tool, path_utils::resolve_existing_spelling,
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

/// Write `content` to `path` through the write tool rooted at `dir`.
fn write_through_tool(dir: &Scratch, path: &str, content: &str) {
    let tool = create_write_tool_definition::<()>(dir.text(), WriteToolOptions::default());
    let arguments = WriteToolInput {
        path: path.into(),
        content: content.into(),
    };
    run((tool.execute)("call".into(), arguments, None, None, None)).unwrap();
}

/// Rewriting a file named with a no-break space changes that file and creates no neighbour.
#[test]
fn write_rewrites_a_file_named_with_a_unicode_space() {
    let dir = Scratch::new().unwrap();
    let written = dir.file("x\u{a0}y.txt", b"before");
    write_through_tool(&dir, "x\u{a0}y.txt", "after");
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
    write_through_tool(&dir, "s\u{a0}dir/new.txt", "body");
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
    write_through_tool(&dir, "s\u{a0}dir/new.txt", "body");
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
    #[cfg(unix)]
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
    write_through_tool(&dir, "new\u{a0}file.txt", "body");
    assert_eq!(
        std::fs::read(dir.path().join("new file.txt")).unwrap(),
        b"body"
    );
    assert!(!dir.path().join("new\u{a0}file.txt").exists());
}

/// A file only its plain spelling holds is read and edited under the written directory.
#[test]
fn existing_plain_space_file_is_reached_beside_a_written_directory() {
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("s\u{a0}dir")).unwrap();
    std::fs::create_dir(dir.path().join("s dir")).unwrap();
    let plain = dir.file("s dir/x.txt", b"alpha\nbeta\n");
    let read = create_read_tool_definition::<()>(dir.text(), ReadToolOptions::default());
    let opened = call(&read, input("s\u{a0}dir/x.txt", None, None), None, None).unwrap();
    assert_eq!(only_text(&opened), "alpha\nbeta\n");
    let edit = create_edit_tool_definition::<()>(dir.text(), EditToolOptions::default());
    let arguments = edit_support::input("s\u{a0}dir/x.txt", "beta", "gamma");
    run((edit.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(std::fs::read(&plain).unwrap(), b"alpha\ngamma\n");
    assert!(!dir.path().join("s\u{a0}dir/x.txt").exists());
}

/// A path nothing on disk holds resolves exactly as the space-replaced whole path does.
#[test]
fn unmatched_paths_keep_the_space_replaced_text() {
    let dir = Scratch::new().unwrap();
    let cwd = dir.text();
    let absolute = format!("{cwd}/x\u{a0}y");
    let mismatched: Vec<_> = [
        absolute.clone(),
        format!("{absolute}/"),
        format!("{cwd}/x\u{a0}y//deep/new.txt"),
        format!("{cwd}/./x\u{a0}y/new.txt"),
        "~/h\u{a0}d/new.txt".to_owned(),
        "x\u{a0}y/".to_owned(),
    ]
    .into_iter()
    .filter_map(|ask| {
        let written = resolve_existing_path(&ask, &cwd).unwrap();
        let normalized = resolve_to_cwd(&ask, &cwd).unwrap();
        (written != normalized).then_some((ask, normalized, written))
    })
    .collect();
    assert!(mismatched.is_empty(), "{mismatched:?}");
}
