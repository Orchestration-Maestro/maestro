//! Bounded domain exports through the application.
#![cfg(test)]

use crate::support::{Workspace, source};
use maestro_test_conventions::check_workspace;

#[test]
fn application_exports_reject_crates_modules_and_wildcards() {
    let workspace = Workspace::new();
    workspace.foundation(&[
        "maestro-app",
        "maestro-daemon",
        "maestro-terminal",
        "maestro-markdown",
    ]);
    for owner in ["maestro-daemon", "maestro-terminal", "maestro-markdown"] {
        source(
            &workspace,
            owner,
            "src/lib.rs",
            "pub mod detail { pub struct Record; pub fn operation() {} }",
        );
        let name = owner.replace('-', "_");
        for export in [
            format!("pub use {name} as domain;"),
            format!("pub use {name}::{{self as domain}};"),
            format!("pub use {name}::*;"),
            format!("pub use {name}::detail as domain;"),
            format!("pub use {name}::detail::{{self as domain, Record}};"),
            format!("pub use {name}::detail::*;"),
            format!("use {name} as renamed; pub use renamed::detail as domain;"),
            format!("use {name}::detail as renamed; pub use renamed as domain;"),
            format!("pub extern crate {name} as domain;"),
        ] {
            source(&workspace, "maestro-app", "src/lib.rs", &export);
            let error = check_workspace(workspace.root.path()).unwrap_err();
            assert!(
                error.contains("named application exports") && error.contains("src/lib.rs:1:"),
                "{error}"
            );
        }
        source(
            &workspace,
            "maestro-app",
            "src/lib.rs",
            &format!("pub mod domain {{ pub use {name}::detail::{{Record, operation}}; }}"),
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn application_exports_follow_alias_chains_and_out_of_line_modules() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-daemon"]);
    source(&workspace, "maestro-daemon", "src/lib.rs", "pub mod pane;");
    source(
        &workspace,
        "maestro-daemon",
        "src/pane.rs",
        "pub mod terminal { pub struct Record; }",
    );
    for export in [
        "pub use last as domain; use middle as last; use maestro_daemon::pane::terminal as middle;",
        "pub use maestro_daemon::pane::terminal as domain;",
        "use maestro_daemon as d; pub use d::pane::terminal::*;",
    ] {
        source(&workspace, "maestro-app", "src/exports.rs", export);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("named application exports"), "{error}");
    }
    source(
        &workspace,
        "maestro-app",
        "src/exports.rs",
        "use maestro_daemon::pane::terminal as t; pub use t::Record as View;",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn application_exports_ignore_private_uses_comments_and_strings() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-terminal"]);
    source(
        &workspace,
        "maestro-app",
        "src/lib.rs",
        r#"
use maestro_terminal as terminal;
// pub use maestro_terminal as terminal;
const EXAMPLE: &str = "pub use maestro_terminal::*;";
fn read() { drop(terminal::Terminal::new(1, 1, 0)); }
"#,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
