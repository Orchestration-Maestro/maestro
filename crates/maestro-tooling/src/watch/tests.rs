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
        Some("/\\[out\\]/".to_owned())
    );
    assert_eq!(
        super::target_ignore(Path::new("/external/target"), Path::new("/workspace")).unwrap(),
        None
    );
}
