//! Immutable block and inline recognizers.
use crate::MarkedOptions;
use regress::Regex;
use std::sync::LazyLock;

/// Recognize block newline syntax without a mutable match cursor.
pub(crate) static BLOCK_NEWLINE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^(?:[ \t]*(?:\n|$))+", "").ok());

/// Recognize block code syntax without a mutable match cursor.
pub(crate) static BLOCK_CODE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^((?: {4}| {0,3}\t)[^\n]+(?:\n(?:[ \t]*(?:\n|$))*)?)+", "").ok()
});

/// Recognize block fences syntax without a mutable match cursor.
pub(crate) static BLOCK_FENCES: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^ {0,3}(`{3,}(?=[^`\n]*(?:\n|$))|~{3,})([^\n]*)(?:\n|$)(?:|([\s\S]*?)(?:\n|$))(?: {0,3}\1[~`]* *(?=\n|$)|$)", "").ok()
});

/// Recognize block heading syntax without a mutable match cursor.
pub(crate) static BLOCK_HEADING: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^ {0,3}(#{1,6})(?=\s|$)(.*)(?:\n+|$)", "").ok());

/// Recognize block hr syntax without a mutable match cursor.
pub(crate) static BLOCK_HR: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"^ {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)",
        "",
    )
    .ok()
});

/// Recognize block blockquote syntax without a mutable match cursor.
pub(crate) static BLOCK_BLOCKQUOTE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^( {0,3}> ?(([^\n]+(?:\n(?! {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<\/?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?: +|\n|\/?>)|<(?:script|pre|style|textarea|!--)| +\n)[^\n]+)*)|[^\n]*)(?:\n|$))+", "").ok()
});

/// Recognize block list syntax without a mutable match cursor.
pub(crate) static BLOCK_LIST: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^( {0,3}(?:[*+-]|\d{1,9}[.)]))([ \t][^\n]+?)?(?:\n|$)", "").ok()
});

/// Recognize block html syntax without a mutable match cursor.
pub(crate) static BLOCK_HTML: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r#"^ {0,3}(?:<(script|pre|style|textarea)[\s>][\s\S]*?(?:<\/\1>[^\n]*\n+|$)|<!--(?:-?>|[\s\S]*?(?:-->|$))[^\n]*(\n+|$)|<\?[\s\S]*?(?:\?>\n*|$)|<![A-Z][\s\S]*?(?:>\n*|$)|<!\[CDATA\[[\s\S]*?(?:\]\]>\n*|$)|<\/?(address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?: +|\n|\/?>)[\s\S]*?(?:(?:\n[ 	]*)+\n|$)|<(?!script|pre|style|textarea)([a-z][\w-]*)(?: +[a-zA-Z:_][\w.:-]*(?: *= *"[^"\n]*"| *= *'[^'\n]*'| *= *[^\s"'=<>`]+)?)*? *\/?>(?=[ \t]*(?:\n|$))[\s\S]*?(?:(?:\n[ 	]*)+\n|$)|<\/(?!script|pre|style|textarea)[a-z][\w-]*\s*>(?=[ \t]*(?:\n|$))[\s\S]*?(?:(?:\n[ 	]*)+\n|$))"#, "i").ok()
});

/// Recognize block def syntax without a mutable match cursor.
pub(crate) static BLOCK_DEF: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r#"^ {0,3}\[((?!\s*\])(?:\\[\s\S]|[^\[\]\\])+)\]: *(?:\n[ \t]*)?([^<\s][^\s]*|<.*?>)(?:(?: +(?:\n[ \t]*)?| *\n[ \t]*)((?:"(?:\\"?|[^"\\])*"|'[^'\n]*(?:\n[^'\n]+)*\n?'|\([^()]*\))))? *(?:\n+|$)"#, "").ok()
});

/// Recognize block table syntax without a mutable match cursor.
pub(crate) static BLOCK_TABLE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^ *([^\n ].*)\n {0,3}((?:\| *)?:?-+:? *(?:\| *:?-+:? *)*(?:\| *)?)(?:\n((?:(?! *\n| {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>|(?: {4}| {0,3}	)[^\n]| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<\/?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?: +|\n|\/?>)|<(?:script|pre|style|textarea|!--)).*(?:\n|$))*)\n*|$)", "").ok()
});

/// Recognize block lheading syntax without a mutable match cursor.
pub(crate) static BLOCK_LHEADING: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^(?!(?:[*+-]|\d{1,9}[.)]) |(?: {4}| {0,3}\t)| {0,3}(?:`{3,}|~{3,})| {0,3}>| {0,3}#{1,6}| {0,3}<[^\n>]+>\n| {0,3}\|?(?:[:\- ]*\|)+[\:\- ]*\n)((?:.|\n(?!\s*?\n|(?:[*+-]|\d{1,9}[.)]) |(?: {4}| {0,3}\t)| {0,3}(?:`{3,}|~{3,})| {0,3}>| {0,3}#{1,6}| {0,3}<[^\n>]+>\n| {0,3}\|?(?:[:\- ]*\|)+[\:\- ]*\n))+?)\n {0,3}(=+|-+) *(?:\n+|$)", "").ok()
});

/// Recognize block paragraph syntax without a mutable match cursor.
pub(crate) static BLOCK_PARAGRAPH: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^([^\n]+(?:\n(?! {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<\/?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?: +|\n|\/?>)|<(?:script|pre|style|textarea|!--)| *([^\n ].*)\n {0,3}((?:\| *)?:?-+:? *(?:\| *:?-+:? *)*(?:\| *)?)(?:\n((?:(?! *\n| {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>|(?: {4}| {0,3}	)[^\n]| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<\/?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?: +|\n|\/?>)|<(?:script|pre|style|textarea|!--)).*(?:\n|$))*)\n*|$)| +\n)[^\n]+)*)", "").ok()
});

/// Recognize inline escape syntax without a mutable match cursor.
pub(crate) static INLINE_ESCAPE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r##"^\\([!"#$%&'()*+,\-./:;<=>?@\[\]\\^_`{|}~])"##, "").ok()
});

/// Recognize inline tag syntax without a mutable match cursor.
pub(crate) static INLINE_TAG: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r#"^<!--(?:-?>|[\s\S]*?-->)|^<\/[a-zA-Z][\w:-]*\s*>|^<[a-zA-Z][\w-]*(?:\s+[a-zA-Z:_][\w.:-]*(?:\s*=\s*"[^"]*"|\s*=\s*'[^']*'|\s*=\s*[^\s"'=<>`]+)?)*?\s*\/?>|^<\?[\s\S]*?\?>|^<![a-zA-Z]+\s[\s\S]*?>|^<!\[CDATA\[[\s\S]*?\]\]>"#, "").ok()
});

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

/// Recognize inline anyPunctuation syntax without a mutable match cursor.
pub(crate) static INLINE_ANY_PUNCTUATION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"\\([\p{P}\p{S}])", "u").ok());

/// Direct links skipped as one operand by delimiter scans.
const LINK_SKIP_PATTERN: &str = r"\[(?:[^\[\]`]|(?<a>`+)[^`]+\k<a>(?!`))*?\]\((?:\\[\s\S]|[^\\\(\)]|\((?:\\[\s\S]|[^\\\(\)])*\))*\)";

/// Angle operands skipped before considering punctuation inside them.
const TAG_SKIP_PATTERN: &str = r"<(?! )[^<>]*?>";

/// Recognize inline blockSkip syntax without a mutable match cursor.
pub(crate) static INLINE_BLOCK_SKIP: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        &format!(r"{LINK_SKIP_PATTERN}|(?<!`)()(?<b>`+)[^`]+\k<b>(?!`)|{TAG_SKIP_PATTERN}"),
        "",
    )
    .ok()
});

/// Code-span syntax shared by prefix admission and delimiter masking.
const CODE_PATTERN: &str = r"(`+)([^`]|[^`][\s\S]*?[^`])\1(?!`)";

/// Recognize inline code syntax without a mutable match cursor.
pub(crate) static INLINE_CODE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(&format!("^{CODE_PATTERN}"), "").ok());

/// Locate code spans while consuming escapes and opaque operands first.
pub(crate) static INLINE_CODE_SEARCH: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        &format!(r"\\[\s\S]|{CODE_PATTERN}|{LINK_SKIP_PATTERN}|{TAG_SKIP_PATTERN}"),
        "",
    )
    .ok()
});

/// Recognize inline br syntax without a mutable match cursor.
pub(crate) static INLINE_BR: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^( {2,}|\\)\n(?!\s*$)", "").ok());

/// Recognize inline autolink syntax without a mutable match cursor.
pub(crate) static INLINE_AUTOLINK: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^<([a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^\s\x00-\x1f<>]*|[a-zA-Z0-9.!#$%&'*+/=?_`{|}~-]+(@)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+(?![-_]))>", "").ok()
});

/// Recognize inline text syntax without a mutable match cursor.
pub(crate) static INLINE_TEXT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^([`~]+|[^`~])(?:(?= {2,}\n)|(?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)|[\s\S]*?(?:(?=[\\<!\[`*~_]|\b_|[hH][tT][tT][pP][sS]?|[fF][tT][pP]:\/\/|www\.|$)|[^ ](?= {2,}\n)|[^a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-](?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)))", "").ok()
});

/// Immutable caller-specific block def terminal recognizer.
static BLOCK_DEF_TERMINAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r#"^ {0,3}\[((?!\s*\])(?:\\.|[^\[\]\\])+)\]: *(?:\n[ \t]*)?([^<\s][^\s]*|<.*?>)(?:(?: +(?:\n[ \t]*)?| *\n[ \t]*)((?:"(?:\\"?|[^"\\])*"|'[^'\n]*(?:\n[^'\n]+)*\n?'|\([^()]*\))))? *(?:\n+|$)"#, "").ok()
});

/// Select block def syntax from the immutable caller grammar.
pub(crate) fn block_def(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Export => BLOCK_DEF_TERMINAL.as_ref(),
        MarkedOptions::Browser { .. } | MarkedOptions::Documentation => BLOCK_DEF.as_ref(),
    }
}

/// Immutable caller-specific block lheading export recognizer.
static BLOCK_LHEADING_EXPORT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^(?!(?:[*+-]|\d{1,9}[.)]) |(?: {4}| {0,3}\t)| {0,3}(?:`{3,}|~{3,})| {0,3}>| {0,3}#{1,6}| {0,3}<[^\n>]+>\n)((?:.|\n(?!\s*?\n|(?:[*+-]|\d{1,9}[.)]) |(?: {4}| {0,3}\t)| {0,3}(?:`{3,}|~{3,})| {0,3}>| {0,3}#{1,6}| {0,3}<[^\n>]+>\n))+?)\n {0,3}(=+|-+) *(?:\n+|$)", "").ok()
});

/// Select block lheading syntax from the immutable caller grammar.
pub(crate) fn block_lheading(options: MarkedOptions) -> Option<&'static Regex> {
    if options == (MarkedOptions::Browser { escape_html: true }) {
        return BLOCK_LHEADING_LITERAL.as_ref();
    }
    match options {
        MarkedOptions::Terminal | MarkedOptions::Browser { .. } => BLOCK_LHEADING.as_ref(),
        MarkedOptions::Export | MarkedOptions::Documentation => BLOCK_LHEADING_EXPORT.as_ref(),
    }
}

/// Immutable caller-specific block paragraph documentation recognizer.
static BLOCK_PARAGRAPH_DOCUMENTATION: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^([^\n]+(?:\n(?! {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<\/?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?: +|\n|\/?>)|<(?:script|pre|style|textarea|!--)| +\n)[^\n]+)*)", "").ok()
});

/// Select block paragraph syntax from the immutable caller grammar.
pub(crate) fn block_paragraph(options: MarkedOptions) -> Option<&'static Regex> {
    if options == (MarkedOptions::Browser { escape_html: true }) {
        return BLOCK_PARAGRAPH_LITERAL.as_ref();
    }
    match options {
        MarkedOptions::Terminal | MarkedOptions::Browser { .. } | MarkedOptions::Export => {
            BLOCK_PARAGRAPH.as_ref()
        }
        MarkedOptions::Documentation => BLOCK_PARAGRAPH_DOCUMENTATION.as_ref(),
    }
}

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

/// Immutable caller-specific inline text terminal recognizer.
static INLINE_TEXT_TERMINAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^([`~]+|[^`~])(?:(?= {2,}\n)|(?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)|[\s\S]*?(?:(?=[\\<!\[`*~_]|\b_|https?:\/\/|ftp:\/\/|www\.|$)|[^ ](?= {2,}\n)|[^a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-](?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)))", "").ok()
});

/// Immutable caller-specific inline text export recognizer.
static INLINE_TEXT_EXPORT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^([`~]+|[^`~])(?:(?= *\n)|(?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)|[\s\S]*?(?:(?=[\\<!\[`*~_]|\b_| *\n|https?:\/\/|ftp:\/\/|www\.|$)|[^ ](?= *\n)|[^a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-](?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)))", "").ok()
});

/// Immutable caller-specific inline text documentation recognizer.
static INLINE_TEXT_DOCUMENTATION: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"^(`+|[^`])(?:(?= {2,}\n)|[\s\S]*?(?:(?=[\\<!\[`*_]|\b_|$)|[^ ](?= {2,}\n)))",
        "",
    )
    .ok()
});

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

/// Immutable caller-specific inline strict del recognizer.
static INLINE_STRICT_DEL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"^(~~)(?=[^\s~])((?:\\.|[^\\])*?(?:\\.|[^\s~\\]))\1(?=[^~]|$)",
        "",
    )
    .ok()
});

/// Immutable caller-specific inline browser del recognizer.
static INLINE_BROWSER_DEL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"^(~~?)(?=[^\s~])((?:\\[\s\S]|[^\\])*?(?:\\[\s\S]|[^\s~\\]))\1(?=[^~]|$)",
        "",
    )
    .ok()
});

/// Match only deletion admitted by the selected caller.
pub(crate) fn inline_del(options: MarkedOptions) -> Option<&'static Regex> {
    match options {
        MarkedOptions::Terminal | MarkedOptions::Export => INLINE_STRICT_DEL.as_ref(),
        MarkedOptions::Browser { .. } => INLINE_BROWSER_DEL.as_ref(),
        MarkedOptions::Documentation => None,
    }
}

/// Block syntax over caller-displayed tag spans, with directive boundaries retained.
static BLOCK_PARAGRAPH_LITERAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^([^\n]+(?:\n(?! {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<!--| *([^\n ].*)\n {0,3}((?:\| *)?:?-+:? *(?:\| *:?-+:? *)*(?:\| *)?)(?:\n((?:(?! *\n| {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>|(?: {4}| {0,3}	)[^\n]| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<\/?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?: +|\n|\/?>)|<(?:script|pre|style|textarea|!--)).*(?:\n|$))*)\n*|$)| +\n)[^\n]+)*)", "").ok()
});

/// Block syntax over caller-displayed tag spans, with directive boundaries retained.
static BLOCK_LHEADING_LITERAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^(?!(?:[*+-]|\d{1,9}[.)]) |(?: {4}| {0,3}\t)| {0,3}(?:`{3,}|~{3,})| {0,3}>| {0,3}#{1,6}| {0,3}<(?![A-Za-z0-9_]|\/[A-Za-z0-9_]+>|\s)[^\n>]+>\n| {0,3}\|?(?:[:\- ]*\|)+[\:\- ]*\n)((?:.|\n(?!\s*?\n|(?:[*+-]|\d{1,9}[.)]) |(?: {4}| {0,3}\t)| {0,3}(?:`{3,}|~{3,})| {0,3}>| {0,3}#{1,6}| {0,3}<(?![A-Za-z0-9_]|\/[A-Za-z0-9_]+>|\s)[^\n>]+>\n| {0,3}\|?(?:[:\- ]*\|)+[\:\- ]*\n))+?)\n {0,3}(=+|-+) *(?:\n+|$)", "").ok()
});

/// Block syntax over caller-displayed tag spans, with directive boundaries retained.
static BLOCK_BLOCKQUOTE_LITERAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^( {0,3}> ?(([^\n]+(?:\n(?! {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<!--| +\n)[^\n]+)*)|[^\n]*)(?:\n|$))+", "").ok()
});

/// Select block blockquote syntax in the caller's literal-markup view.
pub(crate) fn block_blockquote(options: MarkedOptions) -> Option<&'static Regex> {
    if options == (MarkedOptions::Browser { escape_html: true }) {
        BLOCK_BLOCKQUOTE_LITERAL.as_ref()
    } else {
        BLOCK_BLOCKQUOTE.as_ref()
    }
}

/// Block syntax over caller-displayed tag spans, with directive boundaries retained.
static BLOCK_TABLE_LITERAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(r"^ *([^\n ].*)\n {0,3}((?:\| *)?:?-+:? *(?:\| *:?-+:? *)*(?:\| *)?)(?:\n((?:(?! *\n| {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)| {0,3}#{1,6}(?:\s|$)| {0,3}>|(?: {4}| {0,3}	)[^\n]| {0,3}(?:`{3,}(?=[^`\n]*\n)|~{3,})[^\n]*\n| {0,3}(?:[*+-]|1[.)]) |<!--).*(?:\n|$))*)\n*|$)", "").ok()
});

/// Select block table syntax in the caller's literal-markup view.
pub(crate) fn block_table(options: MarkedOptions) -> Option<&'static Regex> {
    if options == (MarkedOptions::Browser { escape_html: true }) {
        BLOCK_TABLE_LITERAL.as_ref()
    } else {
        BLOCK_TABLE.as_ref()
    }
}
