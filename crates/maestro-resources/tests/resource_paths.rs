#[cfg(test)]
pub mod support;
use maestro_resources::{NativeResourceOperations, canonicalize_path, is_local_path};
use support::text;

/// Classify only the six exact lowercase source prefixes.
#[test]
fn maestro_paths_classify_only_known_source_prefixes() {
    for text in [
        "plain",
        "path/file",
        "../relative",
        "NPM:pkg",
        "gitx:x",
        "\u{85}npm:a\u{85}",
        "",
    ] {
        assert!(is_local_path(text), "{text}");
    }
    for text in [
        "npm:pkg",
        "git:repo",
        "github:a/b",
        "http:x",
        "https:x",
        "ssh:x",
        "\u{feff}npm:a\u{feff}",
    ] {
        assert!(!is_local_path(text), "{text}");
    }
}

/// Resolve actual native file and directory links.
#[cfg(unix)]
#[test]
fn maestro_paths_follow_file_and_directory_links() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    let file_link = dir.0.join("file-link");
    let dir_link = dir.0.join("dir-link");
    std::os::unix::fs::symlink(&file, &file_link).unwrap();
    std::os::unix::fs::symlink(dir.0.join("target"), &dir_link).unwrap();
    assert_eq!(
        canonicalize_path(text(&file_link), &NativeResourceOperations),
        file
    );
    assert_eq!(
        canonicalize_path(text(&dir_link), &NativeResourceOperations),
        dir.0.join("target")
    );
}

/// Preserve missing, dangling and failed-canonicalization path spelling.
#[test]
fn maestro_paths_preserve_unresolvable_spelling() {
    let path = std::path::Path::new("relative/../missing");
    assert_eq!(
        canonicalize_path(text(path), &NativeResourceOperations),
        text(path)
    );
    assert_eq!(
        canonicalize_path(text(path), &support::Unresolvable),
        text(path)
    );
    #[cfg(unix)]
    {
        let dir = support::Directory::new();
        let dangling = dir.0.join("dangling");
        std::os::unix::fs::symlink(dir.0.join("missing"), &dangling).unwrap();
        assert_eq!(
            canonicalize_path(text(&dangling), &NativeResourceOperations),
            dangling
        );
    }
}

/// Copy complete explicit provenance without selecting defaults.
#[test]
fn maestro_source_info_preserves_explicit_metadata() {
    use maestro_resources::{PathMetadata, SourceOrigin, SourceScope, create_source_info};
    for base_dir in [None, Some(String::new()), Some("/base".into())] {
        let source = create_source_info(
            "/file".into(),
            PathMetadata {
                source: "custom".into(),
                scope: SourceScope::User,
                origin: SourceOrigin::Package,
                base_dir: base_dir.clone(),
            },
        );
        assert_eq!(source.path, "/file");
        assert_eq!(source.source, "custom");
        assert_eq!(source.scope, SourceScope::User);
        assert_eq!(source.origin, SourceOrigin::Package);
        assert_eq!(source.base_dir, base_dir);
    }
}

/// Default only synthetic scope and origin while retaining explicit fields.
#[test]
fn maestro_source_info_defaults_only_synthetic_fields() {
    use maestro_resources::{
        SourceOrigin, SourceScope, SyntheticSourceOptions, create_synthetic_source_info,
    };
    let source = create_synthetic_source_info(
        "/file".into(),
        SyntheticSourceOptions {
            source: "local".into(),
            scope: None,
            origin: None,
            base_dir: None,
        },
    );
    assert_eq!(source.path, "/file");
    assert_eq!(source.source, "local");
    assert_eq!(source.scope, SourceScope::Temporary);
    assert_eq!(source.origin, SourceOrigin::TopLevel);
    assert_eq!(source.base_dir, None);
    for scope in [SourceScope::User, SourceScope::Project] {
        let source = create_synthetic_source_info(
            "/custom".into(),
            SyntheticSourceOptions {
                source: "custom".into(),
                scope: Some(scope),
                origin: Some(SourceOrigin::Package),
                base_dir: Some(String::new()),
            },
        );
        assert_eq!(source.path, "/custom");
        assert_eq!(source.source, "custom");
        assert_eq!(source.scope, scope);
        assert_eq!(source.origin, SourceOrigin::Package);
        assert_eq!(source.base_dir, Some(String::new()));
    }
}

/// Return an existing file's resolved path in its authored spelling.
#[test]
fn maestro_paths_canonicalize_existing_files() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    std::fs::create_dir(dir.0.join("detour")).unwrap();
    let path = dir.0.join("detour/../target/./skill.md");
    assert_ne!(path.as_os_str(), file.as_os_str());
    assert_eq!(
        canonicalize_path(text(&path), &NativeResourceOperations),
        text(&file)
    );
}

/// Collapse `..` against the preceding text, before any link or existence check.
#[cfg(unix)]
#[test]
fn maestro_paths_collapse_dot_dot_before_inspecting_components() {
    let dir = support::Directory::new();
    let file = dir.file("skill.md", "outer");
    let _ = dir.file("deep/inner/skill.md", "inner");
    let _ = dir.file("deep/skill.md", "linked parent");
    std::os::unix::fs::symlink(dir.0.join("deep/inner"), dir.0.join("link")).unwrap();
    for detour in ["link", "missing"] {
        let path = dir.0.join(detour).join("../skill.md");
        assert_eq!(
            canonicalize_path(text(&path), &NativeResourceOperations),
            text(&file),
            "{detour}"
        );
    }
}

/// Replace a link in any component, repeatedly, by its target text.
#[cfg(unix)]
#[test]
fn maestro_paths_follow_links_in_middle_components() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    std::fs::create_dir(dir.0.join("nested")).unwrap();
    let hop = dir.0.join("hop");
    let alias = dir.0.join("nested/alias");
    std::os::unix::fs::symlink(dir.0.join("target"), &hop).unwrap();
    std::os::unix::fs::symlink(&hop, &alias).unwrap();
    for linked in [hop, alias] {
        assert_eq!(
            canonicalize_path(text(&linked.join("skill.md")), &NativeResourceOperations),
            text(&file)
        );
    }
}

/// Resolve a relative link target against the directory holding the link.
#[cfg(unix)]
#[test]
fn maestro_paths_resolve_relative_link_targets_against_their_parent() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    std::fs::create_dir(dir.0.join("nested")).unwrap();
    let alias = dir.0.join("nested/alias");
    std::os::unix::fs::symlink("../target/./skill.md", &alias).unwrap();
    assert_eq!(
        canonicalize_path(text(&alias), &NativeResourceOperations),
        text(&file)
    );
}

/// Keep the input when a link loops or a component is not a directory.
#[cfg(unix)]
#[test]
fn maestro_paths_preserve_spelling_of_link_loops_and_non_directories() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    std::os::unix::fs::symlink(dir.0.join("loop-b"), dir.0.join("loop-a")).unwrap();
    std::os::unix::fs::symlink(dir.0.join("loop-a"), dir.0.join("loop-b")).unwrap();
    for path in [dir.0.join("loop-a/skill.md"), file.join("child")] {
        assert_eq!(
            canonicalize_path(text(&path), &NativeResourceOperations),
            text(&path)
        );
    }
}

/// Resolve on another thread; `None` means the walk did not finish within the bound.
#[cfg(unix)]
fn canonicalize_within_bound(path: &std::path::Path) -> Option<String> {
    let path = text(path).to_owned();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(canonicalize_path(&path, &NativeResourceOperations));
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .ok()
}

/// Keep the input when a link's target folds back to the link itself.
#[cfg(unix)]
#[test]
fn maestro_paths_preserve_spelling_of_a_link_that_folds_back_to_itself() {
    let dir = support::Directory::new();
    let _ = dir.file("other/alias", "target");
    std::fs::create_dir_all(dir.0.join("other/inner")).unwrap();
    std::fs::create_dir(dir.0.join("base")).unwrap();
    std::os::unix::fs::symlink("../other/inner", dir.0.join("base/dir-link")).unwrap();
    std::os::unix::fs::symlink("dir-link/../alias", dir.0.join("base/alias")).unwrap();
    let alias = dir.0.join("base/alias");
    assert_eq!(
        canonicalize_within_bound(&alias).as_deref(),
        Some(text(&alias))
    );
}

/// Keep the input when links fold into each other through several states.
#[cfg(unix)]
#[test]
fn maestro_paths_preserve_spelling_of_links_that_fold_into_each_other() {
    let dir = support::Directory::new();
    let _ = dir.file("other/a", "first");
    let _ = dir.file("other/b", "second");
    std::fs::create_dir_all(dir.0.join("other/inner")).unwrap();
    std::fs::create_dir(dir.0.join("base")).unwrap();
    std::os::unix::fs::symlink("../other/inner", dir.0.join("base/dir-link")).unwrap();
    std::os::unix::fs::symlink("dir-link/../b", dir.0.join("base/a")).unwrap();
    std::os::unix::fs::symlink("dir-link/../a", dir.0.join("base/b")).unwrap();
    for name in ["base/a", "base/b"] {
        let link = dir.0.join(name);
        assert_eq!(
            canonicalize_within_bound(&link).as_deref(),
            Some(text(&link)),
            "{name}"
        );
    }
}
