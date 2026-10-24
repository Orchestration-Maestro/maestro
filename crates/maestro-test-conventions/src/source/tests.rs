//! Tests for the private manifest reader and associated-item attribute accessors.

#[path = "../../tests/support/scratch.rs"]
mod scratch;

use std::path::Path;

use ra_ap_rustc_lexer::TokenKind;

use self::scratch::Scratch;
use super::{files, impl_item_attributes, read_manifest, tokens, trait_item_attributes};

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
        sources: vec![super::Source {
            path: "example/src/lib.rs".into(),
            contents,
            syntax: None,
        }],
    };
    super::SCANNED_BYTES.set(0);
    crate::repetition::check(&[member]).unwrap();
    super::SCANNED_BYTES.get()
}

#[test]
fn documentation_newline_scans_grow_linearly() {
    let small = documentation_scan_bytes(128);
    let large = documentation_scan_bytes(256);
    println!("newline scan bytes: N={small}, 2N={large}");
    assert!(small > 0, "newline traversal must be observed");
    assert_eq!(large, 2 * small, "doubling documentation doubles scan work");
}

/// Count lexer walk entry identifiers in one file's token stream, macro arguments included.
fn lexer_walks(source: &str) -> usize {
    tokens(source)
        .filter(|(kind, range)| {
            matches!(kind, TokenKind::Ident)
                && ["tokenize", "advance_token"].contains(&&source[range.clone()])
        })
        .count()
}

#[test]
fn token_walks_stay_in_the_source_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let walks: Vec<_> = files(&root)
        .unwrap()
        .into_iter()
        .filter_map(|path| {
            let count = lexer_walks(&std::fs::read_to_string(&path).unwrap());
            let relative = path.strip_prefix(&root).unwrap();
            // The duplicate checker's flat oracle stays independent of the owner it is compared with.
            let allowed = usize::from(
                relative == Path::new("source.rs") || relative == Path::new("duplicates/tests.rs"),
            );
            (count != allowed).then(|| (relative.display().to_string(), count))
        })
        .collect();
    assert_eq!(walks, []);
}

#[test]
fn token_walk_guard_rejects_independent_walks() {
    let bypasses = [
        "fn exact_offset(source: &str) -> Option<usize> {\n    let mut offset = ra_ap_rustc_lexer::strip_shebang(source).unwrap_or(0);\n    for token in ra_ap_rustc_lexer::tokenize(&source[offset..], FrontmatterAllowed::No) {\n        offset += token.len as usize;\n    }\n    None\n}\n",
        "fn counts(s: &str) -> Vec<usize> {\n    vec![ra_ap_rustc_lexer::tokenize(s, FrontmatterAllowed::No).count()]\n}\n",
        "fn walk() {\n    let walk = |s: &str| lexer::tokenize(s, FrontmatterAllowed::No).count();\n}\n",
        "impl Walker {\n    fn walk(&self, s: &str) -> usize {\n        tokenize(s, FrontmatterAllowed::No).count()\n    }\n}\n",
        "use ra_ap_rustc_lexer::tokenize as lex;\n",
        "fn walk(s: &str) {\n    Cursor::new(s, FrontmatterAllowed::No).advance_token();\n}\n",
    ];
    let accepted: Vec<_> = bypasses
        .into_iter()
        .filter(|bypass| lexer_walks(bypass) == 0)
        .collect();
    assert_eq!(accepted, Vec::<&str>::new());
    let control = "fn walk(s: &str) {\n    let _ = crate::source::tokens(s);\n    tokens(s).count();\n    // tokenize\n    let _ = \"advance_token\";\n}\n";
    assert_eq!(lexer_walks(control), 0);
}
