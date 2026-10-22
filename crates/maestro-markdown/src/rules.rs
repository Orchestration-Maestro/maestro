//! Immutable block and inline recognizers.
pub(crate) mod code;
pub(crate) mod definition;
pub(crate) mod fence;
pub(crate) mod html;
pub(crate) mod lines;
pub(crate) mod paragraph;
pub(crate) mod table;
use crate::MarkedOptions;
use regress::Regex;
use std::sync::{LazyLock, OnceLock};

/// Reference thematic break: the `hr` rule and the break that interrupts paragraphs, quotes and tables.
const HR: &str = r" {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)";

/// Reference names of the HTML elements that open a type 6 HTML block.
const TAG: &str = "address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul";

/// The only HTML opener that interrupts in the caller's literal-markup view.
const LITERAL_HTML_OPENERS: &str = "<!--";

/// HTML openers that interrupt a paragraph or table: a type 6 tag, a raw-text tag or a comment.
fn html_openers() -> String {
    [
        r"<\/?(?:",
        TAG,
        r")(?: +|\n|\/?>)|<(?:script|pre|style|textarea|!--)",
    ]
    .concat()
}

/// Openers that end a paragraph line or table row; indented code ends only table rows.
fn interruption(code: bool, html: &str) -> String {
    let code = if code { "|(?: {4}| {0,3}\t)[^\\n]" } else { "" };
    [
        HR,
        r"| {0,3}#{1,6}(?:\s|$)| {0,3}>",
        code,
        r"| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |",
        html,
    ]
    .concat()
}

/// Reference table header and delimiter row before body admission.
const TABLE_HEADER: &str = r" *([^\n ].*)\n {0,3}((?:\| *)?:?-+:? *(?:\| *:?-+:? *)*(?:\| *)?)";

/// Reference quoted paragraph lines up to the first interruption, unanchored.
fn quote_paragraph_source(html: &str) -> String {
    [
        r"([^\n]+(?:\n(?!",
        &interruption(false, html),
        r"| +\n)[^\n]+)*)",
    ]
    .concat()
}

/// Compile a composed source anchored at the match start.
fn anchored(source: &str) -> Option<Regex> {
    Regex::with_flags(&["^", source].concat(), "").ok()
}

/// Compile the reference block quote: quoted paragraphs or single lines.
fn blockquote(html: &str) -> Option<Regex> {
    let source = [
        r"^( {0,3}> ?(",
        &quote_paragraph_source(html),
        r"|[^\n]*)(?:\n|$))+",
    ]
    .concat();
    Regex::with_flags(&source, "").ok()
}

/// Recognize block newline syntax without a mutable match cursor.
pub(crate) static BLOCK_NEWLINE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^(?:[ \t]*(?:\n|$))+", "").ok());

/// Recognize block heading syntax without a mutable match cursor.
pub(crate) static BLOCK_HEADING: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^ {0,3}(#{1,6})(?=\s|$)(.*)(?:\n+|$)", "").ok());

/// Recognize block hr syntax without a mutable match cursor.
pub(crate) static BLOCK_HR: LazyLock<Option<Regex>> = LazyLock::new(|| anchored(HR));

/// Recognize block blockquote syntax without a mutable match cursor.
pub(crate) static BLOCK_BLOCKQUOTE: LazyLock<Option<Regex>> =
    LazyLock::new(|| blockquote(&html_openers()));

/// Recognize block list syntax without a mutable match cursor.
pub(crate) static BLOCK_LIST: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^( {0,3}(?:[*+-]|\d{1,9}[.)]))([ \t][^\n]+?)?(?:\n|$)", "").ok()
});

/// Frame an admitted list's items, including empty marker whitespace.
pub(crate) static LIST_ITEM: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"^( {0,3}(?:[*+-]|\d{1,9}[.)]))((?:[\t ][^\n]*)?(?:\n|$))",
        "",
    )
    .ok()
});

/// Detect a retained blank-line suffix between items of the same list.
pub(crate) static LIST_BLANK_END: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"\n[ \t]*\n[ \t]*$", "").ok());

/// Reference fence test that stops a list item before its line is dedented.
const FENCES_BEGIN: &str = "(?:```|~~~)";

/// Reference heading test that stops a list item before its line is dedented.
const HEADING_BEGIN: &str = "#";

/// Reference HTML test for the default caller grammar: a tag opener or a comment.
const HTML_BEGIN: &str = r"<(?:[a-z].*>|!--)";

/// Reference neighbor-marker test that ends the item at another list's marker.
const NEXT_BULLET: &str = r"(?:[*+-]|\d{1,9}[.)])((?:[ \t][^\n]*)?(?:\n|$))";

/// Reference thematic-break test that stops a list item before its line is dedented.
const HR_BEGIN: &str = r"((?:- *){3,}|(?:_ *){3,}|(?:\* *){3,})(?:\n+|$)";

/// Item lookahead rules shared by every item of one indentation and caller grammar.
pub(crate) struct ItemStops {
    /// Fences, headings, neighbor markers, rules and HTML openers.
    interrupts: Option<Regex>,
    /// The stops that also exclude another lazy continuation line.
    lazy: Option<Regex>,
}

impl ItemStops {
    /// A current line can end framing before it is dedented.
    pub(crate) fn interrupts(&self, line: &str) -> bool {
        rule_matches(self.interrupts.as_ref(), line)
    }

    /// A previous code line or block opener cannot admit another lazy line.
    pub(crate) fn ends_lazy(&self, line: &str) -> bool {
        let code = line
            .find(|scalar| !matches!(scalar, ' ' | '\t'))
            .is_some_and(|end| {
                line[..end]
                    .chars()
                    .map(|scalar| if scalar == '\t' { 4 } else { 1 })
                    .sum::<usize>()
                    >= 4
            });
        code || rule_matches(self.lazy.as_ref(), line)
    }
}

/// Test one composed item stop rule against a line.
fn rule_matches(rule: Option<&Regex>, line: &str) -> bool {
    #[cfg(test)]
    crate::tests::block_operand(line);
    rule.is_some_and(|rule| rule.find(line).is_some())
}

/// Compile the stops shared by every item at one indentation prefix.
fn compile_item_stops(prefix: usize, literal_html: bool) -> ItemStops {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.list_rules += 2);
    let html = if literal_html {
        LITERAL_HTML_OPENERS
    } else {
        HTML_BEGIN
    };
    let compile = |sources: &[&str]| {
        let alternation = sources.join("|");
        Regex::with_flags(&format!("^ {{0,{prefix}}}(?:{alternation})"), "i").ok()
    };
    ItemStops {
        interrupts: compile(&[FENCES_BEGIN, HEADING_BEGIN, html, NEXT_BULLET, HR_BEGIN]),
        lazy: compile(&[FENCES_BEGIN, HEADING_BEGIN, HR_BEGIN]),
    }
}

/// Item stops by indentation prefix and caller HTML grammar.
static ITEM_STOPS: [OnceLock<ItemStops>; 8] = [const { OnceLock::new() }; 8];

/// Select the stops shared by every item at this indentation and caller grammar.
pub(crate) fn item_stops(indent: usize, options: MarkedOptions) -> &'static ItemStops {
    let prefix = indent.saturating_sub(1).min(3);
    let literal_html = options == (MarkedOptions::Browser { escape_html: true });
    ITEM_STOPS[prefix * 2 + usize::from(literal_html)]
        .get_or_init(|| compile_item_stops(prefix, literal_html))
}

/// Recognize inline escape syntax without a mutable match cursor.
pub(crate) static INLINE_ESCAPE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r##"^\\([!"#$%&'()*+,\-./:;<=>?@\[\]\\^_`{|}~])"##, "").ok()
});

/// Reference spelling of inline tag syntax.
pub(crate) const INLINE_TAG_SOURCE: &str = r#"^<!--(?:-?>|[\s\S]*?-->)|^<\/[a-zA-Z][\w:-]*\s*>|^<[a-zA-Z][\w-]*(?:\s+[a-zA-Z:_][\w.:-]*(?:\s*=\s*"[^"]*"|\s*=\s*'[^']*'|\s*=\s*[^\s"'=<>`]+)?)*?\s*\/?>|^<\?[\s\S]*?\?>|^<![a-zA-Z]+\s[\s\S]*?>|^<!\[CDATA\[[\s\S]*?\]\]>"#;

/// Recognize inline tag syntax without a mutable match cursor.
pub(crate) static INLINE_TAG: LazyLock<Option<Regex>> =
    LazyLock::new(|| stepwise(INLINE_TAG_SOURCE, ""));

/// Recognize inline link syntax without a mutable match cursor.
pub(crate) static INLINE_LINK: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r#"^!?\[((?:\[(?:\\[\s\S]|[^\[\]\\])*\]|\\[\s\S]|`+[^`]*?`+(?!`)|[^\[\]\\`])*?)\]\(\s*(<(?:\\.|[^\n<>\\])+>|[^ \t\n\x00-\x1f]*)(?:(?:[ \t]*(?:\n[ \t]*)?)("(?:\\"?|[^"\\])*"|'(?:\\'?|[^'\\])*'|\((?:\\\)?|[^)\\])*\)))?\s*\)"#, "").ok()
});

/// Recognize inline reflink syntax without a mutable match cursor.
pub(crate) static INLINE_REFLINK: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^!?\[((?:\[(?:\\[\s\S]|[^\[\]\\])*\]|\\[\s\S]|`+[^`]*?`+(?!`)|[^\[\]\\`])*?)\]\[((?!\s*\])(?:\\[\s\S]|[^\[\]\\])+)\]", "").ok()
});

/// Recognize inline nolink syntax without a mutable match cursor.
pub(crate) static INLINE_NOLINK: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^!?\[((?!\s*\])(?:\\[\s\S]|[^\[\]\\])+)\](?:\[\])?", "").ok()
});

/// Recognize inline reflinkSearch syntax without a mutable match cursor.
pub(crate) static INLINE_REFLINK_SEARCH: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"!?\[((?:\[(?:\\[\s\S]|[^\[\]\\])*\]|\\[\s\S]|`+[^`]*?`+(?!`)|[^\[\]\\`])*?)\]\[((?!\s*\])(?:\\[\s\S]|[^\[\]\\])+)\]|!?\[((?!\s*\])(?:\\[\s\S]|[^\[\]\\])+)\](?:\[\])?(?!\()", "").ok()
});

/// Recognize inline emStrongLDelim syntax without a mutable match cursor.
pub(crate) static INLINE_EM_STRONG_L_DELIM: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"^(?:\*+(?:((?!\*)(?!~)[\p{P}\p{S}])|[^\s*]))|^_+(?:((?!_)(?!~)[\p{P}\p{S}])|([^\s_]))",
        "u",
    )
    .ok()
});

/// Recognize inline emStrongRDelimAst syntax without a mutable match cursor.
pub(crate) static INLINE_EM_STRONG_R_DELIM_AST: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^[^_*]*?__[^_*]*?\*[^_*]*?(?=__)|[^*]+(?=[^*])|(?!\*)(?!~)[\p{P}\p{S}](\*+)(?=[\s]|$)|(?:[^\s\p{P}\p{S}]|~)(\*+)(?!\*)(?=(?!~)[\s\p{P}\p{S}]|$)|(?!\*)(?!~)[\s\p{P}\p{S}](\*+)(?=(?:[^\s\p{P}\p{S}]|~))|[\s](\*+)(?!\*)(?=(?!~)[\p{P}\p{S}])|(?!\*)(?!~)[\p{P}\p{S}](\*+)(?!\*)(?=(?!~)[\p{P}\p{S}])|(?:[^\s\p{P}\p{S}]|~)(\*+)(?=(?:[^\s\p{P}\p{S}]|~))", "u").ok()
});

/// Recognize inline emStrongRDelimUnd syntax without a mutable match cursor.
pub(crate) static INLINE_EM_STRONG_R_DELIM_UND: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^[^_*]*?\*\*[^_*]*?_[^_*]*?(?=\*\*)|[^_]+(?=[^_])|(?!_)[\p{P}\p{S}](_+)(?=[\s]|$)|[^\s\p{P}\p{S}](_+)(?!_)(?=[\s\p{P}\p{S}]|$)|(?!_)[\s\p{P}\p{S}](_+)(?=[^\s\p{P}\p{S}])|[\s](_+)(?!_)(?=[\p{P}\p{S}])|(?!_)[\p{P}\p{S}](_+)(?!_)(?=[\p{P}\p{S}])", "u").ok()
});

/// Recognize a Unicode letter or number before an underscore delimiter.
pub(crate) static INLINE_UNICODE_ALPHA_NUMERIC: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"[\p{L}\p{N}]", "u").ok());

/// Recognize inline punctuation syntax: whitespace, punctuation or symbols except `*` and `_`.
pub(crate) static INLINE_PUNCTUATION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^((?![*_])[\s\p{P}\p{S}])", "u").ok());

/// Recognize inline anyPunctuation syntax without a mutable match cursor.
pub(crate) static INLINE_ANY_PUNCTUATION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"\\([\p{P}\p{S}])", "u").ok());

/// Recognize inline blockSkip syntax without a mutable match cursor.
pub(crate) static INLINE_BLOCK_SKIP: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"\[(?:[^\[\]`]|(?<a>`+)[^`]+\k<a>(?!`))*?\]\((?:\\[\s\S]|[^\\\(\)]|\((?:\\[\s\S]|[^\\\(\)])*\))*\)|(?<!`)()(?<b>`+)[^`]+\k<b>(?!`)|<(?! )[^<>]*?>", "").ok()
});

/// Recognize inline br syntax without a mutable match cursor.
pub(crate) static INLINE_BR: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^( {2,}|\\)\n(?!\s*$)", "").ok());

/// Recognize inline autolink syntax without a mutable match cursor.
pub(crate) static INLINE_AUTOLINK: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^<([a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^\s\x00-\x1f<>]*|[a-zA-Z0-9.!#$%&'*+/=?_`{|}~-]+(@)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+(?![-_]))>", "").ok()
});

/// Recognize inline text syntax without a mutable match cursor.
static INLINE_TEXT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    stepwise(
        inline_text_source(MarkedOptions::Browser { escape_html: false }),
        "",
    )
});

/// Immutable caller-specific inline link terminal recognizer.
static INLINE_LINK_TERMINAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r#"^!?\[((?:\[(?:\\.|[^\[\]\\])*\]|\\.|`[^`]*`|[^\[\]\\`])*?)\]\(\s*(<(?:\\.|[^\n<>\\])+>|[^ \t\n\x00-\x1f]*)(?:(?:[ \t]*(?:\n[ \t]*)?)("(?:\\"?|[^"\\])*"|'(?:\\'?|[^'\\])*'|\((?:\\\)?|[^)\\])*\)))?\s*\)"#, "").ok()
});

/// Immutable caller-specific inline link export recognizer.
static INLINE_LINK_EXPORT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r#"^!?\[((?:\[(?:\\.|[^\[\]\\])*\]|\\.|`[^`]*`|[^\[\]\\`])*?)\]\(\s*(<(?:\\.|[^\n<>\\])+>|[^\s\x00-\x1f]*)(?:\s+("(?:\\"?|[^"\\])*"|'(?:\\'?|[^'\\])*'|\((?:\\\)?|[^)\\])*\)))?\s*\)"#, "").ok()
});

/// Select inline link syntax from the immutable caller grammar.
pub(crate) fn inline_link(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal => INLINE_LINK_TERMINAL.as_ref(),
        MarkedOptions::Browser { .. } | MarkedOptions::Documentation => INLINE_LINK.as_ref(),
        MarkedOptions::Export => INLINE_LINK_EXPORT.as_ref(),
    }
}

/// Immutable caller-specific inline reflink terminal recognizer.
static INLINE_REFLINK_TERMINAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^!?\[((?:\[(?:\\.|[^\[\]\\])*\]|\\.|`[^`]*`|[^\[\]\\`])*?)\]\[((?!\s*\])(?:\\.|[^\[\]\\])+)\]", "").ok()
});

/// Select inline reflink syntax from the immutable caller grammar.
pub(crate) fn inline_reflink(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Export => INLINE_REFLINK_TERMINAL.as_ref(),
        MarkedOptions::Browser { .. } | MarkedOptions::Documentation => INLINE_REFLINK.as_ref(),
    }
}

/// Immutable caller-specific inline nolink terminal recognizer.
static INLINE_NOLINK_TERMINAL: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^!?\[((?!\s*\])(?:\\.|[^\[\]\\])+)\](?:\[\])?", "").ok());

/// Select inline nolink syntax from the immutable caller grammar.
pub(crate) fn inline_nolink(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Export => INLINE_NOLINK_TERMINAL.as_ref(),
        MarkedOptions::Browser { .. } | MarkedOptions::Documentation => INLINE_NOLINK.as_ref(),
    }
}

/// Immutable caller-specific inline reflink search terminal recognizer.
static INLINE_REFLINK_SEARCH_TERMINAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"!?\[((?:\[(?:\\.|[^\[\]\\])*\]|\\.|`[^`]*`|[^\[\]\\`])*?)\]\[((?!\s*\])(?:\\.|[^\[\]\\])+)\]|!?\[((?!\s*\])(?:\\.|[^\[\]\\])+)\](?:\[\])?(?!\()", "").ok()
});

/// Select inline reflinkSearch syntax from the immutable caller grammar.
pub(crate) fn inline_reflink_search(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Export => INLINE_REFLINK_SEARCH_TERMINAL.as_ref(),
        MarkedOptions::Browser { .. } | MarkedOptions::Documentation => {
            INLINE_REFLINK_SEARCH.as_ref()
        }
    }
}

/// Immutable caller-specific inline em strong l delim export recognizer.
static INLINE_EM_STRONG_L_DELIM_EXPORT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"^(?:\*+(?:((?!\*)[\p{P}\p{S}])|[^\s*]))|^_+(?:((?!_)[\p{P}\p{S}])|([^\s_]))",
        "u",
    )
    .ok()
});

/// Select inline emStrongLDelim syntax from the immutable caller grammar.
pub(crate) fn inline_em_strong_l_delim(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Browser { .. } => {
            INLINE_EM_STRONG_L_DELIM.as_ref()
        }
        MarkedOptions::Export | MarkedOptions::Documentation => {
            INLINE_EM_STRONG_L_DELIM_EXPORT.as_ref()
        }
    }
}

/// Immutable caller-specific inline em strong r delim ast export recognizer.
static INLINE_EM_STRONG_R_DELIM_AST_EXPORT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^[^_*]*?__[^_*]*?\*[^_*]*?(?=__)|[^*]+(?=[^*])|(?!\*)[\p{P}\p{S}](\*+)(?=[\s]|$)|[^\s\p{P}\p{S}](\*+)(?!\*)(?=[\s\p{P}\p{S}]|$)|(?!\*)[\s\p{P}\p{S}](\*+)(?=[^\s\p{P}\p{S}])|[\s](\*+)(?!\*)(?=[\p{P}\p{S}])|(?!\*)[\p{P}\p{S}](\*+)(?!\*)(?=[\p{P}\p{S}])|[^\s\p{P}\p{S}](\*+)(?=[^\s\p{P}\p{S}])", "u").ok()
});

/// Select inline emStrongRDelimAst syntax from the immutable caller grammar.
pub(crate) fn inline_em_strong_r_delim_ast(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Browser { .. } => {
            INLINE_EM_STRONG_R_DELIM_AST.as_ref()
        }
        MarkedOptions::Export | MarkedOptions::Documentation => {
            INLINE_EM_STRONG_R_DELIM_AST_EXPORT.as_ref()
        }
    }
}

/// Immutable caller-specific inline block skip terminal recognizer.
static INLINE_BLOCK_SKIP_TERMINAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"\[[^[\]]*?\]\((?:\\.|[^\\\(\)]|\((?:\\.|[^\\\(\)])*\))*\)|`[^`]*?`|<[^<>]*?>",
        "",
    )
    .ok()
});

/// Select inline blockSkip syntax from the immutable caller grammar.
pub(crate) fn inline_block_skip(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Export => INLINE_BLOCK_SKIP_TERMINAL.as_ref(),
        MarkedOptions::Browser { .. } | MarkedOptions::Documentation => INLINE_BLOCK_SKIP.as_ref(),
    }
}

/// Reference spelling of the inline text recognizer for one caller grammar.
pub(crate) fn inline_text_source(options: MarkedOptions) -> &'static str {
    match options {
        MarkedOptions::Terminal => {
            r"^([`~]+|[^`~])(?:(?= {2,}\n)|(?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)|[\s\S]*?(?:(?=[\\<!\[`*~_]|\b_|https?:\/\/|ftp:\/\/|www\.|$)|[^ ](?= {2,}\n)|[^a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-](?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)))"
        }
        MarkedOptions::Browser { .. } => {
            r"^([`~]+|[^`~])(?:(?= {2,}\n)|(?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)|[\s\S]*?(?:(?=[\\<!\[`*~_]|\b_|[hH][tT][tT][pP][sS]?|[fF][tT][pP]:\/\/|www\.|$)|[^ ](?= {2,}\n)|[^a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-](?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)))"
        }
        MarkedOptions::Export => {
            r"^([`~]+|[^`~])(?:(?= *\n)|(?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)|[\s\S]*?(?:(?=[\\<!\[`*~_]|\b_| *\n|https?:\/\/|ftp:\/\/|www\.|$)|[^ ](?= *\n)|[^a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-](?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)))"
        }
        MarkedOptions::Documentation => {
            r"^(`+|[^`])(?:(?= {2,}\n)|[\s\S]*?(?:(?=[\\<!\[`*_]|\b_|$)|[^ ](?= {2,}\n)))"
        }
    }
}

/// Compile a reference spelling whose lazy any-character loops advance one step at a time.
fn stepwise(pattern: &str, flags: &str) -> Option<Regex> {
    // regress =0.12.0 scans a lazy one-char loop's reach to the input end; `(?!)` avoids that.
    Regex::with_flags(&pattern.replace(r"[\s\S]*?", r"(?:[\s\S]|(?!))*?"), flags).ok()
}

/// Immutable caller-specific inline text terminal recognizer.
static INLINE_TEXT_TERMINAL: LazyLock<Option<Regex>> =
    LazyLock::new(|| stepwise(inline_text_source(MarkedOptions::Terminal), ""));

/// Immutable caller-specific inline text export recognizer.
static INLINE_TEXT_EXPORT: LazyLock<Option<Regex>> =
    LazyLock::new(|| stepwise(inline_text_source(MarkedOptions::Export), ""));

/// Immutable caller-specific inline text documentation recognizer.
static INLINE_TEXT_DOCUMENTATION: LazyLock<Option<Regex>> =
    LazyLock::new(|| stepwise(inline_text_source(MarkedOptions::Documentation), ""));

/// Select inline text syntax from the immutable caller grammar.
pub(crate) fn inline_text(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal => INLINE_TEXT_TERMINAL.as_ref(),
        MarkedOptions::Browser { .. } => INLINE_TEXT.as_ref(),
        MarkedOptions::Export => INLINE_TEXT_EXPORT.as_ref(),
        MarkedOptions::Documentation => INLINE_TEXT_DOCUMENTATION.as_ref(),
    }
}

/// Immutable caller-specific inline br export recognizer.
static INLINE_BR_EXPORT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^( *|\\)\n(?!\s*$)", "").ok());

/// Select inline br syntax from the immutable caller grammar.
pub(crate) fn inline_br(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Browser { .. } | MarkedOptions::Documentation => {
            INLINE_BR.as_ref()
        }
        MarkedOptions::Export => INLINE_BR_EXPORT.as_ref(),
    }
}

/// Block syntax over caller-displayed tag spans, with directive boundaries retained.
static BLOCK_BLOCKQUOTE_LITERAL: LazyLock<Option<Regex>> =
    LazyLock::new(|| blockquote(LITERAL_HTML_OPENERS));

/// Select block blockquote syntax in the caller's literal-markup view.
pub(crate) fn block_blockquote(options: MarkedOptions) -> Option<&'static Regex> {
    if options == (MarkedOptions::Browser { escape_html: true }) {
        BLOCK_BLOCKQUOTE_LITERAL.as_ref()
    } else {
        BLOCK_BLOCKQUOTE.as_ref()
    }
}
