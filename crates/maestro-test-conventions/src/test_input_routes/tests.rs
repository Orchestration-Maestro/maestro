//! Directory-read detection inspects test code alone.

use std::path::PathBuf;

use super::{SPELLINGS, VISITED, reads};
use crate::source::{Member, Source};

#[test]
fn directory_reads_visit_and_spell_only_test_code() {
    let source = |name: &str, contents: &str| {
        Source::new(PathBuf::from(format!("models/src/{name}")), contents.into())
    };
    let member = Member {
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
    };
    VISITED.set(0);
    SPELLINGS.set(0);
    for source in &member.sources {
        assert!(reads(&member, source).is_empty());
    }
    let work = (VISITED.get(), SPELLINGS.get());
    println!("visited sources, spellings: {work:?}");
    // The module without test items is skipped, and only the test's own path is spelled.
    assert_eq!(work, (2, 1));
}
