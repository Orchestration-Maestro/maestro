#[cfg(test)]
pub mod support;
use maestro_resources::{NativeResourceOperations, canonicalize_path, is_local_path};

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
        canonicalize_path(&file_link, &NativeResourceOperations),
        file
    );
    assert_eq!(
        canonicalize_path(&dir_link, &NativeResourceOperations),
        dir.0.join("target")
    );
}

/// Preserve missing, dangling and failed-canonicalization path spelling.
#[test]
fn maestro_paths_preserve_unresolvable_spelling() {
    let path = std::path::Path::new("relative/../missing");
    assert_eq!(canonicalize_path(path, &NativeResourceOperations), path);
    assert_eq!(canonicalize_path(path, &support::Unresolvable), path);
    #[cfg(unix)]
    {
        let dir = support::Directory::new();
        let dangling = dir.0.join("dangling");
        std::os::unix::fs::symlink(dir.0.join("missing"), &dangling).unwrap();
        assert_eq!(
            canonicalize_path(&dangling, &NativeResourceOperations),
            dangling
        );
    }
}

/// Copy complete explicit provenance without selecting defaults.
#[test]
fn maestro_source_info_preserves_explicit_metadata() {
    use maestro_resources::{PathMetadata, SourceOrigin, SourceScope, create_source_info};
    for base_dir in [None, Some(std::path::PathBuf::new()), Some("/base".into())] {
        let source = create_source_info(
            "/file".into(),
            PathMetadata {
                source: "custom".into(),
                scope: SourceScope::User,
                origin: SourceOrigin::Package,
                base_dir: base_dir.clone(),
            },
        );
        assert_eq!(source.path, std::path::Path::new("/file"));
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
    assert_eq!(source.path, std::path::Path::new("/file"));
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
                base_dir: Some(std::path::PathBuf::new()),
            },
        );
        assert_eq!(source.path, std::path::Path::new("/custom"));
        assert_eq!(source.source, "custom");
        assert_eq!(source.scope, scope);
        assert_eq!(source.origin, SourceOrigin::Package);
        assert_eq!(source.base_dir, Some(std::path::PathBuf::new()));
    }
}

/// Return the native canonical path of an existing file.
#[test]
fn maestro_paths_canonicalize_existing_files() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    std::fs::create_dir(dir.0.join("detour")).unwrap();
    let path = dir.0.join("detour/../target/./skill.md");
    assert_ne!(path.as_os_str(), file.as_os_str());
    assert_eq!(
        canonicalize_path(&path, &NativeResourceOperations).as_os_str(),
        std::fs::canonicalize(file).unwrap().as_os_str()
    );
}
