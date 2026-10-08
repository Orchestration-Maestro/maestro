use super::should_build;
use std::path::Path;

#[test]
fn configured_target_events_skip_compilation() {
    let events =
        br#"{"tags":[{"kind":"path","absolute":"/workspace/compiler-output/debug/library"}]}
{"tags":[{"kind":"path","absolute":"/workspace/compiler-output/cache"}]}"#;
    assert!(!should_build(&events[..], Path::new("/workspace/compiler-output")).unwrap());
}

#[test]
fn literal_bracketed_target_events_skip_compilation() {
    let events = br#"{"tags":[{"kind":"path","absolute":"/workspace/[out]/debug/library"}]}"#;
    assert!(!should_build(&events[..], Path::new("/workspace/[out]")).unwrap());
}

#[test]
fn mixed_target_and_source_events_compile() {
    let events = br#"{"tags":[{"kind":"path","absolute":"/workspace/target/debug/library"},{"kind":"path","absolute":"/workspace/src/lib.rs"}]}"#;
    assert!(should_build(&events[..], Path::new("/workspace/target")).unwrap());
}

#[test]
fn events_without_paths_compile() {
    let events = br#"{"tags":[{"kind":"signal","signal":"interrupt"}]}"#;
    assert!(should_build(&events[..], Path::new("/workspace/target")).unwrap());
}

#[test]
fn old_target_events_compile_after_configuration_switch() {
    // A delayed old-output event after a target switch explains the extra rebuild.
    let events = br#"{"tags":[{"kind":"path","absolute":"/workspace/target/debug/library"}]}"#;
    assert!(!should_build(&events[..], Path::new("/workspace/target")).unwrap());
    assert!(should_build(&events[..], Path::new("/workspace/new-output")).unwrap());
}

#[cfg(unix)]
#[test]
fn backslash_target_ignore_matches_only_literal_directory() {
    assert_literal_target_ignore(r"out\put", &["output"]);
}

/// Exercise the watcher's gitignore dialect against literal output and decoys.
fn assert_literal_target_ignore(target: &str, decoys: &[&str]) {
    let root = Path::new("/workspace");
    let path = root.join(target);
    let pattern = super::target_ignore(&path, root).unwrap().unwrap();
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    builder.add_line(None, &pattern).unwrap();
    let matcher = builder.build().unwrap();
    assert!(matcher.matched(&path, true).is_ignore(), "target: {target}");
    for decoy in decoys {
        assert!(
            matcher.matched(root.join(decoy), true).is_none(),
            "decoy: {decoy}"
        );
    }
}

#[test]
fn startup_ignore_uses_literal_target_path() {
    assert_eq!(
        super::target_ignore(Path::new("/workspace/[out]"), Path::new("/workspace")).unwrap(),
        Some(r"/\[out\]/".to_owned())
    );
    assert_eq!(
        super::target_ignore(Path::new("/external/target"), Path::new("/workspace")).unwrap(),
        None
    );
}

#[test]
fn braced_target_ignore_keeps_source_directories_watched() {
    assert_literal_target_ignore("{crates,output}", &["crates", "output"]);
}

#[test]
fn unbalanced_brace_target_ignore_is_valid_and_literal() {
    assert_literal_target_ignore("out{put", &["output"]);
}

#[test]
fn bracket_and_wildcard_target_ignores_match_only_literal_names() {
    for (target, decoys) in [
        ("[out]", &["o"][..]),
        ("[ab]", &["a", "b"][..]),
        ("a*b", &["ab", "axb"][..]),
        ("a?b", &["axb"][..]),
        ("out*put?", &["outXputY"][..]),
    ] {
        assert_literal_target_ignore(target, decoys);
    }
}

#[test]
fn marker_target_ignores_match_only_literal_names() {
    for (target, decoy) in [("!keep", "keep"), ("#hash", "hash")] {
        assert_literal_target_ignore(target, &[decoy]);
    }
}
