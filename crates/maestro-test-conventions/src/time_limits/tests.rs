//! Test-owned call checks inspect test code alone.

use std::path::PathBuf;

use super::{SPELLINGS, check};
use crate::source::{Member, Source};

/// A module without test items, a module that declares its tests and that test module.
fn member() -> Member {
    let source = |name: &str, contents: &str| {
        Source::new(PathBuf::from(format!("models/src/{name}")), contents.into())
    };
    Member {
        name: "maestro-models".into(),
        directory: PathBuf::from("models"),
        sources: vec![
            source(
                "plain.rs",
                "pub fn label() -> String {\n    \"plain\".trim().to_owned() + &String::from(\"x\")\n}\n",
            ),
            source(
                "lib.rs",
                "pub fn total(items: &[u8]) -> usize {\n    items.iter().map(|item| usize::from(*item)).sum()\n}\n\n#[cfg(test)]\nmod tests;\n",
            ),
            source(
                "tests.rs",
                "#[test]\nfn sums() {\n    super::total(&[1]);\n}\n",
            ),
        ],
    }
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
