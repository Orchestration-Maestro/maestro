//! Identifier spellings in the renderer check.

use super::{SPELLINGS, check};
use crate::source::Member;

/// An ordinary production source with paths, method calls and a closure.
const ORDINARY: &str = "use std::fmt::Display;\nfn show(items: &[u8], value: &dyn Display) -> String {\n    items.iter().map(|item| usize::from(*item)).sum::<usize>().to_string() + &value.to_string()\n}\n";

#[test]
fn renderer_check_spells_names_only_in_frontend_production() {
    let member = |name: &str| Member::with_sources(name, "crate", [("crate/src/lib.rs", ORDINARY)]);
    SPELLINGS.set(0);
    assert_eq!(check(&[member("maestro-models")]), Ok(()));
    let other = SPELLINGS.get();
    assert_eq!(check(&[member("maestro-chat")]), Ok(()));
    let frontend = SPELLINGS.get() - other;
    println!("renderer spellings: other={other}, frontend={frontend}");
    assert!(frontend > 0, "the frontend's names stay checked");
    assert_eq!(other, 0);
}
