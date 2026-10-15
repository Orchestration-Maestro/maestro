//! Browser Input operands at the native presentation boundary.
#![cfg(test)]
/// Shared drawing conformance support.
pub mod attachment_support;
mod input_support;
mod support;
use maestro_theme::BrandPresentation;
use maestro_web::{
    I18n, Input,
    browser::attachment_content::{
        AttachmentContent, AttachmentHost, AttachmentSurface, AttachmentUiError,
    },
    components::input::{InputProps, InputSize, InputType},
};
use std::{cell::Cell, rc::Rc};

/// Assert source defaults on the actual presented update.
#[derive(Default)]
struct DefaultHost {
    /// Number of admitted presentations.
    presentations: Cell<usize>,
    /// Number of released surfaces.
    removals: Cell<usize>,
}
impl AttachmentHost for DefaultHost {
    fn reading_layout(
        &self,
        _: egui::Id,
    ) -> Result<maestro_web::browser::attachment_content::ReadingLayout, AttachmentUiError> {
        unreachable!("this suite does not create reading content")
    }
    fn present(
        &self,
        surface: AttachmentSurface<'_>,
        _: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        let AttachmentContent::Input {
            props,
            value_changed,
            ..
        } = surface.content
        else {
            panic!("not input")
        };
        assert!(matches!(props.input_type, InputType::Text));
        assert!(matches!(props.size, InputSize::Md));
        for text in [
            &props.value,
            &props.placeholder,
            &props.label,
            &props.error,
            &props.name,
            &props.autocomplete,
            &props.class_name,
        ] {
            assert!(text.is_empty());
        }
        assert!(!props.disabled);
        assert!(!props.required);
        assert!(props.min.is_none());
        assert!(props.max.is_none());
        assert!(props.step.is_none());
        assert!(
            [
                props.input_ref.is_none(),
                props.on_input.is_none(),
                props.on_change.is_none(),
                props.on_key_down.is_none(),
                props.on_key_up.is_none()
            ]
            .into_iter()
            .all(std::convert::identity)
        );
        assert!((surface.rect.height() - 36.).abs() < f32::EPSILON);
        assert!(value_changed);
        self.presentations.set(self.presentations.get() + 1);
        Ok(())
    }
    fn set_overlay(&self, _: egui::Id, _: bool) -> Result<(), AttachmentUiError> {
        Ok(())
    }
    fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
        Ok(false)
    }
    fn download_bytes(&self, _: &[u8], _: &str, _: &str) -> Result<(), AttachmentUiError> {
        unreachable!("unexpected byte download")
    }
    fn download(
        &self,
        _: Rc<std::cell::RefCell<maestro_web::Attachment>>,
    ) -> Result<(), AttachmentUiError> {
        unreachable!()
    }
    fn take_escape(&self) -> bool {
        false
    }
    fn timer(
        &self,
        _: i32,
        _: bool,
        _: Rc<dyn Fn()>,
    ) -> Result<maestro_web::browser::timer::Timer, AttachmentUiError> {
        unreachable!()
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!()
    }
    fn remove(&self, _: egui::Id) {
        self.removals.set(self.removals.get() + 1);
    }
}

#[test]
fn input_defaults_keep_native_text_and_medium_size() {
    let host = Rc::new(DefaultHost::default());
    let mut input = Input::new(InputProps::default(), host.clone());
    let context = egui::Context::default();
    let pack = maestro_theme::load_brand_pack(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json"),
        &maestro_theme::theme::NativeThemeOperations,
    )
    .unwrap();
    let i18n: I18n = I18n::new(Rc::new(support::Host::default())).unwrap();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        input.render(ui, &i18n, &presentation).unwrap();
    });
    output.textures_delta.clear();
    assert_eq!(host.presentations.get(), 1);
    drop(input);
    assert_eq!(host.removals.get(), 1);
}

/// Strict source-derived mode/size projection.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SizeCase {
    /// Native mode spelling.
    mode: String,
    /// Source size spelling.
    size: String,
    /// Independent source height.
    height: f32,
}
/// Decode a supported SDK mode without accepting unknown fixture values.
fn mode(s: &str) -> InputType {
    match s {
        "text" => InputType::Text,
        "email" => InputType::Email,
        "password" => InputType::Password,
        "number" => InputType::Number,
        "url" => InputType::Url,
        "tel" => InputType::Tel,
        "search" => InputType::Search,
        _ => panic!("unknown mode"),
    }
}
/// Read a mode back at the public boundary.
fn mode_name(v: InputType) -> &'static str {
    match v {
        InputType::Text => "text",
        InputType::Email => "email",
        InputType::Password => "password",
        InputType::Number => "number",
        InputType::Url => "url",
        InputType::Tel => "tel",
        InputType::Search => "search",
    }
}
#[test]
fn input_types_and_sizes_forward_every_supported_choice() {
    let cases: Vec<SizeCase> =
        serde_json::from_str(include_str!("fixtures/input_sizes.json")).unwrap();
    for case in cases {
        let kind = mode(&case.mode);
        let size = match case.size.as_str() {
            "sm" => InputSize::Sm,
            "md" => InputSize::Md,
            "lg" => InputSize::Lg,
            _ => panic!("unknown size"),
        };
        let host = input_support::Host::new(move |s, _| {
            let AttachmentContent::Input { props, .. } = &s.content else {
                panic!("not input")
            };
            assert_eq!(mode_name(props.input_type), case.mode);
            assert!((s.rect.height() - case.height).abs() < f32::EPSILON);
        });
        input_support::render(
            &mut Input::new(
                InputProps {
                    input_type: kind,
                    size,
                    ..Default::default()
                },
                host,
            ),
            320.,
        );
    }
}

#[test]
fn input_labels_required_sign_and_errors_follow_independent_fields() {
    for (label, error) in ["", " ", "<label>"]
        .into_iter()
        .flat_map(|label| ["", " ", "<error>"].map(move |error| (label, error)))
    {
        for required in [false, true] {
            let host = input_support::Host::new(|_, _| {});
            let mut input = Input::new(
                InputProps {
                    label: label.into(),
                    error: error.into(),
                    required,
                    ..Default::default()
                },
                host,
            );
            let texts = input_support::render(&mut input, 320.);
            let expected: Vec<String> = [
                (!label.is_empty()).then_some(label),
                (!label.is_empty() && required).then_some("*"),
                (!error.is_empty()).then_some(error),
            ]
            .into_iter()
            .flatten()
            .map(str::to_owned)
            .collect();
            assert_eq!(texts, expected);
        }
    }
    localized_required_marker();
}

#[test]
fn input_disabled_and_required_properties_reach_the_element() {
    let values = Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = values.clone();
    let host = input_support::Host::new(move |s, _| {
        let AttachmentContent::Input { props, .. } = &s.content else {
            panic!("not input")
        };
        seen.borrow_mut().push((props.disabled, props.required));
    });
    let mut input = Input::new(
        InputProps {
            disabled: true,
            ..Default::default()
        },
        host,
    );
    input_support::render(&mut input, 320.);
    for (disabled, required) in [(false, true), (true, true), (true, false), (false, false)] {
        input.set_props(InputProps {
            disabled,
            required,
            ..Default::default()
        });
        input_support::render(&mut input, 320.);
    }
    assert_eq!(
        *values.borrow(),
        [
            (true, false),
            (false, true),
            (true, true),
            (true, false),
            (false, false)
        ]
    );
}

#[test]
fn input_native_properties_keep_name_autocomplete_and_numeric_bounds() {
    for n in [
        None,
        Some(0.),
        Some(-0.),
        Some(-2.5),
        Some(1e-7),
        Some(1e-6),
        Some(1e20),
        Some(1e21),
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(f64::NEG_INFINITY),
    ] {
        let host = input_support::Host::new(move |s, _| {
            let AttachmentContent::Input { props, .. } = &s.content else {
                panic!("not input")
            };
            assert_eq!(
                (
                    &*props.value,
                    &*props.placeholder,
                    &*props.name,
                    &*props.autocomplete
                ),
                (" <value> ", " hint ", "field", "off")
            );
            for bound in [props.min, props.max, props.step] {
                assert_eq!(bound.map(f64::to_bits), n.map(f64::to_bits));
            }
        });
        let mut input = Input::new(
            InputProps {
                value: " <value> ".into(),
                placeholder: " hint ".into(),
                name: "field".into(),
                autocomplete: "off".into(),
                min: n,
                max: n,
                step: n,
                ..Default::default()
            },
            host,
        );
        input_support::render(&mut input, 320.);
    }
}

#[test]
fn input_events_preserve_callback_identity_and_optional_noops() {
    use maestro_web::components::input::{EventCallback, KeyCallback};
    let events: [EventCallback; 2] = [Rc::new(|_| {}), Rc::new(|_| {})];
    let keys: [KeyCallback; 2] = [Rc::new(|_| {}), Rc::new(|_| {})];
    let expected_events = events.clone();
    let expected_keys = keys.clone();
    let host = input_support::Host::new(move |s, _| {
        let AttachmentContent::Input { props, .. } = &s.content else {
            panic!("not input")
        };
        for (actual, expected) in [
            (&props.on_input, &expected_events[0]),
            (&props.on_change, &expected_events[1]),
        ] {
            assert!(Rc::ptr_eq(actual.as_ref().unwrap(), expected));
        }
        for (actual, expected) in [
            (&props.on_key_down, &expected_keys[0]),
            (&props.on_key_up, &expected_keys[1]),
        ] {
            assert!(Rc::ptr_eq(actual.as_ref().unwrap(), expected));
        }
    });
    let mut input = Input::new(
        InputProps {
            on_input: Some(events[0].clone()),
            on_change: Some(events[1].clone()),
            on_key_down: Some(keys[0].clone()),
            on_key_up: Some(keys[1].clone()),
            ..Default::default()
        },
        host,
    );
    input_support::render(&mut input, 320.);
}

#[test]
fn input_ref_retains_the_real_element_across_renders() {
    let slot = Rc::new(std::cell::RefCell::new(None));
    let expected = slot.clone();
    let values = Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = values.clone();
    let host = input_support::Host::new(move |s, _| {
        let AttachmentContent::Input {
            props,
            value_changed,
            ..
        } = &s.content
        else {
            panic!("not input")
        };
        assert!(Rc::ptr_eq(props.input_ref.as_ref().unwrap(), &expected));
        seen.borrow_mut()
            .push((s.id, *value_changed, props.value.clone()));
    });
    let mut input = Input::new(
        InputProps {
            input_ref: Some(slot.clone()),
            ..Default::default()
        },
        host,
    );
    input_support::render(&mut input, 320.);
    input_support::render(&mut input, 320.);
    input.set_props(InputProps {
        input_ref: Some(slot),
        value: "replacement".into(),
        ..Default::default()
    });
    input_support::render(&mut input, 320.);
    let values = values.borrow();
    assert!(values.iter().all(|v| v.0 == values[0].0));
    assert_eq!(
        values
            .iter()
            .map(|v| (&v.1, v.2.as_str()))
            .collect::<Vec<_>>(),
        [(&true, ""), (&false, ""), (&true, "replacement")]
    );
}

#[test]
fn input_styles_use_caller_tokens_without_replacing_layout() {
    for mode in [
        maestro_theme::BrandMode::Light,
        maestro_theme::BrandMode::Dark,
    ] {
        let mut frame = attachment_support::Frame {
            mode,
            density: 1.5,
            ..Default::default()
        };
        let host = input_support::Host::new(move |s, p| {
            assert!((s.pixels_per_point - 1.5).abs() < f32::EPSILON);
            assert!((s.rect.width() - 274.5).abs() < f32::EPSILON);
            assert!((s.rect.height() - 40.).abs() < f32::EPSILON);
            let css = p.css_properties();
            for role in ["text", "border", "muted", "accent", "on-accent", "error"] {
                assert_eq!(css[&format!("--maestro-color-{role}")], p.colors[role]);
            }
        });
        let mut input = Input::new(
            InputProps {
                size: InputSize::Lg,
                error: "error".into(),
                ..Default::default()
            },
            host,
        );
        frame.run(vec![], |ctx, i, p| {
            let mut ui = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("styles"),
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    egui::pos2(10.25, 20.5),
                    egui::vec2(274.5, 400.),
                )),
            );
            let caller_spacing = ui.spacing().item_spacing;
            input.render(&mut ui, i, p).unwrap();
            assert_eq!(ui.spacing().item_spacing, caller_spacing);
            assert_eq!(
                ui.visuals().selection.bg_fill,
                egui::Color32::from_hex(p.colors["accent"]).unwrap()
            );
            assert_eq!(
                ui.visuals().selection.stroke.color,
                egui::Color32::from_hex(p.colors["on-accent"]).unwrap()
            );
        });
        assert_eq!(frame.texts(), ["error"]);
    }
}

#[test]
fn input_literal_text_and_wrapper_class_are_not_rewritten() {
    let literal = " <b>&\"\u{feff}\u{85} ";
    let host = input_support::Host::new(move |s, _| {
        let AttachmentContent::Input { props, .. } = &s.content else {
            panic!("not input")
        };
        assert_eq!(
            (
                &*props.label,
                &*props.error,
                &*props.value,
                &*props.placeholder
            ),
            (literal, literal, literal, literal)
        );
        assert_eq!(props.class_name, "flex-1 custom");
        assert!((s.rect.width() - 240.).abs() < f32::EPSILON);
    });
    let mut input = Input::new(
        InputProps {
            label: literal.into(),
            error: literal.into(),
            value: literal.into(),
            placeholder: literal.into(),
            class_name: "flex-1 custom".into(),
            ..Default::default()
        },
        host,
    );
    assert_eq!(input_support::render(&mut input, 240.), [literal, literal]);
}

#[test]
fn input_replacement_and_drop_release_only_owned_refs() {
    let old = Rc::new(std::cell::RefCell::new(None));
    let new = Rc::new(std::cell::RefCell::new(None));
    let refs = Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = refs.clone();
    let host = input_support::Host::new(move |s, _| {
        let AttachmentContent::Input { props, .. } = &s.content else {
            panic!("not input")
        };
        seen.borrow_mut().push((s.id, props.input_ref.clone()));
    });
    let mut input = Input::new(
        InputProps {
            input_ref: Some(old.clone()),
            ..Default::default()
        },
        host.clone(),
    );
    input_support::render(&mut input, 320.);
    input.set_props(InputProps {
        input_ref: Some(new.clone()),
        ..Default::default()
    });
    input_support::render(&mut input, 320.);
    input.set_props(InputProps::default());
    input_support::render(&mut input, 320.);
    drop(input);
    let refs = refs.borrow();
    assert!(Rc::ptr_eq(refs[0].1.as_ref().unwrap(), &old));
    assert!(Rc::ptr_eq(refs[1].1.as_ref().unwrap(), &new));
    assert!(refs[2].1.is_none());
    assert_eq!(*host.removed.borrow(), [refs[0].0]);
    assert!(refs.iter().all(|r| r.0 == refs[0].0));
}

/// Exercise the actual localized required marker without shared dictionary state.
fn localized_required_marker() {
    let mut frame = attachment_support::Frame::default();
    let translations = frame.i18n.translations();
    let english = Rc::clone(&translations.borrow()["en"]);
    english.borrow_mut().insert(
        "*".into(),
        maestro_web::TranslationValue::Text("Pflicht".into()),
    );
    let host = input_support::Host::new(|_, _| {});
    let mut input = Input::new(
        InputProps {
            label: "Label".into(),
            required: true,
            ..Default::default()
        },
        host,
    );
    frame.run(vec![], |ctx, i, p| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("localized"),
            egui::UiBuilder::new(),
        );
        input.render(&mut ui, i, p).unwrap();
    });
    assert_eq!(frame.texts(), ["Label", "Pflicht"]);
}

#[test]
fn input_native_label_and_error_update_and_clear_with_localized_required_marker() {
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let observed = seen.clone();
    let host = input_support::Host::new(move |surface, _| {
        let AttachmentContent::Input {
            accessible_label,
            props,
            ..
        } = &surface.content
        else {
            panic!("not input")
        };
        observed
            .borrow_mut()
            .push((accessible_label.to_string(), props.error.clone()));
    });
    let mut frame = attachment_support::Frame::default();
    frame.i18n.translations().borrow()["en"]
        .borrow_mut()
        .insert(
            "*".into(),
            maestro_web::TranslationValue::Text("Pflicht".into()),
        );
    let mut input = Input::new(InputProps::default(), host);
    for (label, required, error) in [
        ("Email", true, "Invalid address"),
        ("Phone", false, " "),
        ("", true, ""),
    ] {
        input.set_props(InputProps {
            label: label.into(),
            required,
            error: error.into(),
            ..Default::default()
        });
        frame.run(vec![], |ctx, i, p| {
            let mut ui = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("semantics"),
                egui::UiBuilder::new(),
            );
            input.render(&mut ui, i, p).unwrap();
        });
    }
    assert_eq!(
        *seen.borrow(),
        [
            ("Email Pflicht".into(), "Invalid address".into()),
            ("Phone".into(), " ".into()),
            (String::new(), String::new())
        ]
    );
}

#[test]
fn input_stacks_in_horizontal_parent_without_changing_caller_spacing() {
    let field = Rc::new(std::cell::Cell::new(egui::Rect::NOTHING));
    let seen = field.clone();
    let host = input_support::Host::new(move |surface, _| seen.set(surface.rect));
    let mut input = Input::new(
        InputProps {
            label: "Label".into(),
            error: "Error".into(),
            ..Default::default()
        },
        host,
    );
    let mut frame = attachment_support::Frame::default();
    frame.run(vec![], |ctx, i, p| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("horizontal-parent"),
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(10., 20.),
                    egui::vec2(240., 400.),
                ))
                .layout(egui::Layout::left_to_right(egui::Align::Min)),
        );
        ui.spacing_mut().item_spacing = egui::vec2(13., 17.);
        input.render(&mut ui, i, p).unwrap();
        assert_eq!(ui.spacing().item_spacing, egui::vec2(13., 17.));
        let after = ui.label("After");
        assert!(after.rect.left() >= field.get().right());
    });
    let label = frame.label("Label");
    let error = frame.label("Error");
    assert!(label.bottom() < field.get().top());
    assert!(field.get().bottom() < error.top());
    assert!((label.left() - field.get().left()).abs() < 1.);
    assert!((error.left() - field.get().left()).abs() < 1.);
}
