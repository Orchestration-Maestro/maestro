use super::{page, pin, screen};
use crate::page::cell::Content;

#[test]
fn screen_cursor_small_moves_keep_pin_coordinates() {
    let mut s = screen(8, 5);
    s.cursor_right(4);
    assert_eq!((s.cursor.x, pin(&s).x), (4, 4));
    s.cursor_left(2);
    assert_eq!((s.cursor.x, pin(&s).x), (2, 2));
    s.cursor_down(3);
    assert_eq!((s.cursor.y, pin(&s).y), (3, 3));
    s.cursor_up(2);
    assert_eq!((s.cursor.y, pin(&s).y), (1, 1));
    s.cursor_horizontal_absolute(6);
    s.cursor_cell_left(2)
        .unwrap()
        .set_content(Content::Codepoint('L'));
    s.cursor_cell_right(1)
        .unwrap()
        .set_content(Content::Codepoint('R'));
    s.cursor_row_up(1).unwrap().set_wrap(true);
    assert_eq!(page(&s).get_cells(1)[4].codepoint(), 'L');
    assert_eq!(page(&s).get_cells(1)[7].codepoint(), 'R');
    assert!(page(&s).get_row(0).wrap());
    assert_eq!((s.cursor.x, s.cursor.y), (6, 1));
    s.cursor_absolute(1, 4);
    assert_eq!((pin(&s).x, pin(&s).y), (1, 4));
    s.cursor_reload();
    assert_eq!((s.cursor.x, s.cursor.y), (1, 4));
}

#[test]
fn screen_cursor_dirty_marks_old_and_new_rows() {
    let mut s = screen(8, 5);
    for y in 0..5 {
        let node = pin(&s).node;
        s.pages
            .page_mut(node)
            .unwrap()
            .get_row_mut(y)
            .set_dirty(false);
    }
    s.cursor_right(1);
    assert!(!page(&s).get_row(0).dirty());
    s.cursor_absolute(2, 2);
    assert!(page(&s).get_row(0).dirty());
    assert!(!page(&s).get_row(1).dirty());
    assert!(page(&s).get_row(2).dirty());
    let node = pin(&s).node;
    s.pages
        .page_mut(node)
        .unwrap()
        .get_row_mut(2)
        .set_dirty(false);
    s.cursor_absolute(2, 2);
    assert!(!page(&s).get_row(2).dirty());
    s.cursor_mark_dirty();
    assert!(page(&s).get_row(2).dirty());
}

#[test]
fn screen_cursor_cell_end_of_prev_across_mixed_width_pages() {
    let mut s = screen(4, 2);
    let (first, second) = super::split_rows(&mut s, 1);
    s.cursor_absolute(0, 1);
    s.pages.page_mut(first).unwrap().size.cols = 2;
    s.pages.page_mut(first).unwrap().get_cells_mut(0)[1].set_content(Content::Codepoint('B'));
    assert_eq!(pin(&s).node, second);
    assert_eq!(s.cursor_cell_end_of_prev().unwrap().codepoint(), 'B');
    s.cursor_cell_end_of_prev()
        .unwrap()
        .set_content(Content::Codepoint('X'));
    assert_eq!(
        s.pages.page(first).unwrap().get_cells(0)[1].codepoint(),
        'X'
    );
}

#[test]
fn screen_cursor_copy_x_y() {
    let mut source = screen(10, 10);
    source.cursor_absolute(2, 3);
    assert_eq!((source.cursor.x, source.cursor.y), (2, 3));
    let mut target = screen(10, 10);
    target.cursor_copy(&source.cursor, true).unwrap();
    assert_eq!((target.cursor.x, target.cursor.y), (2, 3));
    for ch in "Hello".chars() {
        target
            .cursor_cell_right(0)
            .unwrap()
            .set_content(Content::Codepoint(ch));
        target.cursor_right(1);
    }
    let row: String = page(&target).get_cells(3)[..7]
        .iter()
        .map(|cell| {
            if cell.is_empty() {
                ' '
            } else {
                cell.codepoint()
            }
        })
        .collect();
    assert_eq!(row, "  Hello");
    for y in 0..3 {
        assert!(
            page(&target)
                .get_cells(y)
                .iter()
                .copied()
                .all(crate::Cell::is_empty)
        );
    }
    assert_eq!((pin(&target).x, pin(&target).y), (7, 3));
}
