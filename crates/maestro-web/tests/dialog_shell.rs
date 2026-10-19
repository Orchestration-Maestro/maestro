//! Owned dialog lifecycle through the replaceable content host.
#![cfg(test)]
use crate::attachment_support;
use crate::brand;
use crate::input_support;
use maestro_web::{
    browser::attachment_content::ReadingContent,
    dialogs::{
        dialog::{DialogBody, DialogProps, dialog},
        dialog_base::{DialogBase, DialogContext},
    },
};
use num_traits::ToPrimitive as _;
use std::{cell::Cell, rc::Rc};

/// The canvas Close control of the default dialog frame.
const CLOSE: egui::Rect = egui::Rect::from_min_max(egui::pos2(467., 117.), egui::pos2(483., 133.));

#[test]
fn dialog_closed_owner_renders_nothing() {
    let visits = Rc::new(Cell::new(0));
    let visited = visits.clone();
    let host = input_support::Host::new(|_, _| panic!("closed dialog published content"));
    let context = egui::Context::default();
    let dialogs = DialogContext::new(context.clone(), host.clone());
    let body = DialogBody {
        reading: Rc::new(ReadingContent {
            #[cfg(target_arch = "wasm32")]
            mount: Rc::new(|_| panic!("closed dialog mounted content")),
        }),
        control_sizes: vec![],
        render_content: Rc::new(move |_, _, _, _| {
            visited.set(visited.get() + 1);
            Ok(())
        }),
    };
    let owner = dialog(&dialogs, DialogProps::new(false, body)).unwrap();
    let mut frame = attachment_support::Frame {
        ctx: context,
        ..Default::default()
    };
    frame.run(vec![], |_, i18n, presentation| {
        dialogs.render(i18n, presentation).unwrap();
    });
    assert_eq!(visits.get(), 0);
    assert!(host.removed.borrow().is_empty());
    owner.close().unwrap();
}

#[test]
fn dialog_open_is_eager_and_focus_restores_after_removal() {
    let host = input_support::Host::new(|_, _| {});
    let drawing = egui::Context::default();
    let context = DialogContext::new(drawing.clone(), host.clone());
    for _ in 0..3 {
        drawing.begin_pass(egui::RawInput::default());
        drawing.end_pass().textures_delta.clear();
    }
    assert!(!drawing.has_requested_repaint());
    let body = DialogBody {
        reading: Rc::new(ReadingContent {
            #[cfg(target_arch = "wasm32")]
            mount: Rc::new(|_| unreachable!()),
        }),
        control_sizes: vec![],
        render_content: Rc::new(|_, _, _, _| Ok(())),
    };
    let owner = maestro_web::dialogs::dialog_base::DialogBase::new(context, body);
    let predecessor = egui::Id::new("canvas-predecessor");
    let other = egui::Id::new("dialog-focus");
    drawing.memory_mut(|m| m.request_focus(predecessor));
    owner.open().unwrap();
    drawing.memory_mut(|m| m.request_focus(other));
    assert!(drawing.has_requested_repaint());
    owner.open().unwrap();
    assert_eq!(&*host.effects.borrow(), &["capture", "admit"]);
    owner.close().unwrap();
    assert_eq!(
        &*host.effects.borrow(),
        &["capture", "admit", "remove", "release", "schedule"]
    );
    assert_eq!(drawing.memory(egui::Memory::focused), Some(other));
    let restore = host.frame.borrow_mut().take().unwrap();
    restore().unwrap();
    assert_eq!(drawing.memory(egui::Memory::focused), Some(predecessor));
    assert_eq!(host.effects.borrow().last(), Some(&"restore"));
    owner.close().unwrap();
    assert_eq!(
        host.effects
            .borrow()
            .iter()
            .filter(|e| **e == "schedule")
            .count(),
        1
    );
}

#[test]
fn dialog_defaults_and_body_reach_the_rendered_shell() {
    let visits = Rc::new(Cell::new(0));
    let visited = visits.clone();
    let host = input_support::Host::new(|surface, _| {
        assert!(matches!(
            surface.content,
            maestro_web::browser::attachment_content::AttachmentContent::Reading(_)
        ));
    });
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
    let owner = maestro_web::dialogs::dialog_base::DialogBase::new(
        dialogs.clone(),
        DialogBody {
            reading: Rc::new(ReadingContent {
                #[cfg(target_arch = "wasm32")]
                mount: Rc::new(|_| unreachable!()),
            }),
            control_sizes: vec![],
            render_content: Rc::new(move |_, _, _, _| {
                visited.set(visited.get() + 1);
                Ok(())
            }),
        },
    );
    owner.open().unwrap();
    frame.run(vec![], |_, i18n, presentation| {
        dialogs.render(i18n, presentation).unwrap();
    });
    assert_eq!(visits.get(), 1);
    assert_eq!(
        &*host.dimensions.borrow(),
        &[[
            "min(600px, 90vw)".to_string(),
            "min(600px, 80vh)".to_string()
        ]]
    );
}

/// Owned empty native body for controlled lifecycle observations.
fn body(visits: Rc<Cell<usize>>) -> DialogBody {
    DialogBody {
        reading: Rc::new(ReadingContent {
            #[cfg(target_arch = "wasm32")]
            mount: Rc::new(|_| unreachable!()),
        }),
        control_sizes: vec![],
        render_content: Rc::new(move |_, _, _, _| {
            visits.set(visits.get() + 1);
            Ok(())
        }),
    }
}
/// Mount a dialog and capture its live Close identity in the caller's initial frame.
fn mount_close_dialog(
    frame: &mut attachment_support::Frame,
    dialogs: &DialogContext,
) -> (DialogBase, Rc<Cell<egui::Id>>) {
    let id = Rc::new(Cell::new(egui::Id::NULL));
    let saved = id.clone();
    let mut children = body(Rc::default());
    children.render_content = Rc::new(move |_, modal, _, _| {
        saved.set(modal.with("close"));
        Ok(())
    });
    let owner = dialog(dialogs, DialogProps::new(true, children)).unwrap();
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    (owner, id)
}

/// One physical canvas Escape, independent of popup policy.
fn escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::ALT,
    }
}
#[test]
fn dialog_rerender_keeps_one_owned_escape_listener() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
    let visits = Rc::new(Cell::new(0));
    let closes = Rc::new(Cell::new(0));
    let observed = closes.clone();
    let mut props = DialogProps::new(true, body(visits.clone()));
    props.on_close = Some(Rc::new(move || observed.set(observed.get() + 1)));
    let owner = dialog(&dialogs, props).unwrap();
    owner.open().unwrap();
    for _ in 0..2 {
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    assert_eq!(visits.get(), 2);
    frame.run(vec![escape()], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!(closes.get(), 1);
    host.escape.set(true);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!(closes.get(), 1);
    owner.open().unwrap();
    drop(owner);
    host.escape.set(true);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!(visits.get(), 2);
    assert_eq!(closes.get(), 1);
}

#[test]
fn dialog_dimensions_forward_css_without_rewriting() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
    let rows: Vec<Dimensions> =
        serde_json::from_str(include_str!("fixtures/dialog_dimensions.json")).unwrap();
    for row in rows {
        let mut props = DialogProps::new(true, body(Rc::default()));
        if let Some(width) = row.width {
            props.width = width;
        }
        if let Some(height) = row.height {
            props.height = height;
        }
        let owner = dialog(&dialogs, props).unwrap();
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        assert_eq!(host.dimensions.borrow().last().unwrap(), &row.expected);
        drop(owner);
    }
    host.dimensions.borrow_mut().clear();
    let mut props = DialogProps::new(false, body(Rc::default()));
    let owner = dialog(&dialogs, props.clone()).unwrap();
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    assert!(host.dimensions.borrow().is_empty());
    props.is_open = true;
    for dimensions in [
        ["min(600px, 90vw)", "auto"],
        ["calc(70vw - .5px)", "auto"],
        ["calc(70vw - .5px)", "min(700px, 90vh)"],
        ["", "min(700px, 90vh)"],
        ["", ""],
    ] {
        props.width = dimensions[0].into();
        props.height = dimensions[1].into();
        owner.set_props(props.clone()).unwrap();
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        assert_eq!(
            host.dimensions.borrow().last().unwrap(),
            &dimensions.map(str::to_owned)
        );
    }
}

#[test]
fn dialog_close_and_drop_release_owned_registration() {
    let host = input_support::Host::new(|_, _| {});
    let dialogs = DialogContext::new(egui::Context::default(), host.clone());
    let slot = Rc::new(std::cell::RefCell::new(
        None::<std::rc::Weak<maestro_web::dialogs::dialog_base::DialogBase>>,
    ));
    let notification = slot.clone();
    let observed = host;
    let calls = Rc::new(Cell::new(0));
    let called = calls.clone();
    let capture = Rc::new(());
    let weak = Rc::downgrade(&capture);
    let mut children = body(Rc::default());
    children.render_content = Rc::new(move |_, _, _, _| {
        assert_eq!(Rc::strong_count(&capture), 1);
        Ok(())
    });
    let mut props = DialogProps::new(true, children);
    props.on_close = Some(Rc::new(move || {
        assert!(!observed.removed.borrow().is_empty());
        assert!(observed.effects.borrow().contains(&"release"));
        called.set(called.get() + 1);
        let owner = notification.borrow().as_ref().unwrap().upgrade().unwrap();
        owner.close().unwrap();
    }));
    let owner = Rc::new(dialog(&dialogs, props).unwrap());
    *slot.borrow_mut() = Some(Rc::downgrade(&owner));
    owner.close().unwrap();
    owner.close().unwrap();
    assert_eq!(calls.get(), 1);
    owner.open().unwrap();
    drop(owner);
    assert_eq!(calls.get(), 1);
    assert!(weak.upgrade().is_none());
}

#[test]
fn dialog_instances_keep_independent_owned_registrations() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
    let visits = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    let a = dialog(&dialogs, DialogProps::new(true, body(visits[0].clone()))).unwrap();
    let b = dialog(&dialogs, DialogProps::new(true, body(visits[1].clone()))).unwrap();
    a.close().unwrap();
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!([visits[0].get(), visits[1].get()], [0, 1]);
    a.open().unwrap();
    host.escape.set(true);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!(
        host.effects
            .borrow()
            .iter()
            .filter(|e| **e == "release")
            .count(),
        3
    );
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!([visits[0].get(), visits[1].get()], [0, 1]);
    drop(b);
    let other = Rc::new(std::cell::RefCell::new(
        None::<maestro_web::dialogs::dialog_base::DialogBase>,
    ));
    let slot = other.clone();
    let token = Rc::new(());
    let released = Rc::downgrade(&token);
    let mut first = DialogProps::new(true, body(Rc::default()));
    first.on_close = Some(Rc::new(move || {
        let dropped = slot.borrow_mut().take();
        drop(dropped);
        assert!(
            released.upgrade().is_none(),
            "a paint snapshot must not keep another owner's captures alive"
        );
    }));
    let _first = dialog(&dialogs, first).unwrap();
    let mut children = body(Rc::default());
    children.render_content = Rc::new(move |_, _, _, _| {
        let _ = &token;
        Ok(())
    });
    *other.borrow_mut() = Some(dialog(&dialogs, DialogProps::new(true, children)).unwrap());
    host.escape.set(true);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
}

#[test]
fn dialog_optional_notification_is_not_required() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host);
    let mut props = DialogProps::new(true, body(Rc::default()));
    let owner = dialog(&dialogs, props.clone()).unwrap();
    owner.close().unwrap();
    let calls = Rc::new(Cell::new(0));
    let called = calls.clone();
    props.on_close = Some(Rc::new(move || called.set(called.get() + 1)));
    owner.set_props(props.clone()).unwrap();
    let replacement = calls.clone();
    props.on_close = Some(Rc::new(move || replacement.set(replacement.get() + 10)));
    owner.set_props(props).unwrap();
    frame.run(vec![escape()], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!(calls.get(), 10);
}

#[test]
fn dialog_popup_does_not_suppress_real_escape() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
    let owner = dialog(&dialogs, DialogProps::new(true, body(Rc::default()))).unwrap();
    egui::Popup::open_id(&frame.ctx, egui::Id::new("popup"));
    assert!(egui::Popup::is_any_open(&frame.ctx));
    frame.run(vec![escape()], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!(host.removed.borrow().len(), 1);
    drop(owner);
}

#[test]
fn dialog_backdrop_content_and_close_use_one_close_owner() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
    let owner = dialog(&dialogs, DialogProps::new(true, body(Rc::default()))).unwrap();
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    frame.run(
        attachment_support::click(egui::pos2(475., 125.)),
        |_, i, p| dialogs.render(i, p).unwrap(),
    );
    assert_eq!(
        host.removed.borrow().len(),
        1,
        "the first visible frame must admit Close"
    );
    owner.open().unwrap();

    frame.run(
        attachment_support::click(egui::pos2(150., 150.)),
        |_, i, p| dialogs.render(i, p).unwrap(),
    );
    assert_eq!(host.removed.borrow().len(), 1);
    frame.run(
        attachment_support::click(egui::pos2(30., 30.)),
        |_, i, p| dialogs.render(i, p).unwrap(),
    );
    assert_eq!(host.removed.borrow().len(), 2);
    owner.open().unwrap();
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    frame.run(
        attachment_support::click(egui::pos2(475., 125.)),
        |_, i, p| dialogs.render(i, p).unwrap(),
    );
    assert_eq!(host.removed.borrow().len(), 3);
}

#[test]
fn dialog_controls_keep_source_geometry_hover_and_focus_in_both_packs() {
    for filename in ["brand.json", "packs/signal.json"] {
        for mode in [
            maestro_theme::BrandMode::Light,
            maestro_theme::BrandMode::Dark,
        ] {
            geometry(filename, mode);
        }
    }
}

#[test]
fn dialog_failed_mount_and_release_keep_independent_owners() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
    let visits = Rc::new(Cell::new(0));
    let healthy = dialog(&dialogs, DialogProps::new(true, body(visits.clone()))).unwrap();
    for phase in ["capture", "admit"] {
        *host.failure.borrow_mut() = Some(phase);
        let error = dialog(&dialogs, DialogProps::new(true, body(Rc::default())))
            .err()
            .unwrap();
        assert_eq!(error.to_string(), phase);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    for phase in ["measure", "bounds", "publish"] {
        *host.failure.borrow_mut() = Some(phase);
        let error = frame
            .run(vec![], |_, i, p| dialogs.render(i, p))
            .unwrap_err();
        assert_eq!(error.to_string(), phase);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    for phase in ["release", "schedule"] {
        let broken = dialog(&dialogs, DialogProps::new(true, body(Rc::default()))).unwrap();
        *host.failure.borrow_mut() = Some(phase);
        assert_eq!(broken.close().unwrap_err().to_string(), phase);
        broken.close().unwrap();
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    assert!(visits.get() >= 7);
    healthy.close().unwrap();
}

/// Source-derived literal CSS operands and observed dimensions.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Dimensions {
    /// Omitted constructor width versus an explicitly supplied string.
    width: Option<String>,
    /// Omitted constructor height versus an explicitly supplied string.
    height: Option<String>,
    /// Independent native width and height forwarding expectations.
    expected: [String; 2],
}

/// Observe exact control geometry, paint tint changes and visible focus in one caller pack.
fn geometry(filename: &str, mode: maestro_theme::BrandMode) {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame {
        pack: maestro_theme::load_brand_pack(
            &format!(
                "{}/../../assets/brand/{filename}",
                env!("CARGO_MANIFEST_DIR")
            ),
            &maestro_theme::theme::NativeThemeOperations,
        )
        .unwrap(),
        mode,
        ..Default::default()
    };
    let dialogs = DialogContext::new(frame.ctx.clone(), host);
    let (_owner, id) = mount_close_dialog(&mut frame, &dialogs);
    assert!(
        frame.shapes.iter().any(
            |shape| matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.calc_bounds()==CLOSE)
        )
    );
    let resting_images = control_paint(&frame);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    frame.run(
        vec![egui::Event::PointerMoved(CLOSE.center())],
        |_, i, p| dialogs.render(i, p).unwrap(),
    );
    for _ in 0..12 {
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    assert_ne!(control_paint(&frame), resting_images);
    frame
        .ctx
        .memory_mut(|memory| memory.request_focus(id.get()));
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    for _ in 0..12 {
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    assert_eq!(focus_alpha(&frame), 255);
    assert_ring_spread(&frame, 2., 1.);
}

#[test]
fn dialog_close_focus_paint_has_whole_control_opacity_and_transition() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host);
    let (_owner, id) = mount_close_dialog(&mut frame, &dialogs);
    let resting_glyph = frame.images.clone();
    frame.ctx.memory_mut(|m| m.request_focus(id.get()));
    for _ in 0..12 {
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    let resting = focus_alpha(&frame);
    assert_eq!(
        resting, 179,
        "unhovered keyboard focus is seventy percent opaque"
    );
    frame.run(
        vec![egui::Event::PointerMoved(egui::pos2(475., 125.))],
        |_, i, p| dialogs.render(i, p).unwrap(),
    );
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    let intermediate = focus_alpha(&frame);
    // The independent cubic-bezier sample at 1/9 progress is 0.032800723766655.
    assert_eq!(
        intermediate, 181,
        "paint follows the default transition curve"
    );
    let intermediate_glyph = frame.images.clone();
    assert!(!intermediate_glyph.is_empty());
    assert_ne!(intermediate_glyph, resting_glyph);
    assert!(
        resting < intermediate && intermediate < 255,
        "hover must paint an intermediate opacity"
    );
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    // The independent cubic-bezier sample at 2/9 progress is 0.1751682975396243.
    assert_eq!(focus_alpha(&frame), 192);
    let mut endpoint_glyph = intermediate_glyph.clone();
    for _ in 0..12 {
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        if !frame.images.is_empty() {
            endpoint_glyph = frame.images.clone();
        }
    }
    assert_ne!(intermediate_glyph, endpoint_glyph);
    assert_eq!(focus_alpha(&frame), 255);
    focus_fade(&mut frame, &dialogs, id.get());
}

/// Read maximum alpha from visible tessellated focus-ring paint.
fn focus_alpha(frame: &attachment_support::Frame) -> u8 {
    ring_paint(frame)
        .into_iter()
        .fold(0., f32::max)
        .round()
        .to_u8()
        .unwrap()
}

#[test]
fn dialog_pointer_requires_both_endpoints_in_current_clip_and_layer() {
    for (press_blocked, release_blocked, cover) in [(true, false), (false, true), (true, true)]
        .into_iter()
        .flat_map(|(p, r)| [false, true].map(move |cover| (p, r, cover)))
    {
        let host = input_support::Host::new(|_, _| {});
        let mut frame = attachment_support::Frame::default();
        let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
        let owner = dialog(&dialogs, DialogProps::new(true, body(Rc::default()))).unwrap();
        let close = egui::pos2(475., 125.);
        for (down, blocked) in [(true, press_blocked), (false, release_blocked)] {
            frame.screen.max.x = if blocked && !cover { 470. } else { 800. };
            for _ in 0..2 {
                pointer_frame(&mut frame, &dialogs, vec![], cover && blocked);
            }
            let mut events = attachment_support::click(close);
            events.retain(
                |e| !matches!(e, egui::Event::PointerButton { pressed, .. } if *pressed != down),
            );
            pointer_frame(&mut frame, &dialogs, events, cover && blocked);
        }
        assert!(
            host.removed.borrow().is_empty(),
            "blocked endpoints {press_blocked}/{release_blocked}, layer={cover}"
        );
        owner.close().unwrap();
    }
}

/// Place an actual foreground blocker before the dialog consumes its pointer snapshot.
fn pointer_frame(
    frame: &mut attachment_support::Frame,
    dialogs: &DialogContext,
    events: Vec<egui::Event>,
    covered: bool,
) {
    frame.run(events, |ctx, i, p| {
        if covered {
            egui::Area::new(egui::Id::new("cover"))
                .order(egui::Order::Tooltip)
                .fixed_pos(egui::pos2(460., 110.))
                .show(ctx, |ui| {
                    ui.set_min_size(egui::vec2(40., 40.));
                });
        }
        dialogs.render(i, p).unwrap();
    });
}

/// Observe retained control pixels and submitted group opacity.
fn control_paint(
    frame: &attachment_support::Frame,
) -> (Vec<egui::ColorImage>, Vec<egui::epaint::ClippedShape>) {
    (
        frame.images.clone(),
        frame
            .shapes
            .iter()
            .filter(|s| matches!(s.shape, egui::Shape::Mesh(_)))
            .cloned()
            .collect(),
    )
}

/// Observe focus decoration fading after the keyboard target releases focus.
fn focus_fade(frame: &mut attachment_support::Frame, dialogs: &DialogContext, id: egui::Id) {
    frame.ctx.memory_mut(|m| m.surrender_focus(id));
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!(
        outer_alpha(frame),
        247,
        "outer shadow fades through the same curve"
    );
    assert_ring_spread(frame, 1.934_399, 1.);
    for _ in 0..12 {
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    assert!(ring_paint(frame).is_empty());
}

#[test]
fn dialog_interrupted_hover_paint_is_continuous() {
    for (milliseconds, alpha, rgb, duration, next, half) in [
        (37.5, 197, [223, 177, 130], 0.035_488_084, 183, 183),
        (75., 238, [236, 216, 194], 0.116_334_197, 234, 192),
        (112.5, 252, [241, 229, 215], 0.143_905_161, 249, 195),
    ] {
        let mut frame = attachment_support::Frame::default();
        let host = input_support::Host::new(|_, _| {});
        let dialogs = DialogContext::new(frame.ctx.clone(), host);
        let _owner = dialog(&dialogs, DialogProps::new(true, body(Rc::default()))).unwrap();
        frame.time = Some(0.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.time = Some(1.);
        frame.run(
            vec![egui::Event::PointerMoved(egui::pos2(475., 125.))],
            |_, i, p| dialogs.render(i, p).unwrap(),
        );
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.time = Some(1. + milliseconds / 1000.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        assert_eq!(glyph_alpha(&frame), alpha, "enter at {milliseconds} ms");
        assert_glyph_rgb(&frame, rgb);
        let tint = frame.paint.images(&frame.shapes);
        let before = glyph_alpha(&frame);
        frame.run(
            vec![egui::Event::PointerMoved(egui::pos2(20., 20.))],
            |_, i, p| dialogs.render(i, p).unwrap(),
        );
        assert_eq!(
            glyph_alpha(&frame),
            before,
            "same-time reversal must not jump"
        );
        assert_eq!(
            frame.paint.images(&frame.shapes),
            tint,
            "same-time colour reversal"
        );
        assert_glyph_rgb(&frame, rgb);
        let origin = 1. + milliseconds / 1000.;
        for (elapsed, expected) in [
            (1. / 60., next),
            (duration / 2., half),
            (duration + 0.000_001, 179),
        ] {
            frame.time = Some(origin + elapsed);
            frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
            assert_eq!(glyph_alpha(&frame), expected);
        }
    }
}

/// Compare all native glyph pixels using the recorded unpremultiplied tint operand.
fn assert_glyph_rgb(frame: &attachment_support::Frame, [red, green, blue]: [u8; 3]) {
    let color = egui::Color32::from_rgb(red, green, blue);
    let (native, shapes) = attachment_support::canvas_paint::icon(&frame.ctx, CLOSE, "X", color);
    let vertex = f32::from(glyph_alpha(frame)) / 255.;
    for point in attachment_support::canvas_paint::points(&frame.ctx, CLOSE) {
        let expected = native.pixel(&frame.ctx, &shapes, point);
        let actual = frame.paint.pixel(&frame.ctx, &frame.shapes, point);
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual.round() - (expected * vertex).round()).abs() <= 1.,
                "native glyph tint differs"
            );
        }
    }
}

/// Observe the submitted whole-control opacity.
fn glyph_alpha(frame: &attachment_support::Frame) -> u8 {
    attachment_support::dialog_paint::glyphs(&frame.shapes)
        .filter_map(|mesh| mesh.vertices.iter().map(|v| v.color.a()).max())
        .max()
        .unwrap()
}

#[test]
fn dialog_interrupted_focus_paint_is_continuous() {
    for (milliseconds, spread, duration, next, half) in [
        (37.5, 0.473_174, 0.035_488_084, 0.125_703_6, 0.106_198_6),
        (75., 1.551_123, 0.116_334_197, 1.459_396, 0.348_132),
        (112.5, 1.918_735, 0.143_905_161, 1.849_56, 0.430_638),
    ] {
        let mut frame = attachment_support::Frame::default();
        let host = input_support::Host::new(|_, _| {});
        let dialogs = DialogContext::new(frame.ctx.clone(), host);
        frame.time = Some(0.);
        let (_owner, id) = mount_close_dialog(&mut frame, &dialogs);
        frame.ctx.memory_mut(|m| m.request_focus(id.get()));
        frame.time = Some(1.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.time = Some(1. + milliseconds / 1000.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        let before = ring_paint(&frame);
        assert_ring_spread(&frame, spread, 0.7);
        frame.ctx.memory_mut(|m| m.surrender_focus(id.get()));
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        assert_eq!(ring_paint(&frame), before, "focus reversal must not jump");
        assert_eq!(glyph_alpha(&frame), 179);
        if milliseconds.total_cmp(&75.).is_eq() {
            assert_eq!(
                focus_alpha(&frame),
                170,
                "overlap composited before group opacity"
            );
            let opaque = (255. * spread / 2.).round();
            let vertex = f32::from(egui::Color32::WHITE.gamma_multiply(0.7).a()) / 255.;
            assert_eq!(
                outer_alpha(&frame),
                (opaque * vertex).round().to_u8().unwrap()
            );
        }
        let origin = 1. + milliseconds / 1000.;
        for (elapsed, expected) in [(1. / 60., next), (duration / 2., half)] {
            frame.time = Some(origin + elapsed);
            frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
            assert_ring_spread(&frame, expected, 0.7);
            assert_eq!(glyph_alpha(&frame), 179);
        }
    }
}

/// Read the two painted annuli, excluding the panel border.
fn ring_paint(frame: &attachment_support::Frame) -> Vec<f32> {
    frame.paint.top_rows(&frame.ctx, &frame.shapes, CLOSE)
}

/// Read one outer-annulus pixel above the Close control.
fn outer_alpha(frame: &attachment_support::Frame) -> u8 {
    frame
        .paint
        .alpha(&frame.ctx, &frame.shapes, egui::pos2(475.5, 114.5))
        .round()
        .to_u8()
        .unwrap()
}

/// Compare the Close ring's painted coverage with its fractional spread.
fn assert_ring_spread(frame: &attachment_support::Frame, spread: f32, opacity: f32) {
    attachment_support::dialog_paint::assert_spread(
        frame.paint.top_alpha(&frame.ctx, &frame.shapes, CLOSE),
        spread,
        opacity,
    );
}

#[test]
fn dialog_reopen_starts_with_resting_paint() {
    for (focused, milliseconds, closed_frame) in [false, true].into_iter().flat_map(|f| {
        [75., 150.]
            .into_iter()
            .flat_map(move |t| [false, true].map(move |c| (f, t, c)))
    }) {
        let mut frame = attachment_support::Frame::default();
        let host = input_support::Host::new(|_, _| {});
        let dialogs = DialogContext::new(frame.ctx.clone(), host.clone());
        let previous = egui::Id::new("previous-focus");
        frame.ctx.memory_mut(|m| m.request_focus(previous));
        frame.time = Some(0.);
        let (owner, id) = mount_close_dialog(&mut frame, &dialogs);
        let resting = glyph_alpha(&frame);
        if focused {
            frame.ctx.memory_mut(|m| m.request_focus(id.get()));
        }
        frame.time = Some(1.);
        let events = if focused {
            vec![]
        } else {
            vec![egui::Event::PointerMoved(egui::pos2(475., 125.))]
        };
        frame.run(events, |_, i, p| dialogs.render(i, p).unwrap());
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.time = Some(1. + milliseconds / 1000.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        owner.close().unwrap();
        if closed_frame {
            frame.run(
                vec![egui::Event::PointerMoved(egui::pos2(20., 20.))],
                |_, i, p| dialogs.render(i, p).unwrap(),
            );
            (host.frame.borrow_mut().take().unwrap())().unwrap();
            assert_eq!(frame.ctx.memory(egui::Memory::focused), Some(previous));
        }
        owner.open().unwrap();
        frame.run(
            vec![egui::Event::PointerMoved(egui::pos2(20., 20.))],
            |_, i, p| dialogs.render(i, p).unwrap(),
        );
        assert_eq!(glyph_alpha(&frame), resting, "reopened hover paint");
        assert!(ring_paint(&frame).is_empty(), "reopened focus paint");
        frame.time = Some(2.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        assert_eq!(glyph_alpha(&frame), resting);
        assert!(ring_paint(&frame).is_empty());
    }
}

#[test]
fn dialog_repeated_reversal_keeps_painted_progress() {
    for focused in [false, true] {
        let mut frame = attachment_support::Frame::default();
        let host = input_support::Host::new(|_, _| {});
        let dialogs = DialogContext::new(frame.ctx.clone(), host);
        frame.time = Some(0.);
        let (_owner, id) = mount_close_dialog(&mut frame, &dialogs);
        transition_input(&mut frame, &dialogs, id.get(), [focused, true], 1.);
        frame.time = Some(1.075);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        transition_input(&mut frame, &dialogs, id.get(), [focused, false], 1.075);
        let time = 1.075 + 0.116_334_196_668_009 / 2.;
        frame.time = Some(time);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        let before = if focused {
            frame.paint.top_alpha(&frame.ctx, &frame.shapes, CLOSE)
        } else {
            f32::from(glyph_alpha(&frame))
        };
        transition_input(&mut frame, &dialogs, id.get(), [focused, true], time);
        let after = if focused {
            frame.paint.top_alpha(&frame.ctx, &frame.shapes, CLOSE)
        } else {
            f32::from(glyph_alpha(&frame))
        };
        assert_eq!(before.to_bits(), after.to_bits());
        frame.time = Some(time + 0.123_890_105_427_931 / 2.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        if focused {
            assert_ring_spread(&frame, 1.629_257, 0.7);
        } else {
            assert_eq!(glyph_alpha(&frame), 241);
        }
    }
}

/// Change a real pointer or canvas-focus input at one explicit timestamp.
fn transition_input(
    frame: &mut attachment_support::Frame,
    dialogs: &DialogContext,
    id: egui::Id,
    state: [bool; 2],
    time: f64,
) {
    let [focused, active] = state;
    frame.time = Some(time);
    let events = if focused {
        frame.ctx.memory_mut(|m| {
            if active {
                m.request_focus(id);
            } else {
                m.surrender_focus(id);
            }
        });
        vec![]
    } else {
        vec![egui::Event::PointerMoved(if active {
            egui::pos2(475., 125.)
        } else {
            egui::pos2(20., 20.)
        })]
    };
    frame.run(events, |_, i, p| dialogs.render(i, p).unwrap());
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
}

#[test]
fn dialog_unmount_preserves_another_controls_focus() {
    let mut frame = attachment_support::Frame::default();
    let host = input_support::Host::new(|_, _| {});
    let dialogs = DialogContext::new(frame.ctx.clone(), host);
    let owner = dialog(&dialogs, DialogProps::new(true, body(Rc::default()))).unwrap();
    frame.time = Some(0.);
    let (_sibling, other) = mount_close_dialog(&mut frame, &dialogs);
    frame.ctx.memory_mut(|m| m.request_focus(other.get()));
    frame.time = Some(1.);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    frame.time = Some(1.075);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    let before = ring_paint(&frame);
    owner.close().unwrap();
    assert_eq!(frame.ctx.memory(egui::Memory::focused), Some(other.get()));
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    assert_eq!(ring_paint(&frame), before);
}

#[test]
fn dialog_body_callback_close_does_not_retain_hover_on_reopen() {
    for reopen_in_callback in [false, true] {
        let mut frame = attachment_support::Frame::default();
        let host = input_support::Host::new(|_, _| {});
        let dialogs = DialogContext::new(frame.ctx.clone(), host);
        let slot = Rc::new(std::cell::RefCell::new(
            None::<std::rc::Weak<maestro_web::dialogs::dialog_base::DialogBase>>,
        ));
        let closing = Rc::new(Cell::new(false));
        let callback_slot = slot.clone();
        let callback_closing = closing.clone();
        let mut children = body(Rc::default());
        children.render_content = Rc::new(move |_, _, _, _| {
            if !callback_closing.replace(false) {
                return Ok(());
            }
            let owner = callback_slot.borrow().as_ref().unwrap().upgrade().unwrap();
            owner.close()?;
            if reopen_in_callback {
                owner.open()?;
            }
            Ok(())
        });
        let owner = Rc::new(dialog(&dialogs, DialogProps::new(true, children)).unwrap());
        *slot.borrow_mut() = Some(Rc::downgrade(&owner));
        frame.time = Some(0.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.time = Some(1.);
        frame.run(
            vec![egui::Event::PointerMoved(egui::pos2(475., 125.))],
            |_, i, p| dialogs.render(i, p).unwrap(),
        );
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.time = Some(2.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        assert_eq!(glyph_alpha(&frame), 255);
        closing.set(true);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.run(
            vec![egui::Event::PointerMoved(egui::pos2(20., 20.))],
            |_, i, p| dialogs.render(i, p).unwrap(),
        );
        owner.open().unwrap();
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        assert_eq!(
            glyph_alpha(&frame),
            179,
            "reopened first paint must be resting"
        );
    }
}

#[test]
fn dialog_fractional_focus_annuli_preserve_pixel_coverage() {
    for density in [1., 1.25, 2.] {
        let mut frame = attachment_support::Frame {
            density,
            time: Some(0.),
            ..Default::default()
        };
        let host = input_support::Host::new(|_, _| {});
        let dialogs = DialogContext::new(frame.ctx.clone(), host);
        let (_owner, id) = mount_close_dialog(&mut frame, &dialogs);
        frame.ctx.memory_mut(|m| m.request_focus(id.get()));
        frame.time = Some(1.);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        for (time, spread) in [(0.0375, 0.473_174), (0.075, 1.551_123), (0.1125, 1.918_735)] {
            frame.time = Some(1. + time);
            frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
            assert_ring_spread(&frame, spread, 0.7);
            assert_fractional_pixels(&frame, spread);
            let uploads = frame.paint.ring_uploads(&frame.shapes);
            frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
            assert_eq!(
                frame.paint.ring_uploads(&frame.shapes),
                uploads,
                "unchanged paint reuses its texture without upload"
            );
            assert_eq!(
                uploads,
                if time < 0.05 {
                    2
                } else if time < 0.1 {
                    3
                } else {
                    4
                },
                "one retained texture updated in place"
            );
        }
    }
}

/// Independently integrate circular CSS boundaries over physical pixels, not submitted geometry.
fn assert_fractional_pixels(frame: &attachment_support::Frame, spread: f32) {
    let radius = (frame.pack.presentation(frame.mode).unwrap().radii["control"] / 2.)
        .to_f32()
        .unwrap()
        .min(CLOSE.width() / 2.);
    let density = frame.density;
    let left = (462. * density).floor().to_i32().unwrap();
    let top = (112. * density).floor().to_i32().unwrap();
    let extent = (26. * density).ceil().to_i32().unwrap();
    let (glyph, shapes) =
        attachment_support::canvas_paint::icon(&frame.ctx, CLOSE, "X", egui::Color32::WHITE);
    for row in top..top + extent {
        for col in left..left + extent {
            let origin = egui::pos2(
                col.to_f32().unwrap() / density,
                row.to_f32().unwrap() / density,
            );
            let center = origin + egui::vec2(0.5 / density, 0.5 / density);
            let coverage = glyph.pixel(&frame.ctx, &shapes, center)[3] / 255.;
            let expected = expected_pixel_alpha(origin, density, spread, radius) * (1. - coverage)
                + coverage * 0.7 * 255.;
            let actual = frame.paint.alpha(&frame.ctx, &frame.shapes, center);
            assert!(
                (actual - expected).abs() <= 2.,
                "density {density}, spread {spread}, pixel {col}/{row}: {actual} != {expected}"
            );
        }
    }
}

/// Source shadow alpha and disjoint disk-offset coverage, integrated independently on a 64-square grid.
fn expected_pixel_alpha(origin: egui::Pos2, density: f32, spread: f32, radius: f32) -> f32 {
    let mut alpha = 0.;
    let progress = spread / 2.;
    for row in 0_u16..64 {
        for col in 0_u16..64 {
            let point = origin
                + egui::vec2((f32::from(col) + 0.5) / 64., (f32::from(row) + 0.5) / 64.) / density;
            let offset =
                (point - CLOSE.center()).abs() - (CLOSE.size() / 2. - egui::Vec2::splat(radius));
            let distance = offset.max(egui::Vec2::ZERO).length();
            if distance <= radius {
                continue;
            }
            alpha += if distance <= radius + spread {
                progress * (2. - progress)
            } else if distance <= radius + 2. * spread {
                progress
            } else {
                0.
            };
        }
    }
    alpha / 4096. * 0.7 * 255.
}

#[test]
fn dialog_errors_preserve_sources_and_retained_owners() {
    use maestro_web::{
        I18nError, browser::attachment_content::AttachmentHostError, dialogs::dialog::DialogUiError,
    };
    use std::error::Error as _;
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let context = DialogContext::new(frame.ctx.clone(), host.clone());
    let fail = Rc::new(Cell::new(true));
    let body_fail = fail.clone();
    let body = DialogBody {
        reading: Rc::new(ReadingContent {
            #[cfg(target_arch = "wasm32")]
            mount: Rc::new(|_| unreachable!()),
        }),
        control_sizes: vec![],
        render_content: Rc::new(move |_, _, _, _| {
            if body_fail.get() {
                Err(DialogUiError::I18n(I18nError::NotText("body".into())))
            } else {
                Ok(())
            }
        }),
    };
    let owner = dialog(&context, DialogProps::new(true, body)).unwrap();
    *host.failure.borrow_mut() = Some("bounds");
    frame.run(vec![], |_, i, p| {
        let error = context.render(i, p).unwrap_err();
        assert_eq!(error.to_string(), "bounds");
        assert!(matches!(error, DialogUiError::Host(_)));
        assert!(error.source().unwrap().is::<AttachmentHostError>());
    });
    assert!(host.removed.borrow().is_empty());
    frame.run(vec![], |_, i, p| {
        let error = context.render(i, p).unwrap_err();
        assert_eq!(error.to_string(), "Translation is not text: body");
        assert!(error.source().unwrap().is::<I18nError>());
    });
    fail.set(false);
    frame.run(vec![], |_, i, p| context.render(i, p).unwrap());
    assert!(host.removed.borrow().is_empty());
    owner.close().unwrap();
    assert_eq!(host.removed.borrow().len(), 1);
}

/// The glyph and fractional focus annuli retain their native pixels in one image.
#[test]
fn dialog_close_composition_keeps_fractional_ring_and_glyph_pixels() {
    for name in ["brand.json", "packs/signal.json"] {
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            for (density, clipped) in [1., 1.25, 2.]
                .into_iter()
                .flat_map(|density| [false, true].map(move |clipped| (density, clipped)))
            {
                close_composition_case(name, mode, density, clipped);
            }
        }
    }
}

/// Drive a real containing dialog through each fractional focus frame.
fn close_composition_case(name: &str, mode: maestro_theme::BrandMode, density: f32, clipped: bool) {
    let mut frame = attachment_support::Frame {
        pack: maestro_theme::load_brand_pack(name, &brand::Pack).unwrap(),
        mode,
        density,
        time: Some(0.),
        ..Default::default()
    };
    if clipped {
        frame.screen.max.x = CLOSE.center().x;
    }
    let host = input_support::Host::new(|_, _| {});
    let dialogs = DialogContext::new(frame.ctx.clone(), host);
    let (_owner, id) = mount_close_dialog(&mut frame, &dialogs);
    frame.ctx.memory_mut(|m| m.request_focus(id.get()));
    frame.time = Some(1.);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    for (time, spread) in [(0.0375, 0.473_174), (0.075, 1.551_123), (0.1125, 1.918_735)] {
        frame.time = Some(1. + time);
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
        assert_eq!(
            attachment_support::dialog_paint::glyphs(&frame.shapes).count(),
            1
        );
        assert_close_glyph(&frame);
        if !clipped {
            assert_fractional_pixels(&frame, spread);
        }
        assert_close_shadow_colors(&frame, spread);
    }
}

/// Compare the real composed glyph to the original independent SVG producer.
fn assert_close_glyph(frame: &attachment_support::Frame) {
    let p = frame.pack.presentation(frame.mode).unwrap();
    let color = egui::Color32::from_hex(p.colors["muted"]).unwrap();
    let (native, shapes) = attachment_support::canvas_paint::icon(&frame.ctx, CLOSE, "X", color);
    let interior = CLOSE.shrink(2.);
    let vertex = f32::from(egui::Color32::WHITE.gamma_multiply(0.7).a()) / 255.;
    for point in attachment_support::canvas_paint::points(&frame.ctx, CLOSE) {
        if !interior.contains(point) {
            assert!(
                native
                    .pixel(&frame.ctx, &shapes, point)
                    .iter()
                    .all(|value| value.abs() < f32::EPSILON)
            );
            continue;
        }
        let expected = if frame.screen.contains(point) {
            native.pixel(&frame.ctx, &shapes, point).map(|v| v * vertex)
        } else {
            [0.; 4]
        };
        let actual = frame.paint.pixel(&frame.ctx, &frame.shapes, point);
        for (a, b) in actual.into_iter().zip(expected) {
            assert!(
                (a.round() - b.round()).abs() <= 1.,
                "native Close glyph differs at {point:?}: actual={actual:?}, expected={expected:?}, density={}",
                frame.density
            );
        }
    }
}

/// Check the two shadow colors independently of their retained raster.
fn assert_close_shadow_colors(frame: &attachment_support::Frame, spread: f32) {
    let p = frame.pack.presentation(frame.mode).unwrap();
    let background = egui::Color32::from_hex(p.colors["background"])
        .unwrap()
        .to_array();
    let accent = egui::Color32::from_hex(p.colors["accent"])
        .unwrap()
        .to_array();
    let progress = spread / 2.;
    let region = egui::Rect::from_min_max(egui::pos2(474., 112.), egui::pos2(475., 117.));
    for point in attachment_support::canvas_paint::points(&frame.ctx, region) {
        let origin = point - egui::vec2(0.5, 0.5) / frame.density;
        let mut coverage = [0.; 2];
        for (x, y) in (0_u16..64).flat_map(|y| (0_u16..64).map(move |x| (x, y))) {
            let sample = origin
                + egui::vec2((f32::from(x) + 0.5) / 64., (f32::from(y) + 0.5) / 64.)
                    / frame.density;
            let distance = CLOSE.top() - sample.y;
            if distance > 0. && distance <= spread {
                coverage[0] += 1. / 4096.;
            } else if distance > spread && distance <= 2. * spread {
                coverage[1] += 1. / 4096.;
            }
        }
        let actual = frame.paint.pixel(&frame.ctx, &frame.shapes, point);
        for i in 0..3 {
            let inner = f32::from(background[i]) * progress
                + f32::from(accent[i]) * progress * (1. - progress);
            let outer = f32::from(accent[i]) * progress;
            let expected = (inner * coverage[0] + outer * coverage[1]) * 0.7;
            assert!(
                (actual[i] - expected).abs() <= 2.,
                "Close shadow color differs"
            );
        }
    }
}
