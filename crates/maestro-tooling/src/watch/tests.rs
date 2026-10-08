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

#[test]
fn startup_ignore_uses_literal_target_path() {
    assert_eq!(
        super::target_ignore(Path::new("/workspace/[out]"), Path::new("/workspace")).unwrap(),
        Some("/[[]out[]]/".to_owned())
    );
    assert_eq!(
        super::target_ignore(Path::new("/external/target"), Path::new("/workspace")).unwrap(),
        None
    );
}

#[test]
fn braced_target_ignore_keeps_source_directories_watched() {
    let pattern = super::target_ignore(
        Path::new("/workspace/{crates,output}"),
        Path::new("/workspace"),
    )
    .unwrap()
    .unwrap();
    let matcher = globset::Glob::new(&pattern).unwrap().compile_matcher();
    assert!(!matcher.is_match("/crates/"));
    assert!(!matcher.is_match("/output/"));
    assert!(matcher.is_match("/{crates,output}/"));
}

#[test]
fn unbalanced_brace_target_ignore_is_valid_and_literal() {
    let pattern = super::target_ignore(Path::new("/workspace/out{put"), Path::new("/workspace"))
        .unwrap()
        .unwrap();
    let matcher = globset::Glob::new(&pattern).unwrap().compile_matcher();
    assert!(matcher.is_match("/out{put/"));
    assert!(!matcher.is_match("/output/"));
}

#[test]
fn bracket_and_wildcard_target_ignores_match_only_literal_names() {
    for (target, unrelated) in [("[out]", "o"), ("out*put?", "outXputY")] {
        let path = Path::new("/workspace").join(target);
        let pattern = super::target_ignore(&path, Path::new("/workspace"))
            .unwrap()
            .unwrap();
        let matcher = globset::Glob::new(&pattern).unwrap().compile_matcher();
        assert!(matcher.is_match(format!("/{target}/")));
        assert!(!matcher.is_match(format!("/{unrelated}/")));
    }
}
