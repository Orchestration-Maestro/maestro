//! Route witnesses that a real file keeps the spelling its author typed.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::edit_support;
use crate::ls_support::posix_spelling;
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
        create_ls_tool,
        path_utils::{resolve_existing_spelling, resolve_read_path},
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

/// Write `content` to `path` through the write tool rooted at the working directory `cwd`.
fn write_through_tool(cwd: &str, path: &str, content: &str) {
    let tool = create_write_tool_definition::<()>(cwd.to_owned(), WriteToolOptions::default());
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
    write_through_tool(&dir.text(), "x\u{a0}y.txt", "after");
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
    assert_eq!(listing_shows(&dir, "s\u{a0}dir"), "written.txt");
}

/// Searching with `find` reaches a directory named with a no-break space, not its neighbour.
#[cfg(unix)]
#[test]
fn find_reaches_a_directory_named_with_a_unicode_space() {
    let (dir, written) = spaced_directories();
    assert_eq!(find_reaches(&dir, "s\u{a0}dir"), written);
}

/// Searching with `grep` reaches a directory named with a no-break space, not its neighbour.
#[cfg(unix)]
#[test]
fn grep_reaches_a_directory_named_with_a_unicode_space() {
    let (dir, written) = spaced_directories();
    assert_eq!(grep_reaches(&dir, "s\u{a0}dir"), written);
}

/// A new file lands inside the directory the file system holds under its written spelling.
#[test]
fn write_creates_a_new_file_inside_a_directory_named_with_a_unicode_space() {
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("s\u{a0}dir")).unwrap();
    write_through_tool(&dir.text(), "s\u{a0}dir/new.txt", "body");
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
    write_through_tool(&dir.text(), "s\u{a0}dir/new.txt", "body");
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
            posix_spelling(&resolve_existing_path(&ask, &cwd).unwrap()),
            posix_spelling(&expected),
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

/// A deeper plain-space directory on disk still receives a new file authored above it.
#[test]
fn write_follows_the_deeper_directory_the_file_system_holds() {
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("a\u{a0}b")).unwrap();
    std::fs::create_dir_all(dir.path().join("a b/c d")).unwrap();
    write_through_tool(&dir.text(), "a\u{a0}b/c\u{a0}d/new.txt", "body");
    assert_eq!(
        std::fs::read(dir.path().join("a b/c d/new.txt")).unwrap(),
        b"body"
    );
    assert!(!dir.path().join("a\u{a0}b/c d").exists());
}

/// Every route reaches the file the write route created, whatever spelling its directory has.
#[cfg(unix)]
#[test]
fn every_route_reaches_the_file_the_write_route_created() {
    for (directories, authored, asked, created) in [
        (
            vec!["s\u{a0}dir"],
            "s\u{a0}dir",
            "new\u{a0}file.txt",
            "s\u{a0}dir/new file.txt",
        ),
        (
            vec!["s dir"],
            "s\u{a0}dir",
            "new\u{a0}file.txt",
            "s dir/new file.txt",
        ),
        (
            vec!["s\u{a0}dir", "s dir"],
            "s\u{a0}dir",
            "new\u{a0}file.txt",
            "s\u{a0}dir/new file.txt",
        ),
        (
            vec![],
            "s\u{a0}dir",
            "new\u{a0}file.txt",
            "s dir/new file.txt",
        ),
        (
            vec!["a\u{a0}b"],
            "a\u{a0}b/c\u{a0}d",
            "new.txt",
            "a\u{a0}b/c d/new.txt",
        ),
    ] {
        let dir = Scratch::new().unwrap();
        for directory in directories {
            std::fs::create_dir_all(dir.path().join(directory)).unwrap();
        }
        let path = format!("{}/{created}", dir.text());
        let ask = format!("{authored}/{asked}");
        write_then_rewrite(&dir, &ask, &path);
        let (directory, name) = path.rsplit_once('/').unwrap();
        assert_eq!(listing_shows(&dir, authored), name, "{authored:?}");
        assert_eq!(find_reaches(&dir, authored), directory, "{authored:?}");
        assert_eq!(grep_reaches(&dir, authored), directory, "{authored:?}");
    }
}

/// Write `ask`, then read and edit the file it must create at `path`.
#[cfg(unix)]
fn write_then_rewrite(dir: &Scratch, ask: &str, path: &str) {
    write_through_tool(&dir.text(), ask, "body");
    assert_eq!(std::fs::read(path).unwrap(), b"body", "{ask:?}");
    let read = create_read_tool_definition::<()>(dir.text(), ReadToolOptions::default());
    let opened = call(&read, input(ask, None, None), None, None).unwrap();
    assert_eq!(only_text(&opened), "body", "{ask:?}");
    let edit = create_edit_tool_definition::<()>(dir.text(), EditToolOptions::default());
    let arguments = edit_support::input(ask, "body", "edited");
    run((edit.execute)("call".into(), arguments, None, None, None)).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"edited", "{ask:?}");
}

/// The one entry `ls` lists for the authored directory.
#[cfg(unix)]
fn listing_shows(dir: &Scratch, authored: &str) -> String {
    let ls = create_ls_tool(dir.text(), LsToolOptions::default());
    let listed = run((ls.execute)(
        "call".into(),
        json!({"path": authored}),
        None,
        None,
    ))
    .unwrap();
    let listed = ls_support::result_json(&listed.content, listed.details);
    listed["content"][0]["text"].as_str().unwrap().to_owned()
}

/// The directory the controlled `find` program was run over.
#[cfg(unix)]
fn find_reaches(dir: &Scratch, authored: &str) -> String {
    let bin = Scratch::new().unwrap();
    let operands = find_support::recorder(&bin);
    let counted = Arc::new(AtomicUsize::new(0));
    let manager = find_support::manager(bin.text(), &counted);
    let find = create_find_tool(dir.text(), FindToolOptions::default(), manager);
    run((find.execute)(
        "call".into(),
        json!({"pattern": "*", "path": authored}),
        None,
        None,
    ))
    .unwrap();
    recorded(&operands).last().cloned().unwrap_or_default()
}

/// The directory the controlled `grep` program was run over.
#[cfg(unix)]
fn grep_reaches(dir: &Scratch, authored: &str) -> String {
    let bin = Scratch::new().unwrap();
    grep_support::producer(&bin, &grep_support::Input::default());
    let manager = find_support::manager(bin.text(), &Arc::new(AtomicUsize::new(0)));
    let grep = create_grep_tool(dir.text(), GrepToolOptions::default(), manager);
    run((grep.execute)(
        "call".into(),
        json!({"pattern": "hit", "path": authored}),
        None,
        None,
    ))
    .unwrap();
    recorded(&bin.path().join("args.json"))
        .last()
        .cloned()
        .unwrap_or_default()
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
            posix_spelling(&resolve_existing_path(ask, &cwd).unwrap()),
            posix_spelling(&format!("{cwd}/{expected}")),
            "{ask:?}"
        );
    }
    assert_eq!(
        posix_spelling(&resolve_existing_path("a\u{a0}b/c\u{a0}d/new.txt", &cwd).unwrap()),
        posix_spelling(&format!("{cwd}/a\u{a0}b/c d/new.txt"))
    );
    #[cfg(unix)]
    if isolated(test, "no-existing-directory", &[]) {
        let parent = Scratch::new().unwrap();
        parent.enter_removed("gone").unwrap();
        assert_eq!(
            resolve_existing_spelling(
                "up\u{a0}down/new.txt",
                |path| Ok(path.to_owned()),
                |path| Ok(path.to_owned()),
            )
            .unwrap(),
            "up down/new.txt"
        );
    }
}

/// A new file still takes the space-replaced name its author typed.
#[test]
fn write_creates_the_space_replaced_name() {
    let dir = Scratch::new().unwrap();
    write_through_tool(&dir.text(), "new\u{a0}file.txt", "body");
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

/// A screenshot name only the plain-space twin holds opens under the written directory.
#[cfg(unix)]
#[test]
fn read_reaches_a_screenshot_in_the_plain_twin() {
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("s\u{a0}dir")).unwrap();
    std::fs::create_dir(dir.path().join("s dir")).unwrap();
    dir.file("s dir/Shot 10.00\u{202F}AM.txt", b"captured");
    let read = create_read_tool_definition::<()>(dir.text(), ReadToolOptions::default());
    let opened = call(
        &read,
        input("s\u{a0}dir/Shot 10.00 AM.txt", None, None),
        None,
        None,
    )
    .unwrap();
    assert_eq!(only_text(&opened), "captured");
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

/// Every route reaches the directory a `~` path's written parent segment names.
#[cfg(unix)]
#[test]
fn tilde_routes_follow_the_link_their_parent_segment_names() {
    let test = "tilde_routes_follow_the_link_their_parent_segment_names";
    let farm = Scratch::new().unwrap();
    std::fs::create_dir_all(farm.path().join("elsewhere/child")).unwrap();
    std::fs::create_dir(farm.path().join("elsewhere/f")).unwrap();
    std::fs::create_dir(farm.path().join("f")).unwrap();
    std::os::unix::fs::symlink(
        farm.path().join("elsewhere/child"),
        farm.path().join("link"),
    )
    .unwrap();
    let farm_text = farm.text();
    if isolated(test, "link", &[("HOME", Some(farm_text.as_str()))]) {
        // The child inherits the farm the parent prepared as its home directory.
        let home = std::env::var("HOME").unwrap();
        let written = format!("{home}/elsewhere/f/new.txt");
        write_through_tool(&farm_text, "~/link/../f/new.txt", "body");
        assert_eq!(std::fs::read(&written).unwrap(), b"body");
        let read = create_read_tool_definition::<()>(home.clone(), ReadToolOptions::default());
        let opened = call(&read, input("~/link/../f/new.txt", None, None), None, None).unwrap();
        assert_eq!(only_text(&opened), "body");
        let edit = create_edit_tool_definition::<()>(home.clone(), EditToolOptions::default());
        let arguments = edit_support::input("~/link/../f/new.txt", "body", "edited");
        run((edit.execute)("call".into(), arguments, None, None, None)).unwrap();
        assert_eq!(std::fs::read(&written).unwrap(), b"edited");
        assert!(!std::path::Path::new(&home).join("f/new.txt").exists());
    }
}

/// A removed working directory keeps the spelling its author typed.
#[cfg(unix)]
#[test]
fn removed_working_directory_keeps_its_written_spelling() {
    let test = "removed_working_directory_keeps_its_written_spelling";
    if isolated(test, "removed-cwd", &[]) {
        let parent = Scratch::new().unwrap();
        let removed = parent.enter_removed("a\u{a0}b").unwrap();
        let cwd = removed.to_str().unwrap().to_owned();
        let spelled = format!("{cwd}/up down/new.txt");
        for ask in ["up\u{a0}down/new.txt", "./up\u{a0}down/new.txt"] {
            assert_eq!(
                resolve_existing_path(ask, &cwd).unwrap(),
                spelled,
                "{ask:?}"
            );
            assert_eq!(resolve_read_path(ask, &cwd).unwrap(), spelled, "{ask:?}");
        }
        for ask in ["up down/new.txt", "notes.txt"] {
            assert_eq!(
                resolve_existing_path(ask, &cwd).unwrap(),
                format!("{cwd}/{ask}"),
                "{ask:?}"
            );
            assert_eq!(
                resolve_read_path(ask, &cwd).unwrap(),
                format!("{cwd}/{ask}"),
                "{ask:?}"
            );
        }
        let parent_text = parent.text();
        assert_eq!(
            resolve_existing_path("../x\u{a0}y.txt", &cwd).unwrap(),
            format!("{parent_text}/x y.txt")
        );
        assert_eq!(
            resolve_existing_path(&format!("{cwd}/x\u{a0}y.txt"), &cwd).unwrap(),
            format!("{parent_text}/a b/x y.txt")
        );
    }
}

/// A write rooted at a removed working directory recreates it under its written spelling.
#[cfg(unix)]
#[test]
fn write_recreates_a_removed_working_directory_under_its_written_spelling() {
    let test = "write_recreates_a_removed_working_directory_under_its_written_spelling";
    if isolated(test, "removed-cwd", &[]) {
        let parent = Scratch::new().unwrap();
        let removed = parent.enter_removed("a\u{a0}b").unwrap();
        let cwd = removed.to_str().unwrap().to_owned();
        write_through_tool(&cwd, "up\u{a0}down/new.txt", "body");
        assert_eq!(
            std::fs::read(parent.path().join("a\u{a0}b/up down/new.txt")).unwrap(),
            b"body"
        );
        assert!(!parent.path().join("a b").exists());
    }
}

/// A path expansion made absolute resolves without the supplied working directory.
#[cfg(unix)]
#[test]
fn expanded_absolute_paths_resolve_without_the_working_directory() {
    let test = "expanded_absolute_paths_resolve_without_the_working_directory";
    let home = Scratch::new().unwrap();
    let home_text = home.text();
    if isolated(test, "removed-cwd", &[("HOME", Some(home_text.as_str()))]) {
        let scratch = Scratch::new().unwrap();
        scratch.enter_removed("gone").unwrap();
        assert!(std::env::current_dir().is_err());
        let home_text = std::env::var("HOME").unwrap();
        let bare =
            resolve_existing_path(&format!("{home_text}/new\u{a0}file"), "relative").unwrap();
        assert_eq!(bare, format!("{home_text}/new file"));
        let marked =
            resolve_existing_path(&format!("@{home_text}/new\u{a0}file"), "relative").unwrap();
        assert_eq!(marked, bare);
        assert_eq!(
            resolve_existing_path("~/new\u{a0}file", "relative").unwrap(),
            bare
        );
    }
}

/// A component written after climbing out of the supplied directory is normalized.
#[cfg(unix)]
#[test]
fn climbing_out_of_the_supplied_directory_normalizes_what_it_re_authors() {
    let test = "climbing_out_of_the_supplied_directory_normalizes_what_it_re_authors";
    if isolated(test, "removed-cwd", &[]) {
        let parent = Scratch::new().unwrap();
        let removed = parent.enter_removed("a\u{a0}b").unwrap();
        let cwd = removed.to_str().unwrap().to_owned();
        let spelled = format!("{}/a b/x y.txt", parent.text());
        for ask in ["../a\u{a0}b/x\u{a0}y.txt", "./../a\u{a0}b/x\u{a0}y.txt"] {
            assert_eq!(
                resolve_existing_path(ask, &cwd).unwrap(),
                spelled,
                "{ask:?}"
            );
            assert_eq!(resolve_read_path(ask, &cwd).unwrap(), spelled, "{ask:?}");
        }
        write_through_tool(&cwd, "../a\u{a0}b/new\u{a0}file.txt", "body");
        assert_eq!(
            std::fs::read(parent.path().join("a b/new file.txt")).unwrap(),
            b"body"
        );
        assert!(!parent.path().join("a\u{a0}b").exists());
    }
}

/// Embedded climbs discard the supplied components they cancel on every tool route.
#[cfg(unix)]
#[test]
fn embedded_climbs_normalize_the_components_they_re_author() {
    let test = "embedded_climbs_normalize_the_components_they_re_author";
    if isolated(test, "removed-cwd", &[]) {
        let parent = Scratch::new().unwrap();
        let removed = parent.enter_removed("a\u{a0}b").unwrap();
        let cwd = removed.to_str().unwrap();
        let expected = format!("{}/a b/x y.txt", parent.text());
        for prefix in [
            "sub/../../",
            "./sub/../../",
            "@sub/../../",
            "sub/other/../../../",
        ] {
            let ask = format!("{prefix}a\u{a0}b/x\u{a0}y.txt");
            assert_eq!(
                resolve_existing_path(&ask, cwd).unwrap(),
                expected,
                "{ask:?}"
            );
            assert_eq!(resolve_read_path(&ask, cwd).unwrap(), expected, "{ask:?}");
        }
    }
}

/// Climbing above the root keeps an existing written ancestor instead of dropping it.
#[cfg(unix)]
#[test]
fn climbing_above_the_root_keeps_an_existing_written_ancestor() {
    let parent = Scratch::new().unwrap();
    std::fs::create_dir(parent.path().join("a\u{a0}b")).unwrap();
    let suffix = parent.text();
    let suffix = suffix.trim_start_matches('/');
    for prefix in ["../", "../../../", "sub/../../"] {
        let ask = format!("{prefix}{suffix}/a\u{a0}b/new\u{a0}file");
        let expected = format!("{}/a\u{a0}b/new file", parent.text());
        assert_eq!(
            resolve_existing_path(&ask, "/").unwrap(),
            expected,
            "{ask:?}"
        );
        assert_eq!(resolve_read_path(&ask, "/").unwrap(), expected, "{ask:?}");
    }
}

/// A write after an embedded climb reaches only the re-authored directory.
#[cfg(unix)]
#[test]
fn embedded_climb_writes_only_to_the_re_authoring_directory() {
    let test = "embedded_climb_writes_only_to_the_re_authoring_directory";
    if isolated(test, "removed-cwd", &[]) {
        let parent = Scratch::new().unwrap();
        let removed = parent.enter_removed("a\u{a0}b").unwrap();
        let cwd = removed.to_str().unwrap();
        write_through_tool(cwd, "sub/../../a\u{a0}b/new\u{a0}file.txt", "body");
        assert_eq!(
            std::fs::read(parent.path().join("a b/new file.txt")).unwrap(),
            b"body"
        );
        assert!(!parent.path().join("a\u{a0}b").exists());
    }
}

/// A write that climbs above the root stays under the existing written ancestor.
#[cfg(unix)]
#[test]
fn root_clamped_write_preserves_the_existing_ancestor() {
    let parent = Scratch::new().unwrap();
    std::fs::create_dir(parent.path().join("a\u{a0}b")).unwrap();
    let suffix = parent.text();
    let ask = format!(
        "../{}/a\u{a0}b/new\u{a0}file",
        suffix.trim_start_matches('/')
    );
    write_through_tool("/", &ask, "body");
    assert_eq!(
        std::fs::read(parent.path().join("a\u{a0}b/new file")).unwrap(),
        b"body"
    );
    assert!(!parent.path().join("a b").exists());
}
