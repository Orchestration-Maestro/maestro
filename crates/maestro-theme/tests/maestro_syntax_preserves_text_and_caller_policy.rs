//! Terminal rows and escaped HTML keep every code character and each caller's policy.
#[cfg(test)]
mod published;

use maestro_theme::{
    LiveTheme, SyntaxHighlighter, SyntaxSpan, SyntectHighlighter, ThemeColor, ThemeState,
    get_language_from_path, get_markdown_theme, highlight_code, highlight_html,
};
use published::{published, theme};
use serde_json::Value;
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::sync::OnceLock;

/// Supports fixed labels and answers every highlight with one scripted outcome.
struct Scripted {
    /// Labels reported as supported.
    languages: &'static [&'static str],
    /// Spans returned, or the message of the failure returned.
    outcome: Result<Vec<SyntaxSpan>, &'static str>,
    /// `supports:<label>` and `highlight:<label>` entries in call order.
    log: RefCell<Vec<String>>,
}

impl Scripted {
    /// Support `languages` and answer with `outcome`.
    fn new(
        languages: &'static [&'static str],
        outcome: Result<Vec<SyntaxSpan>, &'static str>,
    ) -> Self {
        Self {
            languages,
            outcome,
            log: RefCell::default(),
        }
    }

    /// Fail every highlight of a supported `javascript` label.
    fn failing() -> Self {
        Self::new(&["javascript"], Err("controlled engine failure"))
    }

    /// The decisions recorded so far.
    fn calls(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl SyntaxHighlighter for Scripted {
    fn supports_language(&self, language: &str) -> bool {
        self.log.borrow_mut().push(format!("supports:{language}"));
        self.languages.contains(&language)
    }

    fn highlight(
        &self,
        _code: &str,
        language: &str,
    ) -> Result<Vec<SyntaxSpan>, Box<dyn Error + Send + Sync>> {
        self.log.borrow_mut().push(format!("highlight:{language}"));
        self.outcome.clone().map_err(Into::into)
    }
}

/// The production highlighter, loaded once for the whole target.
#[cfg(test)]
fn native() -> &'static SyntectHighlighter {
    static NATIVE: OnceLock<SyntectHighlighter> = OnceLock::new();
    NATIVE.get_or_init(|| SyntectHighlighter::new().unwrap())
}

/// A span of `range` with a syntax color.
fn span(range: std::ops::Range<usize>, color: Option<ThemeColor>) -> SyntaxSpan {
    SyntaxSpan { range, color }
}

/// Text without any `ESC [ ... m` sequence.
fn strip(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            chars.by_ref().find(|&end| end == 'm');
        } else {
            out.push(c);
        }
    }
    out
}

/// The prefix and reset around `text` in the 256-color palette.
fn color(index: u8, text: &str) -> String {
    format!("\x1b[38;5;{index}m{text}\x1b[39m")
}

/// Palette index of `mdCodeBlock` in the first test theme.
const CODE: u8 = 21;
/// Palette index of `syntaxComment`.
const COMMENT: u8 = 30;
/// Palette index of `syntaxKeyword`.
const KEYWORD: u8 = 31;
/// Palette index of `syntaxFunction`.
const FUNCTION: u8 = 32;
/// Palette index of `syntaxVariable`.
const VARIABLE: u8 = 33;
/// Palette index of `syntaxString`.
const STRING: u8 = 34;
/// Palette index of `syntaxNumber`.
const NUMBER: u8 = 35;
/// Palette index of `syntaxType`.
const TYPE: u8 = 36;
/// Palette index of `syntaxOperator`.
const OPERATOR: u8 = 37;
/// Palette index of `syntaxPunctuation`.
const PUNCTUATION: u8 = 38;

/// A live handle whose published theme colors key `i` of the fixtures with `i + 1`.
fn live() -> (ThemeState, LiveTheme) {
    published(&theme(0))
}

/// Terminal rows of `code` for `lang` through the production highlighter.
fn native_rows(live: &LiveTheme, code: &str, lang: &str) -> Vec<String> {
    highlight_code(live, native(), code, Some(lang))
}

/// Fixture cases of the fallback rows for unlabeled and unknown languages.
#[cfg(test)]
fn fallback_cases(language: Option<&str>) -> Vec<(String, Vec<String>)> {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/terminal_fallbacks.json")).unwrap();
    let rows = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_str().unwrap().to_owned())
            .collect()
    };
    cases
        .iter()
        .filter(|case| case["language"].as_str() == language)
        .map(|case| {
            (
                case["text"].as_str().unwrap().to_owned(),
                rows(&case["expected"]),
            )
        })
        .collect()
}

#[test]
fn terminal_without_language_uses_colored_split_lines() {
    let (_state, live) = live();
    let engine = Scripted::failing();
    let cases = fallback_cases(None);
    assert_eq!(cases.len(), 32);
    for (text, expected) in cases {
        assert_eq!(highlight_code(&live, &engine, &text, None), expected);
        assert_eq!(highlight_code(&live, &engine, &text, Some("")), expected);
    }
    assert_eq!(engine.calls(), Vec::<String>::new());
}

#[test]
fn terminal_unknown_language_is_silent_and_explicit_only() {
    let (_state, live) = live();
    let engine = Scripted::failing();
    let cases = fallback_cases(Some("unknown-syntax"));
    assert_eq!(cases.len(), 32);
    for (text, expected) in &cases {
        assert_eq!(
            highlight_code(&live, &engine, text, Some("unknown-syntax")),
            *expected
        );
    }
    let (text, expected) = &cases[0];
    for label in [
        "javascript extra",
        " javascript",
        "javascript\t",
        "java script",
    ] {
        assert_eq!(
            highlight_code(&live, native(), text, Some(label)),
            *expected,
            "{label:?}"
        );
    }
    assert!(
        engine
            .calls()
            .iter()
            .all(|call| call == "supports:unknown-syntax")
    );
}

#[test]
fn direct_highlight_failure_returns_raw_split_lines() {
    let (_state, live) = live();
    let engine = Scripted::failing();
    let rows = highlight_code(
        &live,
        &engine,
        "const x = \"<&>\";\r\n\n\u{96ea}\n",
        Some("javascript"),
    );
    assert_eq!(rows, ["const x = \"<&>\";\r", "", "\u{96ea}", ""]);
    assert_eq!(
        engine.calls(),
        ["supports:javascript", "highlight:javascript"]
    );
}

#[test]
fn markdown_highlight_failure_returns_colored_split_lines() {
    let (_state, live) = live();
    let engine = Rc::new(Scripted::failing());
    let markdown = get_markdown_theme(live, Rc::clone(&engine) as Rc<dyn SyntaxHighlighter>, true);
    let highlight = markdown.highlight_code.as_ref().unwrap();
    let rows = highlight("const x = \"<&>\";\r\n\n", Some("javascript"));
    let expected = [
        color(CODE, "const x = \"<&>\";\r"),
        color(CODE, ""),
        color(CODE, ""),
    ];
    assert_eq!(rows, expected);
    assert_eq!(
        engine.calls(),
        ["supports:javascript", "highlight:javascript"]
    );
}

#[test]
fn highlighting_reads_the_replaced_live_theme() {
    let keys = [
        ThemeColor::SyntaxComment,
        ThemeColor::SyntaxKeyword,
        ThemeColor::SyntaxFunction,
        ThemeColor::SyntaxVariable,
        ThemeColor::SyntaxString,
        ThemeColor::SyntaxNumber,
        ThemeColor::SyntaxType,
        ThemeColor::SyntaxOperator,
        ThemeColor::SyntaxPunctuation,
    ];
    let mut spans: Vec<_> = keys
        .into_iter()
        .enumerate()
        .map(|(i, key)| span(i..i + 1, Some(key)))
        .collect();
    spans.push(span(9..10, None));
    let engine = Scripted::new(&["javascript"], Ok(spans));
    let (state, live) = live();
    let render = || highlight_code(&live, &engine, "0123456789", Some("javascript"));
    let palette = [
        COMMENT,
        KEYWORD,
        FUNCTION,
        VARIABLE,
        STRING,
        NUMBER,
        TYPE,
        OPERATOR,
        PUNCTUATION,
    ];
    let expect = |shift: u8| -> String {
        "0123456789"
            .char_indices()
            .map(|(i, c)| match palette.get(i) {
                Some(key) => color(key + shift, &c.to_string()),
                None => c.to_string(),
            })
            .collect()
    };
    assert_eq!(render(), [expect(0)]);
    state.set_theme_instance(theme(100)).unwrap();
    assert_eq!(render(), [expect(100)]);
}

#[test]
fn highlighted_rows_keep_empty_and_final_lines() {
    let (_state, live) = live();
    for code in ["", "x", "\n", "a\n\nb\n", "\n\n\n"] {
        let rows = native_rows(&live, code, "javascript");
        let expected: Vec<_> = code.split('\n').collect();
        assert_eq!(
            rows.iter().map(|row| strip(row)).collect::<Vec<_>>(),
            expected,
            "{code:?}"
        );
    }
    assert_eq!(native_rows(&live, "", "javascript"), [""]);
}

#[test]
fn highlighted_text_keeps_crlf_tabs_and_unicode() {
    let (_state, live) = live();
    let samples = [
        "const n = 1;\r\n\t// end\r\n",
        "const s = '\u{96ea} e\u{301} \u{1f469}\u{200d}\u{1f4bb}';\n// \u{3bb}\r\n\r\n",
    ];
    for code in samples {
        let rows = native_rows(&live, code, "javascript");
        assert_eq!(strip(&rows.join("\n")), code);
    }
}

#[test]
fn syntax_state_continues_across_line_boundaries() {
    let (_state, live) = live();
    let rows = native_rows(
        &live,
        "/* start\nconst fake = 42;\n*/\nconst real = 1;",
        "javascript",
    );
    assert_eq!(rows[1], color(COMMENT, "const fake = 42;"));
    assert!(rows[3].starts_with(&color(TYPE, "const")), "{:?}", rows[3]);
    let rows = native_rows(
        &live,
        "value = \"\"\"start\n# not a comment\nend\"\"\"\nprint(value)",
        "python",
    );
    assert_eq!(rows[1], color(STRING, "# not a comment"));
}

#[test]
fn path_languages_use_the_complete_last_dot_table() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/path_languages.json")).unwrap();
    for case in &cases {
        let path = case["path"].as_str().unwrap();
        assert_eq!(
            get_language_from_path(path),
            case["language"].as_str(),
            "{path:?}"
        );
    }
    assert_eq!(cases.len(), 130);
    assert_eq!(
        cases
            .iter()
            .filter(|case| !case["language"].is_null())
            .count(),
        122
    );
}

/// Code and the signature token each representative grammar must classify.
const REPRESENTATIVE: [(&str, &str, &str, ThemeColor); 11] = [
    (
        "typescript",
        "interface A { x: number }",
        "interface",
        ThemeColor::SyntaxType,
    ),
    (
        "tsx",
        "const a = <div className=\"x\">hi</div>;",
        "className",
        ThemeColor::SyntaxVariable,
    ),
    (
        "toml",
        "[package]\nname = \"x\"\n",
        "x",
        ThemeColor::SyntaxString,
    ),
    (
        "dockerfile",
        "FROM rust:1\nRUN cargo build\n",
        "FROM",
        ThemeColor::SyntaxKeyword,
    ),
    (
        "kotlin",
        "fun main() { println(\"hi\") }",
        "fun",
        ThemeColor::SyntaxKeyword,
    ),
    (
        "swift",
        "func f() -> Int { return 1 }",
        "->",
        ThemeColor::SyntaxOperator,
    ),
    ("ini", "[s]\nk=v\n", "v", ThemeColor::SyntaxString),
    ("zig", "pub fn main() void {}", "fn", ThemeColor::SyntaxType),
    (
        "nginx",
        "server { listen 80; }",
        "listen",
        ThemeColor::SyntaxKeyword,
    ),
    ("fs", "let x = 1", "let", ThemeColor::SyntaxKeyword),
    (
        "html",
        "<div class=\"note\">hi</div>",
        "class",
        ThemeColor::SyntaxVariable,
    ),
];

#[test]
fn required_native_syntaxes_highlight_representative_code() {
    for (language, code, token, expected) in REPRESENTATIVE {
        assert!(native().supports_language(language), "{language}");
        let spans = native().highlight(code, language).unwrap();
        let text: String = spans.iter().map(|span| &code[span.range.clone()]).collect();
        assert_eq!(text, code, "{language}");
        let classified = spans.iter().find(|span| &code[span.range.clone()] == token);
        assert_eq!(
            classified.and_then(|span| span.color.clone()),
            Some(expected),
            "{language}"
        );
    }
}

#[test]
fn unavailable_syntaxes_never_borrow_an_unrelated_grammar() {
    let (_state, live) = live();
    let entries: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/language_grammars.json")).unwrap();
    let absent: Vec<_> = entries
        .iter()
        .filter(|entry| entry["native"].is_null())
        .map(|entry| entry["token"].as_str().unwrap())
        .collect();
    assert_eq!(absent.len(), 174);
    for token in &absent {
        assert!(!native().supports_language(token), "{token}");
    }
    for token in ["cls", "m", "re"] {
        assert!(absent.contains(&token), "{token}");
        let rows = native_rows(&live, "value", token);
        assert_eq!(rows, [color(CODE, "value")], "{token}");
    }
}

#[test]
fn production_highlighting_routes_real_tokens_to_theme_keys() {
    let (_state, live) = live();
    let javascript = native_rows(
        &live,
        "const answer = 42; // \u{3bb}\nconsole.log(answer);\n",
        "javascript",
    );
    for (key, text) in [
        (TYPE, "const"),
        (VARIABLE, "answer"),
        (OPERATOR, "="),
        (NUMBER, "42"),
        (PUNCTUATION, ";"),
        (COMMENT, " \u{3bb}"),
        (FUNCTION, "log"),
    ] {
        assert!(
            javascript.iter().any(|row| row.contains(&color(key, text))),
            "{text:?}"
        );
    }
    let python = native_rows(
        &live,
        "def greet(name):\n    return f\"Hello, {name}\"\n",
        "python",
    );
    assert!(python[0].starts_with(&color(KEYWORD, "def")));
    assert!(python[1].contains(&color(STRING, "Hello, ")));
    assert!(javascript[1].starts_with("console"), "{:?}", javascript[1]);
}

const PROSE: &str = "This is an ordinary sentence about a quiet room.";

#[test]
fn explicit_plaintext_terminal_rows_stay_unstyled() {
    let (_state, live) = live();
    assert_eq!(native_rows(&live, PROSE, "plaintext"), [PROSE]);
}

#[test]
fn explicit_plaintext_markdown_rows_stay_unstyled() {
    let (_state, live) = live();
    let markdown = get_markdown_theme(live, Rc::new(SyntectHighlighter::new().unwrap()), true);
    let highlight = markdown.highlight_code.as_ref().unwrap();
    assert_eq!(highlight(PROSE, Some("plaintext")), [PROSE]);
}

/// Undo [`escape`]d HTML text: remove tags, then decode the five entities once.
fn decode(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}

#[test]
fn explicit_html_escapes_metacharacters_and_keeps_text() {
    let code = "const x = \"<&>\" + '\\'' + \"&amp; &lt;\";\n";
    let html = highlight_html(native(), code, Some("javascript"))
        .unwrap()
        .unwrap();
    assert!(html.contains("<span class=\"hljs-string\">"), "{html}");
    assert!(html.contains("&lt;&amp;&gt;"), "{html}");
    assert!(html.contains("&amp;amp; &amp;lt;"), "{html}");
    assert!(html.contains("&#x27;") && html.contains("&quot;"), "{html}");
    assert_eq!(decode(&html), code);
}

#[test]
fn explicit_html_reports_unknown_without_guessing() {
    let engine = Scripted::new(&["javascript"], Ok(Vec::new()));
    for lang in [None, Some(""), Some("unknown-syntax")] {
        assert_eq!(highlight_html(&engine, "const x = 1;", lang).unwrap(), None);
    }
    assert_eq!(engine.calls(), ["supports:unknown-syntax"]);
}

#[test]
fn explicit_html_propagates_engine_failure() {
    let engine = Scripted::failing();
    let error = highlight_html(&engine, "const x = 1;", Some("javascript")).unwrap_err();
    assert_eq!(error.to_string(), "controlled engine failure");
    assert_eq!(
        engine.calls(),
        ["supports:javascript", "highlight:javascript"]
    );
}

#[test]
fn html_keeps_empty_text_final_lines_and_crlf() {
    for code in [
        "",
        "\n",
        "a\n\n\n",
        "x\r\ny\r\n",
        "\tx\t",
        "// \u{96ea} e\u{301} \u{1f469}\u{200d}\u{1f4bb}\n",
    ] {
        let html = highlight_html(native(), code, Some("javascript"))
            .unwrap()
            .unwrap();
        assert_eq!(decode(&html), code, "{code:?}");
    }
}

#[test]
fn native_and_controlled_highlighters_share_public_operations() {
    let (_state, live) = live();
    let controlled = Scripted::new(
        &["javascript"],
        Ok(vec![span(0..2, Some(ThemeColor::SyntaxKeyword))]),
    );
    let failing = Scripted::failing();
    let adapters: [(&dyn SyntaxHighlighter, u8, &str); 2] = [
        (native(), NUMBER, "hljs-number"),
        (&controlled, KEYWORD, "hljs-keyword"),
    ];
    for (adapter, index, class) in adapters {
        assert_eq!(
            highlight_code(&live, adapter, "42", Some("javascript")),
            [color(index, "42")]
        );
        let html = highlight_html(adapter, "42", Some("javascript"))
            .unwrap()
            .unwrap();
        assert_eq!(html, format!("<span class=\"{class}\">42</span>"));
    }
    assert_eq!(
        highlight_code(&live, &failing, "42", Some("javascript")),
        ["42"]
    );
    assert!(highlight_html(&failing, "42", Some("javascript")).is_err());
}
