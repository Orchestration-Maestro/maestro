//! Identifier spellings in the declaration scan.

use super::{Records, SPELLINGS, anchored_sources, application_exports};
use crate::source::{Member, SYNTAX_PARSES};

/// An ordinary production source with paths, method calls and a closure.
const ORDINARY: &str = "use std::fmt::Display;\nfn show(items: &[u8], value: &dyn Display) -> String {\n    items.iter().map(|item| usize::from(*item)).sum::<usize>().to_string() + &value.to_string()\n}\n";

#[test]
fn declaration_scan_compares_identifiers_without_spelling_them() {
    let mut records = Records::default();
    SPELLINGS.set(0);
    records.scan(ORDINARY.parse().unwrap());
    let spellings = SPELLINGS.get();
    println!("declaration scan spellings: {spellings}");
    assert!(records.declarations.is_empty());
    // Only the function name is spelled, for the owned-function comparison.
    assert_eq!(spellings, 1);
}

/// A credentials root re-exporting one name its declared surface carries.
const CREDENTIALS_ROOT: &str = "pub use credentials::AuthStorage;\n";

#[test]
fn credentials_exports_read_the_shared_syntax_walk() {
    SYNTAX_PARSES.set(0);
    let member = Member::with_sources(
        "maestro-credentials",
        "credentials",
        [("credentials/src/lib.rs", CREDENTIALS_ROOT)],
    );
    application_exports::check(&[member]).unwrap();
    let parses = SYNTAX_PARSES.get();
    println!("credential root syntax parses: {parses}");
    // Loading parsed the root once, and the export check reads that walk instead of parsing again.
    assert_eq!(parses, 1);
}

/// A markdown member holding one source at `path`.
fn markdown(path: &str, contents: &str) -> Member {
    Member::with_sources("maestro-markdown", "markdown", [(path, contents)])
}

/// Production sources that build a `^` recognizer on a plain regex: a static, a function, an
/// impl method, a macro definition and a macro argument.
const PLAIN_START_SOURCES: &[&str] = &[
    "static A: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r\"^a\").ok());",
    "fn b() -> Option<Regex> { Regex::with_flags(r#\"^b\"#, \"u\").ok() }",
    "impl X { fn c(&self) -> Option<Regex> { let s = [r\"^c\", \"d\"].concat(); Regex::new(&s).ok() } }",
    "macro_rules! m { () => { Regex::new(\"^d\") }; }",
    "fn e() { let f = format!(\"^e{}\", 1); }",
];

#[test]
fn a_start_anchored_source_on_a_plain_regex_is_found_in_every_position() {
    for source in PLAIN_START_SOURCES {
        let error = anchored_sources::check(&[markdown("markdown/src/rules.rs", source)])
            .expect_err(source);
        assert!(
            error.ends_with("start-anchored source outside Anchored::new")
                && error.contains("markdown/src/rules.rs:1:"),
            "{source}: {error}"
        );
    }
}

/// Sources the rule accepts: the owner's constructor, an inner caret, a negated class, a
/// quoted double quote and an allow-listed opener.
const ACCEPTED_SOURCES: &[&str] = &[
    "static A: LazyLock<Option<Anchored>> =\n    LazyLock::new(|| Anchored::new(r\"^a\", \"\"));",
    "fn b() { Anchored::new(\n        r#\"^b\"#,\n        \"\",\n    ) }",
    "fn c() { Regex::new(r\"a^\"); Regex::new(r\"[^x]\"); let d = '\"'; }",
    "fn e() { Regex::new(r\"^ {0,3}</(?!script|pre|style|textarea)x\") }",
];

#[test]
fn owner_sources_and_allow_listed_openers_are_accepted() {
    for source in ACCEPTED_SOURCES {
        let member = markdown("markdown/src/rules.rs", source);
        assert_eq!(anchored_sources::check(&[member]), Ok(()), "{source}");
    }
}

#[test]
fn start_anchored_sources_outside_production_or_the_member_are_not_scanned() {
    let seeded = PLAIN_START_SOURCES[0];
    let owner = markdown("markdown/src/rules/anchored.rs", seeded);
    let tests = markdown("markdown/src/tests/rules.rs", seeded);
    let foreign = Member::with_sources("maestro-app", "app", [("app/src/rules.rs", seeded)]);
    for member in [owner, tests, foreign] {
        assert_eq!(anchored_sources::check(&[member]), Ok(()));
    }
}
