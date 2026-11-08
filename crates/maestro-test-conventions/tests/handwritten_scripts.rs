#![cfg(test)]

use crate::support::Workspace;
use maestro_test_conventions::check_workspace;

#[test]
fn crate_sources_reject_handwritten_javascript() {
    for name in ["runtime.js", "nested/boot.mjs"] {
        let workspace = Workspace::with_text_member();
        let script = workspace.root.path().join("crates/text/src").join(name);
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(&script, "window.x = 1;\n").unwrap();

        let error = check_workspace(workspace.root.path()).unwrap_err();

        assert!(error.contains(name), "{error}");
        assert!(error.contains("Rust"), "{error}");
    }
}

#[test]
fn generated_assets_and_test_inputs_are_not_crate_sources() {
    let workspace = Workspace::with_text_member();
    for path in [
        "crates/text/assets/runtime.js",
        "crates/text/tests/input.js",
    ] {
        let script = workspace.root.path().join(path);
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(script, "window.x = 1;\n").unwrap();
    }

    check_workspace(workspace.root.path()).unwrap();
}
