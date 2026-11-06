use super::*;
use ra_ap_rustc_lexer::FrontmatterAllowed;
use std::path::PathBuf;

/// Load one source of an ordinary member.
fn source(contents: String) -> Source {
    let member = Member::with_sources("example", "", [(PathBuf::new(), contents)]);
    member.sources.into_iter().next().unwrap()
}

/// Independent flat lexer vector for identity and count comparisons.
fn flat(source: &str) -> Vec<String> {
    let mut offset = 0;
    ra_ap_rustc_lexer::tokenize(source, FrontmatterAllowed::No)
        .filter_map(|token| {
            let end = offset + token.len as usize;
            let text = &source[offset..end];
            offset = end;
            (!matches!(
                token.kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment { .. }
                    | TokenKind::BlockComment { .. }
            ))
            .then(|| text.into())
        })
        .collect()
}

#[test]
fn interned_groups_match_flat_lexer_vectors() {
    let fragments = [
        ";",
        "foo(value);",
        "foo [value];",
        "foo(value) ; /*x*/",
        "foo({[value]});",
        "foo(\"[}]\");",
        "foo(r#value);",
        "foo(1u8);",
        "foo(1);",
        "foo((value));",
    ];
    let sources: Vec<_> = fragments
        .iter()
        .map(|s| source(format!("fn f() {{{s}}}")))
        .collect();
    let mut tokens = Tokens::default();
    let ids: Vec<_> = sources
        .iter()
        .map(|s| {
            let start = s.contents.find('{').unwrap();
            tokens.blocks(s)[&start]
        })
        .collect();
    for (i, a) in fragments.iter().enumerate() {
        for (j, b) in fragments.iter().enumerate() {
            assert_eq!(ids[i].0 == ids[j].0, flat(a) == flat(b), "{a} / {b}");
            assert_eq!(ids[i].1, flat(a).len());
        }
    }
}

#[test]
fn hash_collisions_keep_exact_token_identity() {
    #[derive(Default)]
    struct Same;
    impl std::hash::Hasher for Same {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, _: &[u8]) {}
    }
    let mut keys: HashMap<Key<'_>, usize, std::hash::BuildHasherDefault<Same>> = HashMap::default();
    keys.insert(Key::Token("one"), 0);
    keys.insert(Key::Token("two"), 1);
    keys.insert(Key::Group("(", vec![0, 1]), 2);
    keys.insert(Key::Group("[", vec![0, 1]), 3);
    assert_eq!(keys.len(), 4);
    assert_eq!(keys.get(&Key::Group("(", vec![0, 1])), Some(&2));
    assert_eq!(keys.get(&Key::Token("two")), Some(&1));
}

#[test]
fn nested_body_indexing_grows_linearly() {
    for nested in [false, true] {
        let mut measurements = Vec::new();
        for n in [32, 64, 128] {
            let source = source(if nested {
                format!("{}{}{}", "fn f(){".repeat(n), ";".repeat(30), "}".repeat(n))
            } else {
                format!("fn f() {{{}}}", ";".repeat(30)).repeat(n)
            });
            let length = source.contents.len();
            WORK.set(0);
            let mut tokens = Tokens::default();
            let blocks = tokens.blocks(&source);
            let work = WORK.get() + length;
            let wrong_rescan: usize = blocks.keys().map(|start| length - start).sum();
            assert!(work <= 8 * length);
            measurements.push((n, length, work, wrong_rescan));
        }
        assert!(measurements[1].2 <= 2 * measurements[0].2);
        assert!(measurements[2].2 <= 2 * measurements[1].2);
        assert!(
            measurements[2].3 > 3 * measurements[1].3,
            "deliberate repeated-suffix negative control"
        );
        println!("GROWTH nested={nested} {measurements:?}");
    }
}

#[test]
fn signature_lines_start_at_the_first_qualifier() {
    let signatures = [
        ("\nfn f()", 2),
        ("\n\nconst\nfn f()", 3),
        ("async\n\nfn f()", 1),
        ("\nunsafe\nfn f()", 2),
        ("\n\nextern \"C\"\nfn f()", 3),
        (
            "\nconst\nasync\nunsafe\nextern \"C\"\nfn f<T>(value: T) -> T\nwhere T: Copy",
            2,
        ),
    ];
    for (text, line) in signatures {
        let signature = syn::parse_str::<syn::Signature>(text).unwrap();
        assert_eq!(super::signature_line(&signature), line, "{text:?}");
    }
    let foreign = syn::parse_str::<syn::ForeignItemFn>("\n\nsafe\nfn f();").unwrap();
    assert_eq!(super::signature_line(&foreign.sig), 3, "safe fn");
}
