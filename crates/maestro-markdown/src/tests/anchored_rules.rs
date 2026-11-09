//! A start-anchored recognizer that does not match costs the same on any suffix length.
use crate::{MarkedOptions, autolinks, rules};
use std::time::{Duration, Instant};

/// Browser grammar, whose rule set covers every anchored recognizer.
const OPTIONS: MarkedOptions = MarkedOptions::Browser { escape_html: false };

/// Timed runs per measurement; the fastest one discards scheduler noise.
const TRIALS: usize = 7;

/// Fixed cost any call may take regardless of the suffix, so timer granularity never decides.
const FLOOR: Duration = Duration::from_micros(10);

/// One recognizer call that must miss on the text it is given.
type Miss<'a> = &'a dyn Fn(&str);

/// Plain prose that starts none of the recognizers.
fn prose(bytes: usize) -> String {
    "step 7 takes about ~5 min, "
        .repeat(bytes / 27 + 1)
        .chars()
        .take(bytes)
        .collect()
}

/// Fastest of several timed runs of `call` on `text`.
fn fastest(text: &str, call: Miss<'_>) -> Duration {
    (0..TRIALS)
        .map(|_| {
            let start = Instant::now();
            call(text);
            start.elapsed()
        })
        .min()
        .unwrap_or_default()
}

/// A named recognizer call that must miss on the text it is given.
macro_rules! miss {
    ($name:literal, $rule:expr) => {
        ($name, &|text: &str| assert!($rule.find(text).is_none()))
    };
}

#[test]
fn failed_anchored_rules_cost_the_same_on_any_suffix_length() {
    let rules: [(&str, Miss<'_>); 12] = [
        miss!("escape", rules::INLINE_ESCAPE.as_ref().unwrap()),
        miss!("tag", rules::INLINE_TAG.as_ref().unwrap()),
        miss!("autolink", rules::INLINE_AUTOLINK.as_ref().unwrap()),
        miss!("break", rules::inline_br(OPTIONS).unwrap()),
        miss!("link", rules::inline_link(OPTIONS).unwrap()),
        miss!("reference", rules::inline_reflink(OPTIONS).unwrap()),
        miss!("label", rules::inline_nolink(OPTIONS).unwrap()),
        miss!(
            "delimiter",
            rules::inline_em_strong_l_delim(OPTIONS).unwrap()
        ),
        miss!("heading", rules::BLOCK_HEADING.as_ref().unwrap()),
        miss!("rule", rules::BLOCK_HR.as_ref().unwrap()),
        miss!("list marker", rules::BLOCK_LIST.as_ref().unwrap()),
        ("address", &|text| {
            assert!(autolinks::address(text, OPTIONS, 0, &mut 0).is_none());
        }),
    ];
    let (short, long) = (prose(1 << 10), prose(1 << 20));
    let slow: Vec<String> = rules
        .into_iter()
        .filter_map(|(name, call)| {
            let (small, large) = (fastest(&short, call), fastest(&long, call));
            (large >= 20 * small.max(FLOOR)).then(|| format!("{name}: {small:?} -> {large:?}"))
        })
        .collect();
    assert!(
        slow.is_empty(),
        "suffix length changed the cost of {slow:?}"
    );
}

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
