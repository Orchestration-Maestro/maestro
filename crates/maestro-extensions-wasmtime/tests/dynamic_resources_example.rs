//! The shipped component contributes installed sibling paths independently of the session directory.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, ResourcesDiscoverReason, create_event_bus_for, load_extensions,
};
use maestro_test_directory::Scratch;
use serde_json::Value;
use std::{rc::Rc, sync::RwLock};

/// Startup and reload keep every path beside each independently installed component.
#[test]
fn dynamic_resources_component_returns_shared_paths() {
    let scratch = Scratch::new().unwrap();
    super::components::local(async {
        for name in ["original", "relocated # Ω"] {
            check_installation(&scratch.path().join(name)).await;
        }
    });
}

/// Runs each discovery reason against the same installed instance.
async fn check_installation(directory: &std::path::Path) {
    let artifact = directory.join("resources.wasm");
    super::components::place("MAESTRO_DYNAMIC_RESOURCES_COMPONENT", &artifact);
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let loaded = load_extensions(
        vec![artifact.to_str().unwrap().into()],
        "/unrelated/work",
        &bus,
        &super::components::loader(),
    )
    .await
    .unwrap();
    assert!(loaded.errors.is_empty(), "{:?}", loaded.errors);
    {
        let owner = loaded.extensions[0].read().unwrap();
        assert_eq!(owner.handlers.len(), 1);
        assert!(owner.commands.is_empty());
        assert!(owner.tools.is_empty());
    }
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(loaded.extensions)),
        loaded.runtime,
        super::session::runner_session("/unrelated/work").0,
        Rc::new(|_| panic!("no errors")),
        maestro_theme::LiveTheme::default(),
    );
    let expected: Value =
        serde_json::from_str(&include_str!("dynamic_resources_expected.json").replace(
            "$DIRECTORY",
            &directory.to_string_lossy().replace('\\', "/"),
        ))
        .unwrap();
    for (cwd, reason) in [
        ("/unrelated/work", ResourcesDiscoverReason::Startup),
        ("/another cwd # Ω", ResourcesDiscoverReason::Reload),
        ("/third", ResourcesDiscoverReason::Reload),
    ] {
        let result = runner
            .emit_resources_discover(cwd.into(), reason)
            .await
            .unwrap();
        let paths = |paths: Vec<maestro_extensions::runner::ResourcePath>| {
            paths
                .into_iter()
                .map(|path| path.path.replace('\\', "/"))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            serde_json::json!({
                "skillPaths": paths(result.skill_paths),
                "promptPaths": paths(result.prompt_paths),
                "themePaths": paths(result.theme_paths),
            }),
            expected
        );
    }
}
