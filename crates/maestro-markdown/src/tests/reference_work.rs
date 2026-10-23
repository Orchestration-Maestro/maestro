//! Reference-rule operand work through existing parser seams.
use super::{SELECTIONS, WORK, Work, assert_literal, parsed};
use crate::{Kind, MarkedOptions, source::ScopeSource};
use std::collections::HashMap;

/// Literal workloads with no unescaped closing bracket.
const FAMILIES: [&str; 3] = ["[b ", "[b \\] ", "[b \\\\x "];

/// Root witness with an empty or unrelated definition index.
fn root(text: &str, options: MarkedOptions, defined: bool, range: std::ops::Range<usize>) -> usize {
    let source = ScopeSource::identity(text);
    let mut definitions = HashMap::new();
    if defined {
        definitions.insert(
            "unused".into(),
            crate::Definition {
                destination: "/unused".into(),
                title: String::new(),
            },
        );
    }
    WORK.with_borrow_mut(|work| *work = Work::default());
    let mut tokenizer =
        crate::tokenizer::inline::Tokenizer::new(&source, options, &definitions, None, false);
    let nodes = tokenizer.tokens(range.clone(), &[], None);
    assert_eq!(
        nodes
            .iter()
            .map(|node| node.authored.to_owned_string())
            .collect::<String>(),
        &text[range]
    );
    WORK.with_borrow(|work| work.reference_bytes)
}

#[test]
fn unclosed_reference_root_work_is_bounded() {
    let mut failures = Vec::new();
    for options in SELECTIONS {
        for defined in [false, true] {
            for family in FAMILIES {
                family_work(options, defined, family, &mut failures);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Root work at N, 2N and 4N repeats of one literal family.
fn family_work(options: MarkedOptions, defined: bool, family: &str, failures: &mut Vec<String>) {
    let counts = [32, 64, 128].map(|n| {
        let text = family.repeat(n);
        let work = root(&text, options, defined, 0..text.len());
        if work > 3 * text.len() {
            failures.push(format!(
                "{options:?} defined={defined} {family:?} n={n}: {work} > {}",
                3 * text.len()
            ));
        }
        work
    });
    eprintln!("REFERENCE ROOT {options:?} defined={defined} {family:?}: {counts:?}");
}

#[test]
fn unclosed_reference_public_routes_keep_literal_output() {
    for options in SELECTIONS {
        for extension in [false, true] {
            route_work(options, extension);
        }
    }
}

/// Literal output and the same bound through one public entry route.
fn route_work(options: MarkedOptions, extension: bool) {
    for n in [32, 64, 128] {
        let text = "[b ".repeat(n);
        for defined in [false, true] {
            let input = if defined {
                format!("{text}\n\n[unused]: /unused")
            } else {
                text.clone()
            };
            WORK.with_borrow_mut(|work| *work = Work::default());
            let document = parsed(input, options, extension);
            let Kind::Paragraph(children) = &document.nodes[0].kind else {
                panic!("paragraph");
            };
            assert_literal(children, &text, "reference output");
            let work = WORK.with_borrow(|work| work.reference_bytes);
            assert!(
                work <= 3 * document.source.len(),
                "{options:?} extension={extension} defined={defined} n={n}: {work}"
            );
        }
    }
}

#[test]
fn clipped_reference_windows_do_not_see_outside_closers() {
    let mut failures = Vec::new();
    for options in SELECTIONS {
        for n in [32, 64, 128] {
            let text = format!("xx{}]", "[b ".repeat(n));
            let work = root(&text, options, true, 2..text.len() - 1);
            if work > 3 * text.len() {
                failures.push(format!("{options:?} clipped n={n}: {work}"));
            }
            eprintln!("REFERENCE CLIPPED {options:?} n={n}: {work}");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
