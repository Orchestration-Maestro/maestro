//! Block hints are independent of later inline acceptance.
use super::{Delimited, WORK, Work, hinted_blocks};

#[test]
fn block_hints_clip_before_inline_operands() {
    for source in [
        "See [outer $$x$$ [inner](/in)](/out)",
        "See https://outer.example/[[a]$$x$$](/url)",
        "`[a` $$x$$ `z](/url)",
        "<https://a.test/[b> $$x$$ ](/url)",
        "p <i data-x=\"[a\"> $$x$$ ](/url)",
        "$[a$ $$x$$ z](/url)",
        "p `$$x$$` tail",
        "See <https://a.test/$$x$$> tail",
        "p <i data-x=\"$$x$$\"> t",
        "[a $$x$$](/ok)",
        "See [outer $$x$$ [inner][r]](/out)\n\n[r]: /in",
        "See [outer $$x$$ [r]](/out)\n\n[r]: /in",
    ] {
        for escape_html in [false, true] {
            let options = crate::MarkedOptions::Browser { escape_html };
            let blocks = hinted_blocks(source, options, Delimited("$$", "$$"));
            assert!(
                blocks.iter().any(|block| block == "block $$x$$"),
                "{source:?} {escape_html}: {blocks:?}"
            );
        }
    }
}

#[test]
fn equation_hints_avoid_failed_link_suffix_work() {
    let counts = [32, 64, 128].map(|count| {
        let source = "Intro\n\n".to_owned() + &"\\[x\\]\n\n".repeat(count);
        WORK.with_borrow_mut(|work| *work = Work::default());
        let blocks = hinted_blocks(
            &source,
            crate::MarkedOptions::Browser { escape_html: true },
            Delimited("\\[", "\\]"),
        );
        assert_eq!(blocks.len(), count + 1);
        let bytes = WORK.with_borrow(|work| work.link_bytes);
        eprintln!("FAILED-LINK-WORK n={count} bytes={bytes}");
        bytes
    });
    for pair in counts.windows(2) {
        assert!(pair[1] <= 3 * pair[0], "link suffix bytes: {counts:?}");
    }
}

#[test]
fn block_hint_work_grows_with_equations_and_urls() {
    for family in ["equations", "urls"] {
        let counts = [32, 64, 128].map(|count| {
            let source = if family == "equations" {
                "Intro\n\n".to_owned() + &"\\[x\\]\n\n".repeat(count)
            } else {
                "See https://a.test/".to_owned() + &"\\[https://b.test/".repeat(count)
            };
            WORK.with_borrow_mut(|work| *work = Work::default());
            let blocks = hinted_blocks(
                &source,
                crate::MarkedOptions::Browser { escape_html: true },
                Delimited("\\[", "\\]"),
            );
            if family == "equations" {
                assert_eq!(blocks.len(), count + 1);
            }
            let work = WORK.with_borrow(|work| *work);
            eprintln!("HINT-WORK family={family} n={count} {work:?}");
            work
        });
        for pair in counts.windows(2) {
            assert!(
                pair[1].mask_bytes <= 3 * pair[0].mask_bytes,
                "{family}: {counts:?}"
            );
            assert!(
                pair[1].address_bytes <= 3 * pair[0].address_bytes,
                "{family}: {counts:?}"
            );
            assert!(
                pair[1].operand_admissions <= 3 * pair[0].operand_admissions,
                "{family}: {counts:?}"
            );
        }
    }
}

#[test]
fn code_held_angles_leave_a_later_quote_marker_alone() {
    for (source, expected) in [
        (
            "Compare `a<b`.\n\n> note\n",
            &["paragraph Compare `a<b`.", "gap", "quote > note"][..],
        ),
        (
            "```\n<a\n```\n\n> note\n",
            &["code ```\n<a\n```", "gap", "quote > note"][..],
        ),
    ] {
        let document = crate::parse_with_options(
            source.to_owned(),
            crate::MarkedOptions::Browser { escape_html: true },
        );
        let shapes: Vec<String> = document
            .nodes
            .iter()
            .map(|node| {
                let kind = match &node.kind {
                    crate::Kind::Paragraph(_) => "paragraph",
                    crate::Kind::Quote(..) => "quote",
                    crate::Kind::CodeBlock(..) => "code",
                    crate::Kind::Gap => "gap",
                    _ => "other",
                };
                let text = node.authored.to_owned_string();
                let text = text.trim_end();
                if text.is_empty() {
                    kind.to_owned()
                } else {
                    format!("{kind} {text}")
                }
            })
            .collect();
        assert_eq!(shapes, expected, "{source:?}");
    }
}

#[test]
fn clipped_escaped_angle_keeps_its_closing_angle_literal() {
    let source = "<https://example.test/$$x$$>";
    let escaped = crate::MarkedOptions::Browser { escape_html: true };
    assert_eq!(
        hinted_blocks(source, escaped, Delimited("$$", "$$")),
        ["<https://example.test/", "block $$x$$", ">"]
    );
    assert_eq!(
        hinted_blocks(
            source,
            crate::MarkedOptions::Browser { escape_html: false },
            Delimited("$$", "$$"),
        ),
        ["<https://example.test/", "block $$x$$"]
    );
}
