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
            let workspace = Workspace::new();
            workspace.foundation(&[owner]);
            workspace.external(library);
            let manifest = workspace.root.join(format!("crates/{owner}/Cargo.toml"));
            let baseline = std::fs::read_to_string(&manifest).unwrap();
            for (kind, extra) in DECLARATIONS {
                let declaration = format!(
                    "alias = {{ package = {library:?}, path = \"../../external\"{extra} }}\n"
                );
                // Existing normal dependencies share a table with the external alias.
                let contents = if *kind == "dependencies" && baseline.contains("[dependencies]\n") {
                    baseline.replace(
                        "[dependencies]\n",
                        &format!("[dependencies]\n{declaration}"),
                    )
                } else {
                    format!("{baseline}\n[{kind}]\n{declaration}")
                };
                std::fs::write(&manifest, contents).unwrap();
                if owner == "maestro-extensions-wasmtime" && library != "wasmtime-wasi-http" {
                    assert_eq!(check_workspace(&workspace.root), Ok(()));
                } else {
                    let error = check_workspace(&workspace.root).unwrap_err();
                    assert!(error.contains(owner) && error.contains(library), "{error}");
                }
            }
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
        "src/lib.rs",
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
    let second = workspace.root.join("crates/maestro-extensions-wasm/second");
    std::fs::create_dir(&second).unwrap();
    std::fs::copy(guest.join("world.wit"), second.join("world.wit")).unwrap();
    let copied = workspace
        .root
        .join("crates/maestro-extensions-wasmtime/copied");
    std::fs::create_dir(&copied).unwrap();
    std::fs::copy(guest.join("world.wit"), copied.join("world.wit")).unwrap();
    for (host, expected) in [
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
            r#"wasmtime::component::bindgen!({ path: WIT_PATH });"#,
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
    ] {
        source(&workspace, "maestro-extensions-wasmtime", "build.rs", host);
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(
            error.contains("build.rs:1:") && error.contains(expected),
            "{error}"
        );
        source(
            &workspace,
            "maestro-extensions-wasmtime",
            "build.rs",
            r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/interfaces" });"#,
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/lib.rs",
        r#"wit_bindgen::generate!({ path: "../maestro-extensions-wasmtime/copied" });"#,
    );
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("guest-owned")
    );
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/lib.rs",
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
        "src/lib.rs",
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
        "src/lib.rs",
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
fn wit_static_paths_decode_rust_string_literals() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-extensions-wasmtime"]);
    let member = workspace.root.join("crates/maestro-extensions-wasm");
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
    ] {
        std::fs::create_dir_all(member.join(directory)).unwrap();
        source(
            &workspace,
            "maestro-extensions-wasm",
            "src/lib.rs",
            &format!("wit_bindgen::generate!({{ path: {literal} }});"),
        );
        let host_path = format!("../maestro-extensions-wasm/{directory}");
        source(
            &workspace,
            "maestro-extensions-wasmtime",
            "build.rs",
            &format!("wasmtime::component::bindgen!({{ path: r###\"{host_path}\"### }});"),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Ok(()),
            "literal: {literal}"
        );
    }
    std::fs::create_dir(member.join("wit")).unwrap();
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/lib.rs",
        r#"wit_bindgen::generate!("fixture\0");"#,
    );
    source(
        &workspace,
        "maestro-extensions-wasmtime",
        "build.rs",
        r#"wasmtime::component::bindgen!({ path: "../maestro-extensions-wasm/wit" });"#,
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    for literal in [
        r#""\qinterfaces""#,
        r#""\xFF""#,
        r#""\u{D800}""#,
        r#""\u{110000}""#,
        r#""\u{}""#,
    ] {
        source(
            &workspace,
            "maestro-extensions-wasm",
            "src/lib.rs",
            &format!("wit_bindgen::generate!({{ path: {literal} }});"),
        );
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(error.contains("requires review"), "{literal}: {error}");
    }
}
