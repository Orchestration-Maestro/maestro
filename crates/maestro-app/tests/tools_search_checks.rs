//! Native directory listing through the application's executable tool factory.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::{run_io, session_harness::case};
use serde_json::json;

#[test]
fn ls_keeps_dotfiles_and_directory_suffix() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    std::fs::write(root.path().join(".hidden-file"), "secret").unwrap();
    std::fs::create_dir(root.path().join(".hidden-dir")).unwrap();
    let tool = maestro_app::create_ls_tool(
        root.path().display().to_string(),
        maestro_app::LsToolOptions::default(),
    );
    let result = run_io((tool.execute)("ls".into(), json!({}), None, None)).unwrap();
    assert_eq!(
        json!(result),
        case("tools_root", "ls_keeps_dotfiles_and_directory_suffix")["expected"]["ok"]
    );
}
