//! Test-owned call checks inspect test code alone.

use std::path::PathBuf;

use super::{SPELLINGS, VISITED, check};
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
fn time_limit_checks_visit_and_spell_only_test_code() {
    VISITED.set(0);
    SPELLINGS.set(0);
    assert_eq!(check(&[member()]), Ok(()));
    let work = (VISITED.get(), SPELLINGS.get());
    println!("visited sources, spellings: {work:?}");
    // The module without test items is skipped, and only the test's own call is spelled.
    assert_eq!(work, (2, 1));
}
