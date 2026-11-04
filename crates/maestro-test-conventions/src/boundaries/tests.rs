//! Identifier spellings in the declaration scan and the renderer check.

use std::path::PathBuf;

use super::{Records, SPELLINGS, message_renderers};
use crate::source::{Member, Source};

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

#[test]
fn renderer_check_spells_names_only_in_frontend_production() {
    let member = |name: &str| Member {
        name: name.into(),
        directory: PathBuf::from("crate"),
        sources: vec![Source::new(
            PathBuf::from("crate/src/lib.rs"),
            ORDINARY.into(),
        )],
    };
    message_renderers::SPELLINGS.set(0);
    assert_eq!(
        message_renderers::check(&[member("maestro-models")]),
        Ok(())
    );
    let other = message_renderers::SPELLINGS.get();
    assert_eq!(message_renderers::check(&[member("maestro-chat")]), Ok(()));
    let frontend = message_renderers::SPELLINGS.get() - other;
    println!("renderer spellings: other={other}, frontend={frontend}");
    assert!(frontend > 0, "the frontend's names stay checked");
    assert_eq!(other, 0);
}
