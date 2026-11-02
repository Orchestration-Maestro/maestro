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
        "fn launch() { capture!(Command::new(\"open\")); }",
        "fn launch() { let run = || maestro_process::command(\"open\"); }",
        "impl Host { fn launch(&self) { Command::new(r#\"open\"#); } }",
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
fn platform_viewers_accept_test_gated_items_without_hiding_production() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat"]);
    let cases = [
        "#[cfg(test)] mod tests { #[test] fn viewer_name() { assert_eq!(\"xdg-open\", \"xdg-open\"); } }",
        "mod nested { #[cfg(test)] mod tests { fn launch() { Command::new(\"open\"); } } }",
        "#[cfg(test)] fn launch() { Command::new(\"explorer.exe\"); }",
        "impl Host { #[cfg(test)] fn launch(&self) { Command::new(\"xdg-open\"); } }",
    ];
    for contents in cases {
        let path = source(&workspace, "maestro-chat", "src/viewer.rs", contents);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{contents}");

        std::fs::write(
            &path,
            format!("{contents}\nfn launch() {{ Command::new(\"xdg-open\"); }}"),
        )
        .unwrap();
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("viewer.rs:2: platform viewer"), "{error}");
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn platform_viewers_reject_bound_macos_executables() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat"]);
    let copies = [
        "let program = \"open\"; Command::new(program).arg(target).spawn();",
        "fn launch() { let program = \"open\"; Command::new(program).arg(target).spawn(); }",
        "fn launch() { capture! { let program = \"open\"; Command::new(program); } }",
        "fn launch() { let run = || { let program: &str = r#\"open\"#; maestro_process::command(program); }; }",
        "impl Host { fn launch(&self) { let mut program = \"open\"; Command::new(program); } }",
        "fn launch() { let program = \"open\"; let run = || capture!(Command::new(program)); }",
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
    for contents in [
        "fn label() { let program = \"open\"; Widget::new(program); Command::new(editor); }",
        "fn launch() { let program = \"open\"; { let program = \"editor\"; Command::new(program); } }",
        "fn label() { let program = \"open\"; } fn launch() { Command::new(program); }",
    ] {
        let path = source(&workspace, "maestro-chat", "src/viewer.rs", contents);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{contents}");
        std::fs::remove_file(path).unwrap();
    }
    assert!(accepted.is_empty(), "accepted bound viewers: {accepted:?}");
}

#[test]
fn platform_viewers_reject_shadowed_initializer_executables() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat"]);
    let copies = [
        "fn launch(target: &str) { let program = \"open\"; let mut program = Command::new(program); let _ = program.arg(target).spawn(); }",
        "let program = \"open\"; let program = maestro_process::command(program);",
        "fn launch() { capture! { let program = \"open\"; let program = Command::new(program); } }",
        "impl Host { fn launch(&self) { let program = \"open\"; let program = Command::new(program); } }",
        "fn launch() { let program = \"open\"; let run = || { let program = Command::new(program); }; }",
        "fn launch() { let program = \"open\"; let program = { Command::new(program) }; }",
    ];
    let mut accepted = Vec::new();
    for contents in copies {
        let path = source(&workspace, "maestro-chat", "src/viewer.rs", contents);
        match check_workspace(workspace.root.path()) {
            Ok(()) => accepted.push(contents),
            Err(error) => assert!(error.contains("utils::platform_opener"), "{error}"),
        }
        std::fs::remove_file(path).unwrap();
    }
    assert!(
        accepted.is_empty(),
        "accepted shadowed viewers: {accepted:?}"
    );
}

#[test]
fn platform_viewers_respect_closure_parameters_and_captures() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat"]);
    let cases = [
        "fn launch(editor: &str) { let program = \"open\"; let run = |program: &str| Command::new(program); let _ = (program, run(editor)); }",
        "fn launch(editor: &str) { let program = \"open\"; show_label(program); let run = |program: &str| { Command::new(program).spawn() }; let _ = run(editor); }",
        "let program = \"open\"; let run = |(program, _): (&str, bool)| Command::new(program);",
        "fn launch() { let program = \"open\"; capture!(move |program: &str| Command::new(program)); }",
    ];
    for contents in cases {
        let path = source(&workspace, "maestro-chat", "src/viewer.rs", contents);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{contents}");
        std::fs::remove_file(path).unwrap();
    }
    for contents in [
        "fn launch() { let program = \"open\"; let run = |editor: &str| Command::new(program); }",
        "fn launch() { let program = \"open\"; let run = |program: &str| Command::new(program); Command::new(program); }",
        "fn launch() { let program = \"open\"; let run = |program: &str| { let program = \"open\"; Command::new(program); }; }",
        "fn launch() { let program = \"open\"; let run = |editor: &str| { let run = |program: &str| Command::new(program); Command::new(program); }; }",
    ] {
        let path = source(&workspace, "maestro-chat", "src/viewer.rs", contents);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("utils::platform_opener"), "{error}");
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn platform_viewers_distinguish_clap_names_from_process_operands() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat"]);
    let cases = [
        "use clap::Command; fn cli() { Command::new(\"open\").about(\"Open a URL or path\"); }",
        "fn cli() { clap::Command::new(r#\"open\"#); }",
        "use clap::{Command as CliCommand}; fn cli() { CliCommand::new(\"open\"); }",
        "use clap as cli; fn cli() { cli::Command::new(\"open\"); }",
        "use clap::{self as cli}; fn cli() { cli::Command::new(\"open\"); }",
        "fn cli() { use clap::Command; capture!(Command::new(\"open\")); }",
        "use clap::Command; fn cli() { let build = || Command::new(\"open\"); }",
        "use clap::Command; fn cli() { Command::new(\"xdg-open\"); Command::new(\"explorer.exe\"); }",
        "use clap::Command; fn cli() { let name = \"open\"; Command::new(name); }",
        "use clap::Command; fn cli() { Command::new(\"open\",); }",
    ];
    for contents in cases {
        let path = source(&workspace, "maestro-chat", "src/viewer.rs", contents);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{contents}");
        for launch in [
            "fn launch() { std::process::Command::new(\"open\"); }",
            "fn launch() { maestro_process::command(\"open\"); }",
            "fn launch() { let program = \"open\"; std::process::Command::new(program); }",
            "mod nested { use std::process::Command; fn launch() { Command::new(\"open\"); } }",
            "fn launch() { clap::Command::new({ std::process::Command::new(\"open\"); \"open\" }); }",
            "mod nested { #[cfg(unix)] use std::process::Command; #[cfg(windows)] use clap::Command; fn launch() { Command::new(\"open\"); } }",
            "mod nested { #[cfg(windows)] use clap::Command; #[cfg(unix)] use std::process::Command; fn launch() { Command::new(\"open\"); } }",
        ] {
            std::fs::write(&path, format!("{contents}\n{launch}")).unwrap();
            let error = check_workspace(workspace.root.path()).unwrap_err();
            assert!(error.contains("viewer.rs:2: platform viewer"), "{error}");
            assert!(!error.contains("viewer.rs:1:"), "{error}");
        }
        std::fs::remove_file(path).unwrap();
    }
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
        "fn launch() { utils::platform_opener(target, platform); let _ = utils::platform_opener(target, platform); Command::new(editor).args(files); Command::new(\"pbcopy\"); Command::new(\"cmd\").args([\"/c\", editor]); let label = \"open\"; Widget::new(\"open\"); }",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
