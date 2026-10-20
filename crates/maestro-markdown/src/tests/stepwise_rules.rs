//! Stepwise-spelled recognizers match exactly as their reference spellings do.
use crate::{MarkedOptions, rules};
use regress::Regex;

/// Pieces that reach every text alternative: stops, hard breaks, mail runs, links and wide text.
const TEXT_PIECES: [&str; 37] = [
    "a", "b", "Z", "9", " ", "  ", "\n", "  \n", " \n", "`", "``", "~", "~~", "*", "_", "\\", "<",
    "!", "[", "]", "@", "user", "@x.y", "http://", "https://", "HTTP://", "ftp://", "www.", "é",
    "界", "😀", ".", "-", "\t", "a_b", "x@", "\n\n",
];

/// Pieces that open and close fences, raw HTML, comments, processing instructions and code spans.
const MARKUP_PIECES: [&str; 26] = [
    "```",
    "~~~",
    "`",
    "``",
    "<!--",
    "-->",
    "-",
    ">",
    "<script>",
    "</script>",
    "<PRE ",
    "</pre>",
    "<?",
    "?>",
    "<!X ",
    "<![CDATA[",
    "]]>",
    "<div>",
    "</div>",
    "\n",
    "\n\n",
    " ",
    "x",
    "é",
    "=",
    "\"",
];

/// Compare the compiled recognizer with its reference spelling on generated windows.
fn assert_same_matches(compiled: &Regex, reference: &Regex, pieces: &[&str], label: &str) {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        usize::try_from(state % 1024).unwrap()
    };
    for _ in 0..4096 {
        let window: String = (0..=next() % 12)
            .map(|_| pieces[next() % pieces.len()])
            .collect();
        let groups = |rule: &Regex| {
            rule.find(&window)
                .map(|found| found.groups().collect::<Vec<_>>())
        };
        assert_eq!(groups(compiled), groups(reference), "{label} {window:?}");
    }
}

#[test]
fn inline_text_rules_match_their_reference_spellings() {
    for options in [
        MarkedOptions::Documentation,
        MarkedOptions::Terminal,
        MarkedOptions::Export,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
    ] {
        let reference = Regex::with_flags(rules::inline_text_source(options), "").unwrap();
        let compiled = rules::inline_text(options).unwrap();
        assert_same_matches(compiled, &reference, &TEXT_PIECES, &format!("{options:?}"));
    }
}

#[test]
fn markup_rules_match_their_reference_spellings() {
    for (label, compiled, source, flags) in [
        (
            "fences",
            &rules::BLOCK_FENCES,
            rules::BLOCK_FENCES_SOURCE,
            "",
        ),
        (
            "html",
            &rules::BLOCK_HTML,
            rules::BLOCK_HTML_SOURCE.as_str(),
            "i",
        ),
        ("tag", &rules::INLINE_TAG, rules::INLINE_TAG_SOURCE, ""),
        ("code", &rules::INLINE_CODE, rules::INLINE_CODE_SOURCE, ""),
    ] {
        let reference = Regex::with_flags(source, flags).unwrap();
        assert_same_matches(
            compiled.as_ref().unwrap(),
            &reference,
            &MARKUP_PIECES,
            label,
        );
    }
}
