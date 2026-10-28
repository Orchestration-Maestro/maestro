//! Split layout behaviour.
#![cfg(test)]
use maestro_protocol::schema::SplitDirection;
use maestro_workspace::layout::*;
fn pane(id: u32) -> PaneId {
    PaneId::from_raw(id)
}

fn sample_layout() -> TileLayout {
    TileLayout::from_saved(
        Node::Split {
            direction: SplitDirection::Right,
            ratio: 0.3,
            first: Box::new(Node::Pane(pane(1))),
            second: Box::new(Node::Split {
                direction: SplitDirection::Down,
                ratio: 0.6,
                first: Box::new(Node::Pane(pane(2))),
                second: Box::new(Node::Split {
                    direction: SplitDirection::Right,
                    ratio: 0.4,
                    first: Box::new(Node::Pane(pane(3))),
                    second: Box::new(Node::Pane(pane(4))),
                }),
            }),
        },
        pane(2),
    )
}

fn pane_rects(layout: &TileLayout) -> Vec<(PaneId, Rect)> {
    layout
        .panes(Rect::new(0, 0, 100, 40))
        .into_iter()
        .map(|info| (info.id, info.rect))
        .collect()
}

fn pane_rect(layout: &TileLayout, pane_id: PaneId) -> Rect {
    pane_rects(layout)
        .into_iter()
        .find_map(|(id, rect)| (id == pane_id).then_some(rect))
        .expect("pane should exist")
}

fn split_snapshot(layout: &TileLayout) -> Vec<(SplitDirection, f32)> {
    fn collect(node: &Node, out: &mut Vec<(SplitDirection, f32)>) {
        match node {
            Node::Pane(_) => {}
            Node::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                out.push((direction.clone(), *ratio));
                collect(first, out);
                collect(second, out);
            }
        }
    }

    let mut out = Vec::new();
    collect(layout.root(), &mut out);
    out
}

#[test]
fn leaf_swap_keeps_geometry_ratios_and_focus() {
    let mut layout = sample_layout();
    let before_rects = pane_rects(&layout);
    let before_splits = split_snapshot(&layout);

    assert!(layout.swap_panes(pane(2), pane(4)));

    assert_eq!(layout.pane_count(), 4);
    assert_eq!(split_snapshot(&layout), before_splits);
    assert_eq!(layout.focused(), pane(2));

    let after_rects = pane_rects(&layout);
    assert_eq!(after_rects[0], before_rects[0]);
    assert_eq!(after_rects[1], (pane(4), before_rects[1].1));
    assert_eq!(after_rects[2], before_rects[2]);
    assert_eq!(after_rects[3], (pane(2), before_rects[3].1));
}

#[test]
fn invalid_leaf_swaps_preserve_layout() {
    let mut layout = sample_layout();
    let before_rects = pane_rects(&layout);
    let before_splits = split_snapshot(&layout);
    let before_focus = layout.focused();

    assert!(!layout.swap_panes(pane(2), pane(2)));
    assert!(!layout.swap_panes(pane(2), pane(99)));
    assert!(!layout.swap_panes(pane(99), pane(2)));

    assert_eq!(pane_rects(&layout), before_rects);
    assert_eq!(split_snapshot(&layout), before_splits);
    assert_eq!(layout.focused(), before_focus);
}

fn new_layout() -> (TileLayout, PaneId) {
    let id = pane(1);
    (TileLayout::new(id), id)
}
#[test]
fn moved_leaf_insertion_keeps_ids_and_selects_new_focus() {
    let (mut layout, root) = new_layout();
    let moved = pane(99);

    assert!(layout.insert_pane_near(
        moved,
        &SplitRequest {
            target: root,
            direction: SplitDirection::Right,
            ratio: Some(0.25),
            focus_new_pane: true
        }
    ));

    assert_eq!(layout.pane_count(), 2);
    assert_eq!(layout.pane_ids(), vec![root, moved]);
    assert_eq!(layout.focused(), moved);
    let splits = split_snapshot(&layout);
    assert_eq!(splits, vec![(SplitDirection::Right, 0.25)]);
    assert_eq!(pane_rect(&layout, root), Rect::new(0, 0, 25, 40));
    assert_eq!(pane_rect(&layout, moved), Rect::new(25, 0, 75, 40));
}

trait SplitFixture {
    fn split_focused(&mut self, direction: SplitDirection) -> PaneId;
    fn split_focused_with_ratio(&mut self, direction: SplitDirection, ratio: f32) -> PaneId;
    fn split_assigned(
        &mut self,
        target: PaneId,
        direction: SplitDirection,
        ratio: f32,
    ) -> Option<PaneId>;
}
impl SplitFixture for TileLayout {
    fn split_focused(&mut self, direction: SplitDirection) -> PaneId {
        self.split_focused_with_ratio(direction, 0.5)
    }
    fn split_focused_with_ratio(&mut self, direction: SplitDirection, ratio: f32) -> PaneId {
        let id = self
            .split_assigned(self.focused(), direction, ratio)
            .unwrap();
        self.focus_pane(id);
        id
    }
    fn split_assigned(
        &mut self,
        target: PaneId,
        direction: SplitDirection,
        ratio: f32,
    ) -> Option<PaneId> {
        let id = pane(self.pane_ids().iter().map(|id| id.raw()).max().unwrap() + 1);
        self.split_pane(
            id,
            SplitRequest {
                target,
                direction,
                ratio: Some(ratio),
                focus_new_pane: false,
            },
        )
    }
}

#[test]
fn focused_split_keeps_requested_ratio() {
    let (mut layout, root) = new_layout();
    layout.focus_pane(root);

    layout.split_focused_with_ratio(SplitDirection::Right, 0.333);

    let splits = split_snapshot(&layout);
    assert_eq!(splits.len(), 1);
    assert_eq!(splits[0].0, SplitDirection::Right);
    assert!((splits[0].1 - 0.333).abs() < f32::EPSILON);
}

#[test]
fn targeted_resize_reports_change_without_moving_focus() {
    let mut layout = sample_layout();
    let original_focus = layout.focused();

    assert!(layout.resize_pane(pane(1), NavDirection::Right, 0.05, Rect::new(0, 0, 100, 40),));

    assert_eq!(layout.focused(), original_focus);
    let split = &split_snapshot(&layout)[0];
    assert_eq!(split.0, SplitDirection::Right);
    assert!((split.1 - 0.35).abs() < f32::EPSILON);
}

#[test]
fn second_child_resize_reduces_first_ratio() {
    let (mut layout, root) = new_layout();
    let right = layout.split_focused(SplitDirection::Right);
    layout.focus_pane(root);

    assert!(layout.resize_pane(right, NavDirection::Left, 0.05, Rect::new(0, 0, 100, 40),));

    let split = &split_snapshot(&layout)[0];
    assert_eq!(split.0, SplitDirection::Right);
    assert!((split.1 - 0.45).abs() < f32::EPSILON);
    assert_eq!(layout.focused(), root);
}

#[test]
fn outer_edge_resize_shrinks_each_axis_side() {
    let (mut horizontal, left) = new_layout();
    horizontal.split_focused(SplitDirection::Right);

    assert!(horizontal.resize_pane(left, NavDirection::Left, 0.05, Rect::new(0, 0, 100, 40),));
    let split = &split_snapshot(&horizontal)[0];
    assert_eq!(split.0, SplitDirection::Right);
    assert!((split.1 - 0.45).abs() < f32::EPSILON);

    let (mut horizontal, _left) = new_layout();
    let right = horizontal.split_focused(SplitDirection::Right);

    assert!(horizontal.resize_pane(right, NavDirection::Right, 0.05, Rect::new(0, 0, 100, 40),));
    let split = &split_snapshot(&horizontal)[0];
    assert_eq!(split.0, SplitDirection::Right);
    assert!((split.1 - 0.55).abs() < f32::EPSILON);

    let (mut vertical, top) = new_layout();
    vertical.split_focused(SplitDirection::Down);

    assert!(vertical.resize_pane(top, NavDirection::Up, 0.05, Rect::new(0, 0, 100, 40),));
    let split = &split_snapshot(&vertical)[0];
    assert_eq!(split.0, SplitDirection::Down);
    assert!((split.1 - 0.45).abs() < f32::EPSILON);

    let (mut vertical, _top) = new_layout();
    let bottom = vertical.split_focused(SplitDirection::Down);

    assert!(vertical.resize_pane(bottom, NavDirection::Down, 0.05, Rect::new(0, 0, 100, 40),));
    let split = &split_snapshot(&vertical)[0];
    assert_eq!(split.0, SplitDirection::Down);
    assert!((split.1 - 0.55).abs() < f32::EPSILON);
}

#[test]
fn horizontal_ancestor_resize_preserves_nested_height() {
    let mut layout = TileLayout::from_saved(
        Node::Split {
            direction: SplitDirection::Right,
            ratio: 0.6,
            first: Box::new(Node::Split {
                direction: SplitDirection::Down,
                ratio: 0.5,
                first: Box::new(Node::Pane(pane(1))),
                second: Box::new(Node::Pane(pane(2))),
            }),
            second: Box::new(Node::Pane(pane(3))),
        },
        pane(1),
    );
    let before = pane_rect(&layout, pane(1));

    assert!(layout.resize_pane(pane(1), NavDirection::Left, 0.05, Rect::new(0, 0, 100, 40),));

    let after = pane_rect(&layout, pane(1));
    assert_eq!(after.height, before.height);
    assert!(after.width < before.width);
    let splits = split_snapshot(&layout);
    assert_eq!(splits[0].0, SplitDirection::Right);
    assert!((splits[0].1 - 0.55).abs() < f32::EPSILON);
    assert_eq!(splits[1], (SplitDirection::Down, 0.5));
}

#[test]
fn vertical_ancestor_resize_preserves_nested_width() {
    let mut layout = TileLayout::from_saved(
        Node::Split {
            direction: SplitDirection::Down,
            ratio: 0.6,
            first: Box::new(Node::Split {
                direction: SplitDirection::Right,
                ratio: 0.5,
                first: Box::new(Node::Pane(pane(1))),
                second: Box::new(Node::Pane(pane(2))),
            }),
            second: Box::new(Node::Pane(pane(3))),
        },
        pane(1),
    );
    let before = pane_rect(&layout, pane(1));

    assert!(layout.resize_pane(pane(1), NavDirection::Up, 0.05, Rect::new(0, 0, 100, 40),));

    let after = pane_rect(&layout, pane(1));
    assert_eq!(after.width, before.width);
    assert!(after.height < before.height);
    let splits = split_snapshot(&layout);
    assert_eq!(splits[0].0, SplitDirection::Down);
    assert!((splits[0].1 - 0.55).abs() < f32::EPSILON);
    assert_eq!(splits[1], (SplitDirection::Right, 0.5));
}

#[test]
fn resize_selects_border_in_target_branch() {
    let mut layout = TileLayout::from_saved(
        Node::Split {
            direction: SplitDirection::Down,
            ratio: 0.5,
            first: Box::new(Node::Split {
                direction: SplitDirection::Right,
                ratio: 0.5,
                first: Box::new(Node::Pane(pane(1))),
                second: Box::new(Node::Pane(pane(2))),
            }),
            second: Box::new(Node::Split {
                direction: SplitDirection::Right,
                ratio: 0.5,
                first: Box::new(Node::Pane(pane(3))),
                second: Box::new(Node::Pane(pane(4))),
            }),
        },
        pane(3),
    );

    assert!(layout.resize_pane(pane(3), NavDirection::Right, 0.05, Rect::new(0, 0, 100, 40),));

    let splits = split_snapshot(&layout);
    assert_eq!(splits[0], (SplitDirection::Down, 0.5));
    assert_eq!(splits[1], (SplitDirection::Right, 0.5));
    assert_eq!(splits[2].0, SplitDirection::Right);
    assert!((splits[2].1 - 0.55).abs() < f32::EPSILON);
}

#[test]
fn navigation_prefers_more_shared_edge() {
    let focused = PaneInfo {
        id: pane(1),
        rect: Rect::new(10, 10, 10, 10),
        inner_rect: Rect::new(10, 10, 10, 10),
        scrollbar_rect: None,
        borders: Borders::NONE,
        is_focused: true,
    };
    let small_overlap_first = PaneInfo {
        id: pane(2),
        rect: Rect::new(0, 10, 10, 2),
        inner_rect: Rect::new(0, 10, 10, 2),
        scrollbar_rect: None,
        borders: Borders::NONE,
        is_focused: false,
    };
    let larger_overlap_second = PaneInfo {
        id: pane(3),
        rect: Rect::new(0, 10, 10, 8),
        inner_rect: Rect::new(0, 10, 10, 8),
        scrollbar_rect: None,
        borders: Borders::NONE,
        is_focused: false,
    };
    let panes = vec![focused.clone(), small_overlap_first, larger_overlap_second];

    assert_eq!(
        find_in_direction(&focused, NavDirection::Left, &panes),
        Some(pane(3))
    );
}

#[test]
fn closing_selection_restores_previous_focus() {
    let mut layout = sample_layout();
    layout.focus_pane(pane(4));

    assert!(layout.close_focused());

    assert_eq!(layout.focused(), pane(2));
}

#[test]
fn closing_split_returns_to_its_opener() {
    let (mut layout, first) = new_layout();
    let second = layout.split_focused(SplitDirection::Right);
    let third = layout.split_focused(SplitDirection::Down);
    assert_eq!(layout.pane_ids().len(), 3);

    layout.focus_pane(first);
    let opened = layout.split_focused(SplitDirection::Right);
    assert_eq!(layout.focused(), opened);

    assert!(layout.close_focused());

    assert_eq!(layout.focused(), first);
    assert!(layout.pane_ids().contains(&second));
    assert!(layout.pane_ids().contains(&third));
}

#[test]
fn background_close_preserves_focus_return_path() {
    let mut layout = sample_layout();
    layout.focus_pane(pane(4));

    assert!(layout.close_pane(pane(1)));
    assert_eq!(layout.focused(), pane(4));

    assert!(layout.close_focused());
    assert_eq!(layout.focused(), pane(2));
}

#[test]
fn closing_remembered_pane_clears_return_path() {
    let mut layout = sample_layout();
    layout.focus_pane(pane(4));

    assert!(layout.close_pane(pane(2)));

    assert!(layout.close_focused());
    assert_eq!(layout.focused(), pane(3));
}

#[test]
fn close_without_history_selects_tree_neighbour() {
    let mut layout = sample_layout();

    assert!(layout.close_focused());

    assert_eq!(layout.focused(), pane(3));
}

#[test]
fn close_consumes_history_once() {
    let mut layout = sample_layout();
    layout.focus_pane(pane(4));

    assert!(layout.close_focused());
    assert_eq!(layout.focused(), pane(2));

    assert!(layout.close_focused());
    assert_eq!(layout.focused(), pane(3));
}

#[test]
fn resize_preserves_later_close_target() {
    let mut layout = sample_layout();
    layout.focus_pane(pane(4));
    layout.resize_pane(pane(1), NavDirection::Right, 0.05, Rect::new(0, 0, 100, 40));

    assert!(layout.close_focused());

    assert_eq!(layout.focused(), pane(2));
}

#[test]
fn background_split_preserves_focus_and_return_path() {
    let mut layout = sample_layout();
    layout.focus_pane(pane(4));

    let new_id = layout
        .split_assigned(pane(1), SplitDirection::Right, 0.5)
        .expect("target exists");

    assert!(layout.pane_ids().contains(&new_id));
    assert_eq!(layout.focused(), pane(4));
    assert!(layout.close_focused());
    assert_eq!(layout.focused(), pane(2));
}

#[test]
fn missing_split_target_preserves_leaf_order() {
    let mut layout = sample_layout();
    let ids = layout.pane_ids();

    assert_eq!(
        layout.split_assigned(pane(99), SplitDirection::Right, 0.5),
        None
    );

    assert_eq!(layout.pane_ids(), ids);
}

#[test]
fn unselected_insert_preserves_focus_and_return_path() {
    let mut layout = sample_layout();
    layout.focus_pane(pane(4));

    assert!(layout.insert_pane_near(
        pane(9),
        &SplitRequest {
            target: pane(1),
            direction: SplitDirection::Right,
            ratio: Some(0.5),
            focus_new_pane: false
        }
    ));

    assert_eq!(layout.focused(), pane(4));
    assert!(layout.close_focused());
    assert_eq!(layout.focused(), pane(2));
}

#[test]
fn rolled_back_split_preserves_focus_return_path() {
    let mut layout = sample_layout();
    layout.focus_pane(pane(4));

    let new_id = layout
        .split_assigned(layout.focused(), SplitDirection::Right, 0.5)
        .expect("target exists");
    assert!(layout.close_pane(new_id));

    assert_eq!(layout.focused(), pane(4));
    assert!(layout.close_focused());
    assert_eq!(layout.focused(), pane(2));
}

#[test]
fn layout_geometry_rounds_halves_and_clamps_bounds() {
    for direction in [SplitDirection::Right, SplitDirection::Down] {
        let mut layout = TileLayout::new(pane(1));
        assert!(layout.insert_pane_near(
            pane(2),
            &SplitRequest {
                target: pane(1),
                direction: direction.clone(),
                ratio: None,
                focus_new_pane: false
            }
        ));
        for (extent, expected_first) in [(0, 0), (1, 1), (2, 1), (5, 3)] {
            assert_cell_extents(&layout, &direction, extent, expected_first);
        }
        let area = Rect::new(u16::MAX - 2, u16::MAX - 3, 10, 10);
        assert_eq!(area.width, 2);
        assert_eq!(area.height, 3);
        let panes = layout.panes(area);
        assert_eq!(panes[1].rect.right(), u16::MAX);
        assert_eq!(panes[1].rect.bottom(), u16::MAX);
    }
    assert_restored_negative_extent(SplitDirection::Right, (0, 6));
    assert_restored_negative_extent(SplitDirection::Down, (5, 0));
    let mut layout = sample_layout();
    layout.set_ratio_at(&[true, false], 0.9);
    assert_eq!(
        layout
            .splits(Rect::new(0, 0, 100, 40))
            .iter()
            .map(|s| s.path.clone())
            .collect::<Vec<_>>(),
        [vec![], vec![true], vec![true, true]]
    );
}

#[test]
fn layout_invalid_edits_preserve_state() {
    let mut layout = TileLayout::new(pane(1));
    let before = layout.clone();
    assert!(!layout.close_focused());
    assert!(!layout.close_pane(pane(99)));
    layout.focus_pane(pane(99));
    assert!(!layout.set_ratio_at(&[], 0.9));
    assert_eq!(layout, before);
    for (target, moved) in [(pane(1), pane(1)), (pane(99), pane(2)), (pane(99), pane(1))] {
        assert!(!layout.insert_pane_near(
            moved,
            &SplitRequest {
                target,
                direction: SplitDirection::Right,
                ratio: Some(0.5),
                focus_new_pane: true
            }
        ));
        assert_eq!(layout, before);
    }
    for (ratio, expected) in [
        (-1.0, 0.1),
        (2.0, 0.9),
        (f32::NAN, 0.5),
        (f32::INFINITY, 0.5),
    ] {
        assert_creation_ratio(&before, ratio, expected);
    }
}

#[test]
fn navigation_uses_distance_overlap_center_then_order() {
    let info = |id, rect| PaneInfo {
        id: pane(id),
        rect,
        inner_rect: rect,
        scrollbar_rect: None,
        borders: Borders::NONE,
        is_focused: id == 1,
    };
    let focused = info(1, Rect::new(10, 10, 10, 10));
    for (direction, rect) in [
        (NavDirection::Left, Rect::new(0, 10, 10, 10)),
        (NavDirection::Right, Rect::new(20, 10, 10, 10)),
        (NavDirection::Up, Rect::new(10, 0, 10, 10)),
        (NavDirection::Down, Rect::new(10, 20, 10, 10)),
    ] {
        assert_eq!(
            find_in_direction(&focused, direction, &[focused.clone(), info(2, rect)]),
            Some(pane(2))
        );
    }
    assert_eq!(
        find_in_direction(
            &focused,
            NavDirection::Left,
            &[
                focused.clone(),
                info(2, Rect::new(0, 20, 10, 3)),
                info(3, Rect::new(0, 0, 10, 5))
            ]
        ),
        None
    );
    let empty = info(1, Rect::new(10, 10, 0, 10));
    assert_eq!(
        find_in_direction(&empty, NavDirection::Left, std::slice::from_ref(&empty)),
        None
    );
    let near = info(2, Rect::new(0, 10, 10, 2));
    let far = info(3, Rect::new(0, 10, 8, 10));
    assert_eq!(
        find_in_direction(&focused, NavDirection::Left, &[far, near]),
        Some(pane(2))
    );
    let center_far = info(2, Rect::new(0, 9, 10, 5));
    let center_near = info(3, Rect::new(0, 16, 10, 4));
    assert_eq!(
        find_in_direction(&focused, NavDirection::Left, &[center_far, center_near]),
        Some(pane(3))
    );
    let first = info(4, Rect::new(0, 8, 10, 6));
    let second = info(2, Rect::new(0, 16, 10, 6));
    assert_eq!(
        find_in_direction(&focused, NavDirection::Left, &[first, second]),
        Some(pane(4))
    );
}

fn assert_cell_extents(
    layout: &TileLayout,
    direction: &SplitDirection,
    extent: u16,
    expected_first: u16,
) {
    let area = if *direction == SplitDirection::Right {
        Rect::new(7, 11, extent, 6)
    } else {
        Rect::new(7, 11, 6, extent)
    };
    let panes = layout.panes(area);
    let first = if *direction == SplitDirection::Right {
        panes[0].rect.width
    } else {
        panes[0].rect.height
    };
    let second = if *direction == SplitDirection::Right {
        panes[1].rect.width
    } else {
        panes[1].rect.height
    };
    assert_eq!(first, expected_first);
    assert_eq!(second, extent - expected_first);
    for info in panes {
        assert_eq!(info.inner_rect, info.rect);
        assert_eq!(info.scrollbar_rect, None);
        assert_eq!(info.borders, Borders::NONE);
    }
    let borders = layout.splits(area);
    assert_eq!(
        borders[0].pos,
        if *direction == SplitDirection::Right {
            7 + expected_first
        } else {
            11 + expected_first
        }
    );
    assert!(borders[0].path.is_empty());
}

fn assert_creation_ratio(before: &TileLayout, ratio: f32, expected: f32) {
    let mut layout = before.clone();
    assert!(layout.insert_pane_near(
        pane(2),
        &SplitRequest {
            target: pane(1),
            direction: SplitDirection::Right,
            ratio: Some(ratio),
            focus_new_pane: false
        }
    ));
    assert_eq!(
        layout.splits(Rect::new(0, 0, 100, 40))[0].ratio.to_bits(),
        expected.to_bits()
    );
    let inserted = layout.clone();
    assert!(!layout.insert_pane_near(
        pane(2),
        &SplitRequest {
            target: pane(1),
            direction: SplitDirection::Down,
            ratio: None,
            focus_new_pane: true
        }
    ));
    assert!(!layout.set_ratio_at(&[false], 0.2));
    assert_eq!(layout, inserted);
    assert!(layout.set_ratio_at(&[], 0.0));
    assert_eq!(
        layout.splits(Rect::new(0, 0, 100, 40))[0].ratio.to_bits(),
        0.1f32.to_bits()
    );
    assert!(layout.set_ratio_at(&[], 1.0));
    assert_eq!(
        layout.splits(Rect::new(0, 0, 100, 40))[0].ratio.to_bits(),
        0.9f32.to_bits()
    );
    assert!(!layout.resize_pane(pane(99), NavDirection::Left, 0.1, Rect::new(0, 0, 100, 40)));
}

fn assert_restored_negative_extent(direction: SplitDirection, expected: (u16, u16)) {
    let restored = TileLayout::from_saved(
        Node::Split {
            direction,
            ratio: -1.0,
            first: Box::new(Node::Pane(pane(1))),
            second: Box::new(Node::Pane(pane(2))),
        },
        pane(1),
    );
    let first = restored.panes(Rect::new(0, 0, 5, 6))[0].rect;
    assert_eq!((first.width, first.height), expected);
}

#[test]
fn spans_overlap_only_when_they_share_a_cell() {
    assert!(ranges_overlap(0, 4, 3, 4));
    assert!(ranges_overlap(3, 4, 0, 4));
    assert!(ranges_overlap(2, 1, 0, 8));
    assert!(ranges_overlap(0, 8, 2, 1));
}

#[test]
fn spans_that_only_touch_or_lie_apart_do_not_overlap() {
    assert!(!ranges_overlap(0, 4, 4, 4));
    assert!(!ranges_overlap(4, 4, 0, 4));
    assert!(!ranges_overlap(0, 4, 9, 2));
    assert!(!ranges_overlap(9, 2, 0, 4));
}

#[test]
fn spans_ending_past_the_coordinate_limit_saturate() {
    let top = u16::MAX;
    assert!(ranges_overlap(top - 1, 10, top - 3, 3));
    assert!(ranges_overlap(top - 3, 3, top - 1, 10));
    assert!(!ranges_overlap(top - 1, 10, top - 3, 2));
    assert!(!ranges_overlap(top - 3, 2, top - 1, 10));
}
