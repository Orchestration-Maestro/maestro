//! Retained artifact panel through its public application and native host seams.
#![cfg(test)]
pub mod artifact_support;
pub mod session_header_support;
pub mod support;
use artifact_support::panel::{Harness, Input, corpus, tab, text};
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

/// Recorded map aliases and duplicate names retain literal ordered text and underlines.
#[test]
fn artifact_panel_tabs_keep_literal_record_order() {
    for case in corpus().render {
        let mut harness = Harness::new(&case.input);
        harness.frame.size = egui::vec2(2400., 300.);
        harness.draw().unwrap();
        let output = harness.draw().unwrap();
        assert_eq!(harness.caller.active_filename(), case.output.active);
        assert_eq!(case.output.inline_border, !harness.panel.overlay());
        assert_render(&harness, &case.output, &output);
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
        assert_eq!(
            case.output.header_text,
            [if case.input.language == "de" {
                "Artefakte schließen"
            } else {
                "Close artifacts"
            }]
        );
    }
}

/// Assert the source shell's exact visible text and active treatment.
fn assert_render(
    harness: &Harness,
    expected: &artifact_support::panel::Output,
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
        assert_eq!(span.galley.job.text, tab.filename);
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

/// Actual icon geometry, in drawing order, identifies canvas controls.
fn control(output: &egui::FullOutput, index: usize) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.calc_bounds().center()),
            _ => None,
        })
        .nth(index)
        .unwrap()
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
        assert_eq!(Some(harness.presentation.opened.get()), case.output.opened);
        assert_eq!(Some(harness.closed.get()), case.output.closed);
        assert_visible(
            &harness,
            &harness.caller,
            case.output.visible.as_ref().unwrap(),
        );
        assert!(!harness.panel.collapsed());
        assert!(
            roots
                .iter()
                .all(|id| harness.host.published.borrow().contains_key(id))
        );
        for scroll in case.output.scrolls.unwrap() {
            check_scroll_policy(&scroll);
            assert_eq!(Some(scroll.filename), harness.caller.active_filename());
        }
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
        .draw_panel(&harness.panel, caller, click_events(control(&output, 1)))
        .unwrap();
    assert_eq!(
        [harness.host.downloads.borrow().last().unwrap().0.clone()],
        expected
    );
}

/// The toolkit consumes the recorded animation/alignment policies.
fn check_scroll_policy(scroll: &artifact_support::panel::Scroll) {
    assert_eq!(scroll.options.behavior, "smooth");
    assert_eq!(scroll.options.block, "nearest");
    assert_eq!(scroll.options.inline, "center");
}

/// Smooth native scrolling reaches literal targets rather than a selector or decoy.
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
        let mut output = harness.draw().unwrap();
        for _ in 0..120 {
            output = harness.draw().unwrap();
        }
        let target = tab(&output, &case.input.filename);
        assert_eq!(
            target.center().y.to_bits(),
            tab(&first, "long-leading-name-0.zip").center().y.to_bits()
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
        check_scroll_policy(&case.output.scroll[0]);
    }
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
    assert_visible(harness, caller, expected.visible.as_ref().unwrap());
    assert_eq!(Some(harness.presentation.opened.get()), expected.opened);
    assert_eq!(Some(harness.closed.get()), expected.closed);
    assert!(expected.scrolls.as_ref().unwrap().is_empty());
    assert_eq!(expected.same_roots, Some(true));
    for (id, root) in before {
        assert!(Rc::ptr_eq(
            &root.upgrade().unwrap(),
            &harness.host.published.borrow()[&id].0
        ));
    }
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
            .get_or_create_artifact_element("a.pdf", "AAEC")
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
    assert!(!draw_underlying(&harness, click_events(pos)).0);
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Rect(rect) if rect.fill == egui::Color32::from_black_alpha(100))));
    harness.panel.set_overlay(false);
    draw_underlying(&harness, vec![]);
    draw_underlying(&harness, vec![]);
    assert!(harness.host.overlays.borrow().is_empty());
    assert!(draw_underlying(&harness, click_events(pos)).0);
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

/// One bounded pointer activation, without a timer or input-policy substitute.
fn click_events(pos: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
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
        let activated = harness
            .frame
            .draw_panel(
                &harness.panel,
                &harness.caller,
                click_events(control(&output, 0)),
            )
            .unwrap();
        assert!(
            activated.platform_output.events.iter().any(|event| event
                .widget_info()
                .label
                .as_deref()
                == Some(title))
        );
        harness.frame.i18n.get_translations().borrow()["en"]
            .borrow_mut()
            .insert(
                "Close artifacts".into(),
                maestro_web::TranslationValue::Text("Live close".into()),
            );
        let activated = harness
            .frame
            .draw_panel(
                &harness.panel,
                &harness.caller,
                click_events(control(&output, 0)),
            )
            .unwrap();
        assert!(
            activated.platform_output.events.iter().any(|event| event
                .widget_info()
                .label
                .as_deref()
                == Some("Live close"))
        );
    }
}

/// Supplied packs own panel paint, underlines and both source shadow layers.
#[test]
fn artifact_panel_colors_follow_caller_packs() {
    for file in ["brand.json", "packs/signal.json"] {
        for case in corpus().shell_shadow {
            let mut harness = Harness::new(&Input {
                entries: vec![("a.zip".into(), "a.zip".into())],
                active: Some("a.zip".into()),
                ..Default::default()
            });
            harness.frame.pack = maestro_theme::load_brand_pack(
                &format!("{}/../../assets/brand/{file}", env!("CARGO_MANIFEST_DIR")),
                &maestro_theme::theme::NativeThemeOperations,
            )
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
            }
        }
    }
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
            .get_or_create_artifact_element("a.pdf", "Yg==")
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
    let bounds = harness.host.published.borrow().values().next().unwrap().1;
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
            (bounds.top() - 40. - expected.spread + expected.offset[1]).to_bits()
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
            Err(maestro_web::browser::attachment_content::AttachmentUiError::BorrowConflict)
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
