//! Shared authored values at the parser boundary.
use crate::{Kind, MappedText, Node, parse, tokenizer::block::ListState};
use std::{collections::HashSet, mem::size_of, mem::size_of_val, sync::Arc};

/// Unique allocation payloads one output charged, excluding allocator headers and hash control bytes.
#[derive(Default)]
pub(crate) struct Retained {
    /// Allocation addresses this output already charged.
    seen: HashSet<usize>,
    /// The addresses in `seen`, in the order they were charged, kept only by an accumulator
    /// that releases charges.
    order: Option<Vec<usize>>,
    /// Shared allocation payloads plus independently owned native capacities.
    pub(crate) bytes: usize,
    /// Unique backing string capacity, separate from view and native metadata.
    pub(crate) source_bytes: usize,
    /// Unique retained provenance records.
    pub(crate) mapping_records: usize,
    /// Published native nodes visited by the reply witness.
    pub(crate) nodes: usize,
}

/// The payload and the addresses a [`Retained`] had charged, so a later charge can be released.
#[derive(Clone, Copy)]
struct Taken {
    /// Payload charged before.
    bytes: usize,
    /// Addresses charged before.
    addresses: usize,
}

/// Charges one output holds, each owned by the node that trailed the output when it was taken.
///
/// A node's own allocations, the payload its closed level handed up with it, its adopted list
/// state and the item outputs its list publishes are all taken while it trails, so they sit
/// after its start. Nodes leave an output from its end, so releasing from a node's start
/// releases that node, the nodes after it and the addresses only they charged. The adopted
/// state is the one charge that leaves while its node stays, when its list publishes; it
/// charges into its own accumulator, so it holds no address here.
pub(crate) struct Charges {
    /// Payload and addresses the held nodes charged.
    retained: Retained,
    /// Where each held node's charges start, in output order.
    starts: Vec<Taken>,
    /// Payload the adopted list state charged, released when its list publishes.
    adopted: usize,
}

impl Default for Charges {
    fn default() -> Self {
        Self {
            retained: Retained {
                order: Some(Vec::new()),
                ..Retained::default()
            },
            starts: Vec::new(),
            adopted: 0,
        }
    }
}

impl Charges {
    /// Payload the output's contributions charged.
    pub(crate) fn bytes(&self) -> usize {
        self.retained.bytes
    }

    /// Charge one node the output now holds.
    pub(crate) fn push(&mut self, node: &Node) {
        self.starts.push(self.retained.taken());
        settled_node(node, &mut self.retained);
    }

    /// Charge the trailing node a payload another output already took for it.
    pub(crate) fn add(&mut self, bytes: usize) {
        self.retained.bytes += bytes;
    }

    /// Release every charge the `removed` nodes from `index` on took, as they leave the output.
    ///
    /// The adopted state belongs to the trailing node, so it always leaves with them.
    pub(crate) fn release(&mut self, index: usize, removed: usize) {
        debug_assert_eq!(
            self.starts.len(),
            index + removed,
            "every node an output holds took its charges"
        );
        let Some(start) = self.starts.get(index).copied() else {
            return;
        };
        self.starts.truncate(index);
        self.retained.release(start);
        self.adopted = 0;
    }

    /// Charge the list state the output adopts until its list publishes.
    ///
    /// The state charges into its own accumulator, so releasing it leaves no address behind
    /// and a later adoption of the same allocations charges them again.
    pub(crate) fn adopt(&mut self, list: Option<&mut ListState>) {
        let Some(state) = list else {
            return;
        };
        let mut adopted = Retained::default();
        state.retained(&mut adopted);
        self.retained.bytes += adopted.bytes;
        self.adopted = adopted.bytes;
    }

    /// Release the charge the published list state held.
    pub(crate) fn release_adopted(&mut self) {
        self.retained.bytes -= std::mem::take(&mut self.adopted);
    }
}

impl Retained {
    /// How much this accumulator has charged so far.
    fn taken(&self) -> Taken {
        Taken {
            bytes: self.bytes,
            addresses: self.order.as_ref().map_or(0, Vec::len),
        }
    }

    /// Release the payload and the addresses charged after `taken`.
    fn release(&mut self, taken: Taken) {
        self.bytes = taken.bytes;
        if let Some(order) = &mut self.order {
            for address in order.drain(taken.addresses..) {
                self.seen.remove(&address);
            }
        }
    }

    /// Charge shared string capacity without counting overlapping selected windows twice.
    pub(crate) fn text(&mut self, value: &Arc<String>) {
        if self.arc(value, value.capacity()) {
            self.source_bytes += value.capacity();
        }
    }

    /// Charge shared metadata and its backing capacity once, returning whether it is new.
    pub(crate) fn arc<T>(&mut self, value: &Arc<T>, capacity: usize) -> bool {
        let address = Arc::as_ptr(value) as usize;
        if !self.seen.insert(address) {
            return false;
        }
        if let Some(order) = &mut self.order {
            order.push(address);
        }
        self.bytes += size_of_val(value.as_ref()) + capacity;
        true
    }
}

/// Charge the allocations one settled node owns, returning the children it holds.
pub(crate) fn settled_node<'a>(node: &'a Node, retained: &mut Retained) -> Option<&'a [Node]> {
    retained.nodes += 1;
    node.authored.retained(retained);
    let children = match &node.kind {
        Kind::Quote(children, body) => {
            body.retained(retained);
            Some(children)
        }
        Kind::Paragraph(children)
        | Kind::Inline(children)
        | Kind::List(_, children)
        | Kind::Item(children) => Some(children),
        Kind::Text(text) => {
            retained.bytes += text.authored.capacity() + text.decoded.capacity();
            None
        }
        Kind::CodeBlock(body, language) => {
            retained.bytes += body.capacity() + language.as_ref().map_or(0, String::capacity);
            None
        }
        _ => None,
    };
    let children = children?;
    retained.bytes += children.capacity() * size_of::<Node>();
    Some(children)
}

/// Charge every allocation a settled block tree keeps alive, once per allocation.
pub(crate) fn settled(nodes: &Vec<Node>, retained: &mut Retained) {
    retained.bytes += nodes.capacity() * size_of::<Node>();
    let mut pending = vec![nodes.as_slice()];
    while let Some(nodes) = pending.pop() {
        for node in nodes {
            pending.extend(settled_node(node, retained));
        }
    }
}
#[test]
fn settled_charges_each_child_vector_capacity() {
    let gap = || Node {
        kind: Kind::Gap,
        authored: MappedText::default(),
        range: 0..0,
    };
    let charge = |children: Vec<Node>| {
        let nodes = vec![Node {
            kind: Kind::Paragraph(children),
            authored: MappedText::default(),
            range: 0..0,
        }];
        let mut retained = Retained::default();
        settled(&nodes, &mut retained);
        assert_eq!(retained.nodes, 2);
        retained.bytes
    };
    let mut loose = vec![gap(), gap()];
    loose.pop();
    assert_eq!(loose.capacity(), 2);
    assert_eq!(charge(loose) - charge(vec![gap()]), size_of::<Node>());
}

#[test]
fn mapped_text_preserves_owned_empty_and_unicode_values() {
    for value in [String::new(), "é\n🦀".to_owned()] {
        let text = MappedText::from(value.clone());
        assert_eq!(text.len(), value.len());
        assert_eq!(text.is_empty(), value.is_empty());
        assert_eq!(text.chunks().collect::<String>(), value);
        assert_eq!(text.to_owned_string(), value);
    }
    assert!(MappedText::default().is_empty());
    let source = "> é\n> > 🦀";
    let document = parse(source.to_owned());
    let authored = document.nodes[0].authored.clone();
    drop(document);
    assert_eq!(authored.len(), source.len());
    assert_eq!(authored.chunks().collect::<String>(), source);
    assert_eq!(authored.to_owned_string(), source);
}

#[test]
fn source_node_views_keep_shared_storage() {
    let source = "> é\n> 🦀";
    let owner = crate::source::ScopeSource::identity(source);
    let view = owner.view(0..source.len());
    let first = view.node(0..4, crate::Kind::Gap);
    let second = view.node(5..source.len(), crate::Kind::Gap);
    assert_eq!(
        first.authored.chunks().next().unwrap().as_ptr(),
        owner.text.as_ptr()
    );
    assert_eq!(
        second.authored.chunks().next().unwrap().as_ptr() as usize,
        owner.text.as_ptr() as usize + 5
    );
    let selected = view.select(std::iter::once(5..source.len()));
    assert_eq!(
        selected.text.as_ptr() as usize,
        owner.text.as_ptr() as usize + 5
    );
    drop(owner);
    assert_eq!(first.authored.to_owned_string(), "> é");
    assert_eq!(second.authored.to_owned_string(), "> 🦀");
}

#[test]
fn segmented_cursor_keeps_unmaterialized_source_and_atomic_provenance() {
    use crate::source::{Cursor, ScopeSource};
    let source = ScopeSource::identity("é\\|🦀");
    let transformed = source.pipes();
    let mut cursor = Cursor::default();
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    cursor.append(source.select(std::iter::once(0..2)));
    cursor.append(transformed.select(std::iter::once(2..7)));
    assert_eq!(cursor.len(), 7);
    let prefix = cursor.take(3).materialize();
    assert_eq!(&*prefix.text, "é|");
    assert_eq!(prefix.native(&(2..3)), Some(2..4));
    assert_eq!(prefix.local(&(3..4)), Some(2..3));
    let suffix = cursor.take(cursor.len());
    assert!(cursor.is_empty());
    assert_eq!(
        super::WORK.with_borrow(|work| work.source_bytes + work.joined_bytes),
        3
    );
    let tail = suffix.materialize();
    assert_eq!(&*tail.text, "🦀");
    assert_eq!(tail.text.as_ptr(), transformed.text[3..].as_ptr());
    assert_eq!(tail.extent(0..tail.text.len()), 4..8);
    assert!(cursor.take(0).materialize().text.is_empty());
}

#[test]
fn segmented_cursor_lines_cross_piece_boundaries_once() {
    use crate::source::{Cursor, ScopeSource};
    let source = ScopeSource::identity("é\n🦀\nlast");
    let mut cursor = Cursor::default();
    for range in [0..2, 2..3, 3..7, 7..10, 10..12] {
        cursor.append(source.select(std::iter::once(range)));
    }
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    for (text, native) in [("é\n", 0..3), ("🦀\n", 3..8), ("last", 8..12)] {
        let line = cursor.line().unwrap().materialize();
        assert_eq!(&*line.text, text);
        assert_eq!(line.extent(0..line.text.len()), native);
    }
    assert!(cursor.line().is_none());
    assert_eq!(cursor.len(), 0);
    assert_eq!(super::WORK.with_borrow(|work| work.framing_bytes), 12);
}

#[test]
fn mapped_text_repeated_reads_keep_storage_shared() {
    let source = "> é\n> 🦀";
    let document = parse(source.to_owned());
    let text = document.nodes[0].authored.clone();
    let chunks = text.chunks().map(str::as_ptr).collect::<Vec<_>>();
    let clone = text.clone();
    drop(document);
    let mut retained = Retained::default();
    text.retained(&mut retained);
    let before = retained.bytes;
    clone.retained(&mut retained);
    assert_eq!(retained.bytes, before);
    assert_eq!(retained.source_bytes, source.len());
    for _ in 0..4 {
        assert_eq!(text.len(), source.len());
        assert_eq!(text.chunks().map(str::as_ptr).collect::<Vec<_>>(), chunks);
        assert_eq!(clone.chunks().map(str::as_ptr).collect::<Vec<_>>(), chunks);
        assert_eq!(text.to_owned_string(), source);
        let mut after = Retained::default();
        text.retained(&mut after);
        clone.retained(&mut after);
        assert_eq!(after.bytes, before);
    }
}

#[test]
fn segmented_quote_body_keeps_one_atomic_list_projection() {
    use crate::source::{ScopeSource, Segmented};
    let raw = ScopeSource::identity("> - é\n>   🦀");
    let mut body = Segmented::from(raw.select(std::iter::once(2..6)));
    body.append(&Segmented::from(raw.select(std::iter::once(6..7))));
    body.append(&Segmented::from(raw.select(std::iter::once(11..15))));
    let spelling = Segmented::spelling(body.mapped(), 0..raw.text.len());
    assert_eq!(spelling.mapped().to_owned_string(), "- é\n🦀");
    assert_eq!(spelling.native(&(2..4)), Some(0..15));
    assert_eq!(spelling.local(&(8..10)), Some(0..9));
    let list = spelling.materialize();
    assert_eq!(list.native(&(2..4)), Some(0..15));
    assert_eq!(list.local(&(8..10)), Some(0..9));
    let identity = Segmented::from(raw);
    assert_eq!(identity.native(&(2..4)), Some(2..4));
    assert_eq!(identity.local(&(8..10)), Some(8..10));
}

#[test]
fn flat_container_text_keeps_logarithmic_storage_depth() {
    for lines in [2_000, 4_000, 8_000, 16_000] {
        let source = "- a\n".to_owned() + &"  a\n".repeat(lines);
        let document = parse(source.clone());
        assert_eq!(document.nodes[0].authored.to_owned_string(), source);
        assert_flat_document(&document);
        drop(document);
    }
}

#[test]
fn flat_quote_parse_and_drop() {
    for lines in [2_000, 4_000, 8_000, 16_000] {
        let source = "> a\n".repeat(lines + 1);
        let document = parse(source.clone());
        assert_eq!(document.nodes[0].authored.to_owned_string(), source);
        assert_flat_document(&document);
        let crate::Kind::Quote(_, body) = &document.nodes[0].kind else {
            panic!("quote");
        };
        assert!(body.storage_depth() <= super::segment_balance::depth_bound(body.len()));
        assert_eq!(body.to_owned_string(), "a\n".repeat(lines) + "a");
        drop(document);
        eprintln!("flat quote lines={} parsed and dropped", lines + 1);
    }
}

/// Inspect every retained authored value, including merged item text.
fn assert_flat_document(document: &crate::Document) {
    let mut pending: Vec<_> = document.nodes.iter().collect();
    let mut depth = 0;
    while let Some(node) = pending.pop() {
        depth = depth.max(node.authored.storage_depth());
        match &node.kind {
            crate::Kind::List(_, children)
            | crate::Kind::Item(children)
            | crate::Kind::Inline(children)
            | crate::Kind::Quote(children, _)
            | crate::Kind::Paragraph(children) => pending.extend(children),
            _ => {}
        }
    }
    eprintln!("maximum authored storage depth={depth}");
    assert!(
        depth <= super::segment_balance::depth_bound(document.source.len()),
        "flat input retains a per-line ownership chain: {depth}"
    );
}

#[test]
fn flat_mapped_text_append_drop_and_debug_handle_many_continuations() {
    use std::fmt::Write;
    let word = MappedText::from("a".to_owned());
    let newline = MappedText::from("\n".to_owned());
    let mut text = word.clone();
    for _ in 0..100_000 {
        text.append(&newline);
        text.append(&word);
    }
    assert_eq!(text.len(), 200_001);
    assert_eq!(text.to_owned_string(), "a\n".repeat(100_000) + "a");
    assert!(text.storage_depth() <= super::segment_balance::depth_bound(text.len()));
    let mut diagnostic = String::new();
    write!(&mut diagnostic, "{text:?}").unwrap();
    assert!(!diagnostic.is_empty());
    drop(text);
}

#[test]
fn mapped_text_selected_window_retires_excluded_segments() {
    let mut text = MappedText::from("one".to_owned());
    for word in ["two", "three", "four"] {
        text.append(&MappedText::from(word.to_owned()));
    }
    let selected = text.select(3..11);
    assert_eq!(selected.to_owned_string(), "twothree");
    let mut full = Retained::default();
    text.retained(&mut full);
    let mut window = Retained::default();
    selected.retained(&mut window);
    assert_eq!(full.source_bytes, 15);
    assert_eq!(window.source_bytes, 8);
    assert!(window.bytes < full.bytes);
}
