//! Inspects the built component: native-async lifts and lowers, WASI 0.2 basics only, no polling in the contract.

use std::collections::BTreeSet;

use wasmparser::{CanonicalFunction, CanonicalOption, Parser, Payload};
use wit_component::DecodedWasm;
use wit_parser::{FunctionKind, Resolve, World, WorldItem};

/// Path of the built example component.
const COMPONENT: &str = "../target/wasm32-wasip2/debug/extension_example_component.wasm";

/// Error type of the inspection helpers.
type Failure = Box<dyn std::error::Error>;

/// Names of exported functions, split into async and plain ones.
fn exported_functions(resolve: &Resolve, world: &World) -> (BTreeSet<String>, BTreeSet<String>) {
    let (mut asynchronous, mut synchronous) = (BTreeSet::new(), BTreeSet::new());
    for item in world.exports.values() {
        let WorldItem::Interface { id, .. } = item else {
            continue;
        };
        for (name, function) in &resolve.interfaces[*id].functions {
            let set = if matches!(function.kind, FunctionKind::AsyncFreestanding) {
                &mut asynchronous
            } else {
                &mut synchronous
            };
            set.insert(name.clone());
        }
    }
    (asynchronous, synchronous)
}

/// Counts canonical functions that use the async ABI: (lifts with a callback, lowers).
fn async_canonicals(bytes: &[u8]) -> Result<(usize, usize), Failure> {
    let (mut lifts, mut lowers) = (0, 0);
    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::ComponentCanonicalSection(section) = payload? else {
            continue;
        };
        for function in section {
            match function? {
                CanonicalFunction::Lift { options, .. } if is_async(&options) => {
                    assert!(
                        options
                            .iter()
                            .any(|option| matches!(option, CanonicalOption::Callback(_))),
                        "an async lift carries its callback"
                    );
                    lifts += 1;
                }
                CanonicalFunction::Lower { options, .. } if is_async(&options) => lowers += 1,
                _ => {}
            }
        }
    }
    Ok((lifts, lowers))
}

/// Whether the canonical options select the async ABI.
fn is_async(options: &[CanonicalOption]) -> bool {
    options
        .iter()
        .any(|option| matches!(option, CanonicalOption::Async))
}

/// The exports are async lifts with callbacks and the host operations are async lowers.
#[test]
fn component_exports_use_the_async_canonical_abi() -> Result<(), Failure> {
    let bytes = std::fs::read(COMPONENT)?;
    let DecodedWasm::Component(resolve, world) = wit_component::decode(&bytes)? else {
        return Err("the artifact is not a component".into());
    };
    let (asynchronous, synchronous) = exported_functions(&resolve, &resolve.worlds[world]);
    let expected = [
        "invoke-command",
        "invoke-input",
        "invoke-tool",
        "invoke-with-session",
        "start",
    ];
    assert_eq!(
        asynchronous.iter().map(String::as_str).collect::<Vec<_>>(),
        expected
    );
    for name in ["release", "prepare-arguments", "invoke-renderer"] {
        assert!(synchronous.contains(name), "{name} stays a plain export");
    }
    assert_eq!(
        async_canonicals(&bytes)?,
        (5, 2),
        "five async exports and the two async imports use the async ABI"
    );
    Ok(())
}

/// The component imports only the author world and WASI 0.2 basics.
#[test]
fn component_imports_only_the_author_world_and_wasi_basics() -> Result<(), Failure> {
    let bytes = std::fs::read(COMPONENT)?;
    let DecodedWasm::Component(resolve, world) = wit_component::decode(&bytes)? else {
        return Err("the artifact is not a component".into());
    };
    let imports: Vec<String> = resolve.worlds[world]
        .imports
        .keys()
        .map(|key| resolve.name_world_key(key))
        .collect();
    let author: Vec<&str> = imports
        .iter()
        .map(String::as_str)
        .filter(|name| name.starts_with("maestro:"))
        .collect();
    assert_eq!(
        author,
        [
            "maestro:extension/types@0.1.0",
            "maestro:extension/host@0.1.0"
        ]
    );
    let wasi = imports.iter().filter(|name| name.starts_with("wasi:"));
    assert!(
        wasi.clone().all(|name| name.contains("@0.2.")),
        "only WASI 0.2 basics are imported"
    );
    let contract_functions = resolve
        .interfaces
        .iter()
        .filter(|(_, interface)| {
            interface
                .package
                .is_some_and(|package| resolve.packages[package].name.namespace == "maestro")
        })
        .flat_map(|(_, interface)| interface.functions.keys());
    assert!(
        contract_functions
            .into_iter()
            .all(|name| !name.contains("poll")),
        "the author contract has no polling function"
    );
    Ok(())
}
