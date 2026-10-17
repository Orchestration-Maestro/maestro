//! Owned dialog lifecycle through the replaceable content host.
#![cfg(test)]
pub mod attachment_support;
pub mod input_support;
mod support;
use maestro_web::{
    browser::attachment_content::ReadingContent,
    dialogs::{
        dialog::{DialogBody, DialogProps, dialog},
        dialog_base::DialogContext,
    },
};
use std::{cell::Cell, rc::Rc};

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
    let modal = Rc::new(std::cell::Cell::new(egui::Id::NULL));
    let id = modal.clone();
    let mut children = body(Rc::default());
    children.render_content = Rc::new(move |_, modal, _, _| {
        id.set(modal);
        Ok(())
    });
    let _owner = dialog(&dialogs, DialogProps::new(true, children)).unwrap();
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    let close = egui::Rect::from_min_size(egui::pos2(467., 117.), egui::vec2(16., 16.));
    assert!(
        frame.shapes.iter().any(
            |shape| matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.calc_bounds()==close)
        )
    );
    let resting_images = control_paint(&frame);
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    frame.run(
        vec![egui::Event::PointerMoved(close.center())],
        |_, i, p| dialogs.render(i, p).unwrap(),
    );
    for _ in 0..12 {
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    assert_ne!(control_paint(&frame), resting_images);
    frame
        .ctx
        .memory_mut(|memory| memory.request_focus(modal.get().with("close")));
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    let focus: Vec<_> = frame
        .shapes
        .iter()
        .filter(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if (rect.stroke.width-2.).abs()<f32::EPSILON))
        .collect();
    assert_eq!(focus.len(), 2);
    for shape in focus {
        let primitives = frame.ctx.tessellate(vec![shape.clone()], 1.);
        assert!(primitives.iter().any(|p| matches!(&p.primitive, egui::epaint::Primitive::Mesh(mesh) if !mesh.indices.is_empty()&&mesh.calc_bounds().intersect(p.clip_rect).is_positive())));
    }
}

#[test]
fn dialog_close_focus_paint_has_whole_control_opacity_and_transition() {
    let host = input_support::Host::new(|_, _| {});
    let mut frame = attachment_support::Frame::default();
    let dialogs = DialogContext::new(frame.ctx.clone(), host);
    let modal = Rc::new(Cell::new(egui::Id::NULL));
    let id = modal.clone();
    let mut children = body(Rc::default());
    children.render_content = Rc::new(move |_, modal, _, _| {
        id.set(modal);
        Ok(())
    });
    let _owner = dialog(&dialogs, DialogProps::new(true, children)).unwrap();
    frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    let resting_glyph = frame.images.clone();
    frame
        .ctx
        .memory_mut(|m| m.request_focus(modal.get().with("close")));
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
    focus_fade(&mut frame, &dialogs, modal.get().with("close"));
}

/// Read maximum alpha from visible tessellated focus-ring paint.
fn focus_alpha(frame: &attachment_support::Frame) -> u8 {
    let shapes: Vec<_> = frame
        .shapes
        .iter()
        .filter(|s| matches!(&s.shape, egui::Shape::Rect(r) if (r.stroke.width - 2.).abs() < f32::EPSILON))
        .cloned()
        .collect();
    frame
        .ctx
        .tessellate(shapes, 1.)
        .iter()
        .filter_map(|p| {
            let egui::epaint::Primitive::Mesh(mesh) = &p.primitive else {
                return None;
            };
            mesh.vertices
                .iter()
                .filter(|v| p.clip_rect.contains(v.pos))
                .map(|v| v.color.a())
                .max()
        })
        .max()
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

/// Observe both raster tint and the opacity emitted on the glyph mesh.
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
    assert_eq!(
        focus_alpha(frame),
        247,
        "focus shadow fades through the same curve"
    );
    for _ in 0..12 {
        frame.run(vec![], |_, i, p| dialogs.render(i, p).unwrap());
    }
    assert!(!frame.shapes.iter().any(
        |s| matches!(&s.shape, egui::Shape::Rect(r) if (r.stroke.width-2.).abs() < f32::EPSILON)
    ));
}
