//! Mounted copy-link property transitions observed through actual clipped paint.
#![cfg(test)]
#[path = "support/brand.rs"]
pub mod brand;
#[path = "attachment_support/canvas_paint.rs"]
pub mod canvas_paint;
pub mod effects_support;
pub mod session_links_support;
use maestro_theme::BrandMode;
use session_links_support::Canvas;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    timepoints: Vec<f64>,
    cases: Vec<Case>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Snapshot {
    opacity: f32,
    background: Option<[u8; 3]>,
    color: Option<[u8; 3]>,
    border: Option<[u8; 3]>,
    hover: bool,
    message_hover: bool,
    copied: bool,
}

#[derive(serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Case {
    EnterExit {
        pack: String,
        mode: String,
        initial: Snapshot,
        enter: Vec<Snapshot>,
        exit: Vec<Snapshot>,
    },
    VisibleColorEnterExit {
        pack: String,
        mode: String,
        enter: Vec<Snapshot>,
        exit: Vec<Snapshot>,
    },
    Reversal {
        #[serde(default)]
        visible: bool,
        fraction: f64,
        before: Snapshot,
        after: Snapshot,
        duration: f64,
        reverse: Vec<Snapshot>,
    },
    RepeatedReversal {
        #[serde(default)]
        visible: bool,
        a: Snapshot,
        b: Snapshot,
        c: Snapshot,
        d1: f64,
        d2: f64,
        next: Vec<Snapshot>,
    },
    Copied {
        exit_to: String,
        before: Snapshot,
        success: Snapshot,
        during: Vec<Snapshot>,
        moved: Snapshot,
        moved_during: Vec<Snapshot>,
        restored: Snapshot,
        restore: Vec<Snapshot>,
    },
    IndependentProperties {
        opacity_before: Snapshot,
        hover_on: Snapshot,
        independent: Vec<Snapshot>,
    },
    Remount {
        position: String,
        first: Snapshot,
        next: Snapshot,
    },
    RetainedParentRemount {
        position: String,
        before: Snapshot,
        first: Snapshot,
    },
    PaletteRetarget {
        #[serde(default)]
        visible: bool,
        old: Snapshot,
        zero: Snapshot,
        samples: Vec<Snapshot>,
    },
    SameTime {
        same_first: Snapshot,
        same_second: Snapshot,
    },
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/session_link_transitions.json")).unwrap()
}
struct Harness {
    canvas: Canvas,
    pack: String,
    mode: BrandMode,
}
impl Harness {
    fn new(visible: bool) -> Self {
        let mut this = Self {
            canvas: Canvas::new(),
            pack: "brand.json".into(),
            mode: BrandMode::Dark,
        };
        this.canvas.hovered = visible;
        this.frame(false);
        this
    }
    fn frame(&mut self, hover: bool) -> egui::FullOutput {
        let pos = if hover {
            self.canvas.rect.center()
        } else {
            egui::pos2(300., 300.)
        };
        let (response, output) =
            self.canvas
                .pack_frame(vec![egui::Event::PointerMoved(pos)], &self.pack, self.mode);
        response.unwrap();
        output
    }
    fn observe(&mut self, time: f64, expected: &Snapshot) {
        self.canvas.time = time;
        self.canvas.hovered = expected.message_hover;
        let output = self.frame(expected.hover);
        let canvas = &self.canvas;
        let point = egui::pos2(54., 64.);
        let background = self
            .canvas
            .paint
            .pixel(&canvas.context, &output.shapes, point);
        assert!(
            (background[3] - (expected.opacity * 255.).round()).abs() <= 1.,
            "opacity at {}: {:?} != {}",
            canvas.time,
            background,
            expected.opacity
        );
        if let Some(rgb) = expected.background {
            channels(background, rgb, expected.opacity);
        }
        if let Some(rgb) = expected.border {
            let border =
                self.canvas
                    .paint
                    .pixel(&canvas.context, &output.shapes, egui::pos2(40.5, 54.));
            channels(border, rgb, expected.opacity);
        }
        if let Some(rgb) = expected.color {
            let [r, g, b] = rgb;
            let [br, bg, bb] = expected.background.unwrap();
            session_links_support::assert_glyph(
                &self.canvas,
                &output,
                expected.copied,
                [
                    egui::Color32::from_rgb(br, bg, bb),
                    egui::Color32::from_rgb(r, g, b),
                ],
                expected.opacity,
            );
        }
        assert_eq!(
            session_links_support::check(&self.canvas, &output),
            expected.copied && expected.opacity > 0.
        );
    }
}
fn channels(actual: [f32; 4], expected: [u8; 3], opacity: f32) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!(
            (a - (f32::from(b) * opacity).round()).abs() <= 1.1,
            "paint channel {a} != {b} at opacity {opacity}"
        );
    }
}
#[test]
fn copy_link_transitions_match_ease_enter_and_exit() {
    let fixture = fixture();
    for case in fixture.cases {
        let (pack, mode, initial, enter, exit, visible) = match case {
            Case::EnterExit {
                pack,
                mode,
                initial,
                enter,
                exit,
            } => (pack, mode, Some(initial), enter, exit, false),
            Case::VisibleColorEnterExit {
                pack,
                mode,
                enter,
                exit,
            } => (pack, mode, None, enter, exit, true),
            _ => continue,
        };
        let mut h = Harness::new(visible);
        h.pack = pack;
        h.mode = match mode.as_str() {
            "dark" => BrandMode::Dark,
            "light" => BrandMode::Light,
            _ => panic!("unknown mode"),
        };
        h.frame(false);
        if let Some(initial) = initial {
            h.observe(0., &initial);
        }
        for (samples, base) in [(enter, 1.), (exit, 3.)] {
            assert_eq!(samples.len(), fixture.timepoints.len());
            for (time, sample) in fixture.timepoints.iter().zip(samples) {
                h.observe(base + time / 1000., &sample);
            }
        }
    }
}
#[test]
fn copy_link_interrupted_hover_keeps_current_paint() {
    for case in fixture().cases {
        let visible = matches!(
            case,
            Case::Reversal { visible: true, .. } | Case::RepeatedReversal { visible: true, .. }
        );
        let mut h = Harness::new(visible);
        h.canvas.time = 1.;
        h.canvas.hovered = true;
        h.frame(true);
        match case {
            Case::Reversal {
                fraction,
                before,
                after,
                duration,
                reverse,
                ..
            } => {
                let start = 1. + 0.15 * fraction;
                h.observe(start, &before);
                h.observe(start, &after);
                for (dt, sample) in [
                    0.,
                    1000. / 60.,
                    duration / 2.,
                    duration,
                    duration + 1000. / 60.,
                ]
                .into_iter()
                .zip(reverse)
                {
                    h.observe(start + dt / 1000., &sample);
                }
            }
            Case::RepeatedReversal {
                a,
                b,
                c,
                d1,
                d2,
                next,
                ..
            } => {
                h.observe(1.075, &a);
                h.canvas.hovered = visible;
                h.frame(false);
                let start = 1.075 + d1 / 2000.;
                h.observe(start, &b);
                h.observe(start, &c);
                for (dt, sample) in [0., 1000. / 60., d2 / 2., d2].into_iter().zip(next) {
                    h.observe(start + dt / 1000., &sample);
                }
            }
            _ => {}
        }
    }
}
#[test]
fn copy_link_copied_override_retargets_current_colors() {
    let fixture = fixture();
    for case in fixture.cases {
        let Case::Copied {
            exit_to,
            before,
            success,
            during,
            moved,
            moved_during,
            restored,
            restore,
        } = case
        else {
            continue;
        };
        let mut h = Harness::new(true);
        h.canvas.time = 1.;
        h.frame(true);
        h.observe(1.075, &before);
        h.canvas.activate().unwrap();
        session_links_support::settle(&h.canvas, 0, true);
        h.observe(1.075, &success);
        for (dt, sample) in fixture.timepoints.iter().zip(during) {
            h.observe(1.075 + dt / 1000., &sample);
        }
        assert_eq!(
            (moved.hover, moved.message_hover),
            match exit_to.as_str() {
                "button" => (true, true),
                "message" => (false, true),
                "outside" => (false, false),
                _ => panic!("unknown exit"),
            }
        );
        h.observe(3., &moved);
        for (dt, sample) in fixture.timepoints.iter().zip(moved_during) {
            h.observe(3. + dt / 1000., &sample);
        }
        h.canvas.host.advance(1500);
        h.observe(5., &restored);
        for (dt, sample) in fixture.timepoints.iter().zip(restore) {
            h.observe(5. + dt / 1000., &sample);
        }
        assert_eq!(h.canvas.host.admissions.get(), 1);
        assert_eq!(h.canvas.host.live(), 0);
    }
}
#[test]
fn copy_link_visibility_and_color_transitions_are_independent() {
    for case in fixture().cases {
        let Case::IndependentProperties {
            opacity_before,
            hover_on,
            independent,
        } = case
        else {
            continue;
        };
        let mut h = Harness::new(false);
        h.canvas.time = 1.;
        h.canvas.hovered = true;
        h.frame(false);
        h.observe(1.0375, &opacity_before);
        h.observe(1.0375, &hover_on);
        for (dt, sample) in [0., 37.5, 75., 112.5, 150.].into_iter().zip(independent) {
            h.observe(1.0375 + dt / 1000., &sample);
        }
        assert_eq!(h.canvas.host.admissions.get(), 0);
    }
}
#[test]
fn copy_link_transition_mounts_resolved_and_drops_with_state() {
    for case in fixture().cases {
        let (position, before, first, next) = match case {
            Case::Remount {
                position,
                first,
                next,
            } => (position, None, first, Some(next)),
            Case::RetainedParentRemount {
                position,
                before,
                first,
            } => (position, Some(before), first, None),
            _ => continue,
        };
        let mut h = Harness::new(true);
        let mut sibling = Harness::new(true);
        sibling.canvas.context = h.canvas.context.clone();
        sibling.canvas.rect = sibling.canvas.rect.translate(egui::vec2(100., 0.));
        if let Some(before) = before {
            h.observe(0., &before);
        }
        h.canvas.time = 1.;
        h.frame(true);
        h.canvas.time = 1.075;
        h.frame(true);
        h.canvas.activate().unwrap();
        h.canvas.state =
            maestro_web::export_html::template::CopyLinkState::new(h.canvas.host.clone());
        session_links_support::settle(&h.canvas, 0, true);
        assert_eq!(h.canvas.host.live(), 0);
        assert_eq!(first.message_hover, position == "message");
        h.observe(1.075, &first);
        h.observe(1.075, &first);
        if let Some(next) = next {
            h.observe(2., &next);
        }
        sibling.canvas.time = 2.;
        let out = sibling.frame(false);
        let pixel =
            sibling
                .canvas
                .paint
                .pixel(&sibling.canvas.context, &out.shapes, egui::pos2(154., 64.));
        assert!((pixel[3] - 255.).abs() < f32::EPSILON);
        assert_eq!(sibling.canvas.host.admissions.get(), 0);
    }
}
#[test]
fn copy_link_live_palette_change_retargets_only_changed_properties() {
    let fixture = fixture();
    for case in fixture.cases {
        let Case::PaletteRetarget {
            visible,
            old,
            zero,
            samples,
        } = case
        else {
            continue;
        };
        let mut h = Harness::new(visible);
        h.canvas.time = 1.;
        h.canvas.hovered = true;
        h.frame(true);
        h.observe(1.075, &old);
        h.mode = BrandMode::Light;
        h.observe(1.075, &zero);
        for (dt, sample) in fixture.timepoints.iter().zip(samples) {
            h.observe(1.075 + dt / 1000., &sample);
        }
    }
}
#[test]
fn copy_link_equal_time_and_long_gap_frames_settle() {
    for case in fixture().cases {
        let Case::SameTime {
            same_first,
            same_second,
        } = case
        else {
            continue;
        };
        let mut h = Harness::new(false);
        h.canvas.time = 1.;
        h.canvas.hovered = true;
        h.frame(true);
        h.observe(1.0375, &same_first);
        h.observe(1.0375, &same_second);
        assert!(h.canvas.context.has_requested_repaint());
        h.canvas.time = 2.;
        h.frame(true);
        h.canvas.time = 3.;
        h.frame(false);
        h.canvas.time = 4.;
        h.frame(false);
        h.canvas.frame(vec![]).0.unwrap();
        let one = h.canvas.frame(vec![]).1;
        let two = h.canvas.frame(vec![]).1;
        assert!(!h.canvas.context.has_requested_repaint());
        assert_eq!(
            h.canvas
                .paint
                .pixel(&h.canvas.context, &one.shapes, egui::pos2(54., 64.))
                .map(f32::to_bits),
            h.canvas
                .paint
                .pixel(&h.canvas.context, &two.shapes, egui::pos2(54., 64.))
                .map(f32::to_bits)
        );
        assert_eq!(h.canvas.host.live(), 0);
        assert_eq!(h.canvas.host.admissions.get(), 0);
    }
}
#[test]
fn copy_link_transition_border_and_feedback_are_immediate() {
    let mut h = Harness::new(true);
    h.canvas.clip = egui::Rect::from_min_max(egui::pos2(39., 39.), egui::pos2(55., 70.));
    h.canvas.time = 1.;
    h.frame(true);
    h.canvas.activate().unwrap();
    session_links_support::settle(&h.canvas, 0, true);
    let copied = h.frame(true);
    assert!(session_links_support::check(&h.canvas, &copied));
    channels(
        h.canvas
            .paint
            .pixel(&h.canvas.context, &copied.shapes, egui::pos2(40.5, 54.)),
        [255, 106, 26],
        1.,
    );
    assert_eq!(
        h.canvas
            .paint
            .pixel(&h.canvas.context, &copied.shapes, egui::pos2(56., 54.))
            .map(f32::to_bits),
        [0; 4]
    );
    h.canvas.time = 2.;
    h.frame(true);
    h.canvas.host.advance(1500);
    let expired = h.frame(false);
    assert!(!session_links_support::check(&h.canvas, &expired));
    assert!(session_links_support::link(&h.canvas, &expired).is_some());
    channels(
        h.canvas
            .paint
            .pixel(&h.canvas.context, &expired.shapes, egui::pos2(40.5, 54.)),
        [94, 115, 131],
        1.,
    );
    h.canvas.clip = egui::Rect::NOTHING;
    let clipped = h.frame(false);
    assert_eq!(
        h.canvas
            .paint
            .pixel(&h.canvas.context, &clipped.shapes, egui::pos2(54., 54.))
            .map(f32::to_bits),
        [0; 4]
    );
}
