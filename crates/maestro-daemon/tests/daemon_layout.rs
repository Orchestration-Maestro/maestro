//! Served pane layouts: one projection of the view area, the chrome and the layout events.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support::api::{
    TestApp, add_workspaces, daemon_with, daemon_with_workspaces, root_pane, split_pane,
};
use maestro_daemon::settings::model::Config;
use maestro_protocol::events::EventData;
use maestro_protocol::schema::{PaneDirection, PaneLayoutRect, SplitDirection};
use maestro_protocol::shared::Flag;
use maestro_workspace::layout::{PaneId, Rect};

/// The public handle of a pane in the first workspace.
fn public(app: &TestApp, pane: PaneId) -> String {
    app.state.public_pane_id(0, pane).unwrap()
}

/// A rectangle record in the snapshot's own vocabulary.
fn cells(x: u16, y: u16, width: u16, height: u16) -> PaneLayoutRect {
    PaneLayoutRect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn the_view_area_is_the_headless_surface_until_a_view_supplies_one() {
    let mut config = Config::default();
    assert_eq!(
        daemon_with(&config).state.view_area(),
        Rect::new(0, 0, 120, 40)
    );
    config.server.headless_cols = 30;
    config.server.headless_rows = 10;
    assert_eq!(
        daemon_with(&config).state.view_area(),
        Rect::new(0, 0, 30, 10)
    );
}

#[test]
fn a_layout_snapshot_lists_public_ids_cells_and_dividers() {
    let mut app = daemon_with_workspaces(&["/work"]);
    let root = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, root, SplitDirection::Right);
    let snapshot = app.pane_layout_snapshot(0, 0).unwrap();
    assert_eq!(snapshot.workspace_id, app.state.topology.workspaces[0].id);
    assert_eq!(snapshot.tab_id, app.state.public_tab_id(0, 0).unwrap());
    assert!(!snapshot.zoomed);
    assert_eq!(snapshot.area, cells(0, 0, 120, 40));
    assert_eq!(snapshot.focused_pane_id, public(&app, root));
    let panes: Vec<_> = snapshot
        .panes
        .iter()
        .map(|pane| (pane.pane_id.clone(), pane.focused, pane.rect))
        .collect();
    assert_eq!(
        panes,
        [
            (public(&app, root), true, cells(0, 0, 60, 40)),
            (public(&app, right), false, cells(60, 0, 60, 40)),
        ]
    );
    assert_eq!(snapshot.splits.len(), 1);
    assert_eq!(snapshot.splits[0].id, "split_0_root");
    assert_eq!(snapshot.splits[0].direction, SplitDirection::Right);
    assert!((snapshot.splits[0].ratio - 0.5).abs() < f32::EPSILON);
    assert_eq!(snapshot.splits[0].rect, cells(0, 0, 120, 40));
}

#[test]
fn nested_dividers_are_named_by_their_index_and_path() {
    let mut app = daemon_with_workspaces(&["/work"]);
    let first = root_pane(&app, 0);
    let second = split_pane(&mut app, 0, first, SplitDirection::Right);
    split_pane(&mut app, 0, second, SplitDirection::Down);
    split_pane(&mut app, 0, second, SplitDirection::Right);
    let snapshot = app.pane_layout_snapshot(0, 0).unwrap();
    let ids: Vec<_> = snapshot
        .splits
        .iter()
        .map(|split| split.id.as_str())
        .collect();
    assert_eq!(ids, ["split_0_root", "split_1_1", "split_2_10"]);
}

#[test]
fn a_layout_snapshot_applies_the_accepted_chrome() {
    let mut config = Config::default();
    config.ui.pane_borders = Flag(false);
    let mut app = daemon_with(&config);
    add_workspaces(&mut app, &["/work"]);
    let root = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, root, SplitDirection::Right);
    let snapshot = app.pane_layout_snapshot(0, 0).unwrap();
    let rects: Vec<_> = snapshot.panes.iter().map(|pane| pane.rect).collect();
    assert_eq!(rects, [cells(0, 0, 59, 40), cells(60, 0, 60, 40)]);
    assert_eq!(snapshot.panes[1].pane_id, public(&app, right));
}

#[test]
fn a_layout_snapshot_reports_focus_and_zoom_of_its_tab() {
    let mut app = daemon_with_workspaces(&["/work"]);
    let root = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, root, SplitDirection::Right);
    app.state.topology.workspaces[0].tabs[0]
        .layout
        .focus_pane(right);
    app.state.topology.workspaces[0].tabs[0].zoomed = true;
    let snapshot = app.pane_layout_snapshot(0, 0).unwrap();
    assert!(snapshot.zoomed);
    assert_eq!(snapshot.focused_pane_id, public(&app, right));
    assert!(snapshot.panes[1].focused && !snapshot.panes[0].focused);
}

#[test]
fn no_snapshot_exists_for_a_missing_workspace_or_tab() {
    let app = daemon_with_workspaces(&["/work"]);
    assert!(app.pane_layout_snapshot(0, 0).is_some());
    assert_eq!(app.pane_layout_snapshot(0, 1), None);
    assert_eq!(app.pane_layout_snapshot(1, 0), None);
}

#[test]
fn a_one_cell_view_still_yields_a_snapshot() {
    let mut config = Config::default();
    config.server.headless_cols = 1;
    config.server.headless_rows = 1;
    let mut app = daemon_with(&config);
    add_workspaces(&mut app, &["/work"]);
    let root = root_pane(&app, 0);
    split_pane(&mut app, 0, root, SplitDirection::Right);
    let snapshot = app.pane_layout_snapshot(0, 0).unwrap();
    assert_eq!(snapshot.area, cells(0, 0, 1, 1));
    assert_eq!(snapshot.panes.len(), 2);
    assert_eq!(snapshot.splits.len(), 1);
}

#[test]
fn a_layout_update_publishes_the_complete_snapshot_once() {
    let mut app = daemon_with_workspaces(&["/work"]);
    let root = root_pane(&app, 0);
    split_pane(&mut app, 0, root, SplitDirection::Down);
    let expected = app.pane_layout_snapshot(0, 0).unwrap();
    app.emit_layout_updated_event(0, 0);
    assert_eq!(
        app.event_hub.events_after(0),
        [(1, EventData::LayoutUpdated { layout: expected })]
    );
}

#[test]
fn a_layout_update_for_a_missing_tab_publishes_nothing() {
    let app = daemon_with_workspaces(&["/work"]);
    app.emit_layout_updated_event(0, 4);
    app.emit_layout_updated_event(3, 0);
    assert!(app.event_hub.events_after(0).is_empty());
}

#[test]
fn a_removal_reports_its_tab_only_while_panes_remain() {
    let mut app = daemon_with_workspaces(&["/work"]);
    let root = root_pane(&app, 0);
    assert_eq!(app.layout_update_target_after_pane_removal(0, root), None);
    let right = split_pane(&mut app, 0, root, SplitDirection::Right);
    assert_eq!(
        app.layout_update_target_after_pane_removal(0, right),
        Some((0, 0))
    );
    assert_eq!(
        app.layout_update_target_after_pane_removal(0, PaneId::from_raw(999)),
        None
    );
}

#[test]
fn directional_targets_follow_the_served_geometry() {
    let mut app = daemon_with_workspaces(&["/work"]);
    let root = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, root, SplitDirection::Right);
    let below = split_pane(&mut app, 0, right, SplitDirection::Down);
    let target = |source, direction| app.directional_pane_target(0, 0, source, direction);
    assert_eq!(target(root, PaneDirection::Right), Some(right));
    assert_eq!(target(below, PaneDirection::Left), Some(root));
    assert_eq!(target(right, PaneDirection::Down), Some(below));
    assert_eq!(target(below, PaneDirection::Up), Some(right));
    assert_eq!(target(root, PaneDirection::Left), None);
    assert_eq!(target(root, PaneDirection::Up), None);
    assert_eq!(target(PaneId::from_raw(999), PaneDirection::Right), None);
}
