//! Each loaded component instance reads the location of the artifact it was loaded from.
#![cfg(test)]
use super::components::{loader, local, place};
use maestro_extensions::{
    ExtensionRunner, LoadExtensionsResult, Local, ResourcesDiscoverReason, create_event_bus_for,
    load_extensions, runner::ResourcePath, types::FlagValue,
};
use maestro_test_directory::Scratch;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::RwLock,
};

/// Variable through which the host names each instance's artifact.
const LOCATION: &str = "MAESTRO_EXTENSION_PATH";

/// Variable through which the test setup supplies the built component.
const COMPONENT: &str = "MAESTRO_LOCATION_COMPONENT";

/// Copies the built component into the nested directories below `root` as `index.wasm`.
fn install(root: &Path, directories: &[&str]) -> PathBuf {
    let artifact = directories
        .iter()
        .fold(root.to_owned(), |path, name| path.join(name))
        .join("index.wasm");
    place(COMPONENT, &artifact);
    artifact
}

/// The host spelling of a path as a flag value.
fn location(path: &Path) -> FlagValue {
    FlagValue::String(path.to_str().unwrap().to_owned())
}

/// Loads each path through the artifact factory, as an application's module loader does.
async fn load(paths: &[PathBuf]) -> LoadExtensionsResult<Local> {
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let paths = paths
        .iter()
        .map(|path| path.to_str().unwrap().to_owned())
        .collect();
    let result = load_extensions(paths, "/unrelated/work", &bus, &loader())
        .await
        .unwrap();
    let errors: Vec<_> = result.errors.iter().map(|error| &error.error).collect();
    assert!(errors.is_empty(), "{errors:?}");
    result
}

/// Each instance's location flag default, in load order.
fn flags(loaded: &LoadExtensionsResult<Local>) -> Vec<Option<FlagValue>> {
    loaded
        .extensions
        .iter()
        .map(|extension| {
            let flag = extension.read().unwrap().flags["location"].clone();
            flag.read().unwrap().default.clone()
        })
        .collect()
}

/// The skill, prompt and theme paths the loaded instances contribute to one discovery.
async fn discovered(
    loaded: &LoadExtensionsResult<Local>,
    cwd: &str,
    reason: ResourcesDiscoverReason,
) -> [Vec<String>; 3] {
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(loaded.extensions.clone())),
        loaded.runtime.clone(),
        super::session::runner_session(cwd).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let result = runner
        .emit_resources_discover(cwd.into(), reason)
        .await
        .unwrap();
    let paths = |paths: Vec<ResourcePath>| paths.into_iter().map(|path| path.path).collect();
    [
        paths(result.skill_paths),
        paths(result.prompt_paths),
        paths(result.theme_paths),
    ]
}

/// The skill, prompt and theme paths beside each artifact, in load order.
fn beside(artifacts: &[PathBuf]) -> [Vec<String>; 3] {
    ["SKILL.md", "dynamic.md", "dynamic.json"].map(|name| {
        artifacts
            .iter()
            .map(|artifact| artifact.with_file_name(name).to_str().unwrap().to_owned())
            .collect()
    })
}

#[test]
fn component_location_tracks_the_loaded_artifact() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let installed = [
        &["plain"][..],
        &["relocated # Ω % space"],
        &["nested", "component"],
        &["another-instance"],
    ]
    .map(|directories| install(root, directories));
    let mut paths = installed.to_vec();
    let mut expected = installed.to_vec();
    // Dot segments are removed lexically before the artifact is opened.
    paths.push(
        root.join("plain")
            .join("..")
            .join("another-instance")
            .join("index.wasm"),
    );
    expected.push(installed[3].clone());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("plain"), root.join("link")).unwrap();
        paths.push(root.join("link").join("index.wasm"));
        expected.push(root.join("link").join("index.wasm"));
    }
    local(async {
        let loaded = load(&paths).await;
        let locations: Vec<_> = expected.iter().map(|path| Some(location(path))).collect();
        assert_eq!(flags(&loaded), locations);
        let rows = [
            ("/unrelated/work", ResourcesDiscoverReason::Startup),
            ("/another cwd # Ω", ResourcesDiscoverReason::Reload),
        ];
        for (cwd, reason) in rows {
            let [skills, prompts, themes] = discovered(&loaded, cwd, reason).await;
            let [skill, prompt, theme] = beside(&expected);
            assert_eq!(skills, skill, "{cwd}");
            assert_eq!(prompts, prompt, "{cwd}");
            assert_eq!(themes, theme, "{cwd}");
        }
        std::fs::rename(root.join("nested"), root.join("moved")).unwrap();
        let moved = root.join("moved").join("component").join("index.wasm");
        let relocated = load(std::slice::from_ref(&moved)).await;
        assert_eq!(flags(&relocated), [Some(location(&moved))]);
        assert_eq!(
            discovered(
                &relocated,
                "/unrelated/work",
                ResourcesDiscoverReason::Startup
            )
            .await,
            beside(&[moved])
        );
    });
}

#[test]
fn component_location_is_isolated_and_reserved() {
    let component = std::env::var(COMPONENT).unwrap();
    let cases = [None, Some(""), Some("/wrong/index.wasm")];
    let selected = maestro_test_process::run_per_environment(
        "component_location_is_isolated_and_reserved",
        cases,
        |supplied| {
            let mut environment = BTreeMap::from([(COMPONENT, component.as_str())]);
            if let Some(value) = supplied {
                environment.insert(LOCATION, value);
            }
            environment
        },
    )
    .unwrap();
    for supplied in selected.into_iter().flatten() {
        let scratch = Scratch::new().unwrap();
        let first = install(scratch.path(), &["first"]);
        let second = install(scratch.path(), &["second"]);
        local(async {
            let earlier = load(std::slice::from_ref(&first)).await;
            let later = load(std::slice::from_ref(&second)).await;
            assert_eq!(flags(&earlier), [Some(location(&first))], "{supplied:?}");
            assert_eq!(flags(&later), [Some(location(&second))], "{supplied:?}");
            let reason = ResourcesDiscoverReason::Startup;
            assert_eq!(
                discovered(&earlier, "/unrelated/work", reason).await,
                beside(std::slice::from_ref(&first))
            );
            assert_eq!(
                discovered(&later, "/unrelated/work", reason).await,
                beside(std::slice::from_ref(&second))
            );
        });
        assert_eq!(std::env::var(LOCATION).ok().as_deref(), supplied);
    }
}
