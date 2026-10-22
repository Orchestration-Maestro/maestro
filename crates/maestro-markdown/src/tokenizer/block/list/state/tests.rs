//! Direct continuation-history witnesses at trimmed item boundaries.
use super::{ItemLines, MarkedOptions, ScopeSource};

/// Admit a complete prefix using the same physical-line windows as the list owner.
fn framed(text: &str) -> ItemLines {
    let source = ScopeSource::identity(text);
    let mut lines = crate::lexer::lines(text);
    let first = lines.next().unwrap();
    let marker = super::super::marker(&text[first.clone()]).unwrap();
    let mut item = ItemLines::new(
        &source.select(std::iter::once(first)),
        &marker,
        MarkedOptions::Terminal,
    );
    for line in lines {
        assert!(item.append(&source.select(std::iter::once(line))));
    }
    item
}

#[test]
fn resumed_padding_and_blank_history_follow_the_surviving_raw() {
    for (prefix, joined, expected) in [
        ("- a ", "- a \nlazy", "a \nlazy"),
        ("- a\n  p\t", "- a\n  p\t\n  q", "a\np    \nq"),
        ("- a\np\t", "- a\np\t\nq", "a\np\t\nq"),
        ("- a\n  p\n  \n", "- a\n  p\nlazy", "a\np\nlazy"),
        ("- a\n  p\n", "- a\n  p\n\n  q", "a\np\n\nq"),
    ] {
        let mut item = framed(prefix);
        let source = ScopeSource::identity(joined);
        let offset = item.resume(&source.clone().into(), 0);
        for line in crate::lexer::lines(&joined[offset..]) {
            assert!(
                item.append(
                    &source.select(std::iter::once(offset + line.start..offset + line.end,))
                )
            );
        }
        assert_eq!(
            item.body().mapped().to_owned_string(),
            expected,
            "{prefix:?}"
        );
        assert_eq!(item.raw_end(), joined.trim_end().len());
    }
}

#[test]
fn resumed_empty_marker_and_first_line_code_keep_their_stop_rules() {
    let mut empty = framed("-");
    let source = ScopeSource::identity("-\n\nq");
    let offset = empty.resume(&source.clone().into(), 0);
    assert_eq!(offset, 2);
    assert!(empty.append(&source.select(std::iter::once(2..3))));
    assert!(!empty.append(&source.select(std::iter::once(3..4))));
    assert_eq!(empty.body().len(), 0);

    let mut code = framed("-    a");
    let source = ScopeSource::identity("-    a\nlazy");
    let offset = code.resume(&source.clone().into(), 0);
    assert!(!code.append(&source.select(std::iter::once(offset..source.text.len()))));
}

/// Compare one kind's stop verdicts at every leading-space count around the prefix.
fn check_item_kind(
    prefix: usize,
    options: MarkedOptions,
    kind: &str,
    opener: &str,
    failures: &mut Vec<String>,
) {
    let literal_html = options == (MarkedOptions::Browser { escape_html: true });
    let stops = crate::rules::item_stops(prefix + 1, options);
    let line = |spaces: usize| format!("{}{opener}", " ".repeat(spaces));
    let html = matches!(kind, "html comment" | "html tag");
    for spaces in 0..=prefix + 1 {
        let authored = line(spaces);
        let admitted = spaces <= prefix && !(html && literal_html && kind == "html tag");
        if stops.interrupts(&authored) != admitted {
            failures.push(format!("{kind} {prefix} {options:?} {authored:?}"));
        }
    }
    for spaces in 0..=prefix {
        let authored = line(spaces);
        let lazy = matches!(kind, "fence" | "tilde fence" | "heading" | "rule");
        if stops.ends_lazy(&authored) != lazy {
            failures.push(format!("lazy {kind} {prefix} {options:?} {authored:?}"));
        }
    }
}

#[test]
fn item_stops_admit_each_reference_kind_at_its_prefix() {
    let mut failures = Vec::new();
    for prefix in 0..=3 {
        for options in [
            MarkedOptions::Terminal,
            MarkedOptions::Browser { escape_html: false },
            MarkedOptions::Browser { escape_html: true },
        ] {
            for (kind, opener) in [
                ("fence", "```"),
                ("tilde fence", "~~~"),
                ("heading", "# h"),
                ("marker", "- b"),
                ("ordered marker", "2. b"),
                ("rule", "---"),
                ("html comment", "<!-- c -->"),
                ("html tag", "<div>x</div>"),
            ] {
                check_item_kind(prefix, options, kind, opener, &mut failures);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
