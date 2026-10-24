//! Markdown and SVG through the application, retained panel and artifact interface.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::artifact_support;
use crate::session_header_support;
use artifact_support::panel::{Harness, Input};

/// Filename policy installs specialized views through the actual application caller.
#[test]
fn artifact_factory_installs_markdown_and_svg_views() {
    let harness = Harness::new(&Input::default());
    for filename in ["a.md", "a.markdown", "A.MD", "a.svg", "a.txt", "a.png"] {
        harness.command("create", filename, Some("YQ=="));
        harness.caller.show_artifact(filename);
        harness.draw().unwrap();
        assert!(
            harness
                .caller
                .artifacts()
                .read()
                .unwrap()
                .contains_key(filename)
        );
        assert!(!harness.host.published.borrow().is_empty());
    }
    for mut view in [
        Box::new(MarkdownArtifact::new(harness.host.clone())) as Box<dyn ArtifactElement>,
        Box::new(SvgArtifact::new(harness.host.clone())),
    ] {
        assert_eq!((view.filename(), view.content()), ("", ""));
        harness.frame.draw(view.as_mut(), false, vec![]).unwrap();
    }
    assert!(
        maestro_web::tools::artifacts::create_artifact_element("a.html", "", harness.host).is_err()
    );
}

use crate::canvas_paint;
use crate::effects_support;
use crate::markdown_svg_support;
use crate::text_image_support;
use artifact_support::Frame;
use maestro_web::{
    ArtifactElement, MarkdownArtifact, SvgArtifact,
    components::preview_code_toggle::{PreviewCodeToggle, ViewMode},
};
use markdown_svg_support::{click, corpus, locale};
use std::rc::Rc;

/// Draw the public toggle with real toolkit input and supplied branding.
fn toggle_frame(
    frame: &Frame,
    toggle: &mut PreviewCodeToggle,
    events: Vec<egui::Event>,
    paint: &mut canvas_paint::Paint,
) -> (Option<ViewMode>, egui::FullOutput) {
    let mut transition = None;
    let mut output = frame.context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, frame.size)),
            events,
            ..Default::default()
        },
        |ui| {
            transition = toggle
                .render(
                    ui,
                    &frame.i18n,
                    &frame.pack.presentation(frame.mode).unwrap(),
                )
                .unwrap();
        },
    );
    paint.update(&output);
    output.textures_delta.clear();
    (transition, output)
}
/// Ordered actual interactive slots, never guessed coordinates.
fn slots(context: &egui::Context) -> Vec<egui::Rect> {
    let mut slots = context.interactive_rects_last_pass();
    slots.sort_by(|a, b| a.left().total_cmp(&b.left()));
    slots
}
/// Project the typed transition to the recorded event value.
fn mode_name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Preview => "preview",
        ViewMode::Code => "code",
    }
}
/// User transitions exclude idle, repeated and programmatic selections.
#[test]
fn preview_toggle_reports_only_real_changes() {
    for case in corpus().toggles {
        let frame = Frame::default();
        locale(&frame.i18n, &case.language);
        let mut toggle = PreviewCodeToggle::new();
        let mut paint = canvas_paint::Paint::default();
        let mut transitions = Vec::new();
        let mut states = Vec::new();
        assert_eq!(
            toggle_frame(&frame, &mut toggle, vec![], &mut paint).0,
            None
        );
        states.push([
            toggle.mode() == ViewMode::Preview,
            toggle.mode() == ViewMode::Code,
        ]);
        for index in [0, 1, 1, 0] {
            let position = slots(&frame.context)[index].center();
            let (transition, output) =
                toggle_frame(&frame, &mut toggle, click(position), &mut paint);
            assert!(output.platform_output.events.iter().any(|event| matches!(event, egui::output::OutputEvent::Clicked(info) if info.label.as_ref() == Some(&case.titles[index]))));
            transitions.extend(transition.map(mode_name));
            states.push([
                toggle.mode() == ViewMode::Preview,
                toggle.mode() == ViewMode::Code,
            ]);
        }
        assert_eq!(states, case.states);
        assert_eq!(transitions, case.events);
        let mut configured = PreviewCodeToggle::default();
        configured.set_mode(ViewMode::Code);
        assert_eq!(
            toggle_frame(&frame, &mut configured, vec![], &mut paint).0,
            None
        );
        assert_eq!(
            [
                configured.mode() == ViewMode::Preview,
                configured.mode() == ViewMode::Code
            ],
            case.configured
        );
    }
}
/// Both independent actions receive every original byte and localized title.
#[test]
fn specialized_headers_keep_literal_bytes_and_titles() {
    let corpus = corpus();
    for header in &corpus.headers {
        for case in corpus
            .content
            .iter()
            .filter(|case| case.kind == header.kind)
        {
            header_actions(header, case);
        }
    }
}
/// Copy and download one case through its header under the header's locale.
fn header_actions(header: &markdown_svg_support::Header, case: &markdown_svg_support::Case) {
    let host = Rc::new(effects_support::Host::default());
    let frame = Frame::default();
    locale(&frame.i18n, &header.language);
    let mut view = maestro_web::tools::artifacts::create_artifact_element(
        &case.filename,
        &case.content,
        host.clone(),
    )
    .unwrap();
    for index in [2, 3] {
        let output = frame.activate_header(view.as_mut(), index, 4);
        assert!(output.platform_output.events.iter().any(|event| matches!(event, egui::output::OutputEvent::Clicked(info) if info.label.as_ref() == Some(&header.titles[index - 2]))));
    }
    assert_eq!(host.copies.borrow().as_slice(), [case.content.as_str()]);
    for result in host.results.borrow().iter() {
        result.set(Some(true));
    }
    host.run_work();
    host.advance(2000);
    assert_eq!(host.live(), 0);
    assert_eq!(
        host.downloads.borrow().as_slice(),
        &[(case.filename.clone(), case.mime.clone(), case.bytes.clone())]
    );
}
/// Source selection belongs to its view and persists across every kind of edit.
#[test]
fn artifact_modes_survive_edits_and_stay_independent() {
    for svg in [false, true] {
        let host = Rc::new(session_header_support::Host::default());
        let engine = Rc::new(text_image_support::Highlighter::default());
        let frame = Frame {
            highlighter: engine.clone(),
            ..Frame::default()
        };
        let mut view: Box<dyn ArtifactElement> = if svg {
            Box::new(SvgArtifact::new(host.clone()))
        } else {
            Box::new(MarkdownArtifact::new(host.clone()))
        };
        assert_eq!((view.filename(), view.content()), ("", ""));
        let mut sibling: Box<dyn ArtifactElement> = if svg {
            Box::new(SvgArtifact::new(host))
        } else {
            Box::new(MarkdownArtifact::new(host))
        };
        frame.activate_header(view.as_mut(), 1, 4);
        for (filename, text) in [
            ("original", "one"),
            ("renamed.txt", "one"),
            ("renamed.txt", "two"),
        ] {
            view.set_filename(filename.into());
            view.set_content(text.into());
            frame.draw(view.as_mut(), false, vec![]).unwrap();
            assert_eq!(
                engine.requests.borrow().last().unwrap(),
                &(text.into(), if svg { "xml" } else { "markdown" }.into())
            );
        }
        let before = engine.requests.borrow().len();
        sibling.set_content("ordinary".into());
        frame.draw(sibling.as_mut(), false, vec![]).unwrap();
        assert_eq!(engine.requests.borrow().len(), before);
    }
}
/// Selected, hover and focus treatment survive tessellation and the final clip.
#[test]
fn preview_toggle_paints_selected_hover_and_focus() {
    let valid: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/control_texture_sampling_native.json"
    ))
    .unwrap();
    crate::support::strict_record::<FilteringFixture>(&valid, &["preview"]);
    assert_eq!(valid["preview"].as_array().unwrap().len(), 8);
    crate::support::strict_record::<PreviewRow>(
        &valid["preview"][0],
        &["role", "tint", "size", "samples"],
    );
    crate::support::strict_record::<FilterSample>(
        &valid["preview"][0]["samples"][0],
        &["uv", "expected"],
    );
    crate::support::reject_sections::<FilteringFixture>(&valid, &["rows"]);
    for (case, frame) in branded_frames().into_iter().enumerate() {
        let mut toggle = PreviewCodeToggle::new();
        let mut paint = canvas_paint::Paint::default();
        let output = toggle_frame(&frame, &mut toggle, vec![], &mut paint).1;

        let controls = slots(&frame.context);
        assert_eq!(controls.len(), 2);
        assert_native_toggle_glyphs(&frame, &output, &paint, case);
        let sample = |rect: egui::Rect| rect.shrink2(egui::vec2(12., 4.));
        let preview = paint.region(&frame.context, &output.shapes, sample(controls[0]));
        let code = paint.region(&frame.context, &output.shapes, sample(controls[1]));
        assert_ne!(preview, code);
        toggle.set_mode(ViewMode::Code);
        let output = toggle_frame(&frame, &mut toggle, vec![], &mut paint).1;

        assert_ne!(
            preview,
            paint.region(&frame.context, &output.shapes, sample(controls[0]))
        );
        hover_focus(&frame, &mut toggle, &mut paint, &output, &controls);
    }
}

/// Native selected-role texture samples.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FilteringFixture {
    /// Preview glyph observations.
    preview: Vec<PreviewRow>,
}
/// Recorded `PreviewRow` observations.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRow {
    /// Recorded role.
    pub role: String,
    /// Recorded tint.
    pub tint: [u8; 4],
    /// Recorded size.
    pub size: [usize; 2],
    /// Recorded samples.
    pub samples: Vec<FilterSample>,
}
/// Recorded `FilterSample` observations.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterSample {
    /// Recorded uv.
    pub uv: [f32; 2],
    /// Recorded expected.
    pub expected: [f32; 4],
}
/// Compare every isolated native glyph sample and its unequal-role control.
fn assert_native_toggle_glyphs(
    frame: &Frame,
    output: &egui::FullOutput,
    paint: &canvas_paint::Paint,
    case: usize,
) {
    let presentation = frame.pack.presentation(frame.mode).unwrap();
    let muted = egui::Color32::from_hex(presentation.colors["muted"])
        .unwrap()
        .to_array();
    let text = egui::Color32::from_hex(presentation.colors["text"])
        .unwrap()
        .to_array();
    let recorded: FilteringFixture = serde_json::from_str(include_str!(
        "fixtures/control_texture_sampling_native.json"
    ))
    .unwrap();
    let glyphs: Vec<_> = output
        .shapes
        .iter()
        .filter(|s| matches!(s.shape, egui::Shape::Mesh(_)))
        .collect();
    assert_eq!(glyphs.len(), 2);
    for (index, shape) in glyphs.iter().enumerate() {
        let row = &recorded.preview[case * 2 + index];
        let tint = if index == 0 { text } else { muted };
        assert_eq!(row.tint, tint);
        assert_eq!(row.role, if index == 0 { "text" } else { "muted" });
        let egui::Shape::Mesh(mesh) = &shape.shape else {
            unreachable!()
        };
        let rect = mesh.calc_bounds();
        let images = paint.images(std::slice::from_ref(*shape));
        assert_eq!(row.size, images[0].size);
        let negative = if text == muted {
            egui::Color32::from_hex(presentation.colors["accent"])
                .unwrap()
                .to_array()
        } else {
            [muted, text][index]
        };
        let [r, g, b, a] = negative;
        let (wrong_paint, wrong_shapes) = canvas_paint::icon(
            &frame.context,
            rect,
            if index == 0 { "Eye" } else { "Code" },
            egui::Color32::from_rgba_premultiplied(r, g, b, a),
        );
        let mut rejects_wrong_role = false;
        for sample in &row.samples {
            let position = rect.min + rect.size() * egui::vec2(sample.uv[0], sample.uv[1]);
            let actual = paint.pixel(&frame.context, std::slice::from_ref(*shape), position);
            let wrong = wrong_paint.pixel(&frame.context, &wrong_shapes, position);
            rejects_wrong_role |= wrong
                .into_iter()
                .zip(sample.expected)
                .any(|(a, b)| (a - b).abs() > 0.51);
            for (actual, expected) in actual.into_iter().zip(sample.expected) {
                assert!(
                    (actual - expected).abs() <= 0.51,
                    "native preview pixel differs"
                );
            }
        }
        assert!(rejects_wrong_role);
    }
}

/// Both supplied packs in both supported modes, without invented brand values.
fn branded_frames() -> Vec<Frame> {
    ["brand.json", "packs/signal.json"]
        .into_iter()
        .flat_map(|path| {
            [
                maestro_theme::BrandMode::Dark,
                maestro_theme::BrandMode::Light,
            ]
            .map(|mode| Frame {
                mode,
                pack: maestro_theme::load_brand_pack(&format!(
                    "{}/../../assets/brand/{path}",
                    env!("CARGO_MANIFEST_DIR")
                ))
                .unwrap(),
                ..Frame::default()
            })
        })
        .collect()
}
/// Compare hover and focus paint through the same final-clipped raster owner.
fn hover_focus(
    frame: &Frame,
    toggle: &mut PreviewCodeToggle,
    paint: &mut canvas_paint::Paint,
    output: &egui::FullOutput,
    controls: &[egui::Rect],
) {
    for control in controls {
        let idle = paint.region(&frame.context, &output.shapes, *control);
        let hovered = toggle_frame(
            frame,
            toggle,
            vec![egui::Event::PointerMoved(control.center())],
            paint,
        )
        .1;

        assert_ne!(
            idle,
            paint.region(&frame.context, &hovered.shapes, *control)
        );
    }
    for control in controls {
        let events = markdown_svg_support::key(egui::Key::Tab)
            .into_iter()
            .chain([egui::Event::PointerGone])
            .collect();
        let focused = toggle_frame(frame, toggle, events, paint).1;
        assert!(frame.context.memory(egui::Memory::focused).is_some());
        assert_ne!(
            paint.region(&frame.context, &focused.shapes, *control),
            paint.region(&frame.context, &output.shapes, *control)
        );
    }
}

/// Specialized geometry is retained by the real panel without changing text controls.
#[test]
fn specialized_panel_headers_own_gaps_and_border_box_height() {
    for name in ["a.md", "a.svg", "a.txt"] {
        let harness = Harness::new(&Input::default());
        harness.command("create", name, Some("word"));
        harness.caller.show_artifact(name);
        let mut paint = canvas_paint::Paint::default();
        let output = harness
            .frame
            .draw_panel_observing(&harness.panel, &harness.caller, vec![], |output| {
                paint.update(output);
            })
            .unwrap();
        let controls = slots(&harness.frame.context);
        let actions =
            &controls[controls.len() - if name == "a.txt" { 3 } else { 5 }..controls.len() - 1];
        if name == "a.txt" {
            assert!((actions[1].left() - actions[0].right() - 4.).abs() < f32::EPSILON);
        } else {
            let height = actions[0].height();
            let gap = actions[2].left() - actions[1].right() - 1.;
            let action_gap = actions[3].left() - actions[2].right();
            let border = egui::Color32::from_hex(
                harness
                    .frame
                    .pack
                    .presentation(harness.frame.mode)
                    .unwrap()
                    .colors["border"],
            )
            .unwrap()
            .to_array();
            let top = paint.pixel(
                &harness.frame.context,
                &output.shapes,
                egui::pos2(actions[1].center().x, actions[1].top() - 0.5),
            );
            let bottom = paint.pixel(
                &harness.frame.context,
                &output.shapes,
                egui::pos2(actions[1].center().x, actions[1].bottom() + 0.5),
            );
            assert!(
                (height - 26.).abs() < f32::EPSILON
                    && (gap - 8.).abs() < f32::EPSILON
                    && (action_gap - 8.).abs() < f32::EPSILON
                    && top
                        .into_iter()
                        .zip(border)
                        .all(|(a, b)| (a - f32::from(b)).abs() < f32::EPSILON)
                    && bottom
                        .into_iter()
                        .zip(border)
                        .all(|(a, b)| (a - f32::from(b)).abs() < f32::EPSILON),
                "{name}: control={height}, group gap={gap}, action gap={action_gap}, top={top:?}, bottom={bottom:?}"
            );
        }
    }
}
