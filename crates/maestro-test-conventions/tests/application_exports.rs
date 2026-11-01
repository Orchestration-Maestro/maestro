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
fn application_exports_follow_qualified_aliases_in_their_module_scope() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-daemon"]);
    source(
        &workspace,
        "maestro-daemon",
        "src/lib.rs",
        "pub mod pane { pub mod terminal { pub struct Record; } }",
    );
    for export in [
        "use maestro_daemon as d; pub mod x { pub use super::d::pane::*; }",
        "use maestro_daemon as d; pub use crate::d::pane as p;",
        "use maestro_daemon as d; pub use self::d::pane as p;",
        "mod x { use maestro_daemon as d; pub mod y { pub use super::d::*; } }",
        "use maestro_daemon as d; mod x { pub mod y { pub use super::super::d::*; } }",
        "mod x { use maestro_daemon as d; } pub use crate::x::d::pane as p;",
        "use maestro_daemon::pane::*; pub use terminal as domain;",
    ] {
        source(&workspace, "maestro-app", "src/lib.rs", export);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("named application exports"),
            "{export}: {error}"
        );
    }
}

#[test]
fn application_exports_follow_local_glob_aliases() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-daemon"]);
    source(
        &workspace,
        "maestro-daemon",
        "src/lib.rs",
        "pub mod pane { pub struct Record; }",
    );
    for export in [
        "mod imports { pub(crate) use maestro_daemon as d; } pub use imports::*; pub use d::pane as p;",
        "mod imports { pub(crate) use maestro_daemon as d; } use self::imports::*; pub use d::*;",
        "mod imports { pub(crate) use maestro_daemon as d; } use imports as i; use i::*; pub use d::pane as p;",
        "mod imports { pub(crate) use maestro_daemon as d; } mod relay { pub(crate) use crate::imports::*; } use relay::*; pub use d::pane as p;",
        "mod imports; use imports::*; pub use d::pane as p;",
        "mod imports { pub mod nested { pub(crate) use maestro_daemon as d; } } use imports::*; pub use nested::d::pane as p;",
    ] {
        source(&workspace, "maestro-app", "src/lib.rs", export);
        source(
            &workspace,
            "maestro-app",
            "src/imports.rs",
            "pub(crate) use maestro_daemon as d;",
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("named application exports"),
            "{export}: {error}"
        );
    }
    source(
        &workspace,
        "maestro-app",
        "src/lib.rs",
        "mod imports { pub(crate) use maestro_daemon as d; } use imports::*; pub use d::pane::Record;",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn application_exports_follow_root_external_prelude_aliases() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-terminal"]);
    source(
        &workspace,
        "maestro-terminal",
        "src/lib.rs",
        "pub mod detail { pub struct Record; }",
    );
    for export in [
        "extern crate maestro_terminal as t; pub mod escaped { pub use t::*; }",
        "extern crate maestro_terminal as t; pub mod escaped { use t as alias; pub use alias::detail as d; }",
        "extern crate maestro_terminal as t; pub mod escaped { pub mod nested { pub use t::detail::*; } }",
        "extern crate maestro_terminal as t; pub mod escaped;",
    ] {
        source(&workspace, "maestro-app", "src/lib.rs", export);
        let escaped = if export.ends_with("pub mod escaped;") {
            "pub use t::*;"
        } else {
            ""
        };
        source(&workspace, "maestro-app", "src/escaped.rs", escaped);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("named application exports"),
            "{export}: {error}"
        );
    }
    source(
        &workspace,
        "maestro-app",
        "src/escaped.rs",
        "pub use t::detail::Record;",
    );
    source(
        &workspace,
        "maestro-app",
        "src/lib.rs",
        "extern crate maestro_terminal as t; mod local { pub mod detail {} } mod shadow { use crate::local as t; pub use t::detail; } mod own { pub mod t { pub mod detail {} } pub use t::detail; } pub mod escaped;",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn application_exports_follow_aliases_across_source_files() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-daemon"]);
    source(
        &workspace,
        "maestro-daemon",
        "src/lib.rs",
        "pub mod pane { pub struct Record; }",
    );
    for (root, exports) in [
        (
            "use maestro_daemon as d; pub mod exports;",
            "pub use super::d::pane::*;",
        ),
        (
            "mod imports; pub mod exports;",
            "use crate::imports::d as alias; pub use self::alias::pane as p;",
        ),
        (
            "mod imports; pub use crate::imports::d as domain;",
            "use super::imports::d::pane::Record;",
        ),
    ] {
        source(&workspace, "maestro-app", "src/lib.rs", root);
        source(&workspace, "maestro-app", "src/exports/mod.rs", exports);
        source(
            &workspace,
            "maestro-app",
            "src/imports.rs",
            "pub(crate) use maestro_daemon as d;",
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("named application exports"), "{error}");
    }
}

#[test]
fn application_exports_keep_unrelated_aliases_in_separate_scopes() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-terminal"]);
    source(
        &workspace,
        "maestro-terminal",
        "src/lib.rs",
        "pub mod detail { pub struct Record; }",
    );
    source(
        &workspace,
        "maestro-app",
        "src/lib.rs",
        r"
mod local { pub mod detail { pub struct Record; } }
mod first { use maestro_terminal as t; }
mod second { use crate::local as t; pub use self::t::detail as local_view; }
mod named { use maestro_terminal as t; pub use self::t::detail::Record; }
mod globs { use maestro_terminal::*; pub use self::detail::Record; }
",
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
