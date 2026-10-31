//! Platform viewer selection belongs to one application utility.
#![cfg(test)]

use crate::support::{Workspace, source};
use maestro_test_conventions::check_workspace;

#[test]
fn platform_viewers_reject_bypasses_in_fragments_and_nested_groups() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat"]);
    let copies = [
        "let program = match platform { MacOs => \"open\", Windows => \"explorer.exe\", _ => \"xdg-open\" };",
        "fn launch() { capture!(Command::new(\"xdg-open\")); }",
        "fn launch() { let run = || Command::new(\"explorer.exe\"); }",
        "impl Host { fn launch(&self) { Command::new(\"xdg-open\"); } }",
        "fn launch() { let args = [\"/c\", \"start\", target]; Command::new(\"cmd\").args(args); }",
    ];
    let mut accepted = Vec::new();
    for contents in copies {
        let path = source(&workspace, "maestro-chat", "src/viewer.rs", contents);
        match check_workspace(workspace.root.path()) {
            Ok(()) => accepted.push(contents),
            Err(error) => {
                assert!(error.contains("utils::platform_opener"), "{error}");
                assert!(error.contains(&path.display().to_string()), "{error}");
            }
        }
        std::fs::remove_file(path).unwrap();
    }
    assert!(
        accepted.is_empty(),
        "accepted platform bypasses: {accepted:?}"
    );
}

#[test]
fn platform_viewers_accept_owner_calls_and_distinct_process_policies() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app"]);
    source(
        &workspace,
        "maestro-app",
        "src/utils/platform_opener.rs",
        "fn launch() { Command::new(\"xdg-open\").arg(target); }",
    );
    source(
        &workspace,
        "maestro-app",
        "src/caller.rs",
        "fn launch() { utils::platform_opener(target, platform); let _ = utils::platform_opener(target, platform); Command::new(editor).args(files); Command::new(\"pbcopy\"); Command::new(\"cmd\").args([\"/c\", editor]); }",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
