//! Test-owned call checks inspect test code alone.

use super::{SPELLINGS, check};
use crate::source::Member;

/// A module without test items, a module that declares its tests and that test module.
fn member() -> Member {
    Member::with_sources(
        "maestro-models",
        "models",
        [
            (
                "models/src/plain.rs",
                "pub fn label() -> String {\n    \"plain\".trim().to_owned() + &String::from(\"x\")\n}\n",
            ),
            (
                "models/src/lib.rs",
                "pub fn total(items: &[u8]) -> usize {\n    items.iter().map(|item| usize::from(*item)).sum()\n}\n\n#[cfg(test)]\nmod tests;\n",
            ),
            (
                "models/src/tests.rs",
                "#[test]\nfn sums() {\n    super::total(&[1]);\n}\n",
            ),
        ],
    )
}

#[test]
fn time_limit_checks_spell_only_test_code() {
    SPELLINGS.set(0);
    assert_eq!(check(&[member()]), Ok(()));
    let spellings = SPELLINGS.get();
    println!("spellings: {spellings}");
    // Only the test's own call is spelled.
    assert_eq!(spellings, 1);
}
