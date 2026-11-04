//! Tests for source loading, the private manifest reader and associated-item attribute accessors.

#[path = "../../tests/support/scratch.rs"]
mod scratch;

use self::scratch::Scratch;
use super::{impl_item_attributes, read_manifest, trait_item_attributes};

/// Reduce attributes to their `(path, argument)` identifiers in order.
fn names(attributes: &[syn::Attribute]) -> Vec<(String, String)> {
    attributes
        .iter()
        .map(|attribute| {
            (
                attribute.path().require_ident().unwrap().to_string(),
                attribute.parse_args::<syn::Ident>().unwrap().to_string(),
            )
        })
        .collect()
}

/// Expected identifiers of the repeated-kind attribute sequence used by the fixtures.
fn repeated() -> Vec<(String, String)> {
    [("doc", "one"), ("cfg", "test"), ("doc", "two")]
        .map(|(path, argument)| (path.into(), argument.into()))
        .into()
}

#[test]
fn manifest_tables_keep_native_types() {
    let scratch = Scratch::new();
    let path = scratch.root.path().join("manifest.toml");
    std::fs::write(&path, "").unwrap();
    assert_eq!(read_manifest(&path).unwrap(), toml::Table::new());
    std::fs::write(
        &path,
        "[a.\"quoted key\"]\ncount = 3\nflag = true\nlist = [\"x\", \"y\"]\nname = \"n\"\n[unread]\nextra = 1\n",
    )
    .unwrap();
    let table = read_manifest(&path).unwrap();
    let inner = &table["a"]["quoted key"];
    assert_eq!(inner["count"].as_integer(), Some(3));
    assert_eq!(inner["flag"].as_bool(), Some(true));
    assert_eq!(inner["list"].as_array().unwrap().len(), 2);
    assert_eq!(inner["name"].as_str(), Some("n"));
    assert_eq!(table["unread"]["extra"].as_integer(), Some(1));
    std::fs::write(&path, "[b]\nvalue = 2\n").unwrap();
    let table = read_manifest(&path).unwrap();
    assert!(table.get("a").is_none());
    assert_eq!(table["b"]["value"].as_integer(), Some(2));
}

#[test]
fn manifest_failures_keep_the_requested_path() {
    let scratch = Scratch::new();
    let path = scratch.root.path().join("Cargo.toml");
    let prefix = format!("{}: ", path.display());
    let read = std::fs::read_to_string(&path).unwrap_err();
    assert_eq!(read_manifest(&path).unwrap_err(), format!("{prefix}{read}"));
    std::fs::write(&path, [0xff, 0xfe, b'x']).unwrap();
    let read = std::fs::read_to_string(&path).unwrap_err();
    assert_eq!(read_manifest(&path).unwrap_err(), format!("{prefix}{read}"));
    for text in ["key = ", "key = 1\nkey = 2\n"] {
        std::fs::write(&path, text).unwrap();
        let parse = text.parse::<toml::Table>().unwrap_err();
        assert_eq!(
            read_manifest(&path).unwrap_err(),
            format!("{prefix}{parse}")
        );
    }
}

#[test]
fn associated_attributes_keep_supported_variants() {
    let impls = [
        "#[doc(one)] #[cfg(test)] #[doc(two)] const C: u8 = 0;",
        "#[doc(one)] #[cfg(test)] #[doc(two)] fn f() {}",
        "#[doc(one)] #[cfg(test)] #[doc(two)] type T = u8;",
        "#[doc(one)] #[cfg(test)] #[doc(two)] m!();",
    ];
    for source in impls {
        let item = syn::parse_str::<syn::ImplItem>(source).unwrap();
        assert_eq!(names(impl_item_attributes(&item)), repeated(), "{source}");
    }
    let traits = [
        "#[doc(one)] #[cfg(test)] #[doc(two)] const C: u8;",
        "#[doc(one)] #[cfg(test)] #[doc(two)] fn f() {}",
        "#[doc(one)] #[cfg(test)] #[doc(two)] type T;",
        "#[doc(one)] #[cfg(test)] #[doc(two)] m!();",
    ];
    for source in traits {
        let item = syn::parse_str::<syn::TraitItem>(source).unwrap();
        assert_eq!(names(trait_item_attributes(&item)), repeated(), "{source}");
    }
    let plain = syn::parse_str::<syn::ImplItem>("fn f() {}").unwrap();
    assert!(impl_item_attributes(&plain).is_empty());
    let plain = syn::parse_str::<syn::TraitItem>("fn f();").unwrap();
    assert!(trait_item_attributes(&plain).is_empty());
    let opaque = syn::ImplItem::Verbatim(proc_macro2::TokenStream::new());
    assert!(impl_item_attributes(&opaque).is_empty());
    let opaque = syn::TraitItem::Verbatim(proc_macro2::TokenStream::new());
    assert!(trait_item_attributes(&opaque).is_empty());
}

/// Measure newline traversal through the repetition check on valid documentation.
fn documentation_scan_bytes(comments: usize) -> usize {
    let contents = "/// Alpha.\n/// Bravo.\n".repeat(comments / 2);
    let member = super::Member {
        name: "example".into(),
        directory: "example".into(),
        sources: vec![super::Source::new("example/src/lib.rs".into(), contents)],
    };
    super::SCANNED_BYTES.set(0);
    crate::repetition::check(&[member]).unwrap();
    super::SCANNED_BYTES.get()
}

#[test]
fn documentation_newline_scans_grow_linearly() {
    crate::repetition::MARKDOWN_PARSES.set(0);
    let small = documentation_scan_bytes(128);
    let large = documentation_scan_bytes(256);
    let largest = documentation_scan_bytes(512);
    println!("newline scan bytes: N={small}, 2N={large}, 4N={largest}");
    assert!(small > 0, "newline traversal must be observed");
    assert_eq!(crate::repetition::MARKDOWN_PARSES.get(), 0);
    assert_eq!(
        largest - large,
        2 * (large - small),
        "each added comment adds the same scan work"
    );
}

/// Root manifest of a one-member fixture workspace that passes every check.
const WORKSPACE_MANIFEST: &str = "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n[workspace.dependencies]\nserde_json = { version = \"=1.0.151\", features = [\"float_roundtrip\"] }\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\nforbidden_lint_groups = \"forbid\"\n[workspace.lints.clippy]\npedantic = { level = \"forbid\", priority = -1 }\nmissing_docs_in_private_items = \"forbid\"\nredundant_clone = \"forbid\"\ntoo_many_arguments = \"forbid\"\nfn_params_excessive_bools = \"forbid\"\ntoo_many_lines = \"forbid\"\ncognitive_complexity = \"forbid\"\nexcessive_nesting = \"forbid\"\nunwrap_used = \"forbid\"\nexpect_used = \"forbid\"\npanic = \"forbid\"\n";

#[test]
fn workspace_checks_lex_and_search_each_source_once() {
    let scratch = Scratch::new();
    let root = scratch.root.path();
    let member = root.join("crates").join("models");
    std::fs::create_dir_all(member.join("src")).unwrap();
    std::fs::write(root.join("Cargo.toml"), WORKSPACE_MANIFEST).unwrap();
    std::fs::write(
        root.join("workspace-crates.json"),
        "{\"maestro-models\": \"core\"}",
    )
    .unwrap();
    std::fs::write(
        member.join("Cargo.toml"),
        "[package]\nname = \"maestro-models\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lints]\nworkspace = true\n",
    )
    .unwrap();
    for (name, contents) in [
        (
            "lib.rs",
            "//! Models.\n\n/// The answer.\npub fn answer() -> &'static str {\n    // A literal follows.\n    \"forty-two\"\n}\n\n#[cfg(test)]\nmod tests;\n",
        ),
        (
            "tests.rs",
            "#[test]\nfn answers() {\n    assert_eq!(super::answer(), \"forty-two\");\n}\n",
        ),
        ("fragment.rs", "let unparsed = 1;\n"),
    ] {
        std::fs::write(member.join("src").join(name), contents).unwrap();
    }
    super::LEXER_PASSES.set(0);
    super::TEST_SPAN_SEARCHES.set(0);
    crate::check_workspace(root).unwrap();
    let passes = (super::LEXER_PASSES.get(), super::TEST_SPAN_SEARCHES.get());
    println!("lexer passes, test-span searches: {passes:?}");
    // Three sources, each lexed once; the two production sources searched once.
    assert_eq!(passes, (3, 2));
}
