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

/// One reference stop kind and the verdicts it must reach.
struct StopKind {
    /// Reference rule the opener exercises.
    kind: &'static str,
    /// Spelling tested at every leading-space count around the prefix.
    opener: &'static str,
    /// Stops an item under the default caller grammar.
    interrupts: bool,
    /// Stops an item under the caller's literal-markup view.
    interrupts_literal: bool,
    /// Ends lazy continuation.
    lazy: bool,
}

/// Reference stop kinds with their verdicts, HTML openers last.
const STOP_KINDS: [StopKind; 8] = [
    StopKind {
        kind: "fence",
        opener: "```",
        interrupts: true,
        interrupts_literal: true,
        lazy: true,
    },
    StopKind {
        kind: "tilde fence",
        opener: "~~~",
        interrupts: true,
        interrupts_literal: true,
        lazy: true,
    },
    StopKind {
        kind: "heading",
        opener: "# h",
        interrupts: true,
        interrupts_literal: true,
        lazy: true,
    },
    StopKind {
        kind: "marker",
        opener: "- b",
        interrupts: true,
        interrupts_literal: true,
        lazy: false,
    },
    StopKind {
        kind: "ordered marker",
        opener: "2. b",
        interrupts: true,
        interrupts_literal: true,
        lazy: false,
    },
    StopKind {
        kind: "rule",
        opener: "---",
        interrupts: true,
        interrupts_literal: true,
        lazy: true,
    },
    StopKind {
        kind: "html comment",
        opener: "<!-- c -->",
        interrupts: true,
        interrupts_literal: true,
        lazy: false,
    },
    StopKind {
        kind: "html tag",
        opener: "<div>x</div>",
        interrupts: true,
        interrupts_literal: false,
        lazy: false,
    },
];

/// Compare one kind's stop verdicts at every leading-space count around the prefix.
fn check_item_kind(
    prefix: usize,
    options: MarkedOptions,
    row: &StopKind,
    failures: &mut Vec<String>,
) {
    let stops = crate::rules::item_stops(prefix + 1, options);
    let line = |spaces: usize| format!("{}{}", " ".repeat(spaces), row.opener);
    let interrupts = if options == (MarkedOptions::Browser { escape_html: true }) {
        row.interrupts_literal
    } else {
        row.interrupts
    };
    for spaces in 0..=prefix + 1 {
        let authored = line(spaces);
        if stops.interrupts(&authored) != (interrupts && spaces <= prefix) {
            failures.push(format!("{} {prefix} {options:?} {authored:?}", row.kind));
        }
    }
    for spaces in 0..=prefix + 1 {
        let authored = line(spaces);
        let lazy = row.lazy && spaces <= prefix || spaces >= 4;
        if stops.ends_lazy(&authored) != lazy {
            failures.push(format!(
                "lazy {} {prefix} {options:?} {authored:?}",
                row.kind
            ));
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
            for row in &STOP_KINDS {
                check_item_kind(prefix, options, row, &mut failures);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
