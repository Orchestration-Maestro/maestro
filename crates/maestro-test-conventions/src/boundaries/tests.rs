//! Identifier spellings in the declaration scan.

use super::{Records, SPELLINGS};

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
