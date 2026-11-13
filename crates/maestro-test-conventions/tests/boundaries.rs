#![cfg(test)]

use crate::support;

use std::path::Path;

use maestro_test_conventions::{check_declared_graph, check_workspace};
use support::{Workspace, source};

/// One way a crate declares a dependency on an external library.
struct Declaration {
    /// The manifest table that spells the declaration.
    table: &'static str,
    /// Whether the declaration is optional.
    optional: bool,
    /// Cargo's record of the dependency kind; `None` is a normal dependency.
    kind: Option<&'static str>,
    /// Cargo's record of the target predicate; `None` applies on every target.
    target: Option<&'static str>,
}

/// The predicate of the target-specific declaration tables.
const NONE_TARGET: &str = "cfg(target_os = \"none\")";

/// Every declaration spelling of an external library.
const DECLARATIONS: &[Declaration] = &[
    Declaration {
        table: "dependencies",
        optional: false,
        kind: None,
        target: None,
    },
    Declaration {
        table: "dependencies",
        optional: true,
        kind: None,
        target: None,
    },
    Declaration {
        table: "build-dependencies",
        optional: false,
        kind: Some("build"),
        target: None,
    },
    Declaration {
        table: "dev-dependencies",
        optional: false,
        kind: Some("dev"),
        target: None,
    },
    Declaration {
        table: "target.'cfg(target_os = \"none\")'.dependencies",
        optional: false,
        kind: None,
        target: Some(NONE_TARGET),
    },
    Declaration {
        table: "target.'cfg(target_os = \"none\")'.build-dependencies",
        optional: false,
        kind: Some("build"),
        target: Some(NONE_TARGET),
    },
    Declaration {
        table: "target.'cfg(target_os = \"none\")'.dev-dependencies",
        optional: false,
        kind: Some("dev"),
        target: Some(NONE_TARGET),
    },
];

/// The optional marker the manifest spelling writes after the library path.
fn optional_marker(declaration: &Declaration) -> &'static str {
    if declaration.optional {
        ", optional = true"
    } else {
        ""
    }
}

/// A duplicate interactive session setup outside its shared owner.
const DUPLICATE_SETUP: &str = "fn f() { let value = AgentSessionConfig { agent: owner() }; }";

/// Spellings of a duplicate setup the token walk must still catch.
const DUPLICATE_SETUP_SPELLINGS: &[&str] = &[
    DUPLICATE_SETUP,
    "fn f() { check!(AgentSessionConfig { agent: owner() }); }",
    "fn f() { let make = || AgentSessionConfig { agent: owner() }; }",
    "impl Example { fn make() { drop(AgentSessionConfig { agent: owner() }); } }",
    "fn f() { let _ = session(cwd, None); drop(AgentSessionConfig { agent: owner() }); }",
    "use maestro_app::AgentSessionConfig as Config;\nfn f() { let x = Config { agent: owner() }; }",
    "fn f() { let x = r#AgentSessionConfig { agent: owner() }; }",
];

/// The external libraries only the runtime adapter may declare.
const RUNTIME_LIBRARIES: &[&str] = &[
    "wasmtime",
    "wasmtime-component-macro",
    "wasmtime-wasi",
    "wasmtime-wasi-http",
];

/// The outcome every library declaration by `owner` must report.
fn runtime_expectation(owner: &str, library: &str) -> Result<(), String> {
    if owner == "maestro-extensions-wasmtime" && library != "wasmtime-wasi-http" {
        Ok(())
    } else {
        Err(format!(
            "{owner}: forbidden runtime library dependency {library}"
        ))
    }
}

/// Check every declaration spelling of one library against the shared document.
fn check_runtime_library(root: &Path, declared: &serde_json::Value, owner: &str, library: &str) {
    let external = root.join("external");
    for declaration in DECLARATIONS {
        let document = declared_with(declared, owner, library, declaration, &external);
        assert_eq!(
            check_declared_graph(root, &document),
            runtime_expectation(owner, library),
            "{owner}: {library} in {}",
            declaration.table
        );
    }
}

#[test]
fn runtime_dependencies_stay_in_the_runtime_adapter() {
    let workspace = Workspace::new();
    let owners: Vec<&str> = support::policy::POLICY
        .iter()
        .map(|(owner, _)| *owner)
        .collect();
    workspace.foundation(&owners);
    let declared = workspace.metadata();
    workspace.external("wasmtime");
    // The fixture declares no external library, so every diagnostic below comes from one case.
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for &(owner, _) in support::policy::POLICY {
        for &library in RUNTIME_LIBRARIES {
            // The guest's test-only host dependencies have their own test.
            if owner == "maestro-extensions-wasm" && matches!(library, "wasmtime" | "wasmtime-wasi")
            {
                continue;
            }
            check_runtime_library(workspace.root.path(), &declared, owner, library);
        }
    }
    let workspace = Workspace::new();
    workspace.foundation(&["maestro", "maestro-extensions-wasmtime"]);
    workspace.external("wasmtime");
    let manifest = workspace.root.path().join("crates/maestro/Cargo.toml");
    let baseline = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(manifest, format!(
        "{baseline}maestro-extensions-wasmtime = {{ path = \"../maestro-extensions-wasmtime\" }}\n"
    )).unwrap();
    workspace.member(
        "maestro-extensions-wasmtime",
        "maestro-extensions-wasmtime",
        "[dependencies]\nwasmtime = { path = \"../../external\" }",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions", "maestro-extensions-wasm"]);
    workspace.member(
        "maestro-extensions",
        "maestro-extensions",
        "[dependencies]\nmaestro-extensions-wasm = { path = \"../maestro-extensions-wasm\" }",
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains("maestro-extensions -> maestro-extensions-wasm"),
        "{error}"
    );
    workspace.member("maestro-extensions", "maestro-extensions", "");
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn toolkit_excludes_framework_and_highlighting_dependencies() {
    for library in ["ratatui", "syntect", "two-face"] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-tui"]);
        workspace.external(library);
        for declaration in DECLARATIONS {
            let table = declaration.table;
            let optional = optional_marker(declaration);
            workspace.member(
                "maestro-tui",
                "maestro-tui",
                &format!(
                    "[{table}]\nalias = {{ package = {library:?}, path = \"../../external\"{optional} }}"
                ),
            );
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "maestro-tui: forbidden toolkit library dependency {library}"
                ))
            );
            workspace.member("maestro-tui", "maestro-tui", "");
            assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        }
    }
}

#[test]
fn tool_declarations_have_one_owner() {
    tool_declaration_kinds();
    for name in [
        "ToolDefinition",
        "ToolRenderContext",
        "ToolRenderResultOptions",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-tools", "maestro-tui", "maestro-app"]);
        source(
            &workspace,
            "maestro-tui",
            "src/lib.rs",
            &format!("\npub(crate) struct\n{name}<\nContext\n> {{ context: Context }}"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains(&support::native_path_text("maestro-tui/src/lib.rs:2:"))
                && error.contains(name)
                && error.contains("maestro-tools"),
            "{error}"
        );
        source(
            &workspace,
            "maestro-tui",
            "src/lib.rs",
            &format!("pub use maestro_tools::{name};\nfn use_tool(_: maestro_tools::{name}) {{}}"),
        );
        source(
            &workspace,
            "maestro-tools",
            "src/lib.rs",
            &format!("pub struct {name}<Context> {{ context: Context }}"),
        );
        check_tool_fragment_owner(&workspace, name);
        source(
            &workspace,
            "maestro-tools",
            "other.rs",
            &format!("pub type {name} = ();"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("duplicate declaration") && error.contains(name),
            "{error}"
        );
        std::fs::remove_file(workspace.root.path().join("crates/maestro-tools/other.rs")).unwrap();
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

/// Check admitted, copied and restored tool declarations in a private fragment.
fn check_tool_fragment_owner(workspace: &Workspace, name: &str) {
    source(
        workspace,
        "maestro-app",
        "src/exports/tools.rs",
        &format!("pub use maestro_tools::{name};"),
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        workspace,
        "maestro-app",
        "src/exports/tools.rs",
        &format!("pub struct {name}<Context> {{ context: Context }}"),
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains(&support::native_path_text(
            "maestro-app/src/exports/tools.rs:1:"
        )) && error.contains(name)
            && error.contains("maestro-tools"),
        "{error}"
    );
    source(
        workspace,
        "maestro-app",
        "src/exports/tools.rs",
        &format!("pub use maestro_tools::{name};"),
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn parameter_schema_records_have_one_owner() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-interchange", "maestro-models", "maestro-tui"]);
    source(
        &workspace,
        "maestro-tui",
        "src/lib.rs",
        "\npub(crate) struct\nParameterSchema { }",
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains(&support::native_path_text("maestro-tui/src/lib.rs:2:"))
            && error.contains("maestro-interchange"),
        "{error}"
    );
    source(
        &workspace,
        "maestro-tui",
        "src/lib.rs",
        "pub use maestro_interchange::parameter_schema::{ParameterSchema, SchemaKind};\n\
         fn use_schema(_: ParameterSchema, _: SchemaKind) {}",
    );
    source(
        &workspace,
        "maestro-interchange",
        "src/lib.rs",
        "pub struct ParameterSchema { }\npub enum SchemaKind { String }\n\
         pub struct Tool { parameters: ParameterSchema }\n\
         pub struct ToolInfo<P = ParameterSchema> { parameters: P }",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        &workspace,
        "maestro-interchange",
        "src/lib.rs",
        "pub struct ParameterSchema { }\npub enum SchemaKind { String }",
    );
    source(
        &workspace,
        "maestro-models",
        "src/lib.rs",
        "pub struct ParameterSchema { }",
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains(&support::native_path_text("maestro-models/src/lib.rs:1:"))
            && error.contains("ParameterSchema")
            && error.contains("maestro-interchange"),
        "{error}"
    );
}

#[test]
fn application_selectors_stay_in_chat() {
    let components = [
        "ConfigSelectorComponent",
        "ExtensionSelectorComponent",
        "ModelSelectorComponent",
        "OAuthSelectorComponent",
        "ScopedModelsSelectorComponent",
        "SessionSelectorComponent",
        "SettingsSelectorComponent",
        "ShowImagesSelectorComponent",
        "ThemeSelectorComponent",
        "ThinkingSelectorComponent",
        "TreeSelectorComponent",
        "UserMessageSelectorComponent",
    ]
    .map(|component| format!("pub struct {component};"));
    let modules = ["oauth_selector", "session_selector_search"]
        .map(|module| format!("pub(crate) mod\n{module} {{}}"));
    let declarations: Vec<_> = components.into_iter().chain(modules).collect();
    for owner in ["maestro-tui", "maestro-app"] {
        let workspace = Workspace::new();
        workspace.foundation(&[owner, "maestro-chat"]);
        for declaration in &declarations {
            source(&workspace, owner, "src/lib.rs", declaration);
            let error = check_workspace(workspace.root.path()).unwrap_err();
            assert!(
                error.contains(owner) && error.contains("belongs to maestro-chat"),
                "{error}"
            );
        }
        source(
            &workspace,
            owner,
            "src/lib.rs",
            "pub struct SelectList; pub struct SettingsList;",
        );
        source(
            &workspace,
            "maestro-chat",
            "src/lib.rs",
            &declarations.join("\n"),
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn source_boundary_scans_ignore_literals_and_comments() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tui"]);
    let innocent = r####"
// struct ToolDefinition; mod session_selector_search;
/* struct ToolRenderContext; /* struct ConfigSelectorComponent; */ */
#[doc = "struct ToolRenderResultOptions;"]
const QUOTED: &str = "escaped \" struct ToolDefinition;";
const BYTES: &[u8] = b"mod session_selector_search;";
const RAW: &str = r###"struct ToolRenderContext; /* struct ConfigSelectorComponent; */"###;
const RAW_BYTES: &[u8] = br##"struct ToolDefinition; mod session_selector_search;"##;
const CHARACTER: char = '"';
const ESCAPED: char = '\'';
const UNICODE: char = 'é';
fn borrow<'a>(_: &'a str) {}
pub use maestro_tools::ToolDefinition;
pub struct SelectList;
pub struct SettingsList;
"####;
    source(&workspace, "maestro-tui", "src/lib.rs", innocent);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for declaration in [
        "pub(in crate::inner) struct\nToolDefinition<\nContext\n> { context: Context }",
        "pub(crate) type ToolRenderContext<Context> = Context;",
        "pub(super) mod\nsession_selector_search {}",
        "pub enum ConfigSelectorComponent<Context> { Item(Context) }",
    ] {
        source(
            &workspace,
            "maestro-tui",
            "src/lib.rs",
            &format!("{innocent}\n{declaration}"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        let line = innocent.lines().count() + 2;
        let location = format!("{}:{line}:", support::native_path_text("src/lib.rs"));
        assert!(error.contains(&location), "{error}");
        source(&workspace, "maestro-tui", "src/lib.rs", innocent);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn wit_codegen_inputs_share_one_canonical_source() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    let guest = workspace
        .root
        .path()
        .join("crates/maestro-extensions-wasm/interfaces");
    std::fs::create_dir(&guest).unwrap();
    std::fs::write(
        guest.join("world.wit"),
        "package maestro:fixture; world fixture {}",
    )
    .unwrap();
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        r#"wit_bindgen::generate!({ path: "interfaces", world: "fixture" });"#,
    );
    for host in [
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/interfaces/../interfaces", world: "fixture" });"#,
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/interfaces" });"#,
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/interfaces/world.wit" });"#,
        r##"wasmtime::component::bindgen!({ path: [r#"../maestro-extensions-wasm/interfaces"#, "../maestro-extensions-wasm/interfaces/world.wit"] });"##,
    ] {
        source(&workspace, "maestro-extensions-wasmtime", "build.rs", host);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
    reject_noncanonical_wit(&workspace, &guest);
    wit_ownership_without_both_sides();
    #[cfg(unix)]
    wit_symlink_ownership(&workspace, &guest);
}

fn reject_noncanonical_wit(workspace: &Workspace, guest: &std::path::Path) {
    let second = workspace
        .root
        .path()
        .join("crates/maestro-extensions-wasm/second");
    std::fs::create_dir(&second).unwrap();
    std::fs::copy(guest.join("world.wit"), second.join("world.wit")).unwrap();
    let copied = workspace
        .root
        .path()
        .join("crates/maestro-extensions-wasmtime/copied");
    std::fs::create_dir(&copied).unwrap();
    std::fs::copy(guest.join("world.wit"), copied.join("world.wit")).unwrap();
    for &(host, expected) in NONCANONICAL_WIT {
        source(workspace, "maestro-extensions-wasmtime", "build.rs", host);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("build.rs:1:") && error.contains(expected),
            "{error}"
        );
        source(
            workspace,
            "maestro-extensions-wasmtime",
            "build.rs",
            r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/interfaces" });"#,
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
    source(
        workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        r#"wit_bindgen::generate!({ path: "../maestro-extensions-wasmtime/copied" });"#,
    );
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("guest-owned")
    );
    source(
        workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        r#"wit_bindgen::generate!({ path: ["interfaces"] });"#,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn wit_options_ignore_module_paths_in_nested_values() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    let guest = workspace
        .root
        .path()
        .join("crates/maestro-extensions-wasm/interfaces");
    std::fs::create_dir(&guest).unwrap();
    std::fs::write(
        guest.join("world.wit"),
        "package maestro:fixture; world fixture {}",
    )
    .unwrap();
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        r#"wit_bindgen::generate!({
            path: "interfaces",
            with: {
                "maestro:fixture/types": crate::path::bindings,
                "maestro:fixture/other": crate::inline::bindings,
            },
        });"#,
    );
    for input in [
        r#"wasmtime::component::bindgen!({
            path: "../maestro-extensions-wasm/interfaces",
            with: {
                "maestro:fixture/types": crate::path::bindings,
                "maestro:fixture/other": crate::inline::bindings,
            },
        });"#,
        r#"wasmtime::component::bindgen!({
            with: {
                "maestro:fixture/types": crate::path::bindings,
                "maestro:fixture/other": crate::inline::bindings,
            },
            path: "../maestro-extensions-wasm/interfaces",
        });"#,
    ] {
        source(&workspace, "maestro-extensions-wasmtime", "build.rs", input);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{input}");
        source(
            &workspace,
            "maestro-extensions-wasmtime",
            "build.rs",
            &input.replace("../maestro-extensions-wasm/interfaces", "wrong-root"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("cannot resolve WIT input"), "{error}");
    }
}

#[test]
fn inline_guest_generator_adds_no_shared_wit_root() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    for directory in ["wit", "wit/standard"] {
        let root = workspace
            .root
            .path()
            .join("crates/maestro-extensions-wasm")
            .join(directory);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("world.wit"),
            "package maestro:fixture; world fixture {}",
        )
        .unwrap();
    }
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        concat!(
            r#"wit_bindgen::generate!({ path: "wit" });"#,
            r#"pub mod standard { wit_bindgen::generate!({ inline: "package maestro:fixture; world fixture {}", path: "wit/standard" }); }"#,
        ),
    );
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/wit" });"#,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn wit_shorthand_world_uses_manifest_relative_wit_directory() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    for directory in ["wit", "fixture"] {
        let root = workspace
            .root
            .path()
            .join("crates/maestro-extensions-wasm")
            .join(directory);
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("world.wit"),
            "package maestro:fixture; world fixture {}",
        )
        .unwrap();
    }
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        r#"wit_bindgen::generate!("fixture");"#,
    );
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/fixture" });"#,
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(error.contains("same canonical source"), "{error}");
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/wit" });"#,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!("fixture");"#,
    );
    let host_wit = workspace
        .root
        .path()
        .join("crates/maestro-extensions-wasmtime/wit");
    std::fs::create_dir(&host_wit).unwrap();
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(error.contains("guest-owned"), "{error}");
}

#[test]
fn raw_identifier_declarations_keep_their_owners() {
    for (declaration, owner, name) in [
        (
            "pub struct r#ToolDefinition;",
            "maestro-tools",
            "ToolDefinition",
        ),
        (
            "mod r#session_selector_search {}",
            "maestro-chat",
            "session_selector_search",
        ),
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-tui", owner]);
        source(&workspace, "maestro-tui", "src/lib.rs", declaration);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains(&support::native_path_text("maestro-tui/src/lib.rs:1:"))
                && error.contains(name)
                && error.contains(&format!("belongs to {owner}")),
            "{error}"
        );
        source(&workspace, "maestro-tui", "src/lib.rs", "");
        source(&workspace, owner, "src/lib.rs", declaration);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn maestro_conventions_decodes_static_paths_as_rust() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    for (directory, literal) in [
        ("interfaces", r#""\x69nterfaces""#),
        ("interfaces", r#""\u{69}nterfaces""#),
        ("interfaces", r#""\u{0000_69}nterfaces""#),
        ("interfaces", "\"inter\\\n    faces\""),
        ("interfaces", "\"inter\\\r\n    faces\""),
        ("interfaces", r#"r"interfaces""#),
        ("interfaces", r##"r#"interfaces"#"##),
        ("intérfaces", r#""int\u{e9}rfaces""#),
        ("back\\slash", r#""back\\slash""#),
        ("double\"quote", r#""double\"quote""#),
        ("single'quote", r#""single\'quote""#),
        ("new\nline", r#""new\nline""#),
        ("tab\tname", r#""tab\tname""#),
        ("return\rname", r#""return\rname""#),
        ("physical\nnewline", "\"physical\r\nnewline\""),
        ("physical\nnewline", "r#\"physical\r\nnewline\"#"),
        ("physical\rnewline", r#""physical\rnewline""#),
    ] {
        assert_static_path(&workspace, directory, literal);
    }
    std::fs::create_dir(
        workspace
            .root
            .path()
            .join("crates/maestro-extensions-wasm/wit"),
    )
    .unwrap();
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        r#"wit_bindgen::generate!("fixture\0");"#,
    );
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/wit" });"#,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

/// The declared document with `owner` also depending on the external `library`.
fn declared_with(
    declared: &serde_json::Value,
    owner: &str,
    library: &str,
    declaration: &Declaration,
    external: &Path,
) -> serde_json::Value {
    let mut edge = dependency_template(declared);
    edge["name"] = serde_json::json!(library);
    edge["path"] = serde_json::json!(external);
    edge["rename"] = serde_json::json!("alias");
    edge["kind"] = serde_json::json!(declaration.kind);
    edge["optional"] = serde_json::json!(declaration.optional);
    edge["target"] = serde_json::json!(declaration.target);
    support::with_dependency(declared, owner, &edge)
}

/// Cargo's own record of a dependency edge, from the fixture's declared graph.
fn dependency_template(declared: &serde_json::Value) -> serde_json::Value {
    declared["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|package| package["dependencies"].as_array()?.first())
        .cloned()
        .expect("the fixture graph declares workspace edges")
}

const NONCANONICAL_WIT: &[(&str, &str)] = &[
    (
        r#"wasmtime::component::bindgen!({ path: "copied" });"#,
        "guest-owned",
    ),
    (
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/second" });"#,
        "same canonical source",
    ),
    (
        r#"wasmtime::component::bindgen!({ path: "missing" });"#,
        "cannot resolve",
    ),
    (
        r#"wasmtime::component::bindgen!({ path: env!("WIT_PATH") });"#,
        "requires review",
    ),
    (
        r#"wasmtime::component::bindgen!({ path: concat!("../", "interfaces") });"#,
        "requires review",
    ),
    (
        r"wasmtime::component::bindgen!({ path: WIT_PATH });",
        "requires review",
    ),
    (
        r#"wasmtime::component::bindgen!({ path: ["../maestro-extensions-wasm/interfaces", WIT_PATH] });"#,
        "requires review",
    ),
    (
        r#"wasmtime::component::bindgen!({ inline: "package maestro:fixture; world fixture {}" });"#,
        "guest-owned",
    ),
    (
        r#"wasmtime::component::bindgen!({ world: "fixture" });"#,
        "requires review",
    ),
];

#[test]
fn declarations_in_macro_tokens_keep_their_owners() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tools", "maestro-tui"]);
    for (relative, contents) in [
        (
            "src/lib.rs",
            "macro_rules! records { () => { pub struct ToolDefinition; }; }",
        ),
        (
            "included.rs",
            "{ wrapper! { pub enum ToolDefinition { A } } }",
        ),
        (
            "src/lib.rs",
            "outer! { inner! { pub type ToolDefinition = (); } }",
        ),
    ] {
        source(&workspace, "maestro-tui", relative, contents);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("ToolDefinition declaration belongs to maestro-tools"),
            "{error}"
        );
        source(&workspace, "maestro-tui", relative, "");
        source(&workspace, "maestro-tools", relative, contents);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        source(&workspace, "maestro-tools", relative, "");
    }
    source(
        &workspace,
        "maestro-tui",
        "src/lib.rs",
        r#"#[example(value = nested!(struct ToolDefinition;))]
        fn innocent() { let _ = "struct ToolDefinition"; }
        macro_rules! lookalike { () => { #[example(struct ToolDefinition)] fn fine() {} }; }
        "#,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn trait_alias_declarations_keep_their_owners() {
    for declaration in [
        "pub trait ToolDefinition = Send;",
        "macro_rules! define { () => { pub trait ToolDefinition = Send; }; }",
    ] {
        assert_tool_declaration_owner(declaration);
    }
}

#[test]
fn declarations_after_shebang_keep_their_owners() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tools", "maestro-tui"]);
    let declaration = "#!/usr/bin/env rust-script\npub struct ToolDefinition;";
    source(&workspace, "maestro-tui", "src/lib.rs", declaration);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "{}:2: ToolDefinition declaration belongs to maestro-tools, not maestro-tui",
            workspace
                .root
                .path()
                .join(support::native_path("crates/maestro-tui/src/lib.rs"))
                .display()
        ))
    );
    source(&workspace, "maestro-tui", "src/lib.rs", "");
    source(&workspace, "maestro-tools", "src/lib.rs", declaration);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn metavariable_keywords_do_not_declare_ownership() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tui"]);
    source(
        &workspace,
        "maestro-tui",
        "src/lib.rs",
        "macro_rules! define { ($type:ident) => { $type ToolDefinition; }; }",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn contextual_union_and_raw_keywords_are_not_declarations() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tui"]);
    for contents in [
        "fn union() {} fn innocent() { x.union(ToolDefinition); }",
        "wrapper! { r#type ToolDefinition; struct $ToolDefinition; }",
        "paste! { struct [<Tool Definition>]; } concat_idents! { Tool, Definition }",
    ] {
        source(&workspace, "maestro-tui", "src/lib.rs", contents);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{contents}");
    }
    for declaration in [
        "pub union ToolDefinition<T> { value: T }",
        "pub union ToolDefinition where u8: Copy { value: u8 }",
    ] {
        assert_tool_declaration_owner(declaration);
    }
}

#[test]
fn functions_constants_and_statics_do_not_declare_ownership() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tui"]);
    for declaration in [
        "pub fn oauth_selector() {}",
        "pub const ToolDefinition: () = ();",
        "pub static ToolRenderContext: () = ();",
    ] {
        source(&workspace, "maestro-tui", "src/lib.rs", declaration);
        assert_eq!(
            check_workspace(workspace.root.path()),
            Ok(()),
            "{declaration}"
        );
    }
}

#[test]
fn associated_types_in_macro_templates_keep_their_owners() {
    assert_tool_declaration_owner(
        "macro_rules! associated { () => { type ToolDefinition; }; } pub trait Owner { associated!(); }",
    );
}

#[test]
fn fixed_macro_declarations_with_metavariable_types_keep_their_owners() {
    assert_tool_declaration_owner(
        "macro_rules! define { ($t:ty) => { struct ToolDefinition { value: $t } }; } define!(u8);",
    );
}

#[test]
fn generated_macro_declaration_names_remain_manual_review() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tui"]);
    source(
        &workspace,
        "maestro-tui",
        "src/lib.rs",
        "macro_rules! define { ($name:ident, $t:ty) => { struct $name { value: $t } }; } define!(ToolDefinition, u8);",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

fn assert_tool_declaration_owner(declaration: &str) {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tools", "maestro-tui"]);
    source(&workspace, "maestro-tui", "src/lib.rs", declaration);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "{}:1: ToolDefinition declaration belongs to maestro-tools, not maestro-tui",
            workspace
                .root
                .path()
                .join(support::native_path("crates/maestro-tui/src/lib.rs"))
                .display()
        ))
    );
    source(&workspace, "maestro-tui", "src/lib.rs", "");
    source(&workspace, "maestro-tools", "src/lib.rs", declaration);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

fn assert_static_path(workspace: &Workspace, directory: &str, literal: &str) {
    let member = workspace
        .root
        .path()
        .join(support::native_path("crates/maestro-extensions-wasm"));
    // Windows refuses quotes and control characters in a name, so those spellings cannot
    // be exercised there.
    match std::fs::create_dir_all(member.join(directory)) {
        Ok(()) => {}
        Err(_) if cfg!(windows) => {
            eprintln!("premise not met: this host refuses the directory {directory:?}");
            return;
        }
        Err(error) => panic!("{directory:?}: {error}"),
    }
    source(
        workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        &format!("wit_bindgen::generate!({{ path: {literal} }});"),
    );
    let host_path = format!("../maestro-extensions-wasm/{directory}");
    let host = format!("wasmtime::component::bindgen!({{ path: {host_path:?} }});");
    source(workspace, "maestro-extensions-wasmtime", "build.rs", &host);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Ok(()),
        "literal: {literal}"
    );
    if directory.starts_with("physical") {
        let other = if directory.contains('\r') {
            "physical\nnewline"
        } else {
            "physical\rnewline"
        };
        std::fs::create_dir_all(member.join(other)).unwrap();
        let wrong = format!("../maestro-extensions-wasm/{other}");
        source(
            workspace,
            "maestro-extensions-wasmtime",
            "build.rs",
            &format!("wasmtime::component::bindgen!({{ path: {wrong:?} }});"),
        );
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("same canonical source roots")
        );
        source(workspace, "maestro-extensions-wasmtime", "build.rs", &host);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

fn tool_declaration_kinds() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tools", "maestro-tui"]);
    for name in [
        "ToolDefinition",
        "ToolRenderContext",
        "ToolRenderResultOptions",
    ] {
        for shape in [
            "pub enum NAME<T> { Entry(T) }",
            "pub union NAME { value: u8 }",
            "pub trait NAME<T> {}",
            "pub type NAME<T> = Vec<T>;",
            "pub mod NAME {}",
            "pub trait Owner { type NAME; }",
            "impl Owner for () { type NAME = (); }",
            "unsafe extern \"C\" { type NAME; }",
        ] {
            let declaration = format!("#[example]\n{}", shape.replace("NAME", name));
            source(&workspace, "maestro-tui", "src/lib.rs", &declaration);
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "{}:2: {name} declaration belongs to maestro-tools, not maestro-tui",
                    workspace
                        .root
                        .path()
                        .join(support::native_path("crates/maestro-tui/src/lib.rs"))
                        .display()
                ))
            );
            source(&workspace, "maestro-tui", "src/lib.rs", "");
            source(&workspace, "maestro-tools", "src/lib.rs", &declaration);
            assert_eq!(check_workspace(workspace.root.path()), Ok(()));
            source(&workspace, "maestro-tools", "src/lib.rs", "");
        }
    }
}

fn wit_ownership_without_both_sides() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasmtime"]);
    std::fs::create_dir(workspace.root.path().join(support::native_path(
        "crates/maestro-extensions-wasmtime/interfaces",
    )))
    .unwrap();
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "interfaces" });"#,
    );
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("no guest-owned source member")
    );
    source(&workspace, "maestro-extensions-wasmtime", "build.rs", "");
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    for declaration in [
        r#"wit_bindgen::generate!({ inline: "package maestro:fixture; world fixture {}" });"#,
        r#"nested! { wit_bindgen::generate!({ inline: "package maestro:fixture; world fixture {}" }); }"#,
    ] {
        source(
            &workspace,
            "maestro-extensions-wasm",
            "src/bindings.rs",
            declaration,
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
    for input in [
        "{}",
        "{ path: [] }",
        "{ world: \"fixture\" }",
        "{ path: dynamic() }",
        "{ inline: WIT_TEXT }",
    ] {
        source(
            &workspace,
            "maestro-extensions-wasm",
            "src/bindings.rs",
            &format!("wit_bindgen::generate!({input});"),
        );
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("requires review")
        );
        source(&workspace, "maestro-extensions-wasm", "src/bindings.rs", "");
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[cfg(unix)]
fn wit_symlink_ownership(workspace: &Workspace, guest: &std::path::Path) {
    let host = workspace
        .root
        .path()
        .join(support::native_path("crates/maestro-extensions-wasmtime"));
    std::os::unix::fs::symlink(guest, host.join("linked")).unwrap();
    source(
        workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "linked" });"#,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    std::os::unix::fs::symlink(host.join("copied"), guest.join("outside")).unwrap();
    source(
        workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/interfaces/outside" });"#,
    );
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("guest-owned canonical source")
    );
    source(
        workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "linked" });"#,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn guest_host_probe_keeps_engine_dependencies_test_only() {
    for library in ["wasmtime", "wasmtime-wasi", "wasmtime-wasi-http"] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-extensions-wasm"]);
        workspace.external(library);
        let manifest = workspace
            .root
            .path()
            .join("crates/maestro-extensions-wasm/Cargo.toml");
        let baseline = std::fs::read_to_string(&manifest).unwrap();
        for declaration in DECLARATIONS {
            let table = declaration.table;
            let optional = optional_marker(declaration);
            let declared = format!(
                "alias = {{ package = {library:?}, path = \"../../external\"{optional} }}\n"
            );
            std::fs::write(&manifest, format!("{baseline}\n[{table}]\n{declared}")).unwrap();
            let outcome = check_workspace(workspace.root.path());
            let test_only = table.ends_with("dev-dependencies") && library != "wasmtime-wasi-http";
            if test_only {
                assert_eq!(outcome, Ok(()), "{library} in {table}");
            } else {
                let error = outcome.unwrap_err();
                assert!(
                    error.contains("maestro-extensions-wasm") && error.contains(library),
                    "{library} in {table}: {error}"
                );
            }
        }
    }
}

#[test]
fn guest_tool_wrapper_keeps_domain_declaration_ownership() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-tools", "maestro-tui"]);
    source(
        &workspace,
        "maestro-tools",
        "src/lib.rs",
        "pub struct ToolDefinition;",
    );
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/types/tools.rs",
        "pub struct ToolDefinition;",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/types/tools.rs",
        "pub struct ToolDefinition; pub struct ToolRenderContext; pub struct ToolRenderResultOptions;",
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Ok(()),
        "the guest declares its renderer records beside its tool definition"
    );
    guest_tool_declaration_decoys(&workspace);
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/types/tools.rs",
        "pub struct ToolDefinition; pub type ToolDefinition = ();",
    );
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("duplicate declaration ToolDefinition")
    );
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/types/tools.rs",
        "pub struct ToolDefinition;",
    );
    source(
        &workspace,
        "maestro-tools",
        "other.rs",
        "pub type ToolDefinition = ();",
    );
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("duplicate declaration ToolDefinition")
    );
}

fn guest_tool_declaration_decoys(workspace: &Workspace) {
    for (owner, path, name) in [
        ("maestro-extensions-wasm", "src/lib.rs", "ToolDefinition"),
        ("maestro-tui", "src/types/tools.rs", "ToolDefinition"),
        ("maestro-extensions-wasm", "src/lib.rs", "ToolRenderContext"),
        (
            "maestro-tui",
            "src/types/tools.rs",
            "ToolRenderResultOptions",
        ),
    ] {
        source(workspace, owner, path, &format!("pub struct {name};"));
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("belongs to maestro-tools")
        );
        source(workspace, owner, path, "");
        source(
            workspace,
            "maestro-extensions-wasm",
            "src/types/tools.rs",
            "pub struct ToolDefinition;",
        );
    }
}

#[test]
fn wit_async_selector_keeps_static_path_admission() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm"]);
    let guest = workspace
        .root
        .path()
        .join("crates/maestro-extensions-wasm/wit");
    std::fs::create_dir(&guest).unwrap();
    std::fs::write(
        guest.join("world.wit"),
        "package maestro:fixture; world fixture {}",
    )
    .unwrap();
    let input = r#"wit_bindgen::generate!({
        path: "wit",
        async: ["-import:maestro:extension/host@0.1.0#callback-emit"],
        world: "extension",
    });"#;
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        input,
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        &input.replace("path: \"wit\"", "path: env!(\"WIT_PATH\")"),
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(error.contains("expected static path"), "{error}");
}

#[test]
fn slash_command_records_have_one_shared_owner() {
    for name in ["SlashCommandInfo", "SlashCommandSource"] {
        assert_single_owner(
            name,
            "maestro-interchange",
            &["maestro-app", "maestro-extensions-wasm", "maestro-tui"],
        );
    }
}

#[test]
fn message_render_options_have_one_shared_owner() {
    assert_single_owner(
        "MessageRenderOptions",
        "maestro-interchange",
        &[
            "maestro-session",
            "maestro-extensions",
            "maestro-extensions-wasm",
        ],
    );
}

#[test]
fn session_operation_records_have_one_shared_owner() {
    for (name, owner) in [
        ("MessageDelivery", "maestro-interchange"),
        ("UserMessageDelivery", "maestro-interchange"),
        ("SendMessageOptions", "maestro-interchange"),
        ("SendUserMessageOptions", "maestro-interchange"),
        ("ForkPosition", "maestro-interchange"),
        ("NavigateTreeOptions", "maestro-interchange"),
    ] {
        assert_single_owner(
            name,
            owner,
            &[
                "maestro-session",
                "maestro-extensions",
                "maestro-extensions-wasm",
            ],
        );
    }
}

#[test]
fn office_document_records_have_one_shared_owner() {
    for name in ["NumberFormatter", "OpenXmlPackage", "WorkbookError"] {
        assert_single_owner(name, "maestro-office", &["maestro-app"]);
    }
}

#[test]
fn http_transport_records_have_one_shared_owner() {
    for name in ["FetchError", "PacProxyAgent"] {
        assert_single_owner(name, "maestro-http", &["maestro-models"]);
    }
}

#[test]
fn tool_metadata_has_one_shared_owner_and_one_guest_specialization() {
    let workspace = Workspace::new();
    let root = workspace.root.path();
    workspace.foundation(&[
        "maestro-interchange",
        "maestro-models",
        "maestro-extensions-wasm",
    ]);
    source(
        &workspace,
        "maestro-models",
        "src/lib.rs",
        "pub struct ToolInfo;",
    );
    let error = check_workspace(root).unwrap_err();
    let owner_message = "ToolInfo declaration belongs to maestro-interchange";
    assert!(error.contains(owner_message), "{error}");
    source(
        &workspace,
        "maestro-models",
        "src/lib.rs",
        "pub use maestro_interchange::tool_info::ToolInfo;",
    );
    source(
        &workspace,
        "maestro-interchange",
        "src/lib.rs",
        "pub struct ToolInfo<P = ParameterSchema> { parameters: P }",
    );
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/types/actions.rs",
        "pub type ToolInfo = maestro_interchange::tool_info::ToolInfo<String>;",
    );
    assert_eq!(check_workspace(root), Ok(()));
    source(
        &workspace,
        "maestro-interchange",
        "other.rs",
        "pub struct ToolInfo;",
    );
    let error = check_workspace(root).unwrap_err();
    assert!(
        error.contains(".rs:1:") && error.contains("duplicate declaration ToolInfo"),
        "{error}"
    );
    std::fs::remove_file(root.join("crates/maestro-interchange/other.rs")).unwrap();
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/types/actions.rs",
        "pub struct ToolInfo;",
    );
    let error = check_workspace(root).unwrap_err();
    assert!(
        error.contains(&support::native_path_text("src/types/actions.rs:1:"))
            && error.contains("belongs to maestro-interchange"),
        "{error}"
    );
}

/// Rejects a copy of `name` in each consumer, past a commented and a quoted decoy, until it
/// becomes a re-export; then accepts one declaration in `owner` and rejects a second.
fn assert_single_owner(name: &str, owner: &str, consumers: &[&str]) {
    let workspace = Workspace::new();
    workspace.foundation(&[&[owner], consumers].concat());
    for consumer in consumers {
        source(
            &workspace,
            consumer,
            "src/lib.rs",
            &format!(
                "// struct {name};\nfn f() {{ let _ = \"struct {name}\"; }}\npub struct {name};"
            ),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains(&support::native_path_text("src/lib.rs:3:"))
                && error.contains(&format!("belongs to {owner}")),
            "{error}"
        );
        source(
            &workspace,
            consumer,
            "src/lib.rs",
            &format!("pub use shared::{name};"),
        );
    }
    source(
        &workspace,
        owner,
        "src/lib.rs",
        &format!("pub struct {name};"),
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        &workspace,
        owner,
        "other.rs",
        &format!("pub type {name} = ();"),
    );
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("duplicate declaration")
    );
}

#[test]
fn shared_event_records_have_one_owner() {
    registered_definition_alias_keeps_tools_ownership();
    let workspace = Workspace::new();
    workspace.foundation(&[
        "maestro-interchange",
        "maestro-models",
        "maestro-extensions-wasm",
    ]);
    source(
        &workspace,
        "maestro-models",
        "src/lib.rs",
        "pub struct ContextEvent<M> { messages: Vec<M> }",
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains("ContextEvent declaration belongs to maestro-interchange"),
        "{error}"
    );
    source(
        &workspace,
        "maestro-models",
        "src/lib.rs",
        "pub type ContextEvent<M> = maestro_interchange::extension_events::ContextEvent<M>;",
    );
    source(
        &workspace,
        "maestro-interchange",
        "src/lib.rs",
        "pub struct ContextEvent<M> { messages: Vec<M> }",
    );
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/types/events.rs",
        "pub type ContextEvent = maestro_interchange::extension_events::ContextEvent<String>;",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        &workspace,
        "maestro-interchange",
        "other.rs",
        "pub enum ContextEvent {}",
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains(".rs:1:") && error.contains("duplicate declaration ContextEvent"),
        "{error}"
    );
}

/// The native extension alias specializes the tools owner without duplicating its record.
fn registered_definition_alias_keeps_tools_ownership() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions", "maestro-tools"]);
    source(
        &workspace,
        "maestro-extensions",
        "src/types/registration.rs",
        "pub type ToolDefinition<P,D,S> = maestro_tools::ToolDefinition<ExtensionContext,P,D,S,Local>;",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    source(
        &workspace,
        "maestro-extensions",
        "src/types/registration.rs",
        "pub struct ToolDefinition;",
    );
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("ToolDefinition declaration belongs to maestro-tools")
    );
}

#[test]
fn interactive_session_setup_rejects_bypasses() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat"]);
    source(
        &workspace,
        "maestro-chat",
        "tests/footer_support/harness.rs",
        "fn session() -> Result { AgentSession::new(AgentSessionConfig { agent: owner() }) }",
    );
    for relative in [
        "tests/footer_support/mod.rs",
        "tests/footer_support/corpus.rs",
        "tests/session_picker_support/mod.rs",
        "src/interactive_mode/tests.rs",
        "src/interactive_mode/startup/tests.rs",
        "src/components/tree_selector/tests.rs",
    ] {
        let path = source(&workspace, "maestro-chat", relative, DUPLICATE_SETUP);
        assert_setup_rejected(&workspace, &path, relative);
        std::fs::remove_file(&path).unwrap();
    }
    workspace.library_test_target("maestro-chat", false);
    for spelling in DUPLICATE_SETUP_SPELLINGS {
        let path = source(
            &workspace,
            "maestro-chat",
            "tests/footer_support/mod.rs",
            spelling,
        );
        assert_setup_rejected(&workspace, &path, spelling);
    }
    for accepted in [
        "fn f() -> CreateAgentSessionRuntimeResult { session(cwd, None) }",
        "// AgentSessionConfig\nfn f() { let _ = \"AgentSessionConfig\"; }",
    ] {
        source(
            &workspace,
            "maestro-chat",
            "tests/footer_support/mod.rs",
            accepted,
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{accepted}");
    }
}

/// Assert the checker names `path` and the shared owner when it rejects a second setup.
///
/// Both sides are canonicalized, because a host may spell the scratch root differently
/// from the workspace metadata that the checker walks.
fn assert_setup_rejected(workspace: &Workspace, path: &Path, detail: &str) {
    let error = check_workspace(workspace.root.path()).unwrap_err();
    let Some((reported, owner)) = error.split_once(":1: session setup belongs in ") else {
        panic!("{detail}: {error}");
    };
    assert_eq!(
        owner, "tests/footer_support/harness.rs",
        "{detail}: {error}"
    );
    assert_eq!(
        std::fs::canonicalize(reported).unwrap(),
        std::fs::canonicalize(path).unwrap(),
        "{detail}"
    );
}

#[test]
fn input_presenter_records_reject_duplicate_owners() {
    for name in [
        "PresentedSelector",
        "ConfigSelectorCallbacks",
        "ConfigSelectorFactory",
        "SessionSelectorCallbacks",
        "SessionSelectorRequest",
        "SessionSelectorFactory",
        "SessionsFuture",
        "SessionsLoader",
    ] {
        assert_single_owner(name, "maestro-app", &["maestro-chat", "maestro-cli"]);
    }
}

#[test]
fn listener_collection_declarations_stay_with_owner() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-listeners", "maestro-tui"]);
    let declaration = "pub struct Listeners<P> { entry: P }";
    source(&workspace, "maestro-listeners", "src/lib.rs", declaration);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for fragment in [
        declaration,
        "macro_rules! copy { () => { pub struct Listeners<P> { entry: P } }; }",
        "fn copy() { let make = || pub struct Listeners<P> { entry: P }; }",
        "impl TUI { fn copy() { pub struct Listeners<P> { entry: P } } }",
    ] {
        source(&workspace, "maestro-tui", "src/lib.rs", fragment);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("Listeners declaration belongs to maestro-listeners"),
            "{error}"
        );
        source(&workspace, "maestro-tui", "src/lib.rs", "");
    }
    source(&workspace, "maestro-listeners", "src/copy.rs", declaration);
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(error.contains("duplicate declaration Listeners"), "{error}");
}

#[test]
fn wit_generate_all_keeps_static_path_enforcement() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    let guest = workspace
        .root
        .path()
        .join("crates/maestro-extensions-wasm/interfaces");
    std::fs::create_dir(&guest).unwrap();
    std::fs::write(
        guest.join("world.wit"),
        "package maestro:fixture; world fixture {}",
    )
    .unwrap();
    for input in [
        r#"wit_bindgen::generate!({ generate_all, path: "interfaces", world: "fixture" });"#,
        r#"wit_bindgen::generate!({ path: "interfaces", generate_all, world: "fixture" });"#,
        r#"wit_bindgen::generate!({ path: "interfaces", world: "fixture", generate_all });"#,
    ] {
        source(
            &workspace,
            "maestro-extensions-wasm",
            "src/bindings.rs",
            input,
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{input}");
    }
    for input in [
        r"wit_bindgen::generate!({ generate_all });",
        r#"wit_bindgen::generate!({ generate_all, path: env!("WIT_PATH") });"#,
        r"wit_bindgen::generate!({ generate_all, path: WIT_PATH });",
        r#"wit_bindgen::generate!({ generate_all, path: ["interfaces", WIT_PATH] });"#,
    ] {
        source(
            &workspace,
            "maestro-extensions-wasm",
            "src/bindings.rs",
            input,
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("expected static path"), "{input}: {error}");
    }
}
