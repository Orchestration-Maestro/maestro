//! Item stop admission against the composed stop grammar.
use super::{ItemStops, item_stops, rule_matches};
use crate::{MarkedOptions, tests::WORK};

/// Lines that open with every printable ASCII byte, a tab and one non-ASCII scalar after
/// zero to four spaces.
fn lines() -> Vec<String> {
    let openers = (0x20..0x7f).map(char::from).chain(['\t', 'é']);
    let mut lines = Vec::new();
    for opener in openers {
        for rest in ["", "\n", "--", "``\n", " b", "-!-- c -->", "div>x"] {
            for spaces in 0..=4 {
                lines.push(format!("{}{opener}{rest}", " ".repeat(spaces)));
            }
        }
    }
    lines
}

/// Verdicts of both stop sets and the line bytes the grammar was given for them.
fn admit(stops: &ItemStops, line: &str) -> (bool, bool, usize) {
    WORK.with_borrow_mut(|work| work.item_stop_bytes = 0);
    let verdicts = (stops.interrupts(line), stops.ends_lazy(line));
    let consulted = WORK.with_borrow(|work| work.item_stop_bytes);
    (verdicts.0, verdicts.1, consulted)
}

/// Compare one line's verdicts with the composed grammar's, and check that a line opening
/// with a quote marker or a letter gives the grammar no bytes.
fn check(stops: &ItemStops, line: &str) {
    let (interrupts, lazy, consulted) = admit(stops, line);
    let text = line.trim_start_matches([' ', '\t']);
    let columns = line[..line.len() - text.len()]
        .chars()
        .map(|scalar| if scalar == '\t' { 4 } else { 1 })
        .sum::<usize>();
    let grammar = |rule| rule_matches(rule, line);
    assert_eq!(interrupts, grammar(stops.interrupts.as_ref()), "{line:?}");
    let code = columns >= 4 && !text.is_empty();
    assert_eq!(lazy, code || grammar(stops.lazy.as_ref()), "{line:?}");
    if text.starts_with(|scalar: char| scalar == '>' || scalar.is_alphabetic()) {
        assert_eq!(consulted, 0, "{line:?}");
    }
}

#[test]
fn item_stops_consult_the_grammar_only_where_a_stop_can_begin() {
    let lines = lines();
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: true },
    ] {
        for indent in 1..=4 {
            let stops = item_stops(indent, options);
            for line in &lines {
                check(stops, line);
            }
        }
    }
}
