//! Work and source-identity regressions at the shared inline boundary.
use crate::{InlineAtom, InlineContext, Kind, Node, autolinks, parse};
use std::cell::RefCell;

/// Per-thread counters so independent parser tests cannot affect the witness.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Work {
    /// Actual quote framing line visits.
    pub(crate) quote_lines: usize,
    /// Actual list continuation line visits.
    pub(crate) list_lines: usize,
    /// Item-local lookahead patterns prepared.
    pub(crate) list_rules: usize,
    /// Borrowed source windows created.
    pub(crate) source_views: usize,
    /// Original/local projection calls.
    pub(crate) projections: usize,
    /// Bytes appended to existing mapped buffers.
    pub(crate) joined_bytes: usize,
    /// Last reentry's mapped end before preserving already consumed chunks.
    pub(crate) reentry_end: usize,
    /// Recognized consumption before the last container reentry.
    pub(crate) reentry_previous: usize,
    /// Complete native document parses.
    pub(crate) documents: usize,
    /// Whole-document context indexes.
    pub(crate) indexes: usize,
    /// Native nodes visited during context setup.
    pub(crate) visits: usize,
    /// Bytes supplied to local grammar adaptation.
    pub(crate) local_bytes: usize,
    /// Operand comparisons and selected operand visits during range lookup.
    pub(crate) operand_examinations: usize,
    /// Authored bytes copied when selecting mapped source.
    pub(crate) source_bytes: usize,
    /// Calls to the native operand recognizers, including failed admission.
    pub(crate) operand_admissions: usize,
    /// Raw suffix bytes supplied to the deletion recognizer.
    pub(crate) deletion_bytes: usize,
    /// Authored bytes supplied to actual inline mask construction.
    pub(crate) mask_bytes: usize,
    /// Unit comparisons in actual authored-to-source range searches.
    pub(crate) source_examinations: usize,
    /// Original definition indexes constructed by the document lexer.
    pub(crate) reference_indexes: usize,
    /// Bytes supplied to the fallback text recognizer.
    pub(crate) text_bytes: usize,
    /// Bytes searched for the end of a fallback text line.
    pub(crate) line_bytes: usize,
    /// Authored bytes compared by the plain-text merge check.
    pub(crate) merge_bytes: usize,
    /// Bare-address checks run at inline token starts.
    pub(crate) address_checks: usize,
    /// Bytes up to the end of each address candidate the start patterns matched.
    pub(crate) address_bytes: usize,
}
thread_local! {
    /// Instrument all complete-parse/index and local-parse entry points.
    pub(crate) static WORK: RefCell<Work> = RefCell::default();
}

/// Exercise original scopes and labels without walking the document per fragment.
fn interpret(nodes: &[Node], context: &InlineContext<'_>) {
    for node in nodes {
        let Kind::Paragraph(children) = &node.kind else {
            continue;
        };
        let scope = context.scope(&node.range, false).unwrap();
        let formula = scope.text().find('$').unwrap();
        let end = formula + 1 + scope.text()[formula + 1..].find('$').unwrap() + 1;
        let authored_link = children
            .iter()
            .find(|node| matches!(node.kind, Kind::Link(..)));
        if let Some(link) = authored_link {
            let run = scope.interpret(0..scope.text().trim_end_matches('\n').len(), &[]);
            assert!(run.atoms.iter().any(|atom| matches!(atom, InlineAtom::Resolved(node) if matches!(node.kind, Kind::Link(..)))));
            let label = context.scope(&link.range, true).unwrap();
            let run = label.interpret(0..label.text().len(), std::slice::from_ref(&(0..3)));
            assert!(
                run.atoms
                    .iter()
                    .any(|atom| matches!(atom, InlineAtom::Opaque(0)))
            );
        } else {
            let run = scope.interpret(
                0..scope.text().trim_end_matches('\n').len(),
                std::slice::from_ref(&(formula..end)),
            );
            assert!(
                run.atoms
                    .iter()
                    .any(|atom| matches!(atom, InlineAtom::Opaque(0)))
            );
        }
    }
}

/// Measure actual caller-selected admission without a rewrite preflight.
fn measured(paragraphs: usize, paragraph: &str, escaping: bool) -> Work {
    let source = paragraph.repeat(paragraphs) + "[r]: /docs \"tip\"\n";
    WORK.with_borrow_mut(|work| *work = Work::default());
    let mut document = crate::parse_with_options(
        source,
        crate::MarkedOptions::Browser {
            escape_html: escaping,
        },
    );
    autolinks(&mut document);
    let context = InlineContext::new(&document);
    interpret(&document.nodes, &context);
    WORK.with_borrow(|work| *work)
}

#[test]
fn inline_document_work_stays_constant_as_math_paragraphs_grow() {
    for (references, paragraph) in [
        (false, "**a $x$ b** <b>tag</b>\n\n"),
        (true, "[$x$][r] <b>tag</b>\n\n"),
    ] {
        for escaping in [false, true] {
            let counts = [8, 16, 32].map(|paragraphs| measured(paragraphs, paragraph, escaping));
            for work in counts {
                assert_eq!(work.documents, 1);
                assert_eq!(work.reference_indexes, 1);
                assert_eq!(work.indexes, 1);
            }
            assert!(grows_linearly(counts.map(|work| work.visits)), "{counts:?}");
            assert_eq!(counts[2].local_bytes, 2 * counts[1].local_bytes);
            assert_eq!(counts[1].local_bytes, 2 * counts[0].local_bytes);
            eprintln!("references={references} escaping={escaping} work={counts:?}");
        }
    }
}

/// Follow display-separated ranges within one paragraph, retaining resolved anchors.
fn measured_ranges(count: usize, reference: bool) -> Work {
    let source = if reference {
        "$$x$$ [r][r] ".repeat(count) + "\n\n[r]: /docs"
    } else {
        "$$x$$ [proof](/docs)\n".repeat(count)
    };
    WORK.with_borrow_mut(|work| *work = Work::default());
    let document = parse(source);
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    assert_eq!(scope.operands.len(), count);
    assert!(
        scope
            .operands
            .windows(2)
            .all(|pair| pair[0].0.end <= pair[1].0.start)
    );
    let mut anchors = 0;
    let mut begin = 0;
    for end in scope
        .text()
        .match_indices("$$x$$")
        .map(|(start, _)| start)
        .chain(std::iter::once(scope.text().len()))
    {
        if begin < end {
            let run = scope.interpret(begin..end, &[]);
            assert!(run.atoms.iter().all(|atom| matches!(atom, InlineAtom::Resolved(node) if matches!(&node.kind, Kind::Link(_, _, href, _) if href == "/docs"))));
            anchors += run.atoms.len();
        }
        begin = end + 5;
    }
    assert_eq!(anchors, count);
    WORK.with_borrow(|work| *work)
}

#[test]
fn inline_subrange_operand_work_is_bounded_in_one_paragraph() {
    for reference in [false, true] {
        let counts = [32, 64, 128].map(|count| measured_ranges(count, reference));
        eprintln!("reference={reference} work={counts:?}");
        for (count, work) in [32_usize, 64, 128].into_iter().zip(counts) {
            assert_eq!(work.documents, 1);
            assert_eq!(work.indexes, 1);
            assert!(
                work.operand_examinations <= count * (2 * count.ilog2() as usize + 5),
                "{work:?}"
            );
        }
        assert!(counts[2].operand_examinations < 3 * counts[1].operand_examinations);
    }
}

mod grammar_rules;
mod stepwise_rules;

#[test]
fn parser_does_not_repeat_document_admission() {
    let measure = |count| {
        WORK.with_borrow_mut(|work| *work = Work::default());
        let source = "<div> *tail* [r][r]\n\n".repeat(count) + "[r]: /target\n";
        let document =
            crate::parse_with_options(source, crate::MarkedOptions::Browser { escape_html: true });
        assert!(count == 0 || !document.nodes.is_empty());
        let _context = InlineContext::new(&document);
        WORK.with_borrow(|work| *work)
    };
    let [base, small, large] = [0, 32, 64].map(measure);
    assert_eq!(small.documents, 1);
    assert_eq!(large.documents, 1);
    assert_eq!(
        large.source_bytes - base.source_bytes,
        2 * (small.source_bytes - base.source_bytes)
    );
    assert_eq!(large.mask_bytes, 2 * small.mask_bytes);
    assert_eq!(large.visits, 2 * small.visits);
}

/// Measure selected URL ranges after quote-prefix projection and entity decoding.
fn measured_link_sources(count: usize) -> Work {
    let line = "> https://frame.test/&amp;\n";
    WORK.with_borrow_mut(|work| *work = Work::default());
    let mut document = parse(line.repeat(count));
    autolinks(&mut document);
    let Kind::Quote(children, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    let Kind::Paragraph(children) = &children[0].kind else {
        panic!("paragraph")
    };
    let links: Vec<_> = children
        .iter()
        .filter(|node| matches!(node.kind, Kind::Link(..)))
        .collect();
    assert_eq!(links.len(), count);
    for (index, link) in links.into_iter().enumerate() {
        let start = index * line.len() + 2;
        assert_eq!(link.range, start..start + "https://frame.test/".len());
    }
    WORK.with_borrow(|work| *work)
}

#[test]
fn mapped_link_source_ranges_use_ordered_boundaries() {
    let counts = [32, 64, 128].map(measured_link_sources);
    eprintln!("source-range work={counts:?}");
    assert!(counts[2].source_examinations < 3 * counts[1].source_examinations);
}

/// A caller block from `open` through the next `close`.
struct Delimited(&'static str, &'static str);
impl crate::BlockTokenizer for Delimited {
    fn start(&self, source: &str) -> Option<usize> {
        source.find(self.0)
    }
    fn tokenize(&mut self, source: &str) -> Option<crate::BlockToken> {
        let rest = source.strip_prefix(self.0)?;
        let end = self.0.len() + rest.find(self.1)? + self.1.len();
        Some(crate::BlockToken {
            raw: source[..end].to_owned(),
            kind: "block".to_owned(),
            text: String::new(),
        })
    }
}

/// Paragraph text and caller blocks in document order.
fn hinted_blocks(source: &str, options: crate::MarkedOptions, mut block: Delimited) -> Vec<String> {
    let document =
        crate::parse_with_tokenizer(source.to_owned(), options, Some(&mut block)).unwrap();
    document
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            Kind::Paragraph(_) => Some(node.authored.trim_end().to_owned()),
            Kind::CustomBlock(..) => {
                Some(format!("block {}", node.authored.trim_end_matches('\n')))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn display_hint_barriers_are_scanned_once_per_block_source() {
    let counts = [32, 64].map(|count| {
        WORK.with_borrow_mut(|work| *work = Work::default());
        let source = "ordinary paragraph\n\n".repeat(count) + "$$x$$\n";
        let options = crate::MarkedOptions::Browser { escape_html: true };
        let blocks = hinted_blocks(&source, options, Delimited("$$", "$$"));
        assert_eq!(blocks.len(), count + 1);
        assert_eq!(blocks.last().map(String::as_str), Some("block $$x$$"));
        WORK.with_borrow(|work| *work)
    });
    eprintln!("display hint N/2N work={counts:?}");
    assert!(
        counts[1].mask_bytes <= 2 * counts[0].mask_bytes,
        "{counts:?}"
    );
}

#[test]
fn later_paragraphs_see_the_barriers_of_their_own_suffix() {
    let documentation = crate::MarkedOptions::Documentation;
    let escaped = crate::MarkedOptions::Browser { escape_html: true };
    for (source, options, block, expected) in [
        // A code span of the first paragraph's suffix crosses the next paragraph start.
        (
            "`a\n\nb` c `$$x$$` d\n",
            documentation,
            Delimited("$$", "$$"),
            &["`a", "b` c `", "block $$x$$", "` d"][..],
        ),
        // The backtick before a paragraph start hides a code span from the earlier suffix.
        (
            "p\n\n%%``%%` t\n",
            documentation,
            Delimited("%%", "`"),
            &["p", "block %%`", "`%%` t"][..],
        ),
        // An escaped opening tag pairs with an angle past the next paragraph start.
        (
            "p <a\n\nq <$$x$$> r\n",
            escaped,
            Delimited("$$", "$$"),
            &["p <a", "q <$$x$$> r"][..],
        ),
    ] {
        assert_eq!(
            hinted_blocks(source, options, block),
            expected,
            "{source:?}"
        );
    }
}

/// The browser's ordinary paragraphs reach the extension's absent-start branch.
struct NoBlockHint;
impl crate::BlockTokenizer for NoBlockHint {
    fn start(&self, _source: &str) -> Option<usize> {
        None
    }
    fn tokenize(&mut self, _source: &str) -> Option<crate::BlockToken> {
        None
    }
}

/// Compare actual shared-owner scan work through either public entry route.
fn suffix_work(extension: bool, paragraph: &str) {
    let counts = [32, 64].map(|count| {
        WORK.with_borrow_mut(|work| *work = Work::default());
        let source = paragraph.repeat(count);
        let options = crate::MarkedOptions::Browser { escape_html: true };
        let document = if extension {
            crate::parse_with_tokenizer(source, options, Some(&mut NoBlockHint)).unwrap()
        } else {
            crate::parse_with_options(source, options)
        };
        assert!(!document.nodes.is_empty());
        WORK.with_borrow(|work| *work)
    });
    eprintln!("extension={extension} {paragraph:?} N/2N work={counts:?}");
    assert!(counts[0].operand_admissions > 0);
    assert_eq!(
        counts[1].operand_admissions,
        2 * counts[0].operand_admissions
    );
    assert!(counts[1].deletion_bytes <= 4 * counts[0].deletion_bytes);
    assert_eq!(counts[1].mask_bytes, 2 * counts[0].mask_bytes);
}

#[test]
fn deletion_raw_suffix_work_matches_reference_growth() {
    for paragraph in [
        "~ ",
        "~~[a `b~~` c](/u)~~\n\n",
        "a [[b]](/u \"`\") ~~a~~ `\n\n",
    ] {
        suffix_work(false, paragraph);
        suffix_work(true, paragraph);
    }
}

#[test]
fn absent_extension_hints_skip_extension_scans() {
    suffix_work(true, "ordinary paragraph\n\n");
}

/// Parser work for a source that parses to one paragraph.
fn paragraph_work(source: String, options: crate::MarkedOptions) -> Work {
    WORK.with_borrow_mut(|work| *work = Work::default());
    let document = crate::parse_with_options(source, options);
    assert_eq!(document.nodes.len(), 1);
    WORK.with_borrow(|work| *work)
}

/// Whether work measured at N, 2N and 4N units grows by the same amount per unit.
fn grows_linearly([small, medium, large]: [usize; 3]) -> bool {
    large - medium == 2 * (medium - small)
}

#[test]
fn paragraph_text_work_grows_linearly_with_its_lines() {
    for options in [
        crate::MarkedOptions::Documentation,
        crate::MarkedOptions::Terminal,
        crate::MarkedOptions::Export,
        crate::MarkedOptions::Browser { escape_html: false },
        crate::MarkedOptions::Browser { escape_html: true },
    ] {
        let grows = |shape: &str| {
            let work = [64, 128, 256].map(|count| paragraph_work(shape.repeat(count), options));
            (
                grows_linearly(work.map(|work| work.text_bytes)),
                grows_linearly(work.map(|work| work.line_bytes)),
            )
        };
        assert_eq!(grows("plain line.\n"), (true, true), "{options:?} lines");
        // One line keeps the rest of the line visible to the recognizer, but finds its end once.
        assert!(grows("a_ ").1, "{options:?} one line");
    }
}

#[test]
fn plain_text_merge_work_grows_linearly_with_its_pieces() {
    for options in [
        crate::MarkedOptions::Documentation,
        crate::MarkedOptions::Terminal,
        crate::MarkedOptions::Export,
        crate::MarkedOptions::Browser { escape_html: false },
        crate::MarkedOptions::Browser { escape_html: true },
    ] {
        let merged =
            [64, 128, 256].map(|pieces| paragraph_work("a_".repeat(pieces), options).merge_bytes);
        assert!(grows_linearly(merged), "{options:?} {merged:?}");
    }
}

#[test]
fn address_start_checks_grow_linearly_with_their_lines() {
    for options in [
        crate::MarkedOptions::Terminal,
        crate::MarkedOptions::Export,
        crate::MarkedOptions::Browser { escape_html: false },
        crate::MarkedOptions::Browser { escape_html: true },
    ] {
        for line in ["see www.a.io or b@c.io now\n", "ordinary words only\n"] {
            let work = [32, 64, 128].map(|lines| paragraph_work(line.repeat(lines), options));
            let checks = work.map(|work| work.address_checks);
            let bytes = work.map(|work| work.address_bytes);
            eprintln!("{options:?} {line:?} N/2N/4N checks={checks:?} bytes={bytes:?}");
            assert!(
                checks[0] > 0 && grows_linearly(checks),
                "{options:?} {line:?}"
            );
            assert!(grows_linearly(bytes), "{options:?} {line:?}");
            // Ordinary text never reaches a candidate, so the patterns match nothing.
            assert_eq!(
                bytes[0] == 0,
                line.starts_with("ordinary"),
                "{options:?} {line:?}"
            );
        }
    }
    // Documentation text keeps addresses as plain text and never runs the check.
    let documentation = paragraph_work(
        "see www.a.io now\n".repeat(32),
        crate::MarkedOptions::Documentation,
    );
    assert_eq!(
        (documentation.address_checks, documentation.address_bytes),
        (0, 0)
    );
}

mod container_accumulation;
