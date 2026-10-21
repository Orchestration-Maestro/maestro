//! Viewport-bounded snapshot traversal.

use super::{Dirty, RenderState};
use crate::{Terminal, page_list::Scroll};

/// Count page borrows during an unchanged snapshot over a fixed viewport.
fn clean_page_visits(lines: usize) -> usize {
    let mut terminal = Terminal::new(80, 3, 100_000_000).unwrap();
    crate::stream::Stream::new(false).next_slice(
        "x\r\n".repeat(lines).as_bytes(),
        &mut crate::stream_terminal::Handler::new(&mut terminal),
    );
    terminal.screens.active_mut().pages.scroll(Scroll::Top);
    let mut state = RenderState::default();
    state.update(&mut terminal);
    assert!(state.cursor.viewport.is_none());
    state.dirty = Dirty::Clean;
    for mut row in state.rows() {
        row.clear_dirty();
    }
    crate::tests::WORK.with_borrow_mut(|work| work.page_visits = 0);
    state.update(&mut terminal);
    assert_eq!(state.dirty, Dirty::Clean);
    assert!(state.cursor.viewport.is_none());
    crate::tests::WORK.with_borrow(|work| work.page_visits)
}

#[test]
fn clean_snapshot_page_visits_ignore_offscreen_history_depth() {
    let visits = clean_page_visits(10_000);
    let twice = clean_page_visits(20_000);
    assert!(visits > 0);
    assert_eq!(visits, twice, "cursor lookup must stay within the viewport");
}
