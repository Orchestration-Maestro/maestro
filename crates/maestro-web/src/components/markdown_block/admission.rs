//! Contextual math admission over the shared owned document.
use maestro_app::is_whitespace_scalar;
use maestro_app::markdown;
use std::ops::Range;

/// Delimiter family of one formula.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Delimiter {
    /// `$…$`.
    Dollar,
    /// `\(…\)`.
    Paren,
    /// `$$…$$`.
    DoubleDollar,
    /// `\[…\]`.
    Bracket,
}
impl Delimiter {
    /// Opening and closing spelling.
    pub(super) const fn pair(self) -> (&'static str, &'static str) {
        match self {
            Self::Dollar => ("$", "$"),
            Self::Paren => ("\\(", "\\)"),
            Self::DoubleDollar => ("$$", "$$"),
            Self::Bracket => ("\\[", "\\]"),
        }
    }
    /// Whether the family renders as a display block.
    pub(super) const fn display(self) -> bool {
        matches!(self, Self::DoubleDollar | Self::Bracket)
    }
}

/// An admitted formula with its trimmed body.
#[derive(Clone, Debug)]
pub(super) struct Formula<'a> {
    /// Extent in the scanned source.
    pub(super) range: Range<usize>,
    /// Trimmed body, trimmed after the nonempty check.
    pub(super) tex: &'a str,
    /// Delimiter family.
    pub(super) delimiter: Delimiter,
}

/// Match one formula starting exactly at `index`.
pub(super) fn matching(source: &str, index: usize, display: bool) -> Option<Formula<'_>> {
    let families = if display {
        [Delimiter::DoubleDollar, Delimiter::Bracket]
    } else {
        [Delimiter::Dollar, Delimiter::Paren]
    };
    families.into_iter().find_map(|delimiter| {
        let (open, close) = delimiter.pair();
        let rest = source[index..].strip_prefix(open)?;
        let end = rest.find(close)?;
        let body = &rest[..end];
        let dollar = open.starts_with('$');
        let excluded = dollar && (body.contains('$') || (!display && body.contains('\n')));
        (!body.is_empty() && !excluded).then(|| Formula {
            range: index..index + open.len() + end + close.len(),
            tex: body.trim_matches(is_whitespace_scalar),
            delimiter,
        })
    })
}

/// Formulas of one display class, using original mapped operands without reparsing.
pub(super) fn scan<'a>(
    scope: &'a markdown::InlineScope<'_>,
    range: Range<usize>,
) -> Vec<Formula<'a>> {
    let source = &scope.text()[..range.end];
    if !source[range.start..].contains(['$', '\\']) {
        return Vec::new();
    }
    let opaque = &scope.operands;
    let (mut found, mut next, mut slashes) = (
        Vec::new(),
        opaque.partition_point(|(span, _)| span.end <= range.start),
        0_usize,
    );
    let (mut index, mut open, mut enclosing) = (range.start, 0, Vec::<usize>::new());
    while index < source.len() {
        while opaque
            .get(next)
            .is_some_and(|(range, _)| range.end <= index)
        {
            next += 1;
        }
        while enclosing.last().is_some_and(|&end| end <= index) {
            enclosing.pop();
        }
        while let Some((span, content)) = scope.spans.get(open).filter(|(_, c)| c.start <= index) {
            open += 1;
            let admitted = found.partition_point(|f: &Formula<'_>| f.range.end <= span.start);
            let consumed = found
                .get(admitted)
                .is_some_and(|f| f.range.start <= span.start);
            if content.end > index && !consumed {
                enclosing.push(content.end);
            }
        }
        if let Some((range, _)) = opaque.get(next).filter(|(r, _)| r.contains(&index)) {
            index = range.end;
            slashes = 0;
            continue;
        }
        let formula = (slashes % 2 == 0)
            .then(|| matching(source, index, true))
            .flatten();
        let barrier = scope.opaque.partition_point(|range| range.end <= index);
        if let Some(formula) = formula.filter(|formula| {
            scope
                .opaque
                .get(barrier)
                .is_none_or(|range| range.start >= formula.range.end)
                && enclosing.last().is_none_or(|&end| formula.range.end <= end)
        }) {
            index = formula.range.end;
            slashes = 0;
            found.push(formula);
            continue;
        }
        let ch = source[index..].chars().next().unwrap_or_default();
        slashes = if ch == '\\' { slashes + 1 } else { 0 };
        index += ch.len_utf8().max(1);
    }
    found
}

/// Whether a tight item starts with a display formula and therefore opens block math.
pub(super) fn starts_display(source: &str) -> bool {
    matching(source, 0, true).is_some()
}

/// The browser's block-level display-formula registration.
pub(super) struct MathBlocks;
impl markdown::BlockTokenizer for MathBlocks {
    fn start(&self, source: &str) -> Option<usize> {
        [source.find("$$"), source.find("\\[")]
            .into_iter()
            .flatten()
            .min()
    }
    fn tokenize(&mut self, source: &str) -> Option<markdown::BlockToken> {
        let formula = matching(source, 0, true)?;
        Some(markdown::BlockToken {
            raw: source[..formula.range.end].to_owned(),
            kind: if formula.delimiter == Delimiter::DoubleDollar {
                "blockMathDollar"
            } else {
                "blockMathLatex"
            }
            .to_owned(),
            text: formula.tex.to_owned(),
        })
    }
}

/// Recover the already parsed custom block without matching its source again.
pub(super) fn block_formula<'a>(kind: &str, text: &'a str, length: usize) -> Option<Formula<'a>> {
    let delimiter = match kind {
        "blockMathDollar" => Delimiter::DoubleDollar,
        "blockMathLatex" => Delimiter::Bracket,
        _ => return None,
    };
    Some(Formula {
        range: 0..length,
        tex: text,
        delimiter,
    })
}

/// Browser formula grammar selected by the shared inline loop.
#[derive(Default)]
pub(super) struct MathInline;
impl markdown::InlineTokenizer for MathInline {
    fn start(&self, source: &str) -> Option<usize> {
        [source.find('$'), source.find("\\(")]
            .into_iter()
            .flatten()
            .min()
    }
    fn tokenize(&mut self, source: &str) -> Option<markdown::InlineToken> {
        let formula = matching(source, 0, false)?;
        Some(markdown::InlineToken {
            length: formula.range.end,
            index: 0,
        })
    }
}
