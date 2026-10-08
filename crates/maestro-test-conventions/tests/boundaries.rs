#![cfg(test)]

mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

const DECLARATIONS: &[(&str, &str)] = &[
    ("dependencies", ""),
    ("dependencies", ", optional = true"),
    ("build-dependencies", ""),
    ("dev-dependencies", ""),
    ("target.'cfg(target_os = \"none\")'.dependencies", ""),
    ("target.'cfg(target_os = \"none\")'.build-dependencies", ""),
    ("target.'cfg(target_os = \"none\")'.dev-dependencies", ""),
];

#[test]
fn runtime_dependencies_stay_in_the_runtime_adapter() {
    for library in [
        "wasmtime",
        "wasmtime-component-macro",
        "wasmtime-wasi",
        "wasmtime-wasi-http",
    ] {
        for &(owner, _) in support::policy::POLICY {
            // The guest's test-only host dependencies have their own test.
            if owner == "maestro-extensions-wasm" && matches!(library, "wasmtime" | "wasmtime-wasi")
            {
                continue;
            }
            let workspace = Workspace::new();
            workspace.foundation(&[owner]);
            workspace.external(library);
            let manifest = workspace.root.join(format!("crates/{owner}/Cargo.toml"));
            let baseline = std::fs::read_to_string(&manifest).unwrap();
            check_runtime_declarations(&workspace, owner, library, &manifest, &baseline);
            std::fs::write(&manifest, &baseline).unwrap();
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
    let workspace = Workspace::new();
    workspace.foundation(&["maestro", "maestro-extensions-wasmtime"]);
    workspace.external("wasmtime");
    workspace.member("maestro", "maestro", "[dependencies]\nmaestro-extensions-wasmtime = { path = \"../maestro-extensions-wasmtime\" }");
    workspace.member(
        "maestro-extensions-wasmtime",
        "maestro-extensions-wasmtime",
        "[dependencies]\nwasmtime = { path = \"../../external\" }",
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions", "maestro-extensions-wasm"]);
    workspace.member(
        "maestro-extensions",
        "maestro-extensions",
        "[dependencies]\nmaestro-extensions-wasm = { path = \"../maestro-extensions-wasm\" }",
    );
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(
        error.contains("maestro-extensions -> maestro-extensions-wasm"),
        "{error}"
    );
    workspace.member("maestro-extensions", "maestro-extensions", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn toolkit_excludes_framework_and_highlighting_dependencies() {
    for library in ["ratatui", "syntect", "two-face"] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-tui"]);
        workspace.external(library);
        for (kind, extra) in DECLARATIONS {
            workspace.member("maestro-tui", "maestro-tui", &format!("[{kind}]\nalias = {{ package = {library:?}, path = \"../../external\"{extra} }}"));
            assert_eq!(
                check_workspace(&workspace.root),
                Err(format!(
                    "maestro-tui: forbidden toolkit library dependency {library}"
                ))
            );
            workspace.member("maestro-tui", "maestro-tui", "");
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
}

fn source(workspace: &Workspace, owner: &str, relative: &str, contents: &str) {
    let path = workspace.root.join("crates").join(owner).join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
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
        workspace.foundation(&["maestro-tools", "maestro-tui"]);
        source(
            &workspace,
            "maestro-tui",
            "src/lib.rs",
            &format!("\npub(crate) struct\n{name}<\nContext\n> {{ context: Context }}"),
        );
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(
            error.contains("maestro-tui/src/lib.rs:2:")
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
        assert_eq!(check_workspace(&workspace.root), Ok(()));
        source(
            &workspace,
            "maestro-tools",
            "other.rs",
            &format!("pub type {name} = ();"),
        );
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(
            error.contains("duplicate declaration") && error.contains(name),
            "{error}"
        );
        std::fs::remove_file(workspace.root.join("crates/maestro-tools/other.rs")).unwrap();
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn application_selectors_stay_in_chat() {
    for (component, module) in [
        ("ConfigSelectorComponent", "config_selector"),
        ("ExtensionSelectorComponent", "extension_selector"),
        ("ModelSelectorComponent", "model_selector"),
        ("OAuthSelectorComponent", "oauth_selector"),
        ("ScopedModelsSelectorComponent", "scoped_models_selector"),
        ("SessionSelectorComponent", "session_selector"),
        ("SettingsSelectorComponent", "settings_selector"),
        ("ShowImagesSelectorComponent", "show_images_selector"),
        ("ThemeSelectorComponent", "theme_selector"),
        ("ThinkingSelectorComponent", "thinking_selector"),
        ("TreeSelectorComponent", "tree_selector"),
        ("UserMessageSelectorComponent", "user_message_selector"),
        ("SessionSelectorComponent", "session_selector_search"),
    ] {
        for owner in ["maestro-tui", "maestro-app"] {
            let workspace = Workspace::new();
            workspace.foundation(&[owner, "maestro-chat"]);
            for declaration in [
                format!("pub struct {component};"),
                format!("pub(crate) mod\n{module} {{}}"),
            ] {
                source(&workspace, owner, "src/lib.rs", &declaration);
                let error = check_workspace(&workspace.root).unwrap_err();
                assert!(
                    error.contains(owner) && error.contains("belongs to maestro-chat"),
                    "{error}"
                );
                source(
                    &workspace,
                    owner,
                    "src/lib.rs",
                    "pub struct SelectList; pub struct SettingsList;",
                );
                source(&workspace, "maestro-chat", "src/lib.rs", &declaration);
                assert_eq!(check_workspace(&workspace.root), Ok(()));
            }
        }
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
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
        let error = check_workspace(&workspace.root).unwrap_err();
        let line = innocent.lines().count() + 2;
        assert!(error.contains(&format!("src/lib.rs:{line}:")), "{error}");
        source(&workspace, "maestro-tui", "src/lib.rs", innocent);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn wit_codegen_inputs_share_one_canonical_source() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    let guest = workspace
        .root
        .join("crates/maestro-extensions-wasm/interfaces");
    std::fs::create_dir(&guest).unwrap();
    std::fs::write(
        guest.join("world.wit"),
        "package maestro:fixture; world fixture {}",
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    reject_noncanonical_wit(&workspace, &guest);
    wit_ownership_without_both_sides();
    #[cfg(unix)]
    wit_symlink_ownership(&workspace, &guest);
}

fn reject_noncanonical_wit(workspace: &Workspace, guest: &std::path::Path) {
    let second = workspace.root.join("crates/maestro-extensions-wasm/second");
    std::fs::create_dir(&second).unwrap();
    std::fs::copy(guest.join("world.wit"), second.join("world.wit")).unwrap();
    let copied = workspace
        .root
        .join("crates/maestro-extensions-wasmtime/copied");
    std::fs::create_dir(&copied).unwrap();
    std::fs::copy(guest.join("world.wit"), copied.join("world.wit")).unwrap();
    for &(host, expected) in NONCANONICAL_WIT {
        source(workspace, "maestro-extensions-wasmtime", "build.rs", host);
        let error = check_workspace(&workspace.root).unwrap_err();
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
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    source(
        workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        r#"wit_bindgen::generate!({ path: "../maestro-extensions-wasmtime/copied" });"#,
    );
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("guest-owned")
    );
    source(
        workspace,
        "maestro-extensions-wasm",
        "src/bindings.rs",
        r#"wit_bindgen::generate!({ path: ["interfaces"] });"#,
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn wit_options_ignore_module_paths_in_nested_values() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    let guest = workspace
        .root
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
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{input}");
        source(
            &workspace,
            "maestro-extensions-wasmtime",
            "build.rs",
            &input.replace("../maestro-extensions-wasm/interfaces", "wrong-root"),
        );
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(error.contains("cannot resolve WIT input"), "{error}");
    }
}

#[test]
fn wit_shorthand_world_uses_manifest_relative_wit_directory() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    for directory in ["wit", "fixture"] {
        let root = workspace
            .root
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
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains("same canonical source"), "{error}");
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/wit" });"#,
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!("fixture");"#,
    );
    let host_wit = workspace
        .root
        .join("crates/maestro-extensions-wasmtime/wit");
    std::fs::create_dir(&host_wit).unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
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
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(
            error.contains("maestro-tui/src/lib.rs:1:")
                && error.contains(name)
                && error.contains(&format!("belongs to {owner}")),
            "{error}"
        );
        source(&workspace, "maestro-tui", "src/lib.rs", "");
        source(&workspace, owner, "src/lib.rs", declaration);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
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
    std::fs::create_dir(workspace.root.join("crates/maestro-extensions-wasm/wit")).unwrap();
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
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

fn check_runtime_declarations(
    workspace: &Workspace,
    owner: &str,
    library: &str,
    manifest: &std::path::Path,
    baseline: &str,
) {
    for (kind, extra) in DECLARATIONS {
        let declaration =
            format!("alias = {{ package = {library:?}, path = \"../../external\"{extra} }}\n");
        // Existing normal dependencies share a table with the external alias.
        let contents = if *kind == "dependencies" && baseline.contains("[dependencies]\n") {
            baseline.replace(
                "[dependencies]\n",
                &format!("[dependencies]\n{declaration}"),
            )
        } else {
            format!("{baseline}\n[{kind}]\n{declaration}")
        };
        std::fs::write(manifest, contents).unwrap();
        if owner == "maestro-extensions-wasmtime" && library != "wasmtime-wasi-http" {
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        } else {
            let error = check_workspace(&workspace.root).unwrap_err();
            assert!(error.contains(owner) && error.contains(library), "{error}");
        }
    }
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
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(
            error.contains("ToolDefinition declaration belongs to maestro-tools"),
            "{error}"
        );
        source(&workspace, "maestro-tui", relative, "");
        source(&workspace, "maestro-tools", relative, contents);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
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
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
        check_workspace(&workspace.root),
        Err(format!(
            "{}:2: ToolDefinition declaration belongs to maestro-tools, not maestro-tui",
            workspace
                .root
                .join("crates/maestro-tui/src/lib.rs")
                .display()
        ))
    );
    source(&workspace, "maestro-tui", "src/lib.rs", "");
    source(&workspace, "maestro-tools", "src/lib.rs", declaration);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{contents}");
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
        "pub fn config_selector() {}",
        "pub const ToolDefinition: () = ();",
        "pub static ToolRenderContext: () = ();",
    ] {
        source(&workspace, "maestro-tui", "src/lib.rs", declaration);
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{declaration}");
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
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

fn assert_tool_declaration_owner(declaration: &str) {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tools", "maestro-tui"]);
    source(&workspace, "maestro-tui", "src/lib.rs", declaration);
    assert_eq!(
        check_workspace(&workspace.root),
        Err(format!(
            "{}:1: ToolDefinition declaration belongs to maestro-tools, not maestro-tui",
            workspace
                .root
                .join("crates/maestro-tui/src/lib.rs")
                .display()
        ))
    );
    source(&workspace, "maestro-tui", "src/lib.rs", "");
    source(&workspace, "maestro-tools", "src/lib.rs", declaration);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

fn assert_static_path(workspace: &Workspace, directory: &str, literal: &str) {
    let member = workspace.root.join("crates/maestro-extensions-wasm");
    std::fs::create_dir_all(member.join(directory)).unwrap();
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
        check_workspace(&workspace.root),
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
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("same canonical source roots")
        );
        source(workspace, "maestro-extensions-wasmtime", "build.rs", &host);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
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
                check_workspace(&workspace.root),
                Err(format!(
                    "{}:2: {name} declaration belongs to maestro-tools, not maestro-tui",
                    workspace
                        .root
                        .join("crates/maestro-tui/src/lib.rs")
                        .display()
                ))
            );
            source(&workspace, "maestro-tui", "src/lib.rs", "");
            source(&workspace, "maestro-tools", "src/lib.rs", &declaration);
            assert_eq!(check_workspace(&workspace.root), Ok(()));
            source(&workspace, "maestro-tools", "src/lib.rs", "");
        }
    }
}

fn wit_ownership_without_both_sides() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasmtime"]);
    std::fs::create_dir(
        workspace
            .root
            .join("crates/maestro-extensions-wasmtime/interfaces"),
    )
    .unwrap();
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "interfaces" });"#,
    );
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("no guest-owned source member")
    );
    source(&workspace, "maestro-extensions-wasmtime", "build.rs", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
        assert_eq!(check_workspace(&workspace.root), Ok(()));
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
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("requires review")
        );
        source(&workspace, "maestro-extensions-wasm", "src/bindings.rs", "");
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[cfg(unix)]
fn wit_symlink_ownership(workspace: &Workspace, guest: &std::path::Path) {
    let host = workspace.root.join("crates/maestro-extensions-wasmtime");
    std::os::unix::fs::symlink(guest, host.join("linked")).unwrap();
    source(
        workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "linked" });"#,
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    std::os::unix::fs::symlink(host.join("copied"), guest.join("outside")).unwrap();
    source(
        workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/interfaces/outside" });"#,
    );
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("guest-owned canonical source")
    );
    source(
        workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "linked" });"#,
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn guest_host_probe_keeps_engine_dependencies_test_only() {
    for library in ["wasmtime", "wasmtime-wasi", "wasmtime-wasi-http"] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-extensions-wasm"]);
        workspace.external(library);
        let manifest = workspace
            .root
            .join("crates/maestro-extensions-wasm/Cargo.toml");
        let baseline = std::fs::read_to_string(&manifest).unwrap();
        for (kind, extra) in DECLARATIONS {
            let declaration =
                format!("alias = {{ package = {library:?}, path = \"../../external\"{extra} }}\n");
            std::fs::write(&manifest, format!("{baseline}\n[{kind}]\n{declaration}")).unwrap();
            let outcome = check_workspace(&workspace.root);
            let test_only = kind.ends_with("dev-dependencies") && library != "wasmtime-wasi-http";
            if test_only {
                assert_eq!(outcome, Ok(()), "{library} in {kind}");
            } else {
                let error = outcome.unwrap_err();
                assert!(
                    error.contains("maestro-extensions-wasm") && error.contains(library),
                    "{library} in {kind}: {error}"
                );
            }
        }
    }
}
