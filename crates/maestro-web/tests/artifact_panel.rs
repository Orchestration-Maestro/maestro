//! Retained artifact panel through its public application and native host seams.
#![cfg(all(test, not(target_arch = "wasm32")))]
use crate::artifact_support;
use crate::canvas_paint;
use crate::session_header_support;
use crate::support;
use artifact_support::panel::{Harness, Input, control, corpus, tab, text};
use canvas_paint::Paint;
use std::rc::Rc;

/// Disconnected and empty panels do not paint or admit a modal.
#[test]
fn artifact_panel_hidden_states_draw_nothing() {
    for populated in [false, true] {
        let entries = if populated {
            vec![("a.zip".into(), "a.zip".into())]
        } else {
            vec![]
        };
        for collapsed in [false, true] {
            for overlay in [false, true] {
                let harness = Harness::new(&Input {
                    entries: entries.clone(),
                    active: populated.then(|| "a.zip".into()),
                    collapsed,
                    overlay,
                    ..Default::default()
                });
                harness.draw().unwrap();
                let output = harness.draw().unwrap();
                let hidden = !populated || collapsed;
                assert_eq!(text(&output).is_empty(), hidden);
                assert_eq!(harness.visible().is_empty(), hidden);
                assert_eq!(
                    harness.host.overlays.borrow().is_empty(),
                    hidden || !overlay
                );
                assert_eq!(
                    harness
                        .frame
                        .context
                        .memory(|m| m.top_modal_layer().is_none()),
                    hidden || !overlay
                );
            }
        }
    }
}

/// Pointer activation reaches the application and reenters the real retained panel.
#[test]
fn artifact_panel_tab_click_reaches_application_selection() {
    let harness = Harness::new(&Input {
        entries: vec![
            ("z.zip".into(), "z.zip".into()),
            ("a.zip".into(), "a.zip".into()),
        ],
        active: Some("z.zip".into()),
        ..Default::default()
    });
    let output = harness.draw().unwrap();
    harness.click(tab(&output, "a.zip").center());
    assert_eq!(harness.caller.active_filename().as_deref(), Some("a.zip"));
    harness.draw().unwrap();
    assert_eq!(harness.visible().len(), 1);
    assert_eq!(harness.presentation.opened.get(), 0);
}

/// Recorded aliases and duplicate names keep literal ordered text, underlines, border and close title.
#[test]
fn artifact_panel_tabs_keep_literal_record_order() {
    for case in corpus().render {
        let mut harness = Harness::new(&case.input);
        harness.frame.size = egui::vec2(2400., 300.);
        let paint = observe_paint(&mut harness);
        harness.draw().unwrap();
        let output = harness.draw().unwrap();
        assert_eq!(harness.caller.active_filename(), case.output.active);
        assert_render(&harness, &case.output, &output);
        if !case.output.hidden {
            assert_eq!(
                left_border(&harness, &output, &paint.borrow()),
                case.output.inline_border.unwrap()
            );
            assert_eq!(
                close_labels(&harness, &output),
                case.output.header_text.unwrap()
            );
        }
        // A malformed selected original distinguishes hidden evaluation from visible paint.
        for (key, _) in &case.input.entries {
            if case.input.operation.as_deref() != Some("missing-view") {
                harness
                    .panel
                    .get_or_create_artifact_element(key, "%")
                    .unwrap();
            }
        }
        assert_eq!(harness.draw().is_err(), case.output.header_rendered);
    }
}

/// Retain the actual renderer texture updates for final paint sampling.
fn observe_paint(harness: &mut Harness) -> Rc<std::cell::RefCell<Paint>> {
    let paint = Rc::new(std::cell::RefCell::new(Paint::default()));
    let retained = paint.clone();
    harness.frame.paint = Some(Box::new(move |output| retained.borrow_mut().update(output)));
    paint
}

/// Whether the shell paints its one-pixel border-colored left edge.
fn left_border(harness: &Harness, output: &egui::FullOutput, paint: &Paint) -> bool {
    let presentation = harness.frame.pack.presentation(harness.frame.mode).unwrap();
    let border = egui::Color32::from_hex(presentation.colors["border"]).unwrap();
    let bounds = harness.frame.context.content_rect();
    let pixel = paint.pixel(
        &harness.frame.context,
        &output.shapes,
        egui::pos2(bounds.left() + 0.5, bounds.center().y),
    );
    pixel
        .into_iter()
        .zip(border.to_array())
        .all(|(actual, expected)| (actual - f32::from(expected)).abs() < 1.)
}

/// The border observer rejects displacement, final clipping and later opaque paint.
#[test]
fn artifact_panel_left_border_requires_visible_edge_paint() {
    let mut harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    let paint = observe_paint(&mut harness);
    harness.draw().unwrap();
    let original = harness.draw().unwrap();
    assert!(left_border(&harness, &original, &paint.borrow()));
    for mutation in ["displaced", "clipped", "covered"] {
        let mut output = original.clone();
        for shape in &mut output.shapes {
            let egui::Shape::LineSegment { points, .. } = &mut shape.shape else {
                continue;
            };
            if points[0].x.to_bits() != points[1].x.to_bits() {
                continue;
            }
            if mutation == "displaced" {
                points[0].x += 100.;
                points[1].x += 100.;
            } else if mutation == "clipped" {
                shape.clip_rect = egui::Rect::NOTHING;
            }
        }
        if mutation == "covered" {
            let bounds = harness.frame.context.content_rect();
            output.shapes.push(egui::epaint::ClippedShape {
                clip_rect: bounds,
                shape: egui::Shape::rect_filled(bounds, 0., egui::Color32::BLACK),
            });
        }
        assert!(
            !left_border(&harness, &output, &paint.borrow()),
            "{mutation}"
        );
    }
}

/// Accessible labels the actual close control, the last icon, reports when activated.
fn close_labels(harness: &Harness, output: &egui::FullOutput) -> Vec<String> {
    let close = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.calc_bounds().center()),
            _ => None,
        })
        .next_back()
        .unwrap();
    let activated = harness
        .frame
        .draw_panel(
            &harness.panel,
            &harness.caller,
            support::pointer::click(close),
        )
        .unwrap();
    activated
        .platform_output
        .events
        .iter()
        .filter_map(|event| event.widget_info().label.clone())
        .collect()
}

/// Assert the source shell's exact visible text and active treatment.
fn assert_render(
    harness: &Harness,
    expected: &artifact_support::panel::RenderOutput,
    output: &egui::FullOutput,
) {
    if expected.hidden {
        assert!(
            output
                .shapes
                .iter()
                .all(|shape| matches!(shape.shape, egui::Shape::Noop))
        );
        return;
    }
    let spans = text(output);
    assert_eq!(
        spans
            .iter()
            .map(|span| span.galley.job.text.as_str())
            .collect::<Vec<_>>(),
        expected
            .tabs
            .iter()
            .map(|tab| tab.text.as_str())
            .collect::<Vec<_>>()
    );
    let presentation = harness.frame.pack.presentation(harness.frame.mode).unwrap();
    for (span, tab) in spans.iter().zip(&expected.tabs) {
        assert!(
            harness
                .caller
                .artifacts()
                .read()
                .unwrap()
                .values()
                .any(|record| record.read().unwrap().filename == tab.filename)
        );
        let role = if tab.active { "accent" } else { "muted" };
        let color = span.override_text_color.unwrap_or(span.fallback_color);
        assert_eq!(
            format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b()),
            presentation.colors[role].to_lowercase()
        );
    }
    assert_eq!(output.shapes.iter().filter(|shape| matches!(&shape.shape,egui::Shape::LineSegment {stroke,..} if stroke.width.to_bits()==2_f32.to_bits())).count(),expected.tabs.iter().filter(|tab| tab.active).count());
}

/// Selected headers preserve original bytes; empty and unknown selections supply none.
#[test]
fn artifact_panel_active_header_comes_from_selected_view() {
    let harness = Harness::new(&Input {
        entries: vec![
            ("a.zip".into(), "a.zip".into()),
            ("b.zip".into(), "b.zip".into()),
        ],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    harness
        .panel
        .get_or_create_artifact_element("a.zip", "AAEC")
        .unwrap();
    harness
        .panel
        .get_or_create_artifact_element("b.zip", "/w==")
        .unwrap();
    for (name, bytes) in [("a.zip", vec![0, 1, 2]), ("b.zip", vec![255])] {
        harness.caller.show_artifact(name);
        let output = harness.draw().unwrap();
        harness.click(control(&output, 1));
        assert_eq!(
            harness.host.downloads.borrow().last().unwrap(),
            &(name.into(), "application/zip".into(), bytes)
        );
    }
    for name in ["", "missing.zip"] {
        harness.caller.show_artifact(name);
        let output = harness.draw().unwrap();
        assert_eq!(
            output
                .shapes
                .iter()
                .filter(|shape| matches!(shape.shape, egui::Shape::Mesh(_)))
                .count(),
            1
        );
        assert!(harness.visible().is_empty());
    }
}

/// Source operation cases distinguish selecting, host opening and optional close.
#[test]
fn artifact_panel_open_and_close_notifications_stay_distinct() {
    for case in corpus().operations {
        let harness = Harness::new(&case.input);
        let before = harness.draw().unwrap();
        let roots: Vec<_> = harness.host.published.borrow().keys().copied().collect();
        match case.input.operation.as_deref().unwrap() {
            "tab" => harness.click(tab(&before, "a.zip").center()),
            "open-existing" => harness.caller.open_artifact("a.zip"),
            "open-missing" => harness.caller.open_artifact("absent.zip"),
            "close-handler" | "close-absent" => harness.click(control(&before, 0)),
            _ => unreachable!(),
        }
        harness.draw().unwrap();
        assert_eq!(harness.caller.active_filename(), case.output.active);
        assert_eq!(harness.presentation.opened.get(), case.output.opened);
        assert_eq!(harness.closed.get(), case.output.closed);
        assert_visible(&harness, &harness.caller, &case.output.visible);
        assert!(!harness.panel.collapsed());
        assert!(
            roots
                .iter()
                .all(|id| harness.host.published.borrow().contains_key(id))
        );
    }
    let harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    let weak = Rc::downgrade(&harness.panel);
    harness.panel.set_on_close(Some(Rc::new(move || {
        weak.upgrade().unwrap().set_on_close(None);
    })));
    let output = harness.draw().unwrap();
    harness.click(control(&output, 0));
    let output = harness.draw().unwrap();
    harness.click(control(&output, 0));
}

/// Pair actual body identity with the reached original-file header, not mere presence.
fn assert_visible(
    harness: &Harness,
    caller: &maestro_app::browser::tools::artifacts::LocalArtifactsPanel,
    expected: &[String],
) {
    let visible = harness.visible();
    assert_eq!(visible.len(), expected.len());
    if expected.is_empty() {
        return;
    }
    let output = harness
        .frame
        .draw_panel(&harness.panel, caller, vec![])
        .unwrap();
    let published = harness
        .host
        .events
        .borrow()
        .iter()
        .rev()
        .find_map(|event| match event {
            session_header_support::Event::Publish(id) => Some(*id),
            _ => None,
        })
        .unwrap();
    assert_eq!(visible, [published]);
    harness
        .frame
        .draw_panel(
            &harness.panel,
            caller,
            support::pointer::click(control(&output, 1)),
        )
        .unwrap();
    assert_eq!(
        [harness.host.downloads.borrow().last().unwrap().0.clone()],
        expected
    );
}

/// Smooth native scrolling centers literal targets rather than a selector or decoy.
#[test]
fn artifact_panel_scroll_targets_literal_tabs() {
    for case in corpus().literal_scroll {
        let mut entries: Vec<_> = (0..12)
            .map(|index| {
                let name = format!("long-leading-name-{index}.zip");
                (name.clone(), name)
            })
            .collect();
        entries.push((case.input.filename.clone(), case.input.filename.clone()));
        entries.extend((0..12).map(|index| {
            let name = format!("long-trailing-name-{index}.zip");
            (name.clone(), name)
        }));
        let harness = Harness::new(&Input {
            entries,
            ..Default::default()
        });
        let first = harness.draw().unwrap();
        assert!(
            !text(&first)
                .iter()
                .any(|span| span.galley.job.text == case.input.filename)
        );
        harness.caller.show_artifact(&case.input.filename);
        let (output, positions) = settle(&harness, &case.input.filename);
        let target = tab(&output, &case.input.filename);
        let strip = output
            .shapes
            .iter()
            .find(|shape| matches!(&shape.shape, egui::Shape::Text(span) if span.galley.job.text == case.input.filename))
            .unwrap()
            .clip_rect;
        let options = &case.output.scroll[0].options;
        assert_eq!(positions > 1, options.behavior == "smooth");
        assert_eq!(
            (target.center().x - strip.center().x).abs() < 1.,
            options.inline == "center"
        );
        assert!(target.center().x > 30. && target.center().x < 260.);
        assert_eq!(
            harness.caller.active_filename().as_deref(),
            Some(case.output.active.as_str())
        );
        assert_eq!(harness.visible().len(), case.output.visible.len());
        assert!(case.output.error.is_none());
        assert_eq!(case.output.scroll.len(), 1);
        assert_eq!(case.output.scroll[0].filename, case.input.filename);
    }
}

/// Draw until a requested scroll settles, counting the target's distinct drawn positions.
fn settle(harness: &Harness, filename: &str) -> (egui::FullOutput, usize) {
    let mut output = harness.draw().unwrap();
    let mut positions = vec![];
    for _ in 0..120 {
        positions.extend(
            text(&output)
                .iter()
                .filter(|span| span.galley.job.text == filename)
                .map(|span| span.pos.x.to_bits()),
        );
        output = harness.draw().unwrap();
    }
    positions.dedup();
    (output, positions.len())
}

/// Reconnection restores first keys without rebuilding any already published factory.
#[test]
fn artifact_panel_reconnect_retains_views_and_first_key() {
    for case in corpus().reconnect {
        let harness = Harness::new(&case.input);
        // Publish both real views before restoring the recorded selection.
        for (key, _) in &case.input.entries {
            harness.panel.show_artifact(key).unwrap();
            harness.caller.show_artifact(key);
            harness.draw().unwrap();
        }
        if let Some(active) = &case.input.active {
            harness.caller.show_artifact(active);
        } else {
            // A fresh caller can own the same records with absent selection.
            let empty = maestro_app::browser::tools::artifacts::LocalArtifactsPanel::new_local(
                harness.presentation.clone(),
            );
            *empty.artifacts().write().unwrap() =
                harness.caller.artifacts().read().unwrap().clone();
            reconnect(&harness, &empty, &case.output);
            continue;
        }
        reconnect(&harness, &harness.caller, &case.output);
    }
}

/// Disconnect and reconnect with actual surviving factory handles.
fn reconnect(
    harness: &Harness,
    caller: &maestro_app::browser::tools::artifacts::LocalArtifactsPanel,
    expected: &artifact_support::panel::Output,
) {
    let before: Vec<_> = harness
        .host
        .published
        .borrow()
        .iter()
        .map(|(id, (root, _))| (*id, Rc::downgrade(root)))
        .collect();
    harness.panel.disconnected_callback().unwrap();
    assert!(harness.visible().is_empty());
    harness.panel.connected_callback(caller).unwrap();
    harness.panel.connected_callback(caller).unwrap();
    harness
        .frame
        .draw_panel(&harness.panel, caller, vec![])
        .unwrap();
    assert_eq!(caller.active_filename(), expected.active);
    assert_visible(harness, caller, &expected.visible);
    assert_eq!(harness.presentation.opened.get(), expected.opened);
    assert_eq!(harness.closed.get(), expected.closed);
    let same = before.iter().all(|(id, root)| {
        root.upgrade()
            .is_some_and(|root| Rc::ptr_eq(&root, &harness.host.published.borrow()[id].0))
    });
    assert_eq!(Some(same), expected.same_roots);
}

/// Tool edits keep the published owner while original-byte downloads change.
#[test]
fn artifact_panel_updates_reuse_views_and_original_downloads() {
    let harness = Harness::new(&Input::default());
    harness.panel.disconnected_callback().unwrap();
    harness.command("create", "a.zip", Some("AAEC"));
    assert!(harness.visible().is_empty());
    harness.panel.connected_callback(&harness.caller).unwrap();
    let output = harness.draw().unwrap();
    let (id, root) = harness
        .host
        .published
        .borrow()
        .iter()
        .map(|(id, (root, _))| (*id, Rc::downgrade(root)))
        .next()
        .unwrap();
    harness.click(control(&output, 1));
    assert_eq!(harness.host.downloads.borrow().last().unwrap().2, [0, 1, 2]);
    artifact_support::presentation::ready((harness.caller.tool().execute)("call".into(),serde_json::json!({"command":"update","filename":"a.zip","old_str":"AAEC","new_str":"/w=="}),None,None)).unwrap();
    harness.command("rewrite", "a.zip", Some("Yg=="));
    let output = harness.draw().unwrap();
    assert!(Rc::ptr_eq(
        &root.upgrade().unwrap(),
        &harness.host.published.borrow()[&id].0
    ));
    harness.click(control(&output, 1));
    assert_eq!(harness.host.downloads.borrow().last().unwrap().2, [98]);
    assert!(
        harness
            .panel
            .get_or_create_artifact_element("a.docx", "AAEC")
            .is_err()
    );
    assert_eq!(harness.host.retained.borrow().len(), 1);
}

/// The tool owns fallback and removes only the deleted view and its registration.
#[test]
fn artifact_panel_deleting_selected_file_uses_shared_fallback() {
    let harness = Harness::new(&Input::default());
    for (name, bytes) in [("z.zip", "eg=="), ("a.zip", "YQ=="), ("m.zip", "bQ==")] {
        harness.command("create", name, Some(bytes));
        harness.draw().unwrap();
    }
    let output = harness.draw().unwrap();
    harness.click(tab(&output, "a.zip").center());
    harness.draw().unwrap();
    harness.command("delete", "a.zip", None);
    assert_eq!(harness.caller.active_filename().as_deref(), Some("z.zip"));
    let output = harness.draw().unwrap();
    harness.click(control(&output, 1));
    assert_eq!(harness.host.downloads.borrow().last().unwrap().0, "z.zip");
    assert_eq!(harness.host.downloads.borrow().last().unwrap().2, [122]);
    assert_eq!(harness.host.retained.borrow().len(), 2);
    harness.command("delete", "m.zip", None);
    assert_eq!(harness.caller.active_filename().as_deref(), Some("z.zip"));
    harness.command("delete", "z.zip", None);
    assert_eq!(harness.caller.active_filename(), None);
    let output = harness.draw().unwrap();
    assert!(text(&output).is_empty());
    assert!(harness.host.published.borrow().is_empty());
}

/// Real toolkit modal admission blocks underlying controls without a painted backdrop.
#[test]
fn artifact_panel_modes_use_toolkit_modal_admission() {
    let harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    harness.panel.set_overlay(true);
    draw_underlying(&harness, vec![]);
    let (_, output) = draw_underlying(&harness, vec![]);
    assert!(
        harness
            .frame
            .context
            .memory(|m| m.top_modal_layer().is_some())
    );
    assert_eq!(harness.host.overlays.borrow().len(), 1);
    let pos = egui::pos2(40., 16.);
    assert!(!draw_underlying(&harness, support::pointer::click(pos)).0);
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Rect(rect) if rect.fill == egui::Color32::from_black_alpha(100))));
    harness.panel.set_overlay(false);
    draw_underlying(&harness, vec![]);
    draw_underlying(&harness, vec![]);
    assert!(harness.host.overlays.borrow().is_empty());
    assert!(draw_underlying(&harness, support::pointer::click(pos)).0);
    harness.panel.set_overlay(true);
    harness.draw().unwrap();
    harness.panel.disconnected_callback().unwrap();
    assert!(harness.host.overlays.borrow().is_empty());
    assert!(harness.visible().is_empty());
}

/// Draw a real underlying control before the panel to exercise layer admission.
fn draw_underlying(harness: &Harness, events: Vec<egui::Event>) -> (bool, egui::FullOutput) {
    let presentation = harness.frame.pack.presentation(harness.frame.mode).unwrap();
    let mut clicked = false;
    let mut output = harness.frame.context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                harness.frame.size,
            )),
            events,
            ..Default::default()
        },
        |ui| {
            clicked = ui.button("Underlying").clicked();
            harness
                .panel
                .render(
                    ui,
                    &harness.caller,
                    maestro_web::tools::artifacts::artifact_element::ArtifactView {
                        highlighter: &crate::highlighter_support::NoHighlight,
                        i18n: &harness.frame.i18n,
                        presentation: &presentation,
                    },
                )
                .unwrap();
        },
    );
    output.textures_delta.clear();
    (clicked, output)
}

/// Accessible close titles use exact shipped translations and observe live dictionary edits.
#[test]
fn artifact_panel_close_title_uses_live_localization() {
    for language in ["en", "de"] {
        let harness = Harness::new(&Input {
            entries: vec![("a.zip".into(), "a.zip".into())],
            active: Some("a.zip".into()),
            language: language.into(),
            ..Default::default()
        });
        let output = harness.draw().unwrap();
        harness.click(control(&output, 0));
        let title = if language == "de" {
            "Artefakte schließen"
        } else {
            "Close artifacts"
        };
        assert!(
            close_labels(&harness, &output)
                .iter()
                .any(|label| label == title)
        );
        harness.frame.i18n.get_translations().borrow()["en"]
            .borrow_mut()
            .insert(
                "Close artifacts".into(),
                maestro_web::TranslationValue::Text("Live close".into()),
            );
        assert!(
            close_labels(&harness, &output)
                .iter()
                .any(|label| label == "Live close")
        );
    }
}

/// Supplied packs own panel paint, underlines and both source shadow layers.
#[test]
fn artifact_panel_colors_follow_caller_packs() {
    for file in ["brand.json", "packs/signal.json"] {
        for case in corpus().shell_shadow {
            let mut harness = Harness::new(&Input {
                entries: vec![
                    ("a.zip".into(), "a.zip".into()),
                    ("b.zip".into(), "b.zip".into()),
                ],
                active: Some("a.zip".into()),
                ..Default::default()
            });
            harness.frame.pack = maestro_theme::load_brand_pack(&format!(
                "{}/../../assets/brand/{file}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap();
            harness.frame.mode = if case.input.dark {
                maestro_theme::BrandMode::Dark
            } else {
                maestro_theme::BrandMode::Light
            };
            let presentation = harness.frame.pack.presentation(harness.frame.mode).unwrap();
            let css = presentation.css_properties();
            for overlay in [false, true] {
                harness.panel.set_overlay(overlay);
                harness.draw().unwrap();
                let output = harness.draw().unwrap();
                assert_eq!(harness.host.css.borrow().last(), Some(&css));
                assert_shadows(&harness, &output, &case.output.layers);
                assert_tab_interaction_colors(&harness, &output);
            }
        }
    }
}

/// Observe inactive hover text and the focused tab outline through actual input.
fn assert_tab_interaction_colors(harness: &Harness, output: &egui::FullOutput) {
    let presentation = harness.frame.pack.presentation(harness.frame.mode).unwrap();
    let pos = tab(output, "b.zip").center();
    let output = harness
        .frame
        .draw_panel(
            &harness.panel,
            &harness.caller,
            vec![egui::Event::PointerMoved(pos)],
        )
        .unwrap();
    let span = text(&output)
        .into_iter()
        .find(|span| span.galley.job.text == "b.zip")
        .unwrap();
    assert_eq!(
        span.override_text_color.unwrap_or(span.fallback_color),
        egui::Color32::from_hex(presentation.colors["text"]).unwrap()
    );
    assert!(matches!(
        span.galley.job.sections[0].format.font_id.family,
        egui::FontFamily::Monospace
    ));
    let accent = egui::Color32::from_hex(presentation.colors["accent"]).unwrap();
    assert_tab_focus(harness, pos, accent);
    assert_eq!(harness.caller.active_filename().as_deref(), Some("a.zip"));
}

/// Traverse the four real shell controls and observe the inactive tab's focus ring.
fn assert_tab_focus(harness: &Harness, pos: egui::Pos2, accent: egui::Color32) {
    let mut focused = false;
    for _ in 0..4 {
        harness
            .frame
            .draw_panel(
                &harness.panel,
                &harness.caller,
                vec![
                    egui::Event::PointerGone,
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            )
            .unwrap();
        let output = harness
            .frame
            .draw_panel(
                &harness.panel,
                &harness.caller,
                vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            )
            .unwrap();
        focused |= output.shapes.iter().any(|shape| {
            matches!(&shape.shape,
            egui::Shape::Rect(rect) if rect.rect.contains(pos) && rect.stroke.color == accent)
        });
    }
    assert!(focused);
}

/// Failed publication and hide preserve the surviving owner for a later retry.
#[test]
fn artifact_panel_failures_preserve_owners_and_retry() {
    use session_header_support::Failure;
    let harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    harness.draw().unwrap();
    let (id, root) = harness
        .host
        .published
        .borrow()
        .iter()
        .map(|(id, (root, _))| (*id, Rc::downgrade(root)))
        .next()
        .unwrap();
    harness.host.failure.set(Some(Failure::Publish));
    assert!(harness.draw().is_err());
    assert!(Rc::ptr_eq(
        &root.upgrade().unwrap(),
        &harness.host.published.borrow()[&id].0
    ));
    harness.host.failure.set(None);
    harness.host.fail_visibility.set(true);
    harness.panel.set_collapsed(true);
    assert!(harness.draw().is_err());
    assert!(Rc::ptr_eq(
        &root.upgrade().unwrap(),
        &harness.host.published.borrow()[&id].0
    ));
    harness.host.fail_visibility.set(false);
    harness.draw().unwrap();
    assert!(harness.visible().is_empty());
    harness
        .panel
        .get_or_create_artifact_element("a.zip", "bad!")
        .unwrap();
    assert!(harness.draw().is_err());
    assert!(harness.host.downloads.borrow().is_empty());
    harness
        .panel
        .get_or_create_artifact_element("a.zip", "Yg==")
        .unwrap();
    harness.panel.set_collapsed(false);
    harness.draw().unwrap();
    assert!(Rc::ptr_eq(
        &root.upgrade().unwrap(),
        &harness.host.published.borrow()[&id].0
    ));
    assert!(
        harness
            .panel
            .get_or_create_artifact_element("a.docx", "Yg==")
            .is_err()
    );
    assert_eq!(harness.host.retained.borrow().len(), 1);
}

/// Final owner teardown releases captures and registrations, not independent views.
#[test]
fn artifact_panel_final_drop_releases_only_owned_views() {
    let harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    let host = harness.host.clone();
    let weak = Rc::downgrade(&harness.panel);
    let presentation = Rc::downgrade(&harness.presentation);
    let captured = Rc::new(());
    let captured_weak = Rc::downgrade(&captured);
    harness.panel.set_on_close(Some(Rc::new(move || {
        let _ = &captured;
    })));
    let mut unrelated = maestro_web::tools::artifacts::create_artifact_element(
        "unrelated.zip",
        "AA==",
        host.clone(),
    )
    .unwrap();
    harness
        .frame
        .draw(unrelated.as_mut(), false, vec![])
        .unwrap();
    let independent = *host.published.borrow().keys().next().unwrap();
    harness.panel.set_overlay(true);
    harness.draw().unwrap();
    assert_eq!(host.published.borrow().len(), 2);
    let tool = harness.caller.tool();
    drop(harness);
    assert!(weak.upgrade().is_some());
    drop(tool);
    assert!(weak.upgrade().is_none());
    assert!(presentation.upgrade().is_none());
    assert!(captured_weak.upgrade().is_none());
    assert_eq!(host.published.borrow().len(), 1);
    assert!(host.published.borrow().contains_key(&independent));
    assert!(host.overlays.borrow().is_empty());
    drop(unrelated);
    assert!(host.published.borrow().is_empty());
}

/// Compare actual native shadow geometry with both recorded visible layers.
fn assert_shadows(
    harness: &Harness,
    output: &egui::FullOutput,
    layers: &[artifact_support::panel::Shadow],
) {
    let presentation = harness.frame.pack.presentation(harness.frame.mode).unwrap();
    let body = harness.host.published.borrow().values().next().unwrap().1;
    let bounds = egui::Rect::from_min_max(
        egui::pos2(
            body.left() - if harness.panel.overlay() { 0. } else { 1. },
            body.top() - 41.,
        ),
        body.max,
    );
    assert_eq!(bounds.bottom().to_bits(), 300_f32.to_bits());
    let shadows: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.blur_width > 0. => Some(rect),
            _ => None,
        })
        .collect();
    assert_eq!(shadows.len(), layers.len());
    for (actual, expected) in shadows.iter().zip(layers) {
        assert_eq!(actual.blur_width.to_bits(), expected.blur.to_bits());
        assert_eq!(
            actual.rect.width().to_bits(),
            (bounds.width() + 2. * expected.spread).to_bits()
        );
        assert_eq!(
            actual.rect.left().to_bits(),
            (bounds.left() - expected.spread + expected.offset[0]).to_bits()
        );
        assert_eq!(
            actual.rect.top().to_bits(),
            (bounds.top() - expected.spread + expected.offset[1]).to_bits()
        );
        assert_eq!(
            actual.fill.a(),
            egui::Color32::from_hex(presentation.colors["border"])
                .unwrap()
                .gamma_multiply(expected.opacity)
                .a()
        );
    }
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::LineSegment {stroke,..} if stroke.width.to_bits()==2_f32.to_bits())));
}

/// A host callback sees the existing conflict error, not a borrow panic.
#[test]
fn artifact_panel_reentrant_view_access_returns_conflict() {
    let harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    let panel = Rc::downgrade(&harness.panel);
    let called = Rc::new(std::cell::Cell::new(false));
    let observed = called.clone();
    *harness.host.on_visibility.borrow_mut() = Some(Rc::new(move || {
        assert!(matches!(
            panel.upgrade().unwrap().show_artifact("a.zip"),
            Err(maestro_web::tools::artifacts::ArtifactUiError::BorrowConflict)
        ));
        observed.set(true);
    }));
    harness.panel.show_artifact("a.zip").unwrap();
    assert!(called.get());
    harness.host.on_visibility.borrow_mut().take();
    harness.draw().unwrap();
    assert_eq!(harness.visible().len(), 1);
}

/// Unwinding through a host callback restores the original retained view.
#[test]
fn artifact_panel_callback_unwind_restores_view() {
    let harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    harness.draw().unwrap();
    let (id, root) = harness
        .host
        .published
        .borrow()
        .iter()
        .map(|(id, (root, _))| (*id, root.clone()))
        .next()
        .unwrap();
    *harness.host.on_visibility.borrow_mut() = Some(Rc::new(|| panic!("controlled callback")));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        harness.panel.show_artifact("a.zip").unwrap();
    }));
    assert!(result.is_err());
    harness.host.on_visibility.borrow_mut().take();
    harness
        .panel
        .get_or_create_artifact_element("a.zip", "Yg==")
        .unwrap();
    harness.panel.show_artifact("a.zip").unwrap();
    harness.draw().unwrap();
    assert!(Rc::ptr_eq(&root, &harness.host.published.borrow()[&id].0));
    assert_eq!(harness.visible(), vec![id]);
}

/// Restoring an in-flight view never reinserts an entry removed by its callback.
#[test]
fn artifact_panel_callback_removal_does_not_resurrect_view() {
    let harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    harness.draw().unwrap();
    let original = *harness.host.published.borrow().keys().next().unwrap();
    let panel = Rc::downgrade(&harness.panel);
    *harness.host.on_visibility.borrow_mut() = Some(Rc::new(move || {
        panel.upgrade().unwrap().remove_artifact_element("a.zip");
    }));
    harness.panel.show_artifact("a.zip").unwrap();
    harness.host.on_visibility.borrow_mut().take();
    harness.draw().unwrap();
    assert!(harness.host.published.borrow().is_empty());
    assert!(harness.host.retained.borrow().is_empty());
    harness
        .panel
        .get_or_create_artifact_element("a.zip", "Yg==")
        .unwrap();
    harness.draw().unwrap();
    assert_eq!(harness.host.published.borrow().len(), 1);
    assert!(!harness.host.published.borrow().contains_key(&original));
}

/// Visible labels retain horizontal padding, including a clickable empty filename.
#[test]
fn artifact_panel_tabs_preserve_padding_and_empty_hit_area() {
    let harness = Harness::new(&Input {
        entries: vec![
            ("a.zip".into(), "a.zip".into()),
            ("b.zip".into(), "b.zip".into()),
            (String::new(), String::new()),
        ],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    harness.draw().unwrap();
    let output = harness.draw().unwrap();
    let a = tab(&output, "a.zip");
    let b = tab(&output, "b.zip");
    let padding: f32 = num_traits::cast(
        harness
            .frame
            .pack
            .presentation(harness.frame.mode)
            .unwrap()
            .spacing["md"],
    )
    .unwrap();
    assert!((b.left() - a.right() - 2. * padding).abs() < 1.);
    harness.click(egui::pos2(b.right() + 2. * padding, b.center().y));
    assert_eq!(harness.caller.active_filename().as_deref(), Some(""));
}

/// CSS whitespace presentation stays on one row without changing selection identity.
#[test]
fn artifact_panel_filename_whitespace_is_single_row_and_literal_selection() {
    let name = " \tline\r\n  break\x0c.zip  ";
    let mut harness = Harness::new(&Input {
        entries: vec![
            (name.into(), name.into()),
            ("line break .zip".into(), "line break .zip".into()),
            ("a\u{a0} b.zip".into(), "a\u{a0} b.zip".into()),
        ],
        active: Some("line break .zip".into()),
        ..Default::default()
    });
    harness.frame.size = egui::vec2(1000., 300.);
    harness.draw().unwrap();
    let output = harness.draw().unwrap();
    let spans = text(&output);
    assert!(spans.iter().all(|span| span.galley.rows.len() == 1));
    assert_eq!(spans[0].galley.size(), spans[1].galley.size());
    assert_eq!(spans[0].galley.job.text, "line break .zip");
    assert_eq!(spans[2].galley.job.text, "a\u{a0} b.zip");
    harness.click(egui::Rect::from_min_size(spans[0].pos, spans[0].galley.size()).center());
    assert_eq!(harness.caller.active_filename().as_deref(), Some(name));
    let output = harness.draw().unwrap();
    harness.click(control(&output, 1));
    assert_eq!(harness.host.downloads.borrow().last().unwrap().0, name);
}

/// Tab traversal follows visible source order, ending at the right-aligned close.
#[test]
fn artifact_panel_keyboard_traverses_tabs_header_then_close() {
    for max_passes in [1, 2] {
        let harness = Harness::new(&Input {
            entries: vec![
                ("a.zip".into(), "a.zip".into()),
                ("b.zip".into(), "b.zip".into()),
            ],
            active: Some("a.zip".into()),
            close: true,
            ..Default::default()
        });
        harness
            .frame
            .context
            .options_mut(|options| options.max_passes = max_passes.try_into().unwrap());
        let output = harness.draw().unwrap();
        let download = control(&output, 1);
        let close = control(&output, 0);
        assert!((close.x - (harness.frame.size.x - 16.)).abs() < 1.);
        assert!(download.x < close.x);
        harness.click(close);
        assert_eq!(harness.closed.get(), 1);
        for index in 0..4 {
            let mut current = harness.draw().unwrap();
            for pressed in [true, false] {
                current = harness
                    .frame
                    .draw_panel(
                        &harness.panel,
                        &harness.caller,
                        vec![egui::Event::Key {
                            key: egui::Key::Tab,
                            physical_key: None,
                            pressed,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    )
                    .unwrap();
            }
            let point = match index {
                0 => tab(&current, "a.zip").center(),
                1 => tab(&current, "b.zip").center(),
                2 => control(&current, 1),
                _ => control(&current, 0),
            };
            let id = harness.frame.context.memory(egui::Memory::focused).unwrap();
            let response = harness.frame.context.read_response(id).unwrap();
            assert!(
                response.rect.contains(point),
                "expected focus at {point:?}, got {:?}",
                response.rect
            );
        }
    }
}

/// Header evaluation follows toolkit passes, without a separate measuring call.
#[test]
fn artifact_panel_header_runs_once_per_pass_and_sizing_is_local() {
    let mut harness = Harness::new(&Input {
        entries: vec![("a.zip".into(), "a.zip".into())],
        active: Some("a.zip".into()),
        ..Default::default()
    });
    let language = Rc::new(support::Host::default());
    harness.frame.i18n = maestro_web::I18n::new(language.clone()).unwrap();
    harness
        .frame
        .i18n
        .get_translations()
        .borrow_mut()
        .get_mut("en")
        .unwrap()
        .borrow_mut()
        .shift_remove("Download");
    for _ in 0..2 {
        language.diagnostics.borrow_mut().clear();
        let before = harness.frame.context.cumulative_pass_nr();
        let output = harness.draw().unwrap();
        let passes = harness.frame.context.cumulative_pass_nr() - before;
        assert_eq!(language.diagnostics.borrow().len() as u64, passes);
        assert_eq!(passes, 1);
        assert!((control(&output, 0).x - (harness.frame.size.x - 16.)).abs() < 1.);
    }
    let mut other = Harness::new(&Input {
        entries: vec![("b.zip".into(), "b.zip".into())],
        ..Default::default()
    });
    other.frame.context = harness.frame.context.clone();
    other.draw().unwrap();
    let before = harness.frame.context.cumulative_pass_nr();
    harness.draw().unwrap();
    assert_eq!(harness.frame.context.cumulative_pass_nr() - before, 1);
}

/// Cold overflow uses the actual remaining viewport, including a changed header width.
#[test]
fn artifact_panel_cold_selection_centers_after_header_sizing() {
    for max_passes in [1, 2] {
        for warm_empty_header in [false, true] {
            let entries: Vec<_> = (0..25)
                .map(|index| {
                    let name = format!("long-artifact-name-{index}.zip");
                    (name.clone(), name)
                })
                .collect();
            let harness = Harness::new(&Input {
                entries,
                active: (!warm_empty_header).then(|| "long-artifact-name-12.zip".into()),
                ..Default::default()
            });
            harness.frame.context.options_mut(|options| {
                options.max_passes = max_passes.try_into().unwrap();
            });
            if warm_empty_header {
                harness.draw().unwrap();
                harness.draw().unwrap();
                harness.caller.show_artifact("long-artifact-name-12.zip");
            }
            let mut output = harness.draw().unwrap();
            assert_first_frame_controls(&harness, &output);
            for _ in 0..120 {
                output = harness.draw().unwrap();
            }
            let target = tab(&output, "long-artifact-name-12.zip");
            // The source centers within the tab viewport, not the whole panel.
            assert!((target.center().x - 164.).abs() < 1., "{target:?}");
        }
    }
}

/// First returned header geometry is visible and accepts a download pointer.
fn assert_first_frame_controls(harness: &Harness, output: &egui::FullOutput) {
    let icons: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) if mesh.calc_bounds().center().y < 40. => {
                Some((shape.clip_rect, mesh.calc_bounds()))
            }
            _ => None,
        })
        .collect();
    let (clip, download) = icons[icons.len() - 2];
    let (close_clip, close) = icons[icons.len() - 1];
    assert!(clip.intersect(download).is_positive(), "{icons:?}");
    assert!(close_clip.contains_rect(close), "{icons:?}");
    assert!(
        (close.center().x - (harness.frame.size.x - 16.)).abs() < 1.,
        "{icons:?}"
    );
    harness.click(download.center());
    assert_eq!(
        harness.host.downloads.borrow().last().unwrap().0,
        "long-artifact-name-12.zip"
    );
}
