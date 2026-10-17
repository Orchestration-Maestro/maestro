//! Work and source-identity regressions at the shared inline boundary.
use crate::{InlineAtom, InlineContext, Kind, Node, autolinks, parse};
use std::cell::RefCell;

/// Per-thread counters so independent parser tests cannot affect the witness.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Work {
    /// Complete native document parses.
    pub(crate) documents: usize,
    /// Whole-document context indexes.
    pub(crate) indexes: usize,
    /// Native nodes visited during context setup.
    pub(crate) visits: usize,
    /// Bytes supplied to local grammar adaptation.
    pub(crate) local_bytes: usize,
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

/// Measure the same pipeline with and without the browser's constant escape preflight.
fn measured(paragraphs: usize, paragraph: &str, escaping: bool) -> Work {
    let source = paragraph.repeat(paragraphs) + "[r]: /docs \"tip\"\n";
    WORK.with_borrow_mut(|work| *work = Work::default());
    if escaping {
        drop(parse(source.clone()));
    }
    let source = if escaping {
        source.replace('<', "&lt;").replace('>', "&gt;")
    } else {
        source
    };
    let mut document = parse(source);
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
                assert_eq!(work.documents, 1 + usize::from(escaping));
                assert_eq!(work.indexes, 1);
            }
            assert_eq!(
                counts[2].visits - counts[1].visits,
                2 * (counts[1].visits - counts[0].visits)
            );
            assert_eq!(counts[2].local_bytes, 2 * counts[1].local_bytes);
            assert_eq!(counts[1].local_bytes, 2 * counts[0].local_bytes);
            eprintln!("references={references} escaping={escaping} work={counts:?}");
        }
    }
}
