//! Start-anchored recognizers match only at the start of their input and keep their own groups.
use crate::{MarkedOptions, rules};

/// Browser grammar, whose rule set covers the hard-break recognizer.
const OPTIONS: MarkedOptions = MarkedOptions::Browser { escape_html: false };

#[test]
fn anchored_rules_match_only_at_the_start_and_keep_their_own_groups() {
    let escape = rules::INLINE_ESCAPE.as_ref().unwrap();
    assert!(escape.find("a\\*").is_none());
    assert!(escape.find("").is_none());
    let found = escape.find("\\*b").unwrap();
    assert_eq!(found.range(), 0..2);
    assert_eq!(found.captures, [Some(1..2)]);
    let break_rule = rules::inline_br(OPTIONS).unwrap();
    let hard = break_rule.find("  \nx").unwrap();
    assert_eq!((hard.range(), hard.captures), (0..3, vec![Some(0..2)]));
}
