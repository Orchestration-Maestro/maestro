//! Directory-read detection inspects test code alone.

use super::{SPELLINGS, reads};
use crate::source::Member;

#[test]
fn directory_reads_spell_only_test_code() {
    // The shared walk gathers the reads while the sources load.
    SPELLINGS.set(0);
    let member = Member::with_sources(
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
    );
    for source in &member.sources {
        assert_eq!(reads(source).count(), 0);
    }
    let spellings = SPELLINGS.get();
    println!("spellings: {spellings}");
    // Only the test's own path is spelled.
    assert_eq!(spellings, 1);
}
