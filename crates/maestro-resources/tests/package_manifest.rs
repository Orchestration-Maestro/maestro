//! Native manifest admission and conventional extension indexes.
#![cfg(not(target_arch = "wasm32"))]

use maestro_resources::package_manifest::{read_maestro_manifest_file, resolve_extension_entries};
use maestro_test_directory::Scratch;

/// Non-object branding members are absent, so extension directories keep their index fallback.
#[test]
fn non_object_manifest_members_are_absent_and_keep_extension_indexes() {
    let scratch = Scratch::new().unwrap();
    let dir = scratch.path().to_str().unwrap();
    let manifest = scratch.path().join("package.json");
    let index = maestro_path::join(&[dir, "index.wasm"]);
    std::fs::write(&index, "").unwrap();
    for value in ["5", "true", "\"x\"", "[]"] {
        std::fs::write(&manifest, format!("{{\"maestro\":{value}}}")).unwrap();

        assert!(
            read_maestro_manifest_file(manifest.to_str().unwrap()).is_none(),
            "{value}"
        );
        assert_eq!(
            resolve_extension_entries(dir, true).unwrap(),
            Some(vec![index.clone()])
        );
    }
}
