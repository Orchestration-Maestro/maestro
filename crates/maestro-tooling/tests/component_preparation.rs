//! Prepared component coverage and reuse at the shared Cargo boundary.

use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, selection};
use std::{collections::HashSet, fs, path::Path};

#[path = "../../maestro-extensions-wasm/src/tests/build/cargo.rs"]
mod component_build;

#[test]
fn component_consumers_have_prepared_examples() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut prepared = HashSet::new();
    maestro_tooling::example_files::prepare(
        &[
            workspace.join("crates/maestro-extensions-wasm/examples"),
            workspace.join("examples/extensions"),
        ],
        |name, _| {
            prepared.insert(name.to_owned());
            Ok(name.into())
        },
    )
    .unwrap();
    for folder in [
        "crates/maestro-extensions-wasm/src/tests",
        "crates/maestro-extensions-wasmtime/tests",
    ] {
        for file in maestro_tooling::example_files::collect(&workspace.join(folder)).unwrap() {
            if file.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            assert_prepared_consumers(&file, &prepared);
        }
    }
}

/// Checks the literal example operands accepted by the shared test builders.
fn assert_prepared_consumers(file: &Path, prepared: &HashSet<String>) {
    let source = fs::read_to_string(file).unwrap();
    for call in ["example_component(", "Harness::example(", "\"--example\","] {
        for (offset, _) in source.match_indices(call) {
            let arguments = source[offset + call.len()..].trim_start();
            let Some(arguments) = arguments.strip_prefix('"') else {
                continue;
            };
            let name = arguments.split('"').next().unwrap();
            assert!(
                prepared.contains(name),
                "setup does not prepare {name}: {}",
                file.display()
            );
        }
    }
}

#[test]
fn component_build_reuses_the_prepared_example() {
    if !selection().is_child() {
        let scratch = Scratch::new().unwrap();
        let directory = scratch.path().join("crates/author");
        fs::create_dir_all(directory.join("examples")).unwrap();
        fs::create_dir_all(scratch.path().join("examples/extensions")).unwrap();
        fs::write(
            directory.join("examples/fixture.rs"),
            "// maestro-component: MAESTRO_PREPARED_FIXTURE_COMPONENT\n",
        )
        .unwrap();
        let component = scratch.path().join("fixture.wasm");
        fs::write(&component, b"\0asm\x0d\0\x01\0").unwrap();
        let output = TestProcess::current("component_build_reuses_the_prepared_example")
            .unwrap()
            .case("prepared")
            .nocapture()
            .env("MAESTRO_PREPARED_FIXTURE_COMPONENT", component)
            .env("MAESTRO_COMPONENT_FIXTURE_ROOT", directory)
            .env("CARGO", scratch.path().join("missing-cargo"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let directory = std::env::var_os("MAESTRO_COMPONENT_FIXTURE_ROOT").unwrap();
    let component =
        component_build::component(Path::new(&directory), &["--example", "fixture"], "fixture")
            .expect("prepared example must not invoke Cargo");
    assert_eq!(
        component,
        std::path::PathBuf::from(std::env::var_os("MAESTRO_PREPARED_FIXTURE_COMPONENT").unwrap())
    );
}
