//! Ordered page traversal and history edits.

use super::*;
use crate::Coordinate;

#[test]
fn history_page_iterator_single_page() {
    let list = resize::screen(80, 24);
    assert_eq!(list.first, list.last);
    assert!(list.nodes.get(list.first).unwrap().next.is_none());
    let mut iterator = list.page_iterator(
        Direction::RightDown,
        Point::Active(Coordinate::default()),
        None,
    );
    let chunk = iterator.next(&list).unwrap();
    assert_eq!(chunk.node, list.first);
    assert_eq!((chunk.start, chunk.end), (0, 24));
    assert_eq!(chunk.rows(&list).unwrap().len(), 24);
    assert!(chunk.full_page(&list));
    assert!(iterator.next(&list).is_none());
    assert!(!chunk.overlaps(Chunk {
        node: chunk.node,
        start: 24,
        end: 25
    }));
    assert!(chunk.overlaps(chunk));
}

#[test]
fn history_grow_allocate() {
    let mut list = resize::screen(80, 24);
    let first = list.first;
    let capacity = list.page(first).unwrap().capacity.rows;
    for _ in 24..capacity {
        assert!(list.grow().is_none());
    }
    assert_eq!(list.total_pages(), 1);
    let new = list.grow().unwrap();
    assert_ne!(first, new);
    assert_eq!(list.last, new);
    let cell = list
        .get_cell(Point::Active(Coordinate { x: 0, y: 23 }))
        .unwrap();
    assert_eq!(cell.node, new);
    assert_eq!(
        list.point_from_pin(
            crate::point::Tag::Screen,
            Pin {
                node: cell.node,
                x: cell.col_idx,
                y: cell.row_idx,
                garbage: false
            }
        ),
        Some(Point::Screen(Coordinate {
            x: 0,
            y: u32::from(capacity)
        }))
    );
    assert_eq!(list.total_pages(), 2);
    assert_eq!(list.total_rows(), usize::from(capacity) + 1);
    assert_eq!(list.page(new).unwrap().size.rows, 1);
    assert_eq!(list.nodes.get(new).unwrap().prev, Some(first));
    assert_eq!(list.nodes.get(first).unwrap().next, Some(new));
    assert_eq!(list.charged_bytes(), PageList::standard_bytes() * 2);
}

#[test]
fn split_keeps_untracked_bounds_and_remaps_retained_row() {
    let three = NonZeroU16::new(3).unwrap();
    let mut list = PageList::init(three, three, None);
    let original = list.first;
    for (y, value) in ['A', 'B', 'C'].into_iter().enumerate() {
        list.page_mut(original)
            .unwrap()
            .get_cells_mut(u16::try_from(y).unwrap())[0]
            .set_content(crate::page::cell::Content::Codepoint(value));
    }
    let split = list.pin(Point::Screen(Coordinate { x: 0, y: 1 })).unwrap();
    let handle = list.track_pin(split).unwrap();
    list.split(split).unwrap();
    assert_eq!((list.total_rows(), list.total_pages()), (3, 2));
    assert_eq!(list.page(original).unwrap().size.rows, 1);
    assert_eq!(
        list.page(original).unwrap().get_cells(0)[0].codepoint(),
        'A'
    );
    let retained = *list.tracked_pin(handle).unwrap();
    assert_eq!((retained.x, retained.y), (0, 0));
    assert_ne!(retained.node, original);
    assert_eq!(retained.row_and_cell(&list).unwrap().1.codepoint(), 'B');
    assert_eq!(
        list.page(retained.node).unwrap().get_cells(1)[0].codepoint(),
        'C'
    );
    assert!(list.node_is_valid(original));
    assert!(!list.pin_is_valid(split));
    assert_eq!(list.nodes.get(original).unwrap().next, Some(retained.node));
    assert_eq!(list.nodes.get(retained.node).unwrap().prev, Some(original));
}

#[test]
fn history_point_from_pin_active_with_history() {
    let mut list = resize::screen(80, 24);
    for _ in 0..30 {
        list.grow();
    }
    assert_eq!(
        list.point_from_pin(
            crate::point::Tag::Active,
            Pin {
                node: list.first,
                x: 2,
                y: 30,
                garbage: false
            }
        ),
        Some(Point::Active(Coordinate { x: 2, y: 0 }))
    );
    assert!(
        list.point_from_pin(
            crate::point::Tag::Active,
            Pin {
                node: list.first,
                x: 2,
                y: 21,
                garbage: false
            }
        )
        .is_none()
    );
    active_projection_keeps_viewport_and_cell_coordinates();
}

/// Project active, viewport and borrowed-cell coordinates through one history row.
fn active_projection_keeps_viewport_and_cell_coordinates() {
    let three = NonZeroU16::new(3).unwrap();
    let mut list = PageList::init(three, three, None);
    assert!(list.grow().is_none());
    let active = list.get_top_left(crate::point::Tag::Active);
    assert_eq!((active.x, active.y), (0, 1));
    assert_eq!(
        list.pin(Point::Viewport(Coordinate::default())),
        Some(active)
    );
    assert_eq!(
        list.point_from_pin(crate::point::Tag::Active, active),
        Some(Point::Active(Coordinate::default()))
    );
    let first = list.pin(Point::Screen(Coordinate::default())).unwrap();
    assert!(
        list.point_from_pin(crate::point::Tag::Active, first)
            .is_none()
    );
    assert_eq!(
        list.point_from_pin(crate::point::Tag::Screen, active),
        Some(Point::Screen(Coordinate { x: 0, y: 1 }))
    );
    let cell = list
        .get_cell(Point::Viewport(Coordinate::default()))
        .unwrap();
    assert_eq!((cell.node, cell.row_idx, cell.col_idx), (active.node, 1, 0));
}

#[test]
fn history_cell_iterator() {
    let mut list = resize::screen(2, 2);
    assert_eq!(list.first, list.last);
    for y in 0..2 {
        resize::write(&mut list, y, "\0\u{1}");
    }
    let mut iterator = list.cell_iterator(
        Direction::RightDown,
        Point::Screen(Coordinate::default()),
        None,
    );
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let pin = iterator.next(&list).unwrap();
        assert_eq!(
            list.point_from_pin(crate::point::Tag::Screen, pin),
            Some(Point::Screen(Coordinate { x, y }))
        );
        assert_eq!(
            pin.row_and_cell(&list).unwrap().1.codepoint(),
            char::from_u32(u32::from(x)).unwrap()
        );
    }
    assert!(iterator.next(&list).is_none());
    forward_cells_across_split();
}

/// Start within a row and keep traversal order across a split.
fn forward_cells_across_split() {
    let two = NonZeroU16::new(2).unwrap();
    let mut list = PageList::init(two, two, None);
    let split = list.pin(Point::Screen(Coordinate { x: 0, y: 1 })).unwrap();
    list.split(split).unwrap();
    let mut iterator = list.cell_iterator(
        Direction::RightDown,
        Point::Screen(Coordinate { x: 1, y: 0 }),
        None,
    );
    let mut positions = Vec::new();
    while let Some(pin) = iterator.next(&list) {
        positions.push(
            list.point_from_pin(crate::point::Tag::Screen, pin)
                .unwrap()
                .coord(),
        );
    }
    assert_eq!(
        positions,
        [
            Coordinate { x: 1, y: 0 },
            Coordinate { x: 0, y: 1 },
            Coordinate { x: 1, y: 1 }
        ]
    );
    assert!(iterator.next(&list).is_none());
}

#[test]
fn history_scroll_delta_row_back() {
    let mut list = resize::screen(80, 24);
    for _ in 0..10 {
        list.grow();
    }
    resize::screen_origin(&list, crate::point::Tag::Viewport, 10);
    list.scroll(Scroll::DeltaRow(-1));
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 34,
            offset: 9,
            len: 24
        }
    );
    resize::screen_origin(&list, crate::point::Tag::Viewport, 9);
    for _ in 0..10 {
        list.grow();
    }
    resize::screen_origin(&list, crate::point::Tag::Viewport, 9);
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 44,
            offset: 9,
            len: 24
        }
    );
    list.scroll(Scroll::DeltaRow(-1));
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 44,
            offset: 8,
            len: 24
        }
    );
    scroll_extreme_deltas_saturate();
}

/// Keep the viewport bounded for extreme positive and negative deltas.
fn scroll_extreme_deltas_saturate() {
    let two = NonZeroU16::new(2).unwrap();
    let mut list = PageList::init(two, two, None);
    for _ in 0..4 {
        let _ = list.grow();
    }
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 6,
            offset: 4,
            len: 2
        }
    );
    list.scroll(Scroll::DeltaRow(-2));
    assert_eq!(list.scrollbar().offset, 2);
    assert_eq!(
        list.pin(Point::Viewport(Coordinate::default())).unwrap().y,
        2
    );
    list.scroll(Scroll::DeltaRow(isize::MIN));
    assert_eq!(list.scrollbar().offset, 0);
    list.scroll(Scroll::Row(1));
    assert_eq!(list.scrollbar().offset, 1);
    list.scroll(Scroll::DeltaRow(isize::MAX));
    assert_eq!(list.scrollbar().offset, 4);
    list.scroll(Scroll::Top);
    assert_eq!(list.scrollbar().offset, 0);
    list.scroll(Scroll::Active);
    assert_eq!(list.scrollbar().offset, 4);
}

#[test]
fn pin_views_preserve_cell_domains() {
    use crate::{
        page::{CellPosition, cell::Content},
        style::{Color, Style},
    };
    let three = NonZeroU16::new(3).unwrap();
    let mut list = PageList::init(three, three, None);
    let pin = list.pin(Point::Screen(Coordinate { x: 1, y: 0 })).unwrap();
    assert_eq!(
        pin.style(&list, *pin.row_and_cell(&list).unwrap().1),
        Some(Style::default())
    );
    assert!(pin.grapheme(&list, CellPosition { x: 1, y: 0 }).is_none());
    assert!(pin.hyperlink_uri(&list).is_none());
    {
        let (row, cell) = pin.row_and_cell_mut(&mut list).unwrap();
        row.set_wrap(true);
        cell.set_content(Content::Codepoint('e'));
        assert_eq!(
            (row.wrap(), cell.content()),
            (true, Content::Codepoint('e'))
        );
    }
    let style = Style {
        fg_color: Color::Palette(3),
        ..Style::default()
    };
    populate_pin_views(&mut list, pin, style);
    check_pin_subsets(&list, pin);
    assert_eq!(
        pin.grapheme(&list, CellPosition { x: 1, y: 0 }),
        Some(&['\u{301}'][..])
    );
    assert!(pin.grapheme(&list, CellPosition { x: 3, y: 0 }).is_none());
    assert_eq!(pin.hyperlink_uri(&list), Some(&[b'x', 0xff, 0][..]));
    assert_eq!(
        pin.style(&list, *pin.row_and_cell(&list).unwrap().1),
        Some(style)
    );
    assert_eq!(
        pin.cells(&list, CellSubset::Left).unwrap()[0].codepoint(),
        '\0'
    );
    assert_eq!(
        pin.cells(&list, CellSubset::Right).unwrap()[1].content(),
        Content::BgColorPalette(8)
    );
    pin.cells_mut(&mut list, CellSubset::Right).unwrap()[1].set_content(Content::Codepoint('Z'));
    assert_eq!(
        list.page(pin.node).unwrap().get_cells(0)[2].codepoint(),
        'Z'
    );
    assert!(pin.row_and_cell(&list).unwrap().0.wrap());
    assert_eq!(pin.row_and_cell(&list).unwrap().1.codepoint(), 'e');
    assert_eq!(pin.hyperlink_uri(&list), Some(&[b'x', 0xff, 0][..]));
}

/// Every subset includes the selected cell and only its requested neighbours.
fn check_pin_subsets(list: &PageList, pin: Pin) {
    for (subset, len, selected) in [
        (CellSubset::All, 3, 1),
        (CellSubset::Left, 2, 1),
        (CellSubset::Right, 2, 0),
    ] {
        let cells = pin.cells(list, subset).unwrap();
        assert_eq!(cells.len(), len);
        assert_eq!(cells[selected], *pin.row_and_cell(list).unwrap().1);
    }
}

/// Fill the retained pin's row with managed values and a background-only neighbour.
fn populate_pin_views(list: &mut PageList, pin: Pin, style: crate::Style) {
    use crate::{
        hyperlink::{Hyperlink, HyperlinkId},
        page::{CellPosition, cell::Content},
    };
    let page = list.page_mut(pin.node).unwrap();
    let id = page.styles.add(style).unwrap();
    page.get_cells_mut(0)[1].set_style_id(id);
    page.append_grapheme(CellPosition { x: 1, y: 0 }, '\u{301}')
        .unwrap();
    let link = page
        .insert_hyperlink(Hyperlink {
            id: HyperlinkId::Implicit(7),
            uri: vec![b'x', 0xff, 0],
        })
        .unwrap();
    page.set_hyperlink(CellPosition { x: 1, y: 0 }, link)
        .unwrap();
    page.get_cells_mut(0)[2].set_content(Content::BgColorPalette(8));
}

#[test]
fn history_increase_capacity_to_increase_styles() {
    use crate::{
        page::{CellPosition, cell::Content},
        style::{Color, Style},
    };
    capacity::doubles(IncreaseCapacity::Styles);
    let two = NonZeroU16::new(2).unwrap();
    let mut list = PageList::init(two, two, None);
    let pin = list.pin(Point::Screen(Coordinate::default())).unwrap();
    let tracked = list.track_pin(pin).unwrap();
    let style = Style {
        fg_color: Color::Palette(4),
        ..Style::default()
    };
    let page = list.page_mut(pin.node).unwrap();
    page.dirty = true;
    let old_capacity = page.capacity.styles;
    let id = page.styles.add(style).unwrap();
    page.get_cells_mut(0)[0].set_style_id(id);
    page.get_cells_mut(0)[0].set_content(Content::Codepoint('a'));
    page.append_grapheme(CellPosition { x: 0, y: 0 }, '\u{301}')
        .unwrap();
    let new = list
        .increase_capacity(pin.node, Some(IncreaseCapacity::Styles))
        .unwrap();
    assert!(!list.node_is_valid(pin.node));
    assert_eq!(list.page(new).unwrap().capacity.styles, old_capacity * 2);
    assert!(list.page(new).unwrap().dirty);
    assert_eq!(list.total_pages(), 1);
    let moved = *list.tracked_pin(tracked).unwrap();
    assert_eq!(moved.node, new);
    assert_eq!(moved.row_and_cell(&list).unwrap().1.codepoint(), 'a');
    assert_eq!(
        moved.style(&list, *moved.row_and_cell(&list).unwrap().1),
        Some(style)
    );
    assert_eq!(
        moved.grapheme(&list, CellPosition { x: 0, y: 0 }),
        Some(&['\u{301}'][..])
    );
    assert_eq!(
        list.charged_bytes(),
        PageList::page_charge(list.page(new).unwrap().capacity)
    );
    assert_eq!(
        list.increase_capacity(pin.node, None),
        Err(IncreaseCapacityError::InvalidNode)
    );
}

#[test]
fn history_increase_capacity_from_zero_capacity_dimensions() {
    let mut list = resize::screen(80, 24);
    let mut capacity = list.page(list.first).unwrap().capacity;
    capacity.styles = 0;
    capacity.grapheme_bytes = 0;
    capacity.hyperlink_bytes = 0;
    capacity.string_bytes = 0;
    let mut page = Page::init(capacity);
    page.size.rows = 24;
    list.page_size = PageList::page_charge(capacity);
    list.nodes.get_mut(list.first).unwrap().page = page;
    let zero = list.page(list.first).unwrap().capacity;
    assert_eq!(
        (
            zero.styles,
            zero.grapheme_bytes,
            zero.hyperlink_bytes,
            zero.string_bytes
        ),
        (0, 0, 0, 0)
    );
    let mut node = list.first;
    for adjustment in [
        IncreaseCapacity::Styles,
        IncreaseCapacity::GraphemeBytes,
        IncreaseCapacity::StringBytes,
        IncreaseCapacity::HyperlinkBytes,
    ] {
        node = list.increase_capacity(node, Some(adjustment)).unwrap();
    }
    let capacity = list.page(node).unwrap().capacity;
    assert_eq!(
        capacity.styles,
        crate::page::layout::Capacity::default().styles
    );
    assert_eq!(
        capacity.grapheme_bytes,
        crate::page::layout::GRAPHEME_BYTES_DEFAULT
    );
    assert_eq!(
        capacity.string_bytes,
        crate::page::layout::STRING_BYTES_DEFAULT
    );
    assert_eq!(
        capacity.hyperlink_bytes,
        crate::page::layout::Capacity::default().hyperlink_bytes
    );
    let styles = capacity.styles;
    node = list
        .increase_capacity(node, Some(IncreaseCapacity::Styles))
        .unwrap();
    assert_eq!(list.page(node).unwrap().capacity.styles, styles * 2);
    let capacity = list.page(node).unwrap().capacity;
    let old = node;
    node = list.increase_capacity(node, None).unwrap();
    assert!(!list.node_is_valid(old));
    assert_eq!(list.page(node).unwrap().capacity, capacity);
}

#[test]
fn list_capacity_errors_keep_recovery_categories() {
    let two = NonZeroU16::new(2).unwrap();
    let mut list = PageList::init(two, two, None);
    let node = list.first;
    list.page_mut(node).unwrap().capacity.styles = u16::MAX;
    assert_eq!(
        list.increase_capacity(node, Some(IncreaseCapacity::Styles)),
        Err(IncreaseCapacityError::OutOfSpace)
    );
    list.page_mut(node).unwrap().capacity.grapheme_bytes = u32::MAX / 2;
    assert_eq!(
        list.increase_capacity(node, Some(IncreaseCapacity::GraphemeBytes)),
        Err(IncreaseCapacityError::OutOfSpace)
    );
    assert!(list.node_is_valid(node));
    assert_eq!(list.total_pages(), 1);
    let other = PageList::init(two, two, None);
    assert_eq!(
        list.increase_capacity(other.first, None),
        Err(IncreaseCapacityError::InvalidNode)
    );
    assert_eq!(
        list.split(Pin {
            node: other.first,
            x: 0,
            y: 0,
            garbage: false
        }),
        Err(SplitError::InvalidPin)
    );
}

#[test]
fn pin_views_reject_foreign_owner() {
    let two = NonZeroU16::new(2).unwrap();
    let list = PageList::init(two, two, None);
    let foreign = PageList::init(two, two, None);
    let pin = list.pin(Point::Screen(Coordinate::default())).unwrap();
    assert!(pin.cells(&foreign, CellSubset::All).is_none());
    assert!(pin.style(&foreign, crate::page::Cell::default()).is_none());
}

/// Retain prompt marks on one page, including an orphaned continuation.
fn prompt_list() -> PageList {
    use crate::page::row::SemanticPrompt::{Prompt, PromptContinuation};
    let mut list = PageList::init(
        NonZeroU16::new(2).unwrap(),
        NonZeroU16::new(20).unwrap(),
        None,
    );
    assert_eq!(list.first, list.last);
    let page = list.page_mut(list.first).unwrap();
    for (y, role) in [
        (3, Prompt),
        (6, Prompt),
        (7, PromptContinuation),
        (8, PromptContinuation),
        (12, PromptContinuation),
    ] {
        page.get_row_mut(y).set_semantic_prompt(role);
    }
    list
}

/// Read prompt row positions through the owned traversal cursor.
fn prompt_rows(list: &PageList, direction: Direction, start: u32, end: Option<u32>) -> Vec<u32> {
    let mut iterator = list.prompt_iterator(
        direction,
        Point::Screen(Coordinate { x: 0, y: start }),
        end.map(|y| Point::Screen(Coordinate { x: 0, y })),
    );
    let mut rows = Vec::new();
    while let Some(pin) = iterator.next(list) {
        rows.push(
            list.point_from_pin(crate::point::Tag::Screen, pin)
                .unwrap()
                .coord()
                .y,
        );
        assert_eq!(pin.x, 0);
    }
    assert!(iterator.next(list).is_none());
    rows
}

#[test]
fn history_prompt_iterator_right_down() {
    assert_eq!(
        prompt_rows(&prompt_list(), Direction::RightDown, 0, None),
        [3, 6, 12]
    );
}

#[test]
fn history_prompt_iterator_left_up() {
    assert_eq!(
        prompt_rows(&prompt_list(), Direction::LeftUp, 0, None),
        [12, 6, 3]
    );
}

#[test]
fn history_prompt_iterator_right_down_continuation_at_start() {
    use crate::page::row::SemanticPrompt::{Prompt, PromptContinuation};
    let mut source = resize::screen(2, 20);
    assert_eq!(source.first, source.last);
    let page = source.page_mut(source.first).unwrap();
    page.get_row_mut(0).set_semantic_prompt(PromptContinuation);
    page.get_row_mut(1).set_semantic_prompt(PromptContinuation);
    page.get_row_mut(5).set_semantic_prompt(Prompt);
    assert_eq!(prompt_rows(&source, Direction::RightDown, 0, None), [0, 5]);
    let mut list = prompt_list();
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(0).set_semantic_prompt(PromptContinuation);
    page.get_row_mut(1).set_semantic_prompt(PromptContinuation);
    assert_eq!(
        prompt_rows(&list, Direction::RightDown, 0, None),
        [0, 3, 6, 12]
    );
}

#[test]
fn history_prompt_iterator_right_down_with_prompt_before_continuation() {
    use crate::page::row::SemanticPrompt::{Prompt, PromptContinuation};
    let mut list = resize::screen(2, 20);
    assert_eq!(list.first, list.last);
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(2).set_semantic_prompt(Prompt);
    page.get_row_mut(3).set_semantic_prompt(PromptContinuation);
    page.get_row_mut(4).set_semantic_prompt(PromptContinuation);
    assert_eq!(prompt_rows(&list, Direction::RightDown, 3, None), [3]);
    assert_eq!(
        prompt_rows(&prompt_list(), Direction::RightDown, 7, None),
        [7, 12]
    );
}

#[test]
fn history_prompt_iterator_right_down_limit_inclusive() {
    use crate::page::row::SemanticPrompt::Prompt;
    let mut list = resize::screen(2, 20);
    assert_eq!(list.first, list.last);
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(5).set_semantic_prompt(Prompt);
    page.get_row_mut(10).set_semantic_prompt(Prompt);
    assert_eq!(prompt_rows(&list, Direction::RightDown, 0, Some(5)), [5]);
    assert_eq!(
        prompt_rows(&prompt_list(), Direction::RightDown, 0, Some(6)),
        [3, 6]
    );
    assert_eq!(
        prompt_rows(&prompt_list(), Direction::RightDown, 0, Some(8)),
        [3, 6]
    );
}

#[test]
fn prompt_backward_limit_through_continuations() {
    assert_eq!(
        prompt_rows(&prompt_list(), Direction::LeftUp, 6, Some(15)),
        [12, 6]
    );
}

#[test]
fn history_prompt_iterator_left_up_limit_inclusive() {
    use crate::page::row::SemanticPrompt::Prompt;
    let mut list = PageList::init(
        NonZeroU16::new(2).unwrap(),
        NonZeroU16::new(20).unwrap(),
        None,
    );
    assert_eq!(list.first, list.last);
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(5).set_semantic_prompt(Prompt);
    page.get_row_mut(10).set_semantic_prompt(Prompt);
    assert_eq!(prompt_rows(&list, Direction::LeftUp, 10, Some(15)), [10]);
}

#[test]
fn prompt_backward_limits_preserve_outside_neighbours() {
    let mut list = prompt_list();
    let split = list.pin(Point::Screen(Coordinate { x: 0, y: 7 })).unwrap();
    list.split(split).unwrap();
    assert_eq!(prompt_rows(&list, Direction::LeftUp, 6, Some(15)), [12, 6]);
    assert_eq!(prompt_rows(&list, Direction::LeftUp, 7, Some(15)), [12, 7]);
    assert_eq!(prompt_rows(&list, Direction::LeftUp, 5, Some(15)), [12, 6]);
    assert_eq!(prompt_rows(&list, Direction::LeftUp, 9, Some(15)), [12]);
    assert_eq!(prompt_rows(&list, Direction::RightDown, 0, Some(7)), [3, 6]);
}

mod erase;

mod resize;

mod reflow;

mod reflow_capacity;

mod reflow_pins;

mod reflow_pages;

mod reflow_wide;

mod reflow_metadata;

mod capacity;

mod reset;

mod iteration;

mod split;

mod movement;

mod scroll;

mod projection;

mod growth;
mod retention;

#[test]
fn prompt_reverse_default_boundary_ignores_columns() {
    use super::tests::resize::nz;
    use crate::page::row::SemanticPrompt;
    let mut list = PageList::init(nz(2), nz(20), None);
    list.page_mut(list.first)
        .unwrap()
        .get_row_mut(3)
        .set_semantic_prompt(SemanticPrompt::Prompt);
    assert!(prompt_rows(&list, Direction::LeftUp, 5, None).is_empty());
    let start = list.pin_at_row(19, 1).unwrap();
    let limit = list.pin_at_row(5, 0).unwrap();
    assert!(
        start
            .prompt_iterator(Direction::LeftUp, Some(limit))
            .next(&list)
            .is_none()
    );
    assert_eq!(prompt_rows(&list, Direction::LeftUp, 3, None), [3]);
}

#[test]
fn prompt_reverse_pruned_primary_returns_first_retained_continuation() {
    use crate::page::row::SemanticPrompt;
    let mut list = resize::screen(2, 3);
    list.grow();
    for y in 1..4 {
        list.page_mut(list.first)
            .unwrap()
            .get_row_mut(y)
            .set_semantic_prompt(SemanticPrompt::PromptContinuation);
    }
    list.page_mut(list.first)
        .unwrap()
        .get_row_mut(0)
        .set_semantic_prompt(SemanticPrompt::Prompt);
    list.erase_history(None);
    let start = list.pin_at_row(2, 1).unwrap();
    let mut iterator = start.prompt_iterator(Direction::LeftUp, None);
    let pin = iterator.next(&list).unwrap();
    assert_eq!(
        list.point_from_pin(crate::point::Tag::Screen, pin),
        Some(Point::Screen(Coordinate::default()))
    );
    assert!(iterator.next(&list).is_none());
}

#[test]
fn prompt_continuation_limits_ignore_endpoint_columns() {
    for split in [false, true] {
        let mut list = prompt_list();
        if split {
            list.split(list.pin_at_row(7, 0).unwrap()).unwrap();
        }
        for (direction, start, limit, expected) in [
            (Direction::RightDown, 6, 7, 6),
            (Direction::LeftUp, 8, 7, 7),
        ] {
            let start = list.pin_at_row(start, 1).unwrap();
            let limit = list.pin_at_row(limit, 0).unwrap();
            let mut iterator = start.prompt_iterator(direction, Some(limit));
            assert_eq!(
                list.point_from_pin(crate::point::Tag::Screen, iterator.next(&list).unwrap()),
                Some(Point::Screen(Coordinate { x: 0, y: expected }))
            );
            assert!(iterator.next(&list).is_none());
        }
    }
}

#[test]
fn same_page_full_row_transfer_does_not_visit_cells() {
    for cols in [8, 16] {
        let mut list = resize::screen(cols, 3);
        let source = list.pin(Point::Active(Coordinate { x: 0, y: 0 })).unwrap();
        let destination = source.down(&list, 1).unwrap();
        crate::tests::WORK.with_borrow_mut(|work| work.row_shift = 0);
        list.transfer_row(source, destination, 0..cols).unwrap();
        assert_eq!(crate::tests::WORK.with_borrow(|work| work.row_shift), 0);
    }
}
