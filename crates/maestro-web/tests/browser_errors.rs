//! Typed operation failures through their public owners.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_web::{
    I18nError,
    browser::attachment_content::{AttachmentHostError, ControlError},
    dialogs::attachment_overlay::AttachmentUiError,
    icons::{IconError, IconSize, icon},
    tools::artifacts::{ArtifactUiError, PresentationCause},
};
use std::error::Error;

/// Obtain an actual parser failure without modifying the catalog.
fn icon_error() -> IconError {
    icon(
        &vec![("svg:circle".into(), indexmap::IndexMap::new())],
        IconSize::Md,
        egui::Color32::WHITE,
        1.,
    )
    .unwrap_err()
}

#[test]
fn operation_errors_preserve_literal_messages_and_leaf_sources() {
    for message in ["", " ", "<literal> café"] {
        let errors: Vec<Box<dyn Error>> = vec![
            Box::new(AttachmentHostError::Host(message.into())),
            Box::new(AttachmentHostError::Presentation(message.into())),
            Box::new(ControlError::Host(message.into())),
            Box::new(ControlError::Presentation(message.into())),
            Box::new(AttachmentUiError::Presentation(message.into())),
            Box::new(ArtifactUiError::Presentation(PresentationCause::from(
                message.to_owned(),
            ))),
        ];
        for error in errors {
            assert_eq!(error.to_string(), message);
            assert!(error.source().is_none());
        }
    }
    for error in [
        Box::new(AttachmentHostError::BorrowConflict) as Box<dyn Error>,
        Box::new(AttachmentUiError::BorrowConflict),
        Box::new(ArtifactUiError::BorrowConflict),
    ] {
        assert_eq!(error.to_string(), "Attachment is already borrowed");
        assert!(error.source().is_none());
    }
    for message in ["", " ", "<literal> café"] {
        for make in [ControlError::Host, ControlError::Presentation] {
            let errors: Vec<Box<dyn Error>> = vec![
                Box::new(AttachmentUiError::Control(make(message.into()))),
                Box::new(ArtifactUiError::Control(make(message.into()))),
            ];
            for error in errors {
                assert_eq!(error.to_string(), message);
                assert!(error.source().is_none());
            }
        }
    }
    leaf_sources();
}

/// Check each public operation without relying on a source-chain length.
fn leaf_sources() {
    let localization: Vec<Box<dyn Error>> = vec![
        Box::new(AttachmentUiError::from(I18nError::NotText("*".into()))),
        Box::new(ArtifactUiError::from(I18nError::NotText("*".into()))),
    ];
    for error in localization {
        assert_eq!(error.to_string(), "Translation is not text: *");
        assert!(error.source().unwrap().is::<I18nError>());
    }
    let controls: Vec<Box<dyn Error>> = vec![
        Box::new(ControlError::from(icon_error())),
        Box::new(AttachmentUiError::from(ControlError::from(icon_error()))),
        Box::new(ArtifactUiError::from(ControlError::from(icon_error()))),
    ];
    for error in controls {
        let original = error.source().unwrap().downcast_ref::<IconError>().unwrap();
        assert_eq!(error.to_string(), original.to_string());
        assert!(original.source().is_some());
    }
    let hosts: Vec<Box<dyn Error>> = vec![
        Box::new(AttachmentUiError::from(AttachmentHostError::BorrowConflict)),
        Box::new(ArtifactUiError::from(AttachmentHostError::BorrowConflict)),
    ];
    for error in hosts {
        assert_eq!(error.to_string(), "Attachment is already borrowed");
        assert!(matches!(
            error
                .source()
                .unwrap()
                .downcast_ref::<AttachmentHostError>(),
            Some(AttachmentHostError::BorrowConflict)
        ));
    }
}

use crate::attachment_support;
use crate::highlighter_support;
use crate::input_support;

/// Reach each artifact's real localized control before any native body work.
fn artifact_control_cause(filename: &str) {
    let host = input_support::Host::new(|_, _| panic!("header must not publish a native body"));
    let mut artifact =
        maestro_web::tools::artifacts::create_artifact_element(filename, "", host).unwrap();
    let mut frame = attachment_support::Frame::default();
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut presentation = presentation.clone();
        presentation.colors.insert("background", "not-a-color");
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("artifact-errors"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        let error = artifact
            .get_header_buttons(
                &mut ui,
                maestro_web::tools::artifacts::artifact_element::ArtifactView {
                    i18n,
                    presentation: &presentation,
                    highlighter: &highlighter_support::NoHighlight,
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            ArtifactUiError::Presentation(PresentationCause::Control(_))
        ));
        let control = error
            .source()
            .unwrap()
            .downcast_ref::<maestro_web::components::copy_button::LocalizedControlError>()
            .unwrap();
        let cause = control
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap();
        assert_eq!(cause.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(cause.to_string(), error.to_string());
    });
}

#[test]
fn markdown_artifact_preparation_keeps_original_control_cause() {
    artifact_control_cause("a.md");
}
#[test]
fn svg_artifact_preparation_keeps_original_control_cause() {
    artifact_control_cause("a.svg");
}
#[test]
fn text_artifact_preparation_keeps_original_control_cause() {
    artifact_control_cause("a.txt");
}

use crate::text_image_support;

#[test]
fn markdown_artifact_preparation_keeps_original_highlight_cause() {
    let host = input_support::Host::new(|_, _| panic!("failed preparation published a body"));
    let mut artifact = maestro_web::tools::artifacts::create_artifact_element(
        "a.md",
        "```rust\nlet value = 1;\n```",
        host,
    )
    .unwrap();
    let highlighter = text_image_support::Highlighter::default();
    highlighter.fail.set(true);
    let mut frame = attachment_support::Frame::default();
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("highlight-error"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        let error = artifact
            .render(
                &mut ui,
                maestro_web::tools::artifacts::artifact_element::ArtifactView {
                    i18n,
                    presentation,
                    highlighter: &highlighter,
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            ArtifactUiError::Presentation(PresentationCause::Reading(_))
        ));
        let reading = error
            .source()
            .unwrap()
            .downcast_ref::<maestro_web::components::code_block::ReadingUiError>()
            .unwrap();
        assert!(matches!(
            reading,
            maestro_web::components::code_block::ReadingUiError::Highlight(_)
        ));
        let cause = reading
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap();
        assert_eq!(cause.to_string(), "engine failure");
        assert_eq!(error.to_string(), cause.to_string());
    });
    assert_eq!(highlighter.requests.borrow().len(), 1);
}

#[test]
fn markdown_native_failures_keep_the_host_phase() {
    let host = input_support::Host::new(|_, _| {});
    *host.failure.borrow_mut() = Some("measure");
    let mut artifact = maestro_web::tools::artifacts::create_artifact_element(
        "a.md",
        "literal body",
        host.clone(),
    )
    .unwrap();
    let mut frame = attachment_support::Frame::default();
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("native-error"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        let view = maestro_web::tools::artifacts::artifact_element::ArtifactView {
            i18n,
            presentation,
            highlighter: &highlighter_support::NoHighlight,
        };
        let error = artifact.render(&mut ui, view).unwrap_err();
        assert!(
            matches!(error, ArtifactUiError::Host(AttachmentHostError::Host(_))),
            "native publication must remain a host failure"
        );
        assert_eq!(error.to_string(), "measure");
        assert!(error.source().unwrap().is::<AttachmentHostError>());
        artifact.render(&mut ui, view).unwrap();
    });
    assert!(host.removed.borrow().is_empty());
    drop(artifact);
    assert_eq!(host.removed.borrow().len(), 2);
}

#[test]
fn artifact_localization_failures_keep_the_leaf_phase() {
    for (filename, header, key, content) in [
        ("a.md", true, "Preview", "body"),
        ("a.svg", true, "Preview", ""),
        ("a.md", false, "Copy", "```rust\nlet value = 1;\n```"),
    ] {
        let host =
            input_support::Host::new(|_, _| panic!("localization failed before publication"));
        let mut artifact =
            maestro_web::tools::artifacts::create_artifact_element(filename, content, host)
                .unwrap();
        let mut frame = attachment_support::Frame::default();
        frame.run(vec![], |ctx, i18n, presentation| {
            let english = i18n.get_translations().borrow()["en"].clone();
            let original = english.borrow_mut().insert(
                key.into(),
                maestro_web::TranslationValue::Category(std::rc::Rc::default()),
            );
            let mut ui = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("leaf-phase"),
                egui::UiBuilder::new().max_rect(ctx.content_rect()),
            );
            let highlighter = text_image_support::Highlighter::default();
            let view = maestro_web::tools::artifacts::artifact_element::ArtifactView {
                i18n,
                presentation,
                highlighter: &highlighter,
            };
            let result = if header {
                artifact.get_header_buttons(&mut ui, view)
            } else {
                artifact.render(&mut ui, view)
            };
            if let Some(original) = original {
                english.borrow_mut().insert(key.into(), original);
            }
            let error = result.unwrap_err();
            assert!(
                matches!(error, ArtifactUiError::I18n(I18nError::NotText(_))),
                "localization must remain a leaf failure"
            );
            assert_eq!(error.to_string(), format!("Translation is not text: {key}"));
            assert!(error.source().unwrap().is::<I18nError>());
        });
    }
}

#[test]
fn console_presentation_failures_keep_the_original_cause() {
    let host =
        input_support::Host::new(|_, _| panic!("invalid console style published native content"));
    let mut console = maestro_web::components::console_block::ConsoleBlock::new(host);
    let mut frame = attachment_support::Frame::default();
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut presentation = presentation.clone();
        presentation.colors.insert("background", "not-a-color");
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("console-error"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        let error = console
            .render(&mut ui, i18n, &presentation, None)
            .unwrap_err();
        assert!(matches!(
            error,
            maestro_web::components::code_block::ReadingUiError::Presentation(_)
        ));
        let cause = error
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap();
        assert_eq!(cause.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(error.to_string(), cause.to_string());
    });
}
