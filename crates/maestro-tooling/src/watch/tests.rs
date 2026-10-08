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
fn startup_roots_exclude_only_top_level_target_and_git() {
    let root = Path::new("/workspace");
    let entries: Vec<_> = [
        ".git",
        "Cargo.toml",
        "crates",
        "out\\",
        "{crates,output}",
        "[ab]",
    ]
    .into_iter()
    .map(|entry| root.join(entry))
    .collect();
    for (target, expected) in [
        (
            "/workspace/out\\",
            vec!["Cargo.toml", "crates", "{crates,output}", "[ab]"],
        ),
        (
            "/workspace/{crates,output}",
            vec!["Cargo.toml", "crates", "out\\", "[ab]"],
        ),
        (
            "/workspace/[ab]",
            vec!["Cargo.toml", "crates", "out\\", "{crates,output}"],
        ),
        (
            "/external/target",
            vec!["Cargo.toml", "crates", "out\\", "{crates,output}", "[ab]"],
        ),
        (
            "/workspace/crates/output",
            vec!["Cargo.toml", "crates", "out\\", "{crates,output}", "[ab]"],
        ),
    ] {
        let expected: Vec<_> = expected.into_iter().map(|entry| root.join(entry)).collect();
        assert_eq!(
            super::watch_roots(entries.clone(), Path::new(target)),
            expected,
            "target: {target}"
        );
    }
}
