//! Work and source-identity regressions at the shared inline boundary.
use crate::{InlineAtom, InlineContext, Kind, MarkedOptions, Node, parse};
use std::cell::RefCell;

mod code_progress;
mod code_span_windows;
mod container_depth;
mod container_frames;
mod deletion_scanning;
mod emphasis_work;
mod fixture_corpus;
mod html_progress;
mod link_windows;
#[path = "../tests/support/nodes.rs"]
pub(crate) mod nodes;
mod opener_work;
mod paragraph_progress;
mod reference_windows;
mod reference_work;
mod segment_balance;
mod shared_text;
mod table_progress;

pub(crate) use shared_text::Retained;

/// Worker stack well below the main thread's, so depth cannot lean on it.
pub(crate) const WORKER_STACK: usize = 2 * 1024 * 1024;

/// Worker stack that makes one native frame per container observable at a depth
/// debug can afford.
pub(crate) const REDUCED_STACK: usize = 128 * 1024;

/// Run one case on a worker thread of the witness stack size.
pub(crate) fn on_small_stack<T: Send + 'static>(case: impl FnOnce() -> T + Send + 'static) -> T {
    on_stack(WORKER_STACK, case)
}

/// Run one case on a worker thread of the selected witness stack size.
pub(crate) fn on_stack<T: Send + 'static>(
    stack: usize,
    case: impl FnOnce() -> T + Send + 'static,
) -> T {
    std::thread::Builder::new()
        .stack_size(stack)
        .spawn(case)
        .expect("worker thread")
        .join()
        .expect("worker panicked")
}

/// Reply source the terminal renderer normalizes before parsing: tab expansion and
/// one replacement scalar per binary byte leave a three-byte scalar in a re-entered
/// quote body, between an inner quote marker and the rest of its line.
pub(crate) const MULTIBYTE_REENTRY_SOURCE: &str = " \n> -   \u{fffd}\u{1}\u{fffd}\u{fffd}\u{fffd},\u{fffd}\u{fffd}m\n <!-\u{1}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{8}   \u{fffd}\u{fffd}\u{fffd}\u{fffd} \u{fffd}\n>>\u{fffd}\u{fffd}\u{fffd}\u{fffd} \u{fffd}\n>>\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}temH\u{1}<\n\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{14}\u{fffd}      ";

/// That re-entered quote's semantic body, with one quoting level removed per line.
pub(crate) const MULTIBYTE_REENTRY_BODY: &str = "\u{fffd}\u{fffd}\u{fffd}\u{fffd} \u{fffd}\n\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}temH\u{1}<\n\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{14}\u{fffd}";

/// Every caller selection of the configured parser.
pub(crate) const SELECTIONS: [MarkedOptions; 5] = [
    MarkedOptions::Terminal,
    MarkedOptions::Documentation,
    MarkedOptions::Browser { escape_html: false },
    MarkedOptions::Browser { escape_html: true },
    MarkedOptions::Export,
];

/// Per-thread counters so independent parser tests cannot affect the witness.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Work {
    /// Source bytes prepared, admission queries and attempted reference rule operands.
    pub(crate) reference_bytes: usize,
    /// Bytes visited while constructing or recounting delimiter widths.
    pub(crate) emphasis_left: usize,
    /// Matched bytes visited by right-candidate iteration.
    pub(crate) emphasis_right: usize,
    /// Operand visits while constructing recursive inline masks.
    pub(crate) mask_operands: usize,
    /// Operand visits inside cursor-generated label windows.
    pub(crate) cursor_label_operands: usize,
    /// Bytes supplied to block recognizers, including failed admission.
    pub(crate) block_recognizer_bytes: usize,
    /// Bytes supplied to the definition grammar, including failed admission.
    pub(crate) definition_bytes: usize,
    /// Bytes supplied to retained paragraph predicates.
    pub(crate) paragraph_bytes: usize,
    /// Bytes searched or supplied to table grammar predicates.
    pub(crate) table_bytes: usize,
    /// Bytes supplied to fenced-code grammar.
    pub(crate) fence_bytes: usize,
    /// Bytes supplied to indented-code grammar.
    pub(crate) code_bytes: usize,
    /// Bytes supplied to HTML recognition.
    pub(crate) html_bytes: usize,
    /// Bytes visited by physical-line framing, including repeated visits.
    pub(crate) framing_bytes: usize,
    /// Actual quote framing line visits.
    pub(crate) quote_lines: usize,
    /// Actual list continuation line visits.
    pub(crate) list_lines: usize,
    /// Item stop rule sets compiled for a prefix and grammar not yet seen.
    pub(crate) list_rules: usize,
    /// Borrowed source windows created.
    pub(crate) source_views: usize,
    /// Original/local projection calls.
    pub(crate) projections: usize,
    /// Bytes appended to existing mapped buffers.
    pub(crate) joined_bytes: usize,
    /// Largest unique source payload a list state kept alive until its release.
    pub(crate) frame_bytes: usize,
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
    /// Label preparation steps, one per label query, plus each attempted rule window's byte length.
    pub(crate) link_bytes: usize,
    /// Source bytes each code-span preparation reads, plus one per reached-opener lookup.
    pub(crate) inline_code_bytes: usize,
    /// Source bytes each deletion preparation reads, plus one per reached-opener lookup.
    pub(crate) deletion_bytes: usize,
    /// Authored bytes supplied to actual inline mask construction.
    pub(crate) mask_bytes: usize,
    /// Original definition indexes constructed by the document lexer.
    pub(crate) reference_indexes: usize,
    /// Bytes supplied to the fallback text recognizer.
    pub(crate) text_bytes: usize,
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

/// Charge the exact operand supplied to one block recognizer.
pub(crate) fn block_operand(source: &str) {
    WORK.with_borrow_mut(|work| work.block_recognizer_bytes += source.len());
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
    let document = crate::parse_with_options(
        source,
        crate::MarkedOptions::Browser {
            escape_html: escaping,
        },
    );

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
    let document = parse(line.repeat(count));

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
    assert!(counts[1].projections <= 2 * counts[0].projections);
    assert!(counts[2].projections <= 2 * counts[1].projections);
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
            Kind::Paragraph(_) => Some(node.authored.to_owned_string().trim_end().to_owned()),
            Kind::CustomBlock(..) => Some(format!(
                "block {}",
                node.authored.to_owned_string().trim_end_matches('\n')
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn unmatched_escaped_block_hint_separates_link_operands() {
    let source = "See HTTPS://outer.example/\\[HTTPS://target.example/?url=https://last.example";
    for options in [
        crate::MarkedOptions::Browser { escape_html: false },
        crate::MarkedOptions::Browser { escape_html: true },
        crate::MarkedOptions::Terminal,
        crate::MarkedOptions::Export,
    ] {
        let browser = matches!(options, crate::MarkedOptions::Browser { .. });
        let mut block = Delimited("\\[", "\\]");
        let document = crate::parse_with_tokenizer(
            source.to_owned(),
            options,
            if browser { Some(&mut block) } else { None },
        )
        .unwrap();
        let Kind::Paragraph(children) = &document.nodes[0].kind else {
            panic!("paragraph")
        };
        let links: Vec<_> = children
            .iter()
            .filter_map(|node| match &node.kind {
                Kind::Link(_, label, href, _) => Some((label.as_str(), href.as_str())),
                _ => None,
            })
            .collect();
        let target = (
            "HTTPS://target.example/?url=https://last.example",
            "HTTPS://target.example/?url=https://last.example",
        );
        let expected = if browser {
            vec![("HTTPS://outer.example/", "HTTPS://outer.example/"), target]
        } else {
            vec![target]
        };
        assert_eq!(links, expected);
        assert_eq!(document.nodes[0].range, 0..source.len());
        assert_eq!(document.source, source);
    }
}

#[test]
fn escaped_link_labels_clip_at_display_block_hints() {
    for (source, blocks, escape_html) in [
        (r#"[\[ $x$ \]](/docs "tip")"#, 1),
        (r#"[\\[ $x$ ]](/docs "tip")"#, 0),
        (r#"[\\\[ $x$ \]](/docs "tip")"#, 1),
        (
            r#"[\[ $x$ \]](/docs "tip")

[$x$]: /in"#,
            1,
        ),
    ]
    .into_iter()
    .flat_map(|(source, blocks)| {
        [false, true].map(move |escape_html| (source, blocks, escape_html))
    }) {
        let document = crate::parse_with_tokenizer(
            source.to_owned(),
            crate::MarkedOptions::Browser { escape_html },
            Some(&mut Delimited("\\[", "\\]")),
        )
        .unwrap();
        assert_eq!(document.source, source);
        assert_eq!(
            document
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, Kind::CustomBlock(..)))
                .count(),
            blocks
        );
        assert!(
            document.nodes.iter().all(|node| {
                let Kind::Paragraph(children) = &node.kind else {
                    return true;
                };
                !children
                    .iter()
                    .any(|child| matches!(child.kind, Kind::Link(..)))
            }),
            "{source:?}"
        );
    }
}

#[test]
fn display_hint_mask_work_stays_bounded() {
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
fn later_paragraphs_clip_at_their_own_block_hints() {
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
            &["p", "block %%`", "`", "block %%`", " t"][..],
        ),
        // An escaped opening tag does not suppress a later block hint.
        (
            "p <a\n\nq <$$x$$> r\n",
            escaped,
            Delimited("$$", "$$"),
            &["p <a", "q <", "block $$x$$"][..],
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

/// Parse one source through the chosen public entry route.
pub(crate) fn parsed(
    source: String,
    options: crate::MarkedOptions,
    extension: bool,
) -> crate::Document {
    if extension {
        crate::parse_with_tokenizer(source, options, Some(&mut NoBlockHint)).unwrap()
    } else {
        crate::parse_with_options(source, options)
    }
}

/// Compare actual shared-owner scan work through either public entry route.
fn suffix_work(extension: bool, paragraph: &str) {
    let counts = [32, 64].map(|count| {
        WORK.with_borrow_mut(|work| *work = Work::default());
        let source = paragraph.repeat(count);
        let options = crate::MarkedOptions::Browser { escape_html: true };
        let document = parsed(source, options, extension);
        assert!(!document.nodes.is_empty());
        WORK.with_borrow(|work| *work)
    });
    eprintln!("extension={extension} {paragraph:?} N/2N work={counts:?}");
    assert!(counts[0].operand_admissions > 0);
    assert_eq!(
        counts[1].operand_admissions,
        2 * counts[0].operand_admissions
    );
    assert!(counts[1].deletion_bytes <= 2 * counts[0].deletion_bytes);
    assert_eq!(counts[1].mask_bytes, 2 * counts[0].mask_bytes);
}

#[test]
fn deletion_scans_bound_each_selected_scope() {
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

/// Whether work measured at N, 2N and 4N units grows by at most the same amount per unit.
///
/// The step from 2N to 4N may add at most twice the step from N to 2N.
fn grows_at_most_linearly([small, medium, large]: [usize; 3]) -> bool {
    large + 2 * small <= 3 * medium
}

#[test]
fn growth_bound_accepts_sublinear_work_and_rejects_quadratic_work() {
    assert!(grows_at_most_linearly([32, 48, 64]));
    assert!(!grows_at_most_linearly([26_400, 104_000, 412_800]));
}

/// Require only text and line breaks, spelled exactly as the source.
///
/// The closing line ending ends the paragraph rather than adding a node.
fn assert_literal(children: &[Node], source: &str, case: &str) {
    assert!(
        children
            .iter()
            .all(|node| matches!(node.kind, Kind::Text(_) | Kind::Break)),
        "{case}"
    );
    assert_eq!(
        children
            .iter()
            .map(|node| node.authored.to_owned_string())
            .collect::<String>(),
        source.trim_end_matches('\n'),
        "{case}"
    );
}

#[test]
fn plain_text_merge_work_grows_linearly_with_its_pieces() {
    for options in SELECTIONS {
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

/// Static label slots visit only the selected label window.
fn scoped_label_operands_use_selected_windows() {
    for n in [32, 64] {
        let document = crate::parse_with_options(
            "[a $x$](u) ".repeat(n),
            crate::MarkedOptions::Browser { escape_html: true },
        );
        let context = InlineContext::new(&document);
        let scope = context.scope(&document.nodes[0].range, false).unwrap();
        let spans: Vec<_> = scope
            .text()
            .match_indices("$x$")
            .map(|(start, _)| start..start + 3)
            .collect();
        WORK.with_borrow_mut(|work| work.cursor_label_operands = 0);
        let run = scope.interpret(scope.content_range(), &spans);
        let Kind::Paragraph(children) = &run.nodes[0].kind else {
            panic!("paragraph")
        };
        assert_eq!(
            children
                .iter()
                .filter(|node| matches!(node.kind, Kind::Link(..)))
                .count(),
            0
        );
        assert_eq!(run.atoms.len(), n);
        let visits = WORK.with_borrow(|work| work.cursor_label_operands);
        println!("SELECTED-LABEL-WORK n={n} visited_operands={visits}");
        assert_eq!(
            visits, n,
            "only the one contained atom is visited per selected label"
        );
    }
}

/// Fixed caller prefix for measuring parser work rather than formula recognition.
struct WorkToken;
impl crate::InlineTokenizer for WorkToken {
    fn start(&self, source: &str) -> Option<usize> {
        source.find("$x$")
    }
    fn tokenize(&mut self, source: &str) -> Option<crate::InlineToken> {
        source.starts_with("$x$").then_some(crate::InlineToken {
            length: 3,
            index: 0,
        })
    }
}

#[test]
fn cursor_admission_work_tracks_selected_windows() {
    scoped_label_operands_use_selected_windows();
    let work = [32, 64].map(|count| {
        check_operand_windows(count);
        check_cursor_work(count)
    });
    assert_eq!(work[1].local_bytes, 2 * work[0].local_bytes);
    assert_eq!(
        work[1].operand_examinations,
        2 * work[0].operand_examinations
    );
}

/// Count document setup and selected recursive windows.
fn check_cursor_work(count: usize) -> Work {
    WORK.with_borrow_mut(|work| *work = Work::default());
    let document = crate::parse_with_options(
        "*a $x$* `q` [r](u)\n\n".repeat(count),
        crate::MarkedOptions::Browser { escape_html: true },
    );
    let context = InlineContext::new(&document);
    let mut retained = 0;
    for node in document
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, Kind::Paragraph(_)))
    {
        let scope = context.scope(&node.range, false).unwrap();
        let run = scope.interpret_with_tokenizer(scope.content_range(), &mut WorkToken);
        retained += run
            .atoms
            .iter()
            .filter(|atom| matches!(atom, InlineAtom::Resolved(_)))
            .count();
        assert_eq!(
            run.atoms
                .iter()
                .filter(|atom| matches!(atom, InlineAtom::Opaque(0)))
                .count(),
            1
        );
    }
    assert_eq!(retained, 2 * count);
    let work = WORK.with_borrow(|work| *work);
    assert_eq!(work.documents, 1);
    assert_eq!(work.reference_indexes, 1);
    assert_eq!(work.indexes, 1);
    work
}

/// Compare repeated and ordered selection over the same actual native code operands.
fn check_operand_windows(count: usize) {
    let document = crate::parse_with_options(
        "a $x$ `q` ".repeat(count),
        crate::MarkedOptions::Browser { escape_html: true },
    );
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let mut negative = Vec::new();
    let mut control = Vec::new();
    let mut negative_work = 0;
    let mut control_work = 0;
    let mut next = 0;
    for index in 0..count {
        let window = index * 10..(index + 1) * 10;
        negative.extend(
            scope
                .opaque
                .iter()
                .filter(|span| {
                    negative_work += 1;
                    span.start < window.end && window.start < span.end
                })
                .cloned(),
        );
        while let Some(span) = scope
            .opaque
            .get(next)
            .filter(|span| span.start < window.end)
        {
            control_work += 1;
            if window.start < span.end {
                control.push(span.clone());
            }
            next += 1;
        }
    }
    assert_eq!(negative, scope.opaque);
    assert_eq!(control, negative);
    assert_eq!(negative_work, count * count);
    assert_eq!(control_work, count);
    println!("OPERAND-CONTROL n={count} rescan={negative_work} selected={control_work}");
}

/// Measure masking through the cursor entry while retaining each original code operand.
fn sibling_mask_work(span: &str, count: usize, strike: bool) -> usize {
    let document = crate::parse_with_options(
        span.repeat(count),
        crate::MarkedOptions::Browser { escape_html: true },
    );
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    WORK.with_borrow_mut(|work| work.mask_operands = 0);
    let run = scope.interpret_with_tokenizer(scope.content_range(), &mut WorkToken);
    let Kind::Paragraph(children) = &run.nodes[0].kind else {
        panic!("paragraph")
    };
    let spans: Vec<_> = children
        .iter()
        .filter_map(|node| match &node.kind {
            Kind::Emphasis(children) if !strike => Some(children),
            Kind::Strike(children) if strike => Some(children),
            _ => None,
        })
        .collect();
    assert_eq!(spans.len(), count);
    assert_eq!(run.atoms.len(), count);
    for (index, children) in spans.iter().enumerate() {
        assert!(
            children
                .iter()
                .any(|node| matches!(node.kind, Kind::Atom(slot) if slot == index))
        );
        let InlineAtom::Resolved(original) = &run.atoms[index] else {
            panic!("retained code")
        };
        assert!(matches!(&original.kind, Kind::Code(text) if text == "q"));
        assert_eq!(
            original.range,
            span.len() * index + 3..span.len() * index + 6
        );
    }
    let visits = WORK.with_borrow(|work| work.mask_operands);
    println!("SIBLING-MASK-WORK strike={strike} n={count} visited_operands={visits}");
    visits
}

#[test]
fn sibling_emphasis_masks_visit_selected_operands() {
    let [small, large] = [32, 64].map(|count| sibling_mask_work("*a `q`* ", count, false));
    assert!(large <= 2 * small + 2, "mask visits: {small} -> {large}");
}

#[test]
fn sibling_deletion_masks_visit_selected_operands() {
    let [small, large] = [32, 64].map(|count| sibling_mask_work("~a `q`~ ", count, true));
    assert!(large <= 2 * small + 2, "mask visits: {small} -> {large}");
}

mod block_hints;
