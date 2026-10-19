//! Wide cells, spacer heads and multi-codepoint clusters through rewrapping.

use super::reflow::{flags, reflow};
use super::resize::{integrity, nz, text, write};
use super::*;
use crate::page::{
    CellPosition,
    cell::{Cell as PackedCell, Wide},
};

/// Write a wide cell with its following spacer.
fn wide(list: &mut PageList, x: u16, y: u16, cp: char) {
    let page = list.page_mut(list.first).unwrap();
    page.get_cells_mut(y)[usize::from(x)] = PackedCell::init(cp);
    page.get_cells_mut(y)[usize::from(x)].set_wide(Wide::Wide);
    page.get_cells_mut(y)[usize::from(x + 1)].set_wide(Wide::SpacerTail);
}

/// Read the width role at a retained screen coordinate.
fn role(list: &PageList, x: u16, y: usize) -> Wide {
    let pin = list.pin_at_row(y, x).unwrap();
    pin.row_and_cell(list).unwrap().1.wide()
}

#[test]
fn history_resize_reflow_more_cols_unwrap_wide_spacer_head() {
    let mut list = PageList::init(nz(2), nz(2), Some(0));
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 0, "x");
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(0).set_wrap(true);
    page.get_row_mut(1).set_wrap_continuation(true);
    page.get_cells_mut(0)[1].set_wide(Wide::SpacerHead);
    wide(&mut list, 0, 1, '😀');
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, 2));
    assert_eq!(list.total_pages(), 1);
    assert_eq!(text(&list, 0), "x😀\0\0");
    assert_eq!(flags(&list, 0), (false, false));
    for (x, expected) in [Wide::Narrow, Wide::Wide, Wide::SpacerTail, Wide::Narrow]
        .into_iter()
        .enumerate()
    {
        assert_eq!(role(&list, u16::try_from(x).unwrap(), 0), expected);
    }
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_unwrap_wide_spacer_head_across_two_rows() {
    let mut list = PageList::init(nz(2), nz(3), Some(0));
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 0, "xx");
    write(&mut list, 1, "x");
    let page = list.page_mut(list.first).unwrap();
    for y in 0..3 {
        page.get_row_mut(y).set_wrap(y < 2);
        page.get_row_mut(y).set_wrap_continuation(y > 0);
    }
    page.get_cells_mut(1)[1].set_wide(Wide::SpacerHead);
    wide(&mut list, 0, 2, '😀');
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, 3));
    assert_eq!(list.total_pages(), 1);
    assert_eq!(text(&list, 0), "xxx\0");
    assert_eq!(text(&list, 1), "😀\0\0\0");
    assert_eq!(flags(&list, 0), (true, false));
    for x in 0..3 {
        assert_eq!(role(&list, x, 0), Wide::Narrow);
    }
    assert_eq!(role(&list, 3, 0), Wide::SpacerHead);
    assert_eq!(role(&list, 0, 1), Wide::Wide);
    assert_eq!(role(&list, 1, 1), Wide::SpacerTail);
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_unwrap_still_requires_wide_spacer_head() {
    let mut list = PageList::init(nz(2), nz(2), Some(0));
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 0, "xx");
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(0).set_wrap(true);
    page.get_row_mut(1).set_wrap_continuation(true);
    wide(&mut list, 0, 1, '😀');
    reflow(&mut list, 3);
    assert_eq!((list.cols(), list.total_rows()), (3, 2));
    assert_eq!(list.total_pages(), 1);
    assert_eq!(text(&list, 0), "xx\0");
    assert_eq!(text(&list, 1), "😀\0\0");
    assert_eq!(flags(&list, 0), (true, false));
    assert_eq!(role(&list, 0, 0), Wide::Narrow);
    assert_eq!(role(&list, 1, 0), Wide::Narrow);
    assert_eq!(role(&list, 2, 0), Wide::SpacerHead);
    assert_eq!(role(&list, 0, 1), Wide::Wide);
    assert_eq!(role(&list, 1, 1), Wide::SpacerTail);
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_wraps_spacer_head() {
    let mut list = PageList::init(nz(4), nz(3), Some(0));
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 0, "xxx");
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(0).set_wrap(true);
    page.get_row_mut(1).set_wrap_continuation(true);
    page.get_cells_mut(0)[3].set_wide(Wide::SpacerHead);
    wide(&mut list, 0, 1, '😀');
    reflow(&mut list, 3);
    assert_eq!((list.cols(), list.total_rows()), (3, 3));
    assert_eq!(list.total_pages(), 1);
    assert_eq!(text(&list, 0), "xxx");
    assert_eq!(text(&list, 1), "😀\0\0");
    assert_eq!(flags(&list, 0), (true, false));
    for x in 0..3 {
        assert_eq!(role(&list, x, 0), Wide::Narrow);
    }
    assert_eq!(role(&list, 0, 1), Wide::Wide);
    assert_eq!(role(&list, 1, 1), Wide::SpacerTail);
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_to_wrap_a_multi_codepoint_grapheme_with_a_spacer_head() {
    let mut list = PageList::init(nz(4), nz(2), Some(0));
    assert_eq!(list.total_pages(), 1);
    let suffix = ['\u{200d}', '👨', '\u{200d}', '👦', '\u{200d}', '👦'];
    for x in [0, 2] {
        wide(&mut list, x, 0, '👨');
        list.page_mut(list.first)
            .unwrap()
            .set_graphemes(CellPosition { x, y: 0 }, &suffix)
            .unwrap();
    }
    reflow(&mut list, 3);
    assert_eq!((list.cols(), list.total_rows()), (3, 2));
    assert_eq!(list.total_pages(), 1);
    assert_eq!(flags(&list, 0), (true, false));
    for y in 0..2 {
        assert_eq!(text(&list, y), "👨\0\0");
        assert_eq!(role(&list, 0, y), Wide::Wide);
        assert_eq!(role(&list, 1, y), Wide::SpacerTail);
        let pin = list.pin_at_row(y, 0).unwrap();
        assert_eq!(
            pin.grapheme(&list, CellPosition { x: pin.x, y: pin.y }),
            Some(&suffix[..])
        );
    }
    assert_eq!(role(&list, 2, 0), Wide::SpacerHead);
    integrity(&list);
}
