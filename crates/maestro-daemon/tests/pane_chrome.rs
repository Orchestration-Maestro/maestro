//! Pane borders, gaps and inner rectangles as the daemon serves them.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_daemon::app::layout::{PaneChrome, apply_pane_chrome, pane_inner_rect};
use maestro_daemon::settings::model::Config;
use maestro_protocol::schema::SplitDirection;
use maestro_protocol::shared::Flag;
use maestro_workspace::layout::{Borders, PaneId, PaneInfo, Rect, SplitRequest, TileLayout};

/// Allocator-style identity for a fixture pane.
fn pane(raw: u32) -> PaneId {
    PaneId::from_raw(raw)
}

/// Every chrome switch on, as in the default preferences.
const FULL: PaneChrome = PaneChrome {
    borders: true,
    gaps: true,
    outer_borders: true,
};

/// Lays out two panes along `direction` in the given area and applies `chrome`.
fn two_panes(direction: SplitDirection, area: Rect, chrome: PaneChrome) -> (PaneInfo, PaneInfo) {
    let mut layout = TileLayout::new(pane(1));
    layout
        .split_pane(
            pane(2),
            SplitRequest {
                target: pane(1),
                direction,
                ratio: None,
                focus_new_pane: false,
            },
        )
        .unwrap();
    let infos = apply_pane_chrome(layout.panes(area), chrome);
    let find = |id| infos.iter().find(|info| info.id == id).unwrap().clone();
    (find(pane(1)), find(pane(2)))
}

#[test]
fn side_by_side_panes_share_one_divider_column() {
    let chrome = PaneChrome {
        gaps: false,
        ..FULL
    };
    let (left, right) = two_panes(SplitDirection::Right, Rect::new(0, 0, 100, 20), chrome);
    assert_eq!(left.rect.right(), right.rect.x);
    assert!(!left.borders.contains(Borders::RIGHT));
    assert!(right.borders.contains(Borders::LEFT));
}

#[test]
fn stacked_panes_share_one_divider_row() {
    let chrome = PaneChrome {
        gaps: false,
        ..FULL
    };
    let (top, bottom) = two_panes(SplitDirection::Down, Rect::new(0, 0, 100, 20), chrome);
    assert_eq!(top.rect.bottom(), bottom.rect.y);
    assert!(!top.borders.contains(Borders::BOTTOM));
    assert!(bottom.borders.contains(Borders::TOP));
}

#[test]
fn hidden_outer_borders_keep_only_the_shared_divider() {
    let chrome = PaneChrome {
        gaps: false,
        outer_borders: false,
        ..FULL
    };
    let (left, right) = two_panes(SplitDirection::Right, Rect::new(0, 0, 100, 20), chrome);
    assert_eq!(left.borders, Borders::NONE);
    assert_eq!(right.borders, Borders::LEFT);
}

#[test]
fn gaps_keep_every_bordered_pane_complete() {
    let (left, right) = two_panes(SplitDirection::Right, Rect::new(0, 0, 100, 20), FULL);
    assert_eq!(left.rect.right(), right.rect.x);
    assert_eq!(left.borders, Borders::ALL);
    assert_eq!(right.borders, Borders::ALL);
}

#[test]
fn borderless_gaps_leave_one_empty_column_between_panes() {
    let chrome = PaneChrome {
        borders: false,
        ..FULL
    };
    let (left, right) = two_panes(SplitDirection::Right, Rect::new(0, 0, 100, 20), chrome);
    assert_eq!(left.rect, Rect::new(0, 0, 49, 20));
    assert_eq!(right.rect, Rect::new(50, 0, 50, 20));
    assert!(left.borders.is_empty() && right.borders.is_empty());
}

#[test]
fn borderless_gaps_leave_one_empty_row_between_stacked_panes() {
    let chrome = PaneChrome {
        borders: false,
        ..FULL
    };
    let (top, bottom) = two_panes(SplitDirection::Down, Rect::new(0, 0, 100, 20), chrome);
    assert_eq!(top.rect, Rect::new(0, 0, 100, 9));
    assert_eq!(bottom.rect, Rect::new(0, 10, 100, 10));
}

#[test]
fn a_one_cell_pane_keeps_its_cell_when_a_gap_is_added() {
    let chrome = PaneChrome {
        borders: false,
        ..FULL
    };
    let (left, right) = two_panes(SplitDirection::Right, Rect::new(0, 0, 2, 4), chrome);
    assert_eq!((left.rect.width, right.rect.width), (1, 1));
}

#[test]
fn a_lone_pane_has_no_chrome() {
    let layout = TileLayout::new(pane(1));
    let infos = apply_pane_chrome(layout.panes(Rect::new(0, 0, 80, 24)), FULL);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].rect, Rect::new(0, 0, 80, 24));
    assert_eq!(infos[0].borders, Borders::NONE);
    assert_eq!(infos[0].inner_rect, infos[0].rect);
}

#[test]
fn inner_rectangles_follow_the_borders_each_pane_keeps() {
    let chrome = PaneChrome {
        gaps: false,
        ..FULL
    };
    let (left, right) = two_panes(SplitDirection::Right, Rect::new(0, 0, 100, 20), chrome);
    assert_eq!(left.inner_rect, Rect::new(1, 1, 49, 18));
    assert_eq!(right.inner_rect, Rect::new(51, 1, 48, 18));
}

#[test]
fn borderless_panes_have_inner_rectangles_equal_to_their_cells() {
    let chrome = PaneChrome {
        borders: false,
        gaps: false,
        outer_borders: true,
    };
    let (left, right) = two_panes(SplitDirection::Right, Rect::new(0, 0, 100, 20), chrome);
    for info in [left, right] {
        assert!(info.borders.is_empty());
        assert_eq!(pane_inner_rect(info.rect, info.borders), info.rect);
        assert_eq!(info.inner_rect, info.rect);
    }
}

#[test]
fn an_inner_rectangle_loses_one_cell_for_each_present_border() {
    let area = Rect::new(2, 3, 10, 6);
    for (borders, expected) in [
        (Borders::NONE, area),
        (Borders::ALL, Rect::new(3, 4, 8, 4)),
        (Borders::LEFT, Rect::new(3, 3, 9, 6)),
        (Borders::TOP, Rect::new(2, 4, 10, 5)),
        (Borders::RIGHT, Rect::new(2, 3, 9, 6)),
        (Borders::BOTTOM, Rect::new(2, 3, 10, 5)),
    ] {
        assert_eq!(pane_inner_rect(area, borders), expected, "{borders:?}");
    }
}

#[test]
fn an_inner_rectangle_never_inverts_in_a_tiny_cell() {
    assert_eq!(
        pane_inner_rect(Rect::new(5, 5, 1, 1), Borders::ALL),
        Rect::new(6, 6, 0, 0)
    );
    assert_eq!(
        pane_inner_rect(Rect::new(0, 0, 0, 0), Borders::ALL),
        Rect::new(0, 0, 0, 0)
    );
}

#[test]
fn chrome_follows_the_accepted_interface_preferences() {
    assert_eq!(PaneChrome::from_config(&Config::default()), FULL);
    let mut config = Config::default();
    config.ui.pane_borders = Flag(false);
    config.ui.pane_gaps = Flag(false);
    config.ui.pane_outer_borders = Flag(false);
    assert_eq!(
        PaneChrome::from_config(&config),
        PaneChrome {
            borders: false,
            gaps: false,
            outer_borders: false,
        }
    );
}
