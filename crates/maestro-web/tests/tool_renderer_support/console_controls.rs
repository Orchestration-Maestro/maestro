//! Console copy policies on the shared deterministic effect and frame owners.
#![cfg(test)]
use crate::{canvas_paint, copy_button_support, effects_support::Host};
use maestro_web::{
    ConsoleBlock, browser::attachment_content::ClipboardMode,
    components::console_block::ConsoleVariant,
};
use std::rc::Rc;

struct ConsoleCanvas {
    console: ConsoleBlock,
    host: Rc<Host>,
    context: egui::Context,
    paint: canvas_paint::Paint,
}
impl ConsoleCanvas {
    fn new() -> Self {
        let host = Rc::new(Host::default());
        Self {
            console: ConsoleBlock::new(host.clone()),
            host,
            context: egui::Context::default(),
            paint: canvas_paint::Paint::default(),
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let console = &mut self.console;
        copy_button_support::draw(
            &self.context,
            &mut self.paint,
            events,
            (egui::Rect::EVERYTHING, true),
            |ui, i18n, presentation| console.render(ui, i18n, presentation, None).unwrap(),
        )
        .1
    }
    fn activate(&mut self) {
        self.frame(vec![]);
        let center = self.host.controls.borrow()[0].center();
        self.frame(copy_button_support::pointer(center, true));
        self.frame(copy_button_support::pointer(center, false));
    }
    fn settle(&self, index: usize, success: bool) {
        self.host.results.borrow()[index].set(Some(success));
        self.host.run_work();
    }
    fn copied(&mut self) -> bool {
        let output = self.frame(vec![]);
        self.paint
            .text_visible(&self.context, &output.shapes, "Copied!")
    }
}
#[test]
fn console_copy_uses_exact_text_and_fifteen_hundred_ms() {
    for (variant, text) in [
        (ConsoleVariant::Default, ""),
        (ConsoleVariant::Default, "雪 <b>&</b>\n"),
        (ConsoleVariant::Error, " \t\r\n\0"),
    ] {
        let mut canvas = ConsoleCanvas::new();
        canvas.console.content = text.into();
        canvas.console.variant = variant;
        canvas.activate();
        assert!(!canvas.copied());
        assert_eq!(&*canvas.host.copies.borrow(), &[text]);
        assert_eq!(&*canvas.host.modes.borrow(), &[ClipboardMode::Direct]);
        canvas.settle(0, true);
        assert!(canvas.copied());
        canvas.host.advance(1499);
        assert!(canvas.copied());
        canvas.host.advance(1500);
        assert!(!canvas.copied());
        canvas.activate();
        canvas.settle(1, true);
        canvas.host.advance(2000);
        canvas.activate();
        canvas.settle(2, true);
        canvas.host.advance(3000);
        assert!(canvas.copied());
        canvas.host.advance(3499);
        assert!(canvas.copied());
        canvas.host.advance(3500);
        assert!(!canvas.copied());
        assert_eq!(canvas.host.live(), 0);
    }
}
#[test]
fn console_copy_failure_and_pending_completion_keep_state() {
    let mut canvas = ConsoleCanvas::new();
    canvas.console.content = "before edit".into();
    canvas.activate();
    canvas.console.content = "after edit".into();
    assert!(!canvas.copied());
    assert_eq!(&*canvas.host.copies.borrow(), &["before edit"]);
    canvas.settle(0, false);
    assert!(!canvas.copied());
    assert_eq!(canvas.host.admissions.get(), 0);
    canvas.activate();
    canvas.settle(1, true);
    assert!(canvas.copied());
    canvas.activate();
    canvas.settle(2, false);
    assert!(canvas.copied());
    assert_eq!(canvas.host.admissions.get(), 1);
    assert_eq!(
        &*canvas.host.copies.borrow(),
        &["before edit", "after edit", "after edit"]
    );
    assert!(
        canvas
            .host
            .modes
            .borrow()
            .iter()
            .all(|mode| *mode == ClipboardMode::Direct)
    );
}
#[test]
fn console_release_revokes_feedback_and_neighbour_is_untouched() {
    let mut first = ConsoleCanvas::new();
    let mut neighbour = ConsoleCanvas::new();
    first.activate();
    neighbour.activate();
    neighbour.settle(0, true);
    let host = first.host.clone();
    drop(first);
    host.results.borrow()[0].set(Some(true));
    host.run_work();
    assert_eq!(host.admissions.get(), 0);
    assert_eq!(host.removed.borrow().len(), 1);
    assert!(neighbour.copied());
    assert_eq!(neighbour.host.live(), 1);
    assert!(neighbour.host.removed.borrow().is_empty());
    let host = neighbour.host.clone();
    drop(neighbour);
    assert_eq!(host.live(), 0);
    assert_eq!(host.removed.borrow().len(), 1);
}
