//! Native cwd binding and failure isolation in a fresh test process.
use super::{
    harness::{Harness, colors},
    protocol::{resolved, scopes},
    records::Scopes,
};
use maestro_app::NativeResourceOperations;
use maestro_chat::components::config_selector::ConfigSelectorOptions;
use maestro_theme::Collator;
use maestro_tui::{Component, tui::InputHandler};
use serde_json::json;
use std::{path::Path, rc::Rc};
/// Run cwd mutations only inside the existing isolated child seam.
pub fn paths() {
    if std::env::var_os("MAESTRO_RESOURCE_DIRECTORY_CHILD").is_some() {
        child();
        println!("\nPROBE native directory outcomes matched");
        return;
    }
    let _guard = crate::keybindings::bindings_guard();
    let scratch = crate::scratch::Scratch::new().unwrap();
    assert_eq!(
        crate::probe::probe(
            "resource_relative_paths_observe_live_directory_and_preserve_failures",
            &scratch.0,
            &[("MAESTRO_RESOURCE_DIRECTORY_CHILD", "1")],
            &[]
        ),
        ["native directory outcomes matched"]
    );
}
/// Builds the real native path adapter without changing the supplied project directory.
fn native(path: &str, base: &str, settings: &Scopes) -> Harness {
    let (_, theme) = crate::published::published(&colors(0));
    Harness::build(
        resolved(path, Some(base)),
        settings,
        ConfigSelectorOptions {
            cwd: "/unrelated/project".into(),
            agent_dir: "/unrelated/agent".into(),
            path_operations: Rc::new(NativeResourceOperations),
            theme,
            collator: Rc::new(Collator::new(&|_| Some("en_US.UTF-8".into())).unwrap()),
            styles_enabled: Rc::new(|| true),
        },
        true,
    )
}
/// Expected package update, preserving the independent project document.
fn updated(pattern: &str) -> Scopes {
    Scopes {
        global: json!({"packages":[{"source":"npm:target","extensions":[pattern]}]}),
        project: json!({"decoy":true}),
    }
}
/// Exercises early construction, live observations, failure and no-read branches.
fn child() {
    let root = std::env::current_dir().unwrap();
    let a = root.join("a");
    let b = a.join("b");
    let gone = root.join("gone");
    std::fs::create_dir_all(&b).unwrap();
    std::fs::create_dir(&gone).unwrap();
    let target = a
        .join("target/extensions/a.wasm")
        .to_string_lossy()
        .into_owned();
    std::env::set_current_dir(&a).unwrap();
    let live = native(&target, "", &scopes());
    std::env::set_current_dir(&b).unwrap();
    live.component
        .get_resource_list()
        .handle_input(" ")
        .unwrap();
    live.persisted(&updated("-../target/extensions/a.wasm"));
    std::env::set_current_dir(&root).unwrap();
    let failed = native(&target, "", &scopes());
    let before = failed.frame(80);
    std::env::set_current_dir(&gone).unwrap();
    std::fs::remove_dir(&gone).unwrap();
    failures(&failed, &before);
    no_directory_needed(&target, &a);
    std::env::set_current_dir(&root).unwrap();
    failed.persisted(&scopes());
    assert!(Path::new(&target).is_absolute());
}
/// Selected branches must succeed even when the native cwd is unavailable.
fn no_directory_needed(target: &str, a: &Path) {
    let empty = Scopes {
        global: json!({"packages":[]}),
        project: json!({"decoy":true}),
    };
    let unmatched = native(target, "", &empty);
    unmatched
        .component
        .get_resource_list()
        .handle_input(" ")
        .unwrap();
    assert!(
        unmatched
            .component
            .get_resource_list()
            .render(80)
            .iter()
            .any(|line| line.contains("[ ]"))
    );
    assert_eq!(*unmatched.events.borrow(), [json!(["render"])]);
    unmatched.persisted(&Scopes {
        global: json!({"packages":[]}),
        project: json!({"decoy":true}),
    });
    let identical = native("same.wasm", "same.wasm", &scopes());
    identical
        .component
        .get_resource_list()
        .handle_input(" ")
        .unwrap();
    identical.persisted(&updated("-"));
    let anchored = native(target, &a.to_string_lossy(), &scopes());
    anchored
        .component
        .get_resource_list()
        .handle_input(" ")
        .unwrap();
    anchored.persisted(&updated("-target/extensions/a.wasm"));
}
/// All input entry points preserve the original native failure and unchanged state.
fn failures(harness: &Harness, before: &super::records::Frame) {
    let list = harness.component.get_resource_list();
    let error = list.handle_input(" ").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(error.raw_os_error(), Some(2));
    assert_eq!(&harness.frame(80), before);
    let error = list.try_handle_input(" ").unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .raw_os_error(),
        Some(2)
    );
    assert_eq!(&harness.frame(80), before);
    InputHandler::handle_input(list.as_ref(), " ");
    assert_eq!(&harness.frame(80), before);
}
