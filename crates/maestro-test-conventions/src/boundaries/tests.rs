//! Identifier spellings in the declaration scan.

use super::{Records, SPELLINGS, application_exports};
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
