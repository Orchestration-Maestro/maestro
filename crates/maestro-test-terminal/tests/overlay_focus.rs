//! Overlay controls observed through focus flags, input and rendered frames.

#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod checks;
    pub mod components;
    pub mod manual_runtime;
    pub mod recording_terminal;
    pub mod scene;
    pub mod virtual_terminal;
}

use maestro_tui::OverlayOptions;
use std::cell::RefCell;
use std::rc::Rc;
use support::{components::Probe, scene::Scene};

#[test]
fn maestro_overlays_non_capturing_overlay_preserves_focus_on_creation() {
    let scene = Scene::new(20, 6);
    scene.start(&["editor"]);
    scene.tui.set_focus(Some(scene.probe.clone()));
    let overlay = Probe::shared(&["overlay"]);
    let options = Rc::new(RefCell::new(OverlayOptions {
        non_capturing: Some(true),
        ..OverlayOptions::default()
    }));
    let handle = scene
        .tui
        .show_overlay(overlay.clone(), Some(options))
        .unwrap();
    assert!(!handle.is_focused());
    scene.terminal.send_input("x");
    assert_eq!(*scene.probe.inputs.borrow(), ["x"]);
    assert!(overlay.inputs.borrow().is_empty());
    scene.stop();
}

/// Starts a writer with an editor input target.
fn focused_scene() -> Scene {
    let scene = Scene::new(20, 6);
    scene.start(&[]);
    scene.tui.set_focus(Some(scene.probe.clone()));
    scene
}

/// Shows one one-cell overlay, capturing unless requested otherwise.
fn show(
    scene: &Scene,
    label: &str,
    non_capturing: bool,
) -> std::io::Result<(Rc<Probe>, Box<dyn maestro_tui::OverlayHandle>)> {
    let probe = Probe::shared(&[label]);
    let handle = scene.tui.show_overlay(
        probe.clone(),
        Some(Rc::new(RefCell::new(OverlayOptions {
            width: Some(maestro_tui::SizeValue::Cells(1)),
            row: Some(maestro_tui::SizeValue::Cells(0)),
            col: Some(maestro_tui::SizeValue::Cells(0)),
            non_capturing: Some(non_capturing),
            ..OverlayOptions::default()
        }))),
    )?;
    Ok((probe, handle))
}

/// Observes the composited top cell after all requested renders complete.
fn top(scene: &Scene, expected: &str) {
    scene.render();
    assert_eq!(&scene.viewport()[0][..1], expected);
}

#[test]
fn maestro_overlays_focus_transfers_focus_to_the_overlay() {
    let scene = focused_scene();
    let (overlay, mut handle) = show(&scene, "A", true).unwrap();
    handle.focus();
    assert!(overlay.focus.get() && handle.is_focused());
    assert!(!scene.probe.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_unfocus_restores_previous_focus() {
    let scene = focused_scene();
    let (overlay, mut handle) = show(&scene, "A", true).unwrap();
    handle.focus();
    handle.unfocus();
    assert!(scene.probe.focus.get());
    assert!(!overlay.focus.get() && !handle.is_focused());
    scene.stop();
}

#[test]
fn maestro_overlays_sethidden_false_on_non_capturing_overlay_does_not_auto_focus() {
    let scene = focused_scene();
    let (overlay, mut handle) = show(&scene, "A", true).unwrap();
    handle.set_hidden(true);
    handle.set_hidden(false);
    assert!(scene.probe.focus.get() && !overlay.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_hide_when_overlay_is_not_focused_does_not_change_focus() {
    let scene = focused_scene();
    let (_, mut handle) = show(&scene, "A", true).unwrap();
    handle.hide().unwrap();
    assert!(scene.probe.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_hide_when_focused_restores_focus_correctly() {
    let scene = focused_scene();
    let (overlay, mut handle) = show(&scene, "A", true).unwrap();
    handle.focus();
    handle.hide().unwrap();
    assert!(scene.probe.focus.get() && !overlay.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_capturing_overlay_removed_with_non_capturing_below_restores_focus_to_editor() {
    let scene = focused_scene();
    let (noncapturing, _) = show(&scene, "N", true).unwrap();
    let (capturing, mut handle) = show(&scene, "C", false).unwrap();
    assert!(capturing.focus.get());
    handle.hide().unwrap();
    assert!(scene.probe.focus.get() && !noncapturing.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_sub_overlay_cleanup_then_hideoverlay_restores_focus_and_input_to_editor() {
    let scene = focused_scene();
    let (timer, mut handle) = show(&scene, "T", true).unwrap();
    let (controller, _) = show(&scene, "C", false).unwrap();
    assert!(controller.focus.get() && !scene.probe.focus.get());
    handle.hide().unwrap();
    scene.tui.hide_overlay().unwrap();
    scene.terminal.send_input("x");
    assert_eq!(*scene.probe.inputs.borrow(), ["x"]);
    assert!(!timer.focus.get() && !controller.focus.get());
    assert!(timer.inputs.borrow().is_empty() && controller.inputs.borrow().is_empty());
    scene.stop();
}

#[test]
fn maestro_overlays_deferred_sub_overlay_restores_focus() {
    use maestro_tui::tui::TuiRuntime;
    use std::cell::Cell;
    use std::time::Duration;
    let scene = focused_scene();
    let (timer, timer_handle) = show(&scene, "T", true).unwrap();
    let controller = Probe::shared(&["C"]);
    let owner = scene.tui.clone();
    let target = controller.clone();
    scene.runtime.schedule(
        Duration::ZERO,
        Box::new(move || {
            owner.show_overlay(target, None)?;
            Ok(())
        }),
    );
    scene.runtime.settle().unwrap();
    assert!(controller.focus.get());
    let completed = Rc::new(Cell::new(false));
    let witness = completed.clone();
    let owner = scene.tui.clone();
    let timer_handle = RefCell::new(timer_handle);
    controller.on_input(move |_| {
        timer_handle.borrow_mut().hide().unwrap();
        owner.hide_overlay().unwrap();
        witness.set(true);
    });
    scene.terminal.send_input("\x1b");
    assert!(completed.get());
    scene.terminal.send_input("x");
    assert_eq!(*scene.probe.inputs.borrow(), ["x"]);
    assert!(!timer.focus.get() && !controller.focus.get());
    controller.clear_callbacks();
    scene.stop();
}

#[test]
fn maestro_overlays_handleinput_redirection_skips_non_capturing_overlays_when_focused_overlay_becomes_invisible()
 {
    use std::cell::Cell;
    let scene = focused_scene();
    let (fallback, _) = show(&scene, "F", false).unwrap();
    let (noncapturing, _) = show(&scene, "N", true).unwrap();
    let visible = Rc::new(Cell::new(true));
    let flag = visible.clone();
    let primary = Probe::shared(&["P"]);
    scene
        .tui
        .show_overlay(
            primary.clone(),
            Some(Rc::new(RefCell::new(OverlayOptions {
                visible: Some(Rc::new(move |_, _| flag.get())),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    visible.set(false);
    scene.terminal.send_input("x");
    assert_eq!(*fallback.inputs.borrow(), ["x"]);
    assert!(primary.inputs.borrow().is_empty() && noncapturing.inputs.borrow().is_empty());
    scene.stop();
}

#[test]
fn maestro_overlays_hideoverlay_does_not_reassign_focus_when_topmost_overlay_is_non_capturing() {
    let scene = focused_scene();
    let (capturing, _) = show(&scene, "C", false).unwrap();
    show(&scene, "N", true).unwrap();
    scene.tui.hide_overlay().unwrap();
    assert!(capturing.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_multiple_capturing_and_non_capturing_overlays_restore_focus_through_removals() {
    let scene = focused_scene();
    let (first, mut first_handle) = show(&scene, "A", false).unwrap();
    show(&scene, "N", true).unwrap();
    let (second, mut second_handle) = show(&scene, "B", false).unwrap();
    show(&scene, "M", true).unwrap();
    assert!(second.focus.get());
    second_handle.hide().unwrap();
    assert!(first.focus.get());
    first_handle.hide().unwrap();
    assert!(scene.probe.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_capturing_overlay_unfocus_on_topmost_capturing_overlay_falls_back_to_prefocus()
{
    let scene = focused_scene();
    let (overlay, mut handle) = show(&scene, "C", false).unwrap();
    assert!(overlay.focus.get());
    handle.unfocus();
    assert!(scene.probe.focus.get() && !overlay.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_focus_on_hidden_overlay_is_a_no_op() {
    let scene = focused_scene();
    let (_, mut handle) = show(&scene, "A", true).unwrap();
    handle.set_hidden(true);
    handle.focus();
    assert!(scene.probe.focus.get() && !handle.is_focused());
    scene.stop();
}

#[test]
fn maestro_overlays_focus_after_hide_is_a_no_op() {
    let scene = focused_scene();
    let (_, mut handle) = show(&scene, "A", true).unwrap();
    handle.hide().unwrap();
    handle.focus();
    assert!(scene.probe.focus.get() && !handle.is_focused());
    scene.stop();
}

#[test]
fn maestro_overlays_unfocus_when_overlay_does_not_have_focus_is_a_no_op() {
    let scene = focused_scene();
    let (overlay, mut handle) = show(&scene, "A", true).unwrap();
    handle.unfocus();
    assert!(scene.probe.focus.get() && !overlay.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_unfocus_with_null_prefocus_clears_focus_and_does_not_route_input_back_to_overlay()
 {
    let scene = Scene::new(20, 6);
    scene.start(&[]);
    let (overlay, mut handle) = show(&scene, "A", false).unwrap();
    assert!(overlay.focus.get());
    handle.unfocus();
    scene.terminal.send_input("x");
    assert!(!overlay.focus.get() && !handle.is_focused());
    assert!(overlay.inputs.borrow().is_empty());
    scene.stop();
}

#[test]
fn maestro_overlays_toggle_focus_between_non_capturing_overlays_then_unfocus_returns_to_editor() {
    let scene = focused_scene();
    let (a, mut ah) = show(&scene, "A", true).unwrap();
    let (b, mut bh) = show(&scene, "B", true).unwrap();
    ah.focus();
    bh.focus();
    ah.focus();
    ah.unfocus();
    assert!(scene.probe.focus.get() && !a.focus.get() && !b.focus.get());
    scene.stop();
}

#[test]
fn maestro_overlays_focus_on_already_focused_overlay_bumps_visual_order() {
    let scene = focused_scene();
    let (_, mut a) = show(&scene, "A", true).unwrap();
    show(&scene, "B", true).unwrap();
    a.focus();
    show(&scene, "C", true).unwrap();
    top(&scene, "C");
    a.focus();
    top(&scene, "A");
    assert!(a.is_focused());
    scene.stop();
}

#[test]
fn maestro_overlays_default_rendering_order_for_overlapping_overlays_follows_creation_order() {
    let scene = focused_scene();
    show(&scene, "A", true).unwrap();
    show(&scene, "B", true).unwrap();
    top(&scene, "B");
    scene.stop();
}

#[test]
fn maestro_overlays_focus_on_lower_overlay_renders_it_on_top() {
    let scene = focused_scene();
    let (_, mut lower) = show(&scene, "A", true).unwrap();
    show(&scene, "B", true).unwrap();
    top(&scene, "B");
    lower.focus();
    top(&scene, "A");
    scene.stop();
}

#[test]
fn maestro_overlays_focusing_middle_overlay_places_it_on_top_while_preserving_others_relative_order()
 {
    let scene = focused_scene();
    show(&scene, "A", true).unwrap();
    let (_, mut middle) = show(&scene, "B", true).unwrap();
    let (_, mut upper) = show(&scene, "C", true).unwrap();
    top(&scene, "C");
    middle.focus();
    top(&scene, "B");
    middle.hide().unwrap();
    top(&scene, "C");
    upper.hide().unwrap();
    top(&scene, "A");
    scene.stop();
}

#[test]
fn maestro_overlays_capturing_overlay_hidden_and_shown_again_renders_on_top_after_unhide() {
    let scene = focused_scene();
    show(&scene, "A", true).unwrap();
    let (_, mut capturing) = show(&scene, "B", false).unwrap();
    top(&scene, "B");
    capturing.set_hidden(true);
    show(&scene, "C", true).unwrap();
    top(&scene, "C");
    capturing.set_hidden(false);
    top(&scene, "B");
    scene.stop();
}

#[test]
fn maestro_overlays_unfocus_does_not_change_visual_order_until_another_overlay_is_focused() {
    let scene = focused_scene();
    let (_, mut a) = show(&scene, "A", true).unwrap();
    let (_, mut b) = show(&scene, "B", true).unwrap();
    top(&scene, "B");
    a.focus();
    top(&scene, "A");
    a.unfocus();
    top(&scene, "A");
    b.focus();
    top(&scene, "B");
    scene.stop();
}

#[test]
fn overlay_removed_handles_cannot_mutate_focus_or_visibility() {
    let scene = focused_scene();
    let (_, mut handle) = show(&scene, "A", true).unwrap();
    handle.set_hidden(true);
    handle.hide().unwrap();
    scene.render();
    handle.set_hidden(false);
    handle.focus();
    handle.unfocus();
    handle.hide().unwrap();
    assert!(handle.is_hidden() && !handle.is_focused());
    assert_eq!(scene.runtime.pending(), 0);
    scene.terminal.send_input("x");
    assert_eq!(*scene.probe.inputs.borrow(), ["x"]);
    let (_, dropped) = show(&scene, "B", true).unwrap();
    drop(dropped);
    assert!(scene.tui.has_overlay());
    scene.stop();
}

#[test]
fn overlay_restoration_skips_unavailable_predecessors() {
    use std::cell::Cell;
    for unavailable in [
        "removed",
        "hidden",
        "invisible",
        "two-invisible",
        "null",
        "duplicate",
        "noncapturing",
    ] {
        let scene = focused_scene();
        if unavailable == "null" {
            scene.tui.set_focus(None);
        }
        let visible = Rc::new(Cell::new(true));
        let a = Probe::shared(&["A"]);
        let mut ah = visibility_overlay(
            &scene,
            a.clone(),
            visible.clone(),
            unavailable == "noncapturing",
        )
        .unwrap();
        if unavailable == "noncapturing" {
            ah.focus();
        }
        if unavailable == "two-invisible" {
            visibility_overlay(&scene, Probe::shared(&["M"]), visible.clone(), false).unwrap();
        }
        let mut bh = if unavailable == "duplicate" {
            scene.tui.show_overlay(a.clone(), None).unwrap()
        } else {
            show(&scene, "B", false).unwrap().1
        };
        match unavailable {
            "removed" | "duplicate" => ah.hide().unwrap(),
            "hidden" => ah.set_hidden(true),
            "invisible" | "two-invisible" | "null" => visible.set(false),
            _ => {}
        }
        bh.hide().unwrap();
        scene.terminal.send_input("x");
        match unavailable {
            "null" => {
                assert!(scene.probe.inputs.borrow().is_empty() && a.inputs.borrow().is_empty());
            }
            "noncapturing" => assert_eq!(*a.inputs.borrow(), ["x"]),
            _ => assert_eq!(*scene.probe.inputs.borrow(), ["x"], "{unavailable}"),
        }
        scene.stop();
    }
}

#[test]
fn overlay_visibility_observes_dimensions_and_short_circuits_hidden() {
    use std::cell::Cell;
    for allowed in [false, true] {
        let scene = focused_scene();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let visible = Rc::new(Cell::new(allowed));
        let flag = visible.clone();
        let mut handle = scene
            .tui
            .show_overlay(
                Probe::shared(&["A"]),
                Some(Rc::new(RefCell::new(OverlayOptions {
                    visible: Some(Rc::new(move |width, height| {
                        log.borrow_mut().push((width, height));
                        flag.get()
                    })),
                    ..OverlayOptions::default()
                }))),
            )
            .unwrap();
        assert_eq!(handle.is_focused(), allowed);
        handle.focus();
        assert_eq!(handle.is_focused(), allowed);
        handle.set_hidden(true);
        seen.borrow_mut().clear();
        assert!(!scene.tui.has_overlay());
        assert!(seen.borrow().is_empty());
        visible.set(true);
        handle.set_hidden(false);
        scene.resize(30, 7);
        seen.borrow_mut().clear();
        let tail = visibility_tail(&scene).unwrap();
        assert!(scene.tui.has_overlay());
        assert_eq!(*seen.borrow(), [(30, 7)]);
        assert_eq!(tail.get(), 0);
        handle.set_hidden(true);
        assert!(scene.tui.has_overlay());
        assert_eq!(tail.get(), 1);
        scene.stop();
    }
}

#[test]
fn overlay_options_remain_live_across_render_callbacks() {
    let scene = focused_scene();
    let options = Rc::new(RefCell::new(OverlayOptions {
        width: Some(maestro_tui::SizeValue::Cells(2)),
        max_height: Some(maestro_tui::SizeValue::Cells(1)),
        row: Some(maestro_tui::SizeValue::Cells(0)),
        col: Some(maestro_tui::SizeValue::Cells(0)),
        non_capturing: Some(true),
        ..OverlayOptions::default()
    }));
    let probe = Probe::shared(&["ABCD", "EFGH"]);
    let live = options.clone();
    probe.on_render(move |_| {
        let mut options = live.borrow_mut();
        options.width = Some(maestro_tui::SizeValue::Cells(4));
        options.row = Some(maestro_tui::SizeValue::Cells(3));
        options.col = Some(maestro_tui::SizeValue::Cells(5));
    });
    let mut handle = scene
        .tui
        .show_overlay(probe.clone(), Some(options.clone()))
        .unwrap();
    scene.render();
    assert_eq!(*probe.widths.borrow(), [2]);
    assert_eq!(scene.viewport()[3], "     AB");
    assert!(scene.viewport()[4].is_empty());
    options.borrow_mut().non_capturing = Some(false);
    handle.set_hidden(true);
    handle.set_hidden(false);
    assert!(handle.is_focused());
    probe.clear_callbacks();
    scene.stop();
}

#[test]
fn overlay_render_selection_captures_visible_entries() {
    let scene = focused_scene();
    let (a, _) = show(&scene, "A", true).unwrap();
    let (b, bh) = show(&scene, "B", true).unwrap();
    let bh = RefCell::new(bh);
    let owner = scene.tui.clone();
    let added = Probe::shared(&["C"]);
    let c = added.clone();
    a.on_render(move |count| {
        if count == 1 {
            bh.borrow_mut().hide().unwrap();
            owner.show_overlay(c.clone(), None).unwrap();
        }
    });
    scene.tui.request_render(true);
    scene
        .runtime
        .run_due()
        .into_iter()
        .for_each(|result| result.unwrap());
    assert_eq!(b.renders.get(), 1);
    assert_eq!(added.renders.get(), 0);
    scene.render();
    assert_eq!(b.renders.get(), 1);
    assert_eq!(added.renders.get(), 1);
    a.clear_callbacks();
    scene.stop();
    let scene = focused_scene();
    let appended = Probe::shared(&["N"]);
    let new = appended.clone();
    let owner = scene.tui.clone();
    let flag = std::cell::Cell::new(false);
    let first = scene
        .tui
        .show_overlay(
            Probe::shared(&["A"]),
            Some(Rc::new(RefCell::new(OverlayOptions {
                non_capturing: Some(true),
                visible: Some(Rc::new(move |_, _| {
                    if !flag.replace(true) {
                        owner.hide_overlay().unwrap();
                        owner.show_overlay(new.clone(), None).unwrap();
                    }
                    true
                })),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    show(&scene, "B", true).unwrap();
    scene.tui.request_render(true);
    scene
        .runtime
        .run_due()
        .into_iter()
        .for_each(|result| result.unwrap());
    assert_eq!(appended.renders.get(), 1);
    drop(first);
    scene.stop();
}

#[test]
fn overlay_invalidation_walks_live_entries() {
    let scene = focused_scene();
    let order = Rc::new(RefCell::new(Vec::new()));
    let log = order.clone();
    scene
        .probe
        .on_invalidate(move || log.borrow_mut().push("base"));
    let (a, mut ah) = show(&scene, "A", true).unwrap();
    ah.set_hidden(true);
    let (b, bh) = show(&scene, "B", true).unwrap();
    let log = order.clone();
    b.on_invalidate(move || log.borrow_mut().push("B"));
    let c = Probe::shared(&["C"]);
    let log = order.clone();
    c.on_invalidate(move || log.borrow_mut().push("C"));
    let log = order.clone();
    let owner = scene.tui.clone();
    let target = c.clone();
    let bh = RefCell::new(bh);
    a.on_invalidate(move || {
        log.borrow_mut().push("A");
        bh.borrow_mut().hide().unwrap();
        owner.show_overlay(target.clone(), None).unwrap();
    });
    scene.tui.invalidate();
    assert_eq!(*order.borrow(), ["base", "A", "C"]);
    assert_eq!(b.invalidated.get(), 0);
    a.clear_callbacks();
    b.clear_callbacks();
    c.clear_callbacks();
    scene.probe.clear_callbacks();
    let owner = scene.tui.clone();
    a.on_input(move |_| owner.invalidate());
    ah.focus();
    ah.set_hidden(false);
    ah.focus();
    scene.terminal.send_input("x");
    assert_eq!(a.invalidated.get(), 2);
    a.clear_callbacks();
    let owner = scene.tui.clone();
    a.on_render(move |_| owner.invalidate());
    scene.tui.show_overlay(a.clone(), None).unwrap();
    scene.render();
    assert_eq!(a.invalidated.get(), 6);
    a.clear_callbacks();
    scene.stop();
}

#[test]
fn overlay_capture_restore_uses_creation_order_not_visual_order() {
    let scene = focused_scene();
    let (a, mut ah) = show(&scene, "A", false).unwrap();
    let (b, mut bh) = show(&scene, "B", false).unwrap();
    ah.focus();
    top(&scene, "A");
    scene.tui.hide_overlay().unwrap();
    assert!(a.focus.get() && !b.focus.get());
    let (c, mut ch) = show(&scene, "C", false).unwrap();
    ah.focus();
    ah.set_hidden(true);
    assert!(c.focus.get());
    ch.hide().unwrap();
    assert!(scene.probe.focus.get());
    bh.focus();
    assert!(!bh.is_focused());
    scene.stop();
}

#[test]
fn overlay_handle_noops_preserve_hidden_state_and_render_requests() {
    let scene = focused_scene();
    let (_, mut handle) = show(&scene, "A", true).unwrap();
    scene.render();
    handle.set_hidden(false);
    handle.unfocus();
    assert_eq!(scene.runtime.pending(), 0);
    handle.set_hidden(true);
    assert!(handle.is_hidden());
    scene.render();
    handle.set_hidden(true);
    handle.focus();
    assert_eq!(scene.runtime.pending(), 0);
    handle.set_hidden(false);
    assert!(!handle.is_hidden());
    scene.render();
    handle.hide().unwrap();
    scene.render();
    handle.hide().unwrap();
    scene.tui.hide_overlay().unwrap();
    assert_eq!(scene.runtime.pending(), 0);
    let mut invisible = scene
        .tui
        .show_overlay(
            Probe::shared(&["I"]),
            Some(Rc::new(RefCell::new(OverlayOptions {
                visible: Some(Rc::new(|_, _| false)),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    scene.render();
    invisible.focus();
    assert_eq!(scene.runtime.pending(), 0);
    scene.stop();
}

#[test]
fn overlay_cursor_failures_keep_completed_state_changes() {
    use maestro_tui::{TUI, TerminalImage};
    use std::io::ErrorKind;
    use support::{manual_runtime::ManualRuntime, recording_terminal::RecordingTerminal};
    let terminal = RecordingTerminal::new(20, 6);
    let runtime = ManualRuntime::new();
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        None,
    );
    let base = Probe::shared(&[]);
    tui.set_focus(Some(base.clone()));
    terminal.set_hide_error(Some(ErrorKind::BrokenPipe));
    let overlay = Probe::shared(&["A"]);
    let Err(error) = tui.show_overlay(overlay.clone(), None) else {
        panic!("expected cursor error");
    };
    assert_eq!(error.kind(), ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "cursor failure");
    assert!(tui.has_overlay() && overlay.focus.get());
    assert_eq!(runtime.pending(), 0);
    assert_eq!(
        tui.hide_overlay().unwrap_err().kind(),
        ErrorKind::BrokenPipe
    );
    assert!(!tui.has_overlay() && base.focus.get());
    assert_eq!(runtime.pending(), 0);
    terminal.set_hide_error(None);
    let mut handle = tui.show_overlay(overlay, None).unwrap();
    runtime.settle().unwrap();
    terminal.clear_writes();
    terminal.set_hide_error(Some(ErrorKind::BrokenPipe));
    assert_eq!(handle.hide().unwrap_err().kind(), ErrorKind::BrokenPipe);
    assert!(!tui.has_overlay() && base.focus.get());
    assert_eq!(runtime.pending(), 0);
    assert!(terminal.writes().is_empty());
}

#[test]
fn overlay_focus_fallback_precedes_key_release_filtering() {
    use maestro_tui::tui::InputListenerResult;
    use std::cell::Cell;
    let scene = focused_scene();
    let visible = Rc::new(Cell::new(true));
    let calls = Rc::new(Cell::new(0));
    let flag = visible.clone();
    let count = calls.clone();
    let overlay = Probe::shared(&["A"]);
    overlay.wants_release.set(false);
    scene.probe.wants_release.set(true);
    scene
        .tui
        .show_overlay(
            overlay.clone(),
            Some(Rc::new(RefCell::new(OverlayOptions {
                visible: Some(Rc::new(move |_, _| {
                    count.set(count.get() + 1);
                    flag.get()
                })),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    calls.set(0);
    let remove = scene
        .tui
        .add_input_listener(Rc::new(|_| InputListenerResult::Consume));
    scene.terminal.send_input("x");
    remove();
    assert_eq!(calls.get(), 0);
    scene.terminal.send_input("\x1b[6;16;8t");
    assert_eq!(calls.get(), 0);
    scene.tui.set_on_debug(Some(Rc::new(|| {})));
    scene.terminal.send_input("\x1b[100;6u");
    assert_eq!(calls.get(), 0);
    visible.set(false);
    scene.terminal.send_input("\x1b[97;1:3u");
    assert_eq!(*scene.probe.inputs.borrow(), ["\x1b[97;1:3u"]);
    assert!(overlay.inputs.borrow().is_empty());
    reject_release_after_repair(&scene, &overlay);
    let owner = scene.tui.clone();
    let target = overlay;
    scene
        .probe
        .on_input(move |_| owner.set_focus(Some(target.clone())));
    scene.terminal.send_input("\x03");
    assert_eq!(scene.probe.inputs.borrow().last().unwrap(), "\x03");
    scene.probe.clear_callbacks();
    scene.stop();
}

#[test]
fn overlay_cursor_markers_follow_composition_without_repainting() {
    use maestro_tui::SizeValue;
    let scene = focused_scene();
    scene.tui.set_show_hardware_cursor(true).unwrap();
    scene.probe.set_lines(&[
        String::new(),
        String::new(),
        format!("abc{}def", maestro_tui::CURSOR_MARKER),
    ]);
    let overlay = Probe::shared(&["A"]);
    overlay.emits_marker.set(true);
    let options = Rc::new(RefCell::new(OverlayOptions {
        width: Some(SizeValue::Cells(5)),
        row: Some(SizeValue::Cells(2)),
        col: Some(SizeValue::Cells(3)),
        ..OverlayOptions::default()
    }));
    scene
        .tui
        .show_overlay(overlay.clone(), Some(options))
        .unwrap();
    scene.render();
    assert_eq!(scene.terminal.cursor(), (4, 2));
    assert!(
        scene
            .terminal
            .writes()
            .iter()
            .all(|write| !write.contains(maestro_tui::CURSOR_MARKER))
    );
    overlay.emits_marker.set(false);
    overlay.set_lines(&[format!("{}A", maestro_tui::CURSOR_MARKER)]);
    let before = scene.terminal.writes().len();
    scene.render();
    assert_eq!(scene.terminal.cursor(), (3, 2));
    assert!(
        scene.terminal.writes()[before..]
            .iter()
            .all(|write| !write.contains("\x1b[?2026h"))
    );
    scene.stop();
    clipped_cursor_markers();
}

#[test]
fn overlay_padding_ignores_highwater_and_suspends_clear_on_shrink() {
    let scene = focused_scene();
    scene.tui.set_clear_on_shrink(true);
    let (_, mut handle) = show(&scene, "A", true).unwrap();
    handle.set_hidden(true);
    scene.show(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    let redraws = scene.tui.full_redraws();
    scene.show(&["1", "2", "3", "4", "5"]);
    assert_eq!(scene.tui.full_redraws(), redraws);
    scene.resize(30, 6);
    assert_eq!(scene.viewport().len(), 6);
    assert!(scene.viewport()[0].starts_with('1'));
    let redraws = scene.tui.full_redraws();
    handle.hide().unwrap();
    scene.render();
    assert_eq!(scene.tui.full_redraws(), redraws + 1);
    scene.stop();
}

#[test]
fn overlay_handles_preserve_component_identity_when_owner_is_cloned() {
    let scene = focused_scene();
    let owner = scene.tui.clone();
    let same_a = Probe::shared(&["same"]);
    let same_b = Probe::shared(&["same"]);
    let mut a = owner.show_overlay(same_a.clone(), None).unwrap();
    let mut b = scene.tui.show_overlay(same_b.clone(), None).unwrap();
    a.focus();
    scene.terminal.send_input("a");
    b.focus();
    scene.terminal.send_input("b");
    assert_eq!(*same_a.inputs.borrow(), ["a"]);
    assert_eq!(*same_b.inputs.borrow(), ["b"]);
    let mut duplicate = owner.show_overlay(same_b, None).unwrap();
    duplicate.hide().unwrap();
    b.hide().unwrap();
    a.hide().unwrap();
    assert!(!owner.has_overlay());
    drop((a, b, duplicate));
    let mut retained = owner.show_overlay(same_a, None).unwrap();
    let terminal = scene.terminal.clone();
    drop(owner);
    drop(scene);
    retained.hide().unwrap();
    terminal.send_input("x");
    assert!(!retained.is_focused());
}

/// Shows a component whose visibility is controlled independently of its options.
fn visibility_overlay(
    scene: &Scene,
    probe: Rc<Probe>,
    flag: Rc<std::cell::Cell<bool>>,
    non_capturing: bool,
) -> std::io::Result<Box<dyn maestro_tui::OverlayHandle>> {
    scene.tui.show_overlay(
        probe,
        Some(Rc::new(RefCell::new(OverlayOptions {
            visible: Some(Rc::new(move |_, _| flag.get())),
            non_capturing: Some(non_capturing),
            ..OverlayOptions::default()
        }))),
    )
}

/// Proves release rejection uses the repaired recipient rather than the invisible one.
fn reject_release_after_repair(scene: &Scene, overlay: &Rc<Probe>) {
    scene.probe.wants_release.set(false);
    overlay.wants_release.set(true);
    scene.tui.set_focus(Some(overlay.clone()));
    scene.terminal.send_input("\x1b[97;1:3u");
    assert_eq!(*scene.probe.inputs.borrow(), ["\x1b[97;1:3u"]);
    assert!(overlay.inputs.borrow().is_empty());
}

/// Height clipping and a covering upper overlay both discard lower cursor markers.
fn clipped_cursor_markers() {
    use maestro_tui::{CURSOR_MARKER, SizeValue};
    let scene = focused_scene();
    support::checks::succeeds(scene.tui.set_show_hardware_cursor(true));
    let lower = Probe::shared(&[]);
    lower.set_lines(&[format!("{CURSOR_MARKER}L"), format!("X{CURSOR_MARKER}")]);
    let shown = scene.tui.show_overlay(
        lower,
        Some(Rc::new(RefCell::new(OverlayOptions {
            width: Some(SizeValue::Cells(1)),
            max_height: Some(SizeValue::Cells(1)),
            row: Some(SizeValue::Cells(0)),
            col: Some(SizeValue::Cells(0)),
            ..OverlayOptions::default()
        }))),
    );
    assert!(shown.is_ok());
    let upper = show(&scene, "U", false);
    assert!(upper.is_ok());
    scene.render();
    assert!(
        scene
            .terminal
            .writes()
            .iter()
            .all(|write| !write.contains(CURSOR_MARKER))
    );
    assert_eq!(
        scene.terminal.writes().last().map(String::as_str),
        Some("\x1b[?25l")
    );
    scene.stop();
}

/// Appends a noncapturing visibility callback whose calls are observable synchronously.
fn visibility_tail(scene: &Scene) -> std::io::Result<Rc<std::cell::Cell<usize>>> {
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    scene.tui.show_overlay(
        Probe::shared(&["B"]),
        Some(Rc::new(RefCell::new(OverlayOptions {
            non_capturing: Some(true),
            visible: Some(Rc::new(move |_, _| {
                count.set(count.get() + 1);
                true
            })),
            ..OverlayOptions::default()
        }))),
    )?;
    Ok(calls)
}

#[test]
fn overlay_focus_restoration_uses_post_callback_stack_slot() {
    for replacement_state in ["capturing", "noncapturing", "hidden", "invisible"] {
        let scene = focused_scene();
        let armed = Rc::new(std::cell::Cell::new(false));
        let replacement = Probe::shared(&["C"]);
        let replacement_handle = Rc::new(RefCell::new(None::<Box<dyn maestro_tui::OverlayHandle>>));
        let options = Rc::new(RefCell::new(OverlayOptions::default()));
        let original = Probe::shared(&["B"]);
        let removed = Rc::new(RefCell::new(
            scene
                .tui
                .show_overlay(original.clone(), Some(options.clone()))
                .unwrap(),
        ));
        let (_, mut top) = show(&scene, "A", false).unwrap();
        let calls = Rc::new(std::cell::Cell::new(0));
        let callback_calls = calls.clone();
        let tui = scene.tui.clone();
        let callback_armed = armed.clone();
        let callback_removed = removed.clone();
        let callback_replacement = replacement.clone();
        let callback_handle = replacement_handle.clone();
        options.borrow_mut().visible = Some(Rc::new(move |_, _| {
            callback_calls.set(callback_calls.get() + 1);
            if !callback_armed.replace(false) {
                return true;
            }
            callback_removed.borrow_mut().hide().unwrap();
            *callback_handle.borrow_mut() =
                Some(replacement_overlay(&tui, &callback_replacement, replacement_state).unwrap());
            true
        }));
        armed.set(true);
        top.hide().unwrap();
        assert_eq!(calls.get(), 1);
        let available = matches!(replacement_state, "capturing" | "noncapturing");
        assert_eq!(replacement.focus.get(), available);
        assert_eq!(scene.probe.focus.get(), !available);
        if available {
            scene.terminal.send_input("replacement");
            assert_eq!(*replacement.inputs.borrow(), ["replacement"]);
        } else {
            scene.terminal.send_input("fallback");
            assert!(replacement.inputs.borrow().is_empty());
            assert_eq!(*scene.probe.inputs.borrow(), ["fallback"]);
            scene.probe.inputs.borrow_mut().clear();
        }
        assert!(original.inputs.borrow().is_empty());
        replacement_handle
            .borrow_mut()
            .as_mut()
            .unwrap()
            .hide()
            .unwrap();
        scene.terminal.send_input("editor");
        assert_eq!(*scene.probe.inputs.borrow(), ["editor"]);
        scene.stop();
    }
}

#[test]
fn overlay_focus_candidates_cannot_commit_callback_removed_targets() {
    for operation in ["create", "unhide", "focus"] {
        let scene = focused_scene();
        let candidate = Probe::shared(&["candidate"]);
        let options = Rc::new(RefCell::new(OverlayOptions {
            non_capturing: Some(operation == "focus"),
            ..OverlayOptions::default()
        }));
        let callback = Rc::new({
            let tui = scene.tui.clone();
            move |_, _| {
                tui.hide_overlay().unwrap();
                true
            }
        });
        if operation == "create" {
            options.borrow_mut().visible = Some(callback.clone());
        }
        let mut handle = scene
            .tui
            .show_overlay(candidate.clone(), Some(options.clone()))
            .unwrap();
        if operation != "create" {
            if operation == "unhide" {
                handle.set_hidden(true);
            }
            options.borrow_mut().visible = Some(callback);
            if operation == "unhide" {
                handle.set_hidden(false);
            } else {
                handle.focus();
            }
        }
        scene.terminal.send_input("editor");
        assert!(candidate.inputs.borrow().is_empty(), "{operation}");
        assert_eq!(*scene.probe.inputs.borrow(), ["editor"], "{operation}");
        scene.stop();
    }
}

#[test]
fn overlay_predecessor_rechecks_callback_hidden_or_removed_identity() {
    for remove in [false, true] {
        let scene = focused_scene();
        let candidate = Probe::shared(&["candidate"]);
        let options = Rc::new(RefCell::new(OverlayOptions {
            non_capturing: Some(true),
            ..OverlayOptions::default()
        }));
        let handle = Rc::new(RefCell::new(
            scene
                .tui
                .show_overlay(candidate.clone(), Some(options.clone()))
                .unwrap(),
        ));
        handle.borrow_mut().focus();
        let (_, mut top) = show(&scene, "top", true).unwrap();
        top.focus();
        let calls = Rc::new(std::cell::Cell::new(0));
        options.borrow_mut().visible = Some(Rc::new({
            let callback = unavailable_callback(handle.clone(), remove);
            let calls = calls.clone();
            move |width, height| {
                calls.set(calls.get() + 1);
                callback(width, height)
            }
        }));
        top.hide().unwrap();
        assert!(scene.probe.focus.get());
        assert_eq!(calls.get(), 1);
        scene.terminal.send_input("editor");
        assert_eq!(calls.get(), 1);
        assert!(candidate.inputs.borrow().is_empty());
        assert_eq!(*scene.probe.inputs.borrow(), ["editor"]);
        scene.stop();
    }
}

#[test]
fn overlay_visibility_query_returns_callback_observation_after_self_removal() {
    let scene = focused_scene();
    let options = Rc::new(RefCell::new(OverlayOptions {
        non_capturing: Some(true),
        ..OverlayOptions::default()
    }));
    scene
        .tui
        .show_overlay(Probe::shared(&["overlay"]), Some(options.clone()))
        .unwrap();
    options.borrow_mut().visible = Some(Rc::new({
        let tui = scene.tui.clone();
        move |_, _| {
            tui.hide_overlay().unwrap();
            true
        }
    }));
    assert!(scene.tui.has_overlay());
    assert!(!scene.tui.has_overlay());
    scene.stop();
}

/// Makes a retained predecessor unavailable during its visibility observation.
fn unavailable_callback(
    handle: Rc<RefCell<Box<dyn maestro_tui::OverlayHandle>>>,
    remove: bool,
) -> Rc<dyn Fn(usize, usize) -> bool> {
    Rc::new(move |_, _| {
        if remove {
            assert!(handle.borrow_mut().hide().is_ok());
        } else {
            handle.borrow_mut().set_hidden(true);
        }
        true
    })
}

#[test]
fn overlay_captured_component_survives_removal_of_its_original_alias() {
    let scene = focused_scene();
    let component = Probe::shared(&["X"]);
    let options = || {
        Some(Rc::new(RefCell::new(OverlayOptions {
            non_capturing: Some(true),
            ..OverlayOptions::default()
        })))
    };
    let mut original = scene
        .tui
        .show_overlay(component.clone(), options())
        .unwrap();
    original.focus();
    let mut alias = scene
        .tui
        .show_overlay(component.clone(), options())
        .unwrap();
    let (_, mut capturing) = show(&scene, "C", false).unwrap();
    original.hide().unwrap();
    capturing.hide().unwrap();
    assert!(alias.is_focused());
    scene.terminal.send_input("alias");
    assert_eq!(*component.inputs.borrow(), ["alias"]);
    alias.hide().unwrap();
    assert!(scene.probe.focus.get());
    scene.terminal.send_input("editor");
    assert_eq!(*scene.probe.inputs.borrow(), ["editor"]);
    scene.stop();
}

#[test]
fn overlay_input_resamples_false_visibility_without_excluding_its_entry() {
    let scene = focused_scene();
    let candidate = Probe::shared(&["X"]);
    let calls = Rc::new(std::cell::Cell::new(0));
    let options = Rc::new(RefCell::new(OverlayOptions::default()));
    let handle = scene
        .tui
        .show_overlay(candidate.clone(), Some(options.clone()))
        .unwrap();
    options.borrow_mut().visible = Some(Rc::new({
        let calls = calls.clone();
        move |_, _| {
            let call = calls.get();
            calls.set(call + 1);
            call != 0
        }
    }));
    scene.terminal.send_input("resampled");
    assert!(handle.is_focused());
    assert_eq!(calls.get(), 2);
    assert_eq!(*candidate.inputs.borrow(), ["resampled"]);
    assert!(scene.probe.inputs.borrow().is_empty());
    scene.stop();
}

#[test]
fn overlay_top_walk_stops_when_an_accepted_slot_is_vacated() {
    let scene = focused_scene();
    let lower_calls = Rc::new(std::cell::Cell::new(0));
    let lower_options = Rc::new(RefCell::new(OverlayOptions::default()));
    let lower = Probe::shared(&["D"]);
    scene
        .tui
        .show_overlay(lower.clone(), Some(lower_options.clone()))
        .unwrap();
    lower_options.borrow_mut().visible = Some(Rc::new({
        let calls = lower_calls.clone();
        move |_, _| {
            calls.set(calls.get() + 1);
            true
        }
    }));
    let options = Rc::new(RefCell::new(OverlayOptions::default()));
    let middle = Rc::new(RefCell::new(
        scene
            .tui
            .show_overlay(Probe::shared(&["B"]), Some(options.clone()))
            .unwrap(),
    ));
    let external = Probe::shared(&["Q"]);
    scene.tui.set_focus(Some(external.clone()));
    let (_, mut top) = show(&scene, "A", false).unwrap();
    let calls = Rc::new(std::cell::Cell::new(0));
    options.borrow_mut().visible = Some(Rc::new({
        let middle = middle.clone();
        let calls = calls.clone();
        move |_, _| {
            calls.set(calls.get() + 1);
            middle.borrow_mut().hide().unwrap();
            true
        }
    }));
    top.hide().unwrap();
    assert!(external.focus.get());
    assert_eq!(calls.get(), 1);
    assert_eq!(lower_calls.get(), 0);
    scene.terminal.send_input("external");
    assert_eq!(*external.inputs.borrow(), ["external"]);
    assert!(lower.inputs.borrow().is_empty());
    scene.stop();
}

#[test]
fn overlay_input_keeps_the_first_alias_visibility_trigger() {
    for hidden in [false, true] {
        let scene = focused_scene();
        let candidate = Probe::shared(&["X"]);
        let options = Rc::new(RefCell::new(OverlayOptions {
            non_capturing: Some(true),
            ..OverlayOptions::default()
        }));
        let mut original = scene
            .tui
            .show_overlay(candidate.clone(), Some(options.clone()))
            .unwrap();
        original.focus();
        let alias = scene
            .tui
            .show_overlay(
                candidate.clone(),
                Some(Rc::new(RefCell::new(OverlayOptions {
                    non_capturing: Some(true),
                    ..OverlayOptions::default()
                }))),
            )
            .unwrap();
        let (_, mut top) = show(&scene, "C", false).unwrap();
        let calls = Rc::new(std::cell::Cell::new(0));
        if hidden {
            original.set_hidden(true);
        }
        options.borrow_mut().visible = Some(Rc::new({
            let calls = calls.clone();
            move |_, _| {
                calls.set(calls.get() + 1);
                false
            }
        }));
        top.hide().unwrap();
        assert!(alias.is_focused());
        assert_eq!(calls.get(), usize::from(!hidden));
        if hidden {
            scene.tui.set_focus(Some(candidate.clone()));
        }
        scene.terminal.send_input("editor");
        assert!(scene.probe.focus.get());
        assert_eq!(calls.get(), 2 * usize::from(!hidden));
        assert_eq!(*scene.probe.inputs.borrow(), ["editor"]);
        assert!(candidate.inputs.borrow().is_empty());
        scene.stop();
    }
}

#[test]
fn overlay_creation_retains_its_component_when_callback_replaces_the_entry() {
    let scene = focused_scene();
    let candidate = Probe::shared(&["X"]);
    let calls = Rc::new(std::cell::Cell::new(0));
    let options = Rc::new(RefCell::new(OverlayOptions {
        visible: Some(Rc::new({
            let tui = scene.tui.clone();
            let candidate = candidate.clone();
            let calls = calls.clone();
            move |_, _| {
                calls.set(calls.get() + 1);
                tui.hide_overlay().unwrap();
                tui.show_overlay(
                    candidate.clone(),
                    Some(Rc::new(RefCell::new(OverlayOptions {
                        non_capturing: Some(true),
                        ..OverlayOptions::default()
                    }))),
                )
                .unwrap();
                true
            }
        })),
        ..OverlayOptions::default()
    }));
    let detached = scene
        .tui
        .show_overlay(candidate.clone(), Some(options))
        .unwrap();
    assert!(detached.is_focused());
    assert_eq!(calls.get(), 1);
    scene.terminal.send_input("alias");
    assert_eq!(*candidate.inputs.borrow(), ["alias"]);
    scene.stop();
}

#[test]
fn overlay_same_label_is_not_a_component_availability_witness() {
    let scene = focused_scene();
    let (original, mut handle) = show(&scene, "X", true).unwrap();
    handle.focus();
    let (decoy, _) = show(&scene, "X", true).unwrap();
    let (_, mut top) = show(&scene, "C", false).unwrap();
    handle.hide().unwrap();
    top.hide().unwrap();
    assert!(scene.probe.focus.get());
    scene.terminal.send_input("editor");
    assert_eq!(*scene.probe.inputs.borrow(), ["editor"]);
    assert!(original.inputs.borrow().is_empty());
    assert!(decoy.inputs.borrow().is_empty());
    scene.stop();
}

/// Appends the replacement selected by a visibility callback.
fn replacement_overlay(
    tui: &maestro_tui::TUI,
    probe: &Rc<Probe>,
    state: &str,
) -> std::io::Result<Box<dyn maestro_tui::OverlayHandle>> {
    let options = Rc::new(RefCell::new(OverlayOptions {
        non_capturing: Some(state != "capturing"),
        ..OverlayOptions::default()
    }));
    let mut handle = tui.show_overlay(probe.clone(), Some(options.clone()))?;
    if state == "invisible" {
        options.borrow_mut().visible = Some(Rc::new(|_, _| false));
    }
    if state == "hidden" {
        handle.set_hidden(true);
    }
    Ok(handle)
}
