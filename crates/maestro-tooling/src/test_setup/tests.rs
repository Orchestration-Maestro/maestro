//! Unit coverage for the preparation build: its environment filter and the inputs it fingerprints.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use cargo_metadata::Metadata;

use super::examples::stage_components;
use super::inputs::listed;
use super::inputs::{Inputs, locked_closure};
use super::plan::{Artifacts, closure_directories, component_inputs, plan};
use super::*;

/// Every name component discovery accepts is removed, including the bare and doubled
/// underscore forms, and no other name is.
#[test]
fn inherited_components_follow_the_discovery_rule() {
    let names = [
        "MAESTRO_AUTHOR_COMPONENT",
        "MAESTRO_COMPONENT",
        "MAESTRO__COMPONENT",
        "MAESTRO_HOME",
        "MAESTRO_lower_COMPONENT",
        "PATH",
    ]
    .map(OsString::from);
    assert_eq!(
        inherited_components(names.into_iter()),
        [
            "MAESTRO_AUTHOR_COMPONENT",
            "MAESTRO_COMPONENT",
            "MAESTRO__COMPONENT"
        ]
        .map(OsString::from)
    );
}

/// Native file names reach the real copy boundary when staging and restoring a bundle.
#[test]
fn executable_names_survive_bundle_copy_and_restore() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let built = scratch.path().join("built");
    let bundle = scratch.path().join("bundle");
    let restored = scratch.path().join("restored");
    for directory in [&built, &bundle, &restored] {
        std::fs::create_dir_all(directory).unwrap();
    }
    for (extension, example, exporter) in [
        ("exe", "prepare_author_component.exe", "development.exe"),
        ("", "prepare_author_component", "development"),
    ] {
        for (name, native) in [(EXAMPLE, example), (EXPORTER, exporter)] {
            std::fs::write(built.join(native), b"native executable").unwrap();
            let file = executable(name, extension);

            copy(&built.join(&file), &bundle.join(&file)).unwrap();
            std::fs::remove_file(built.join(native)).unwrap();
            copy(&bundle.join(&file), &restored.join(&file)).unwrap();

            assert_eq!(
                std::fs::read(restored.join(native)).unwrap(),
                b"native executable"
            );
        }
    }
}

/// Write a member crate holding `name`, which the fixed components name.
fn member(root: &Path, name: &str) {
    let directory = root.join("crates").join(name);
    std::fs::create_dir_all(directory.join("src")).unwrap();
    std::fs::write(directory.join("src/lib.rs"), "").unwrap();
    std::fs::write(
        directory.join("Cargo.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\n"),
    )
    .unwrap();
}

/// Bundle installation verifies and copies the native setup example beside test artifacts, and a
/// later installation replaces the example from the earlier installation with a new file.
#[test]
fn bundle_export_installs_the_native_example() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\n",
    )
    .unwrap();
    for (_, package, _) in FIXED_COMPONENTS {
        member(root, package);
    }
    let cargo = Path::new(env!("CARGO"));
    let bundle = root.join("bundle");
    std::fs::create_dir_all(&bundle).unwrap();
    let inputs = Inputs::new(root, BTreeSet::new(), BTreeSet::new(), BTreeSet::new()).unwrap();
    let components = FIXED_COMPONENTS
        .iter()
        .map(|(_, _, variable)| {
            std::fs::write(bundle.join(format!("{variable}.wasm")), b"component").unwrap();
            (
                (*variable).to_owned(),
                Component {
                    file: digest(b"component"),
                    inputs: inputs.clone(),
                },
            )
        })
        .collect();
    let native = format!("prepare_author_component{}", std::env::consts::EXE_SUFFIX);
    std::fs::write(bundle.join(&native), b"native example").unwrap();
    let manifest = Manifest {
        shared: inputs.clone(),
        tool: inputs.clone(),
        browser: inputs,
        workspace_sources: None,
        components,
        example: Some(digest(b"native example")),
        compiled: None,
        executable: None,
    };
    std::fs::write(
        bundle.join(MANIFEST),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let artifacts = root.join("debug");
    let environment = root.join("nextest-env");

    export(cargo, root, &bundle, &artifacts, &environment).unwrap();

    let installed = artifacts.join("examples").join(native);
    assert_eq!(std::fs::read(&installed).unwrap(), b"native example");
    let exported = std::fs::read_to_string(&environment).unwrap();
    assert_eq!(exported.lines().count(), FIXED_COMPONENTS.len());
    for line in exported.lines() {
        let (_, file) = line.split_once('=').unwrap();
        assert_eq!(std::fs::read(file).unwrap(), b"component");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // The open handle holds the installed file, so its replacement cannot reuse its identity.
        let earlier = std::fs::File::open(&installed).unwrap();
        export(cargo, root, &bundle, &artifacts, &environment).unwrap();
        assert_ne!(
            std::fs::metadata(&installed).unwrap().ino(),
            earlier.metadata().unwrap().ino()
        );
    }
}

/// A workspace that builds on Maestro: its one member is the component crate
/// `crates/component-fixture`, whose package is `example-components`.
fn consuming_workspace(root: &Path) -> Workspace {
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    let component = root.join("crates/component-fixture");
    std::fs::create_dir_all(component.join("src")).unwrap();
    std::fs::create_dir_all(component.join("examples")).unwrap();
    std::fs::write(component.join("src/lib.rs"), "").unwrap();
    std::fs::write(
        component.join("examples/one.rs"),
        "// maestro-component: MAESTRO_ONE_COMPONENT\n",
    )
    .unwrap();
    std::fs::write(
        component.join("Cargo.toml"),
        format!(
            "[package]\nname = \"example-components\"\nversion = \"0.1.0\"\n\
             {}{}\n",
            crate::workspace::REGION_START,
            crate::workspace::REGION_END
        ),
    )
    .unwrap();
    Workspace::read(Path::new(env!("CARGO")), root).unwrap()
}

/// The setup example builds the component crate's own package, which the arguments name beside
/// the crate, and no fixed component the workspace does not hold.
#[test]
fn the_setup_example_builds_the_component_crate_package() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    let workspace = consuming_workspace(root);

    let arguments = example_arguments(&workspace).unwrap();

    assert_eq!(
        arguments[1..],
        [
            OsString::from(root.join("crates/component-fixture")),
            OsString::from("example-components"),
            OsString::new(),
        ]
    );
}

/// A workspace that builds on Maestro plans its own component crate's components, and none of
/// the fixed components or example inputs whose packages it does not hold.
#[test]
fn a_consuming_workspace_plans_its_own_components_alone() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    let workspace = consuming_workspace(root);
    let planned = plan(&workspace, Some(&workspace.component_crate().unwrap())).unwrap();

    assert!(fixed_components(&workspace).is_empty());
    assert!(
        planned
            .shared
            .iter()
            .any(|path| path == "crates/component-fixture"),
        "{:?}",
        planned.shared
    );
    let named: Vec<&String> = planned
        .shared
        .iter()
        .chain(&planned.tool)
        .chain(&planned.browser)
        .collect();
    assert!(
        named
            .iter()
            .all(|path| !path.contains("maestro-extensions-wasm")
                && !path.contains("maestro-subagents")),
        "{named:?}"
    );
    assert_eq!(
        named
            .iter()
            .filter(|path| path.starts_with("crates/"))
            .map(|path| path.as_str())
            .collect::<Vec<_>>(),
        ["crates/component-fixture"]
    );
}

/// A workspace whose component crate builds on `library`, which has a unit test file, an
/// integration test and a development dependency on `helper`; `spare` is a member nothing uses.
fn closure_workspace(root: &Path) {
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    for (name, manifest) in [
        (
            "component-fixture",
            format!(
                "[dependencies]\nlibrary = {{ path = \"../library\" }}\n{}{}\n",
                crate::workspace::REGION_START,
                crate::workspace::REGION_END
            ),
        ),
        (
            "library",
            "[dev-dependencies]\nhelper = { path = \"../helper\" }\n".to_owned(),
        ),
        ("helper", String::new()),
        ("spare", String::new()),
    ] {
        let directory = root.join("crates").join(name);
        for file in [
            "src/lib.rs",
            "src/tests.rs",
            "tests/it.rs",
            "examples/one.rs",
        ] {
            let path = directory.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(
                path,
                if file == "examples/one.rs" {
                    "// maestro-component: MAESTRO_ONE_COMPONENT\n"
                } else {
                    "// before\n"
                },
            )
            .unwrap();
        }
        std::fs::write(
            directory.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n{manifest}"),
        )
        .unwrap();
    }
}

/// The digest the components share in the workspace `closure_workspace` wrote at `root`.
fn shared_digest(root: &Path) -> String {
    let workspace = Workspace::read(Path::new(env!("CARGO")), root).unwrap();
    let component = workspace.component_crate().unwrap();
    Artifacts::read(root, &workspace, Some(&component))
        .unwrap()
        .shared
        .digest
}

/// The components build with `cargo build`, so the tests and the development dependencies of
/// the crates they build on are never compiled: changing them leaves the components valid, and
/// changing the source of such a crate does not.
#[test]
fn component_digest_ignores_tests_and_development_dependencies_of_dependencies() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    closure_workspace(root);
    let before = shared_digest(root);

    std::fs::write(root.join("crates/library/tests/it.rs"), "// after\n").unwrap();
    std::fs::write(root.join("crates/library/src/tests.rs"), "// after\n").unwrap();
    let unit = root.join("crates/library/src/tests/one.rs");
    std::fs::create_dir_all(unit.parent().unwrap()).unwrap();
    std::fs::write(unit, "// after\n").unwrap();
    std::fs::write(
        root.join("crates/library/Cargo.toml"),
        "[package]\nname = \"library\"\nversion = \"0.1.0\"\n\
         [dev-dependencies]\nspare = { path = \"../spare\" }\n\
         [target.'cfg(unix)'.dev-dependencies]\nspare = { path = \"../spare\" }\n",
    )
    .unwrap();
    assert_eq!(shared_digest(root), before);

    std::fs::write(root.join("crates/library/src/lib.rs"), "// after\n").unwrap();
    assert_ne!(shared_digest(root), before);
}

/// The build reads everything else of a dependency's manifest, and the manifest of the crate
/// that holds the examples in full, since `cargo build` compiles that crate's development
/// dependencies for them.
#[test]
fn component_digest_follows_the_dependency_manifest_and_the_example_crate_manifest() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    closure_workspace(root);
    let before = shared_digest(root);

    let library = root.join("crates/library/Cargo.toml");
    let original = std::fs::read_to_string(&library).unwrap();
    std::fs::write(&library, original.replace("0.1.0", "0.2.0")).unwrap();
    let changed = shared_digest(root);
    assert_ne!(changed, before);

    std::fs::write(&library, original).unwrap();
    let component = root.join("crates/component-fixture/Cargo.toml");
    let original = std::fs::read_to_string(&component).unwrap();
    std::fs::write(
        &component,
        format!("{original}[dev-dependencies]\nlibrary = {{ path = \"../library\" }}\n"),
    )
    .unwrap();
    assert_ne!(shared_digest(root), before);
}

/// What each artifact hashes, from the real workspace's metadata.
fn workspace_plan(root: &Path) -> super::plan::Plan {
    let workspace = Workspace::read(Path::new(env!("CARGO")), root).unwrap();
    let component = workspace.component_crate().unwrap();
    plan(&workspace, Some(&component)).unwrap()
}

/// Whether a `test_prepare_inputs` pattern covers a file: the pattern names the file, or it is a
/// directory followed by `/**`, which covers everything below the directory at any depth.
fn covers(pattern: &str, name: &str) -> bool {
    pattern
        .strip_suffix("/**")
        .map_or(pattern == name, |directory| {
            name.strip_prefix(directory)
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

/// Shared CI keys the cached bundle on the files these patterns match, so the patterns name
/// exactly the inputs the preparation fingerprints: a crate it builds from that no pattern
/// covers could change while a stale bundle is restored, and a pattern for anything else
/// rebuilds the bundle for no reason or fails the job when it matches nothing.
#[test]
fn ci_cache_key_names_exactly_the_files_the_preparation_reads() {
    let ci: toml::Table = include_str!("../../../../.github/ci.toml").parse().unwrap();
    let declared: BTreeSet<String> = ci["test_prepare_inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pattern| pattern.as_str().unwrap().to_owned())
        .collect();
    for pattern in &declared {
        let literal = pattern.strip_suffix("/**").unwrap_or(pattern);
        assert!(
            !literal.contains(['*', '?', '[', '{']),
            "unsupported {pattern}"
        );
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plan = workspace_plan(&root);
    let names: BTreeSet<&String> = [&plan.shared, &plan.tool, &plan.browser]
        .into_iter()
        .flatten()
        .filter(|name| !name.starts_with(':'))
        .collect();
    let read = listed(&root, names.iter().map(|name| name.as_str())).unwrap();

    let derived: BTreeSet<String> = names
        .into_iter()
        .filter_map(|name| {
            let directory = format!("{name}/**");
            if read.contains(name) {
                Some(name.clone())
            } else {
                read.iter()
                    .any(|file| covers(&directory, file))
                    .then_some(directory)
            }
        })
        .collect();

    let missing: Vec<&String> = derived.difference(&declared).collect();
    let extra: Vec<&String> = declared.difference(&derived).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "no pattern for {missing:?}; patterns outside the inputs: {extra:?}"
    );
}

/// The locked metadata of a workspace whose members `a` to `g` depend on each other.
const FIXTURE: &str = include_str!("../../tests/fixtures/selection-metadata.json");

/// The directories the closure of `built` names in the fixture workspace.
fn closure_of(built: &[&str]) -> Vec<String> {
    let metadata: Metadata = serde_json::from_str(FIXTURE).unwrap();
    closure_directories(&metadata, built)
        .unwrap()
        .into_iter()
        .collect()
}

/// The closure follows normal, development and build dependencies and cycles, and leaves out
/// the callers of a package and a dependency that sits outside the workspace.
#[test]
fn closure_directories_hold_the_built_packages_and_what_they_depend_on() {
    assert_eq!(
        closure_of(&["d"]),
        ["crates/a", "crates/b", "crates/c", "crates/d", "crates/f"]
    );
    assert_eq!(closure_of(&["external-reader"]), ["crates/external-reader"]);
    assert_eq!(
        closure_of(&["alone", "nested"]),
        ["crates/a/nested", "crates/alone"]
    );
}

/// A built package the workspace does not contain is an error, so a rename cannot empty the set.
#[test]
fn closure_directories_reject_a_missing_built_package() {
    let metadata: Metadata = serde_json::from_str(FIXTURE).unwrap();
    assert!(closure_directories(&metadata, &["missing"]).is_err());
}

/// The components build for WebAssembly, so the native-only edges of their crates, the models
/// and the application among them, are left out.
#[test]
fn component_crates_hold_only_what_the_wasm_build_uses() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shared = workspace_plan(&root).shared;
    assert_eq!(
        shared
            .iter()
            .filter(|path| path.starts_with("crates/"))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "crates/maestro-entry",
            "crates/maestro-extensions-wasm",
            "crates/maestro-json",
            "crates/maestro-listing",
            "crates/maestro-package",
            "crates/maestro-request",
            "crates/maestro-subagents",
            "crates/maestro-text",
            "crates/maestro-tool"
        ]
    );
}

/// The shared bundle digest follows a fixed component's package, not unrelated workspace code.
#[test]
fn component_bundle_tracks_subagent_sources_but_not_unrelated_packages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(scratch.path())
            .status()
            .unwrap()
            .success()
    );
    let source = scratch.path().join("crates/maestro-subagents/src/lib.rs");
    let unrelated = scratch.path().join("crates/maestro-chat/src/lib.rs");
    for file in [&source, &unrelated] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "before").unwrap();
    }
    let inputs = Inputs::new(
        scratch.path(),
        workspace_plan(&root).shared,
        BTreeSet::new(),
        BTreeSet::new(),
    )
    .unwrap();

    std::fs::write(&unrelated, "after").unwrap();
    assert!(inputs.is_current(scratch.path()).unwrap());
    std::fs::write(&source, "after").unwrap();
    assert!(!inputs.is_current(scratch.path()).unwrap());
}

/// A lock file with a package `a` that builds on `b` and `c`, and a package `d` nothing needs.
const LOCK: &str = "version = 4\n\n\
    [[package]]\nname = \"a\"\nversion = \"1.0.0\"\ndependencies = [\n \"b 1.0.0\",\n \"c\",\n]\n\n\
    [[package]]\nname = \"b\"\nversion = \"1.0.0\"\ndependencies = [\n \"c\",\n]\n\n\
    [[package]]\nname = \"c\"\nversion = \"1.0.0\"\n\n\
    [[package]]\nname = \"d\"\nversion = \"1.0.0\"\n";

/// `LOCK` with the version of package `name` raised.
fn bumped(name: &str) -> String {
    LOCK.replace(
        &format!("name = \"{name}\"\nversion = \"1.0.0\""),
        &format!("name = \"{name}\"\nversion = \"2.0.0\""),
    )
}

/// The lock closure holds the named packages and everything they build on, and the digest
/// follows the entries of exactly those packages.
#[test]
fn lock_digest_follows_only_the_closure_of_the_named_packages() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let lock = scratch.path().join("Cargo.lock");
    std::fs::write(&lock, LOCK).unwrap();
    let roots = BTreeSet::from(["b".to_owned()]);
    let closure = locked_closure(scratch.path(), &roots).unwrap();
    assert_eq!(closure, BTreeSet::from(["b".to_owned(), "c".to_owned()]));
    let digest = |text: &str| {
        std::fs::write(&lock, text).unwrap();
        Inputs::new(
            scratch.path(),
            BTreeSet::new(),
            closure.clone(),
            BTreeSet::new(),
        )
        .unwrap()
        .digest
    };
    let before = digest(LOCK);
    assert_eq!(digest(&bumped("d")), before);
    assert_eq!(digest(&bumped("a")), before);
    assert_ne!(digest(&bumped("c")), before);
}

/// The files a dependency rule names, in a checkout of `examples/a.rs`, `examples/b.rs`,
/// `examples/c.rs` and the shared `lib.rs`, with the rule `sources` of a component built at
/// `out/a.wasm`.
fn rule_files(sources: &str) -> std::io::Result<BTreeSet<String>> {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    for name in ["examples/a.rs", "examples/b.rs", "examples/c.rs", "lib.rs"] {
        std::fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
        std::fs::write(root.join(name), name).unwrap();
    }
    std::fs::create_dir_all(root.join("out")).unwrap();
    let root_text = root.display().to_string();
    std::fs::write(
        root.join("out/a.d"),
        format!("@/out/a.wasm: {sources}\n\n").replace('@', &root_text),
    )
    .unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap()
            .success()
    );
    let shared = BTreeSet::from(["lib.rs".to_owned()]);
    let inputs = component_inputs(root, &root.join("out/a.wasm"), &shared)?;
    inputs.files(root)
}

/// A component records the files of its dependency rule that sit in the checkout and that the
/// shared inputs do not cover, whether the rule spells them absolutely, relatively or through
/// `..`; and it refuses a rule it cannot read exactly.
#[test]
fn component_inputs_hold_the_example_files_of_the_dependency_rule() {
    let held = rule_files(
        "@/crates/x/../../examples/a.rs examples/b.rs @/lib.rs /elsewhere/c.rs @/missing.rs @/examples",
    )
    .unwrap();
    assert_eq!(
        held,
        BTreeSet::from(["examples/a.rs".to_owned(), "examples/b.rs".to_owned()])
    );
    let escaped = rule_files("@/examples/a\\ b.rs").unwrap_err();
    assert!(
        escaped.to_string().contains("unsupported path"),
        "{escaped}"
    );
}

/// The setup example exports the pinned search programs beside the components; staging takes the
/// components alone, so a search program with no dependency file is never read as a component.
#[test]
fn staging_stages_components_alone() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    let checkout = root.join("checkout");
    let output = root.join("output");
    let target = root.join("target");
    for directory in [
        checkout.join("examples"),
        checkout.join("out"),
        output.clone(),
        target.clone(),
    ] {
        std::fs::create_dir_all(directory).unwrap();
    }
    let built = checkout.join("out/a.wasm");
    std::fs::write(checkout.join("examples/a.rs"), "example").unwrap();
    std::fs::write(&built, b"component").unwrap();
    std::fs::write(
        checkout.join("out/a.d"),
        format!("{}: examples/a.rs\n\nexamples/a.rs:\n", built.display()),
    )
    .unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&checkout)
            .status()
            .unwrap()
            .success()
    );
    // A hosted tool install keeps its program below a versioned directory with no extension.
    let install = checkout
        .join(".local/share/mise/installs/ripgrep/15.2.0/ripgrep-15.2.0-x86_64-unknown-linux-musl");
    std::fs::create_dir_all(&install).unwrap();
    let rg = install.join(executable("rg", EXE_EXTENSION));
    std::fs::write(&rg, b"search program").unwrap();
    std::fs::write(
        environment_file(&target),
        format!(
            "MAESTRO_TEST_RG={}\nMAESTRO_AUTHOR_COMPONENT={}\n",
            rg.display(),
            built.display()
        ),
    )
    .unwrap();
    let shared = Inputs::new(&checkout, BTreeSet::new(), BTreeSet::new(), BTreeSet::new()).unwrap();
    let staged = stage_components(&checkout, &output, &target, &shared, &[]).unwrap();
    assert_eq!(
        staged.keys().map(String::as_str).collect::<Vec<_>>(),
        ["MAESTRO_AUTHOR_COMPONENT"]
    );
}
