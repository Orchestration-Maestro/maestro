use super::words::{find, fnv1a, spans};
use super::{Exemption, Lists, check_with, exemptions};
use crate::source::Member;
use maestro_test_directory::Scratch;
use std::path::Path;

/// A member holding one source file with the given contents.
fn member(contents: &str) -> Member {
    Member::with_sources(
        "maestro-example",
        "/workspace/crates/example",
        [("/workspace/crates/example/src/lib.rs", contents)],
    )
}

/// The readable words these tests deny; neither is on the real list.
const WORDS: &[&str] = &["quartz", "marble_slab"];

/// A word that stands in for another project's name in the hashed list.
const STAND_IN: &str = "zebra";

/// The exemption key of the stand-in word.
const STAND_IN_KEY: &str = "0xf7197331669181af";

/// The readable test words found in a name.
fn denied(name: &str) -> Vec<String> {
    find(name, WORDS, &[])
        .into_iter()
        .map(|found| found.word)
        .collect()
}

/// A name's lowercase segments.
fn segments(name: &str) -> Vec<String> {
    spans(name)
        .into_iter()
        .map(|span| name[span].to_lowercase())
        .collect()
}

/// The exemption that hides `quartz` in the example source.
fn quartz_exemption() -> Exemption {
    Exemption {
        path: "crates/example/src/lib.rs",
        word: "quartz",
        reason: "a stored file carries the name",
    }
}

/// The diagnostics of the example source under the test words and the given exemptions.
fn diagnostics(contents: &str, exempt: &[Exemption]) -> String {
    let lists = Lists {
        denied: WORDS,
        hashed: &[fnv1a(STAND_IN)],
        exempt,
    };
    check_with(Path::new("/workspace"), &[member(contents)], &lists)
        .err()
        .unwrap_or_default()
}

#[test]
fn names_split_at_separators_and_case_changes() {
    assert_eq!(segments("quartz_load-table"), ["quartz", "load", "table"]);
    assert_eq!(segments("QuartzLoadTable"), ["quartz", "load", "table"]);
    assert_eq!(segments("HTMLParser"), ["html", "parser"]);
    assert_eq!(segments("marble.slab/x"), ["marble", "slab", "x"]);
}

#[test]
fn a_denied_word_matches_only_whole_segments() {
    assert_eq!(denied("parse_quartz_value"), ["quartz"]);
    assert_eq!(denied("Quartz"), ["quartz"]);
    assert_eq!(denied("MarbleSlab"), ["marble_slab"]);
    assert_eq!(denied("marble-slab-browser"), ["marble_slab"]);
    assert!(denied("quartzite").is_empty());
    assert!(denied("quartzes").is_empty());
    assert!(denied("marble_floor").is_empty());
}

#[test]
fn a_denied_word_is_joined_by_underscore_hyphen_or_case_alone() {
    assert!(denied("marble.slab").is_empty());
    assert!(denied("marble/slab").is_empty());
    assert!(denied("marble slab").is_empty());
    assert_eq!(denied("a marble_slab b"), ["marble_slab"]);
}

#[test]
fn the_hash_is_fnv_1a_over_the_utf8_bytes() {
    assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a("foobar"), 0x8594_4171_f739_67e8);
    assert_eq!(format!("{:#018x}", fnv1a(STAND_IN)), STAND_IN_KEY);
}

#[test]
fn a_hashed_word_is_found_as_the_text_spells_it() {
    let hashed = [fnv1a(STAND_IN)];
    let found = find("the Zebra_crossing", &[], &hashed);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].word, "Zebra");
    assert_eq!(found[0].key, STAND_IN_KEY);
    assert_eq!(found[0].offset, 4);
    assert!(find("zebras", &[], &hashed).is_empty());
}

#[test]
fn literals_and_comments_are_read_with_their_lines() {
    let error = diagnostics(
        "//! Zebra.\nconst A: &str = \"a\\nquartz\";\n/// quartz\nfn x() {} // ok\nconst B: &[u8] = br#\"quartz\"#;\n",
        &[],
    );
    let lines = error.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 4, "{error}");
    assert!(
        lines[0].contains("lib.rs:1: doc comment holds the word `Zebra`"),
        "{error}"
    );
    assert!(
        lines[1].contains("lib.rs:2: string literal holds the word `quartz`"),
        "{error}"
    );
    assert!(
        lines[2].contains("lib.rs:3: doc comment holds the word `quartz`"),
        "{error}"
    );
    assert!(
        lines[3].contains("lib.rs:5: string literal holds the word `quartz`"),
        "{error}"
    );
    assert!(
        lines.iter().all(|line| line.contains("DENIED and HASHED")),
        "{error}"
    );
}

#[test]
fn a_line_comment_and_a_block_comment_are_read() {
    let error = diagnostics("// quartz\n/* zebra */\nfn x() {}\n", &[]);
    assert!(
        error.contains("lib.rs:1: comment holds the word `quartz`"),
        "{error}"
    );
    assert!(
        error.contains("lib.rs:2: comment holds the word `zebra`"),
        "{error}"
    );
}

#[test]
fn a_clean_source_passes() {
    assert_eq!(
        diagnostics("//! Text.\nconst A: &str = \"ok\";\nfn read() {}\n", &[]),
        ""
    );
}

#[test]
fn an_exemption_that_matches_a_word_hides_it() {
    assert_eq!(
        diagnostics(
            "fn quartz_value() {}\nconst A: &str = \"quartz\";\n",
            &[quartz_exemption()]
        ),
        ""
    );
}

#[test]
fn an_exemption_names_a_hashed_word_by_its_hash() {
    let row = Exemption {
        path: "crates/example/src/lib.rs",
        word: STAND_IN_KEY,
        reason: "a detected program carries the name",
    };
    assert_eq!(diagnostics("const A: &str = \"Zebra\";\n", &[row]), "");
}

#[test]
fn an_exemption_that_matches_nothing_is_reported() {
    let error = diagnostics("fn value() {}\n", &[quartz_exemption()]);
    assert!(
        error.contains("exemption for `crates/example/src/lib.rs` and `quartz`")
            && error.contains("remove the row from EXEMPT"),
        "{error}"
    );
}

#[test]
fn an_exemption_covers_its_file_and_word_only() {
    let other_file = Exemption {
        path: "crates/example/src/other.rs",
        ..quartz_exemption()
    };
    let other_word = Exemption {
        word: "marble_slab",
        ..quartz_exemption()
    };
    for row in [other_file, other_word] {
        let error = diagnostics("fn quartz_value() {}\n", &[row]);
        assert!(
            error.contains("lib.rs:1: identifier `quartz_value`"),
            "{error}"
        );
        assert!(error.contains("matches no word"), "{error}");
    }
}

#[test]
fn a_row_whose_path_is_gone_from_the_workspace_is_reported() {
    let root = Scratch::new().unwrap();
    let list = root.path().join(super::LIST_FILE);
    std::fs::create_dir_all(list.parent().unwrap()).unwrap();
    std::fs::write(list, "").unwrap();
    assert_eq!(
        exemptions(root.path()).len(),
        super::DENIED.len() + super::EXEMPT.len()
    );
}

#[test]
fn a_tree_without_the_list_file_has_no_rows() {
    let root = Scratch::new().unwrap();
    assert!(exemptions(root.path()).is_empty());
}
