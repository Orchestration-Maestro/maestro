#![cfg(test)]
use crate::effects_support;
use crate::support;
use maestro_web::{I18n, components::copy_button::CopyButton};
use std::rc::Rc;

#[test]
fn copy_button_keeps_titles_glyphs_and_optional_success_text() {
    let host = Rc::new(effects_support::Host::default());
    let i18n: I18n = I18n::new(Rc::new(support::Host::default())).unwrap();
    let button = CopyButton::new(host, &i18n).unwrap();
    assert!(button.text.is_empty());
    assert_eq!(button.title, "Copy");
    assert!(!button.show_text);
    for title in [None, Some(""), Some("custom")] {
        for show_text in [false, true] {
            let mut canvas = Canvas::new();
            if let Some(title) = title {
                canvas.button.title = title.into();
            }
            canvas.button.show_text = show_text;
            let output = canvas.activate();
            assert!(output.platform_output.events.iter().any(|event| matches!(event,
                egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == Some(title.unwrap_or("Copy")))));
            let glyph = egui::Rect::from_min_size(
                canvas.rect.min + egui::vec2(8., 8.),
                egui::vec2(16., 16.),
            );
            let before = canvas.paint.region(&canvas.context, &output.shapes, glyph);
            canvas.settle(0, true);
            let output = canvas.frame(vec![]).1;
            assert_eq!(copied(&output, &canvas), show_text);
            let after = canvas.paint.region(&canvas.context, &output.shapes, glyph);
            assert!(before.iter().any(|p| p[3] > 0) && after.iter().any(|p| p[3] > 0));
            assert_ne!(
                before, after,
                "the copy and check glyphs paint different pixels"
            );
        }
    }
}
use crate::copy_button_support;
use copy_button_support::{Canvas, copied, pointer};

#[test]
fn copy_button_waits_for_success_and_restores_at_2000_ms() {
    let mut canvas = Canvas::new();
    canvas.button.text = "original 雪".into();
    canvas.button.show_text = true;
    assert!(!copied(&canvas.activate(), &canvas));
    assert_eq!(&*canvas.host.copies.borrow(), &["original 雪"]);
    canvas.button.text = "later".into();
    canvas.host.advance(500);
    canvas.settle(0, true);
    assert!(copied(&canvas.frame(vec![]).1, &canvas));
    canvas.host.advance(2499);
    assert!(copied(&canvas.frame(vec![]).1, &canvas));
    canvas.host.advance(2500);
    assert!(!copied(&canvas.frame(vec![]).1, &canvas));
    assert_eq!(canvas.host.live(), 0);
}

#[test]
fn copy_button_replaces_only_its_success_timer() {
    let mut canvas = Canvas::new();
    canvas.button.show_text = true;
    canvas.activate();
    canvas.activate();
    canvas.settle(1, true);
    canvas.host.advance(1500);
    assert!(copied(&canvas.frame(vec![]).1, &canvas));
    canvas.settle(0, true);
    assert_eq!(canvas.host.live(), 1);
    canvas.host.advance(2000);
    assert!(copied(&canvas.frame(vec![]).1, &canvas));
    canvas.host.advance(3499);
    assert!(copied(&canvas.frame(vec![]).1, &canvas));
    canvas.host.advance(3500);
    assert!(!copied(&canvas.frame(vec![]).1, &canvas));
}

#[test]
fn copy_button_failure_preserves_prior_feedback() {
    let mut canvas = Canvas::new();
    canvas.button.show_text = true;
    canvas.activate();
    canvas.settle(0, false);
    assert!(!copied(&canvas.frame(vec![]).1, &canvas));
    canvas.activate();
    canvas.settle(1, true);
    canvas.activate();
    canvas.settle(2, false);
    assert!(copied(&canvas.frame(vec![]).1, &canvas));
    canvas.activate();
    *canvas.host.failure.borrow_mut() =
        Some(maestro_web::browser::attachment_content::AttachmentHostError::Host("refused".into()));
    canvas.settle(3, true);
    assert!(!copied(&canvas.frame(vec![]).1, &canvas));
    assert_eq!(canvas.host.live(), 0);
}

#[test]
fn copy_button_drop_prevents_late_feedback() {
    let mut canvas = Canvas::new();
    canvas.activate();
    let host = canvas.host.clone();
    drop(canvas);
    host.results.borrow()[0].set(Some(true));
    host.run_work();
    assert_eq!(host.admissions.get(), 0);
    let mut neighbor = Canvas::new();
    neighbor.button.show_text = true;
    neighbor.activate();
    neighbor.settle(0, true);
    assert!(copied(&neighbor.frame(vec![]).1, &neighbor));
    let host = neighbor.host.clone();
    drop(neighbor);
    assert_eq!(host.live(), 0);
}

#[test]
fn copy_button_clips_pointer_but_keeps_keyboard_action() {
    for enabled in [true, false] {
        let mut canvas = Canvas::new();
        canvas.enabled = enabled;
        canvas.frame(vec![]);
        let id = canvas.frame(vec![]).0.id;
        canvas.clip = egui::Rect::NOTHING;
        canvas.activate();
        assert!(canvas.host.copies.borrow().is_empty());
        canvas.context.memory_mut(|memory| memory.request_focus(id));
        canvas.frame(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_eq!(canvas.host.copies.borrow().len(), usize::from(enabled));
    }
    let mut canvas = Canvas::new();
    canvas.frame(vec![]);
    canvas.frame(pointer(canvas.rect.center(), true));
    canvas.frame(pointer(
        canvas.rect.right_bottom() + egui::vec2(10., 10.),
        false,
    ));
    assert!(canvas.host.copies.borrow().is_empty());
}

mod highlighter_support;

/// Activate one native reading slot and settle its exact clipboard operand.
/// # Panics
/// Panics if the slot is missing or the clipboard admission differs from `expected`.
fn copy_reading(
    canvas: &mut Canvas,
    expected: &str,
    mut render: impl FnMut(
        &mut egui::Ui,
        &I18n,
        &maestro_theme::BrandPresentation<'_>,
    ) -> egui::Response,
) {
    let before = canvas.host.copies.borrow().len();
    for pressed in [None, Some(true), Some(false)] {
        let events = pressed.map_or_else(Vec::new, |pressed| {
            pointer(canvas.host.controls.borrow()[0].center(), pressed)
        });
        copy_button_support::draw(
            &canvas.context,
            &mut canvas.paint,
            events,
            (egui::Rect::EVERYTHING, true),
            &mut render,
        );
    }
    assert_eq!(canvas.host.copies.borrow().len(), before + 1);
    assert_eq!(canvas.host.copies.borrow().last().unwrap(), expected);
    canvas.host.results.borrow().last().unwrap().set(Some(true));
    canvas.host.run_work();
}

/// Standalone preparation, replacement and activation retain the decoded clipboard operand.
#[test]
fn code_block_copies_decoded_and_replaced_native_input() {
    use maestro_web::components::code_block::{CodeBlock, CodeBlockView};
    let mut canvas = Canvas::new();
    let mut block = CodeBlock::new(canvas.host.clone());
    assert!(block.code.is_empty() && block.language.is_empty());
    for (input, expected) in [("6Zuq", "雪"), ("<&amp;>", "<&amp;>"), ("/w==", "/w==")] {
        block.code = input.into();
        block.language = "unknown".into();
        copy_reading(&mut canvas, expected, |ui, i18n, presentation| {
            block
                .render(
                    ui,
                    CodeBlockView {
                        i18n,
                        presentation,
                        highlighter: &highlighter_support::NoHighlight,
                        modal: None,
                    },
                )
                .unwrap()
        });
    }
    drop(block);
    assert_eq!(canvas.host.removed.borrow().len(), 1);
    assert_eq!(canvas.host.live(), 0);
}

/// The retained Markdown consumer publishes new code operands, not stale controls.
#[test]
fn markdown_block_copies_current_native_listing_after_replacement() {
    use maestro_web::components::{code_block::CodeBlockView, markdown_block::MarkdownBlock};
    let mut canvas = Canvas::new();
    let mut block = MarkdownBlock::new(canvas.host.clone());
    assert!(block.content.is_empty() && block.escape_html && !block.is_thinking);
    for (content, expected) in [
        ("```text\nfirst <&amp;>\n```", "first <&amp;>\n"),
        ("```text\n雪\n```", "雪\n"),
    ] {
        block.content = content.into();
        block.is_thinking = !block.is_thinking;
        copy_reading(&mut canvas, expected, |ui, i18n, presentation| {
            block
                .render(
                    ui,
                    CodeBlockView {
                        i18n,
                        presentation,
                        highlighter: &highlighter_support::NoHighlight,
                        modal: None,
                    },
                )
                .unwrap()
        });
    }
    drop(block);
    assert_eq!(canvas.host.removed.borrow().len(), 1);
    assert_eq!(canvas.host.live(), 0);
}
