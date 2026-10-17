//! Real-browser standalone code rendering through the public native component.
#![cfg(all(test, target_arch = "wasm32"))]
#[path = "attachment_support/canvas_paint.rs"]
pub mod canvas_paint;
pub mod markdown_support;
pub mod support;

#[cfg(test)]
mod tests {
    use crate::markdown_support::{
        CodeRow, Surface, check, code_rows, first_slot_center, publish, settle,
    };
    use maestro_theme::{SyntaxHighlighter, SyntaxSpan};
    use maestro_web::components::code_block::CodeBlock;
    use std::{cell::RefCell, rc::Rc};
    use wasm_bindgen::prelude::*;

    /// Draw a row's code until its native listing is placed.
    fn mount(surface: &mut Surface, row: &CodeRow) -> CodeBlock {
        let mut block = CodeBlock::new(surface.host());
        block.code.clone_from(&row.code);
        block.language.clone_from(&row.language);
        for _ in 0..2 {
            surface.draw(|ui, view| block.render(ui, view).unwrap());
        }
        block
    }
    /// Require the label, class, body text and exact copy bytes of a row.
    async fn observe(surface: &mut Surface, row: &CodeRow, block: &mut CodeBlock) {
        publish("expectedLabel", &row.display_language.clone().into());
        publish(
            "expectedClass",
            &format!("hljs language-{}", row.css_language).into(),
        );
        publish("expectedText", &row.decoded.clone().into());
        check(
            "document.querySelector('.maestro-code-label').textContent === globalThis.expectedLabel",
        );
        check(
            "document.querySelector('.maestro-code code').className === globalThis.expectedClass",
        );
        check(
            "document.querySelector('.maestro-code code').textContent === globalThis.expectedText",
        );
        let copies = surface.effects.copies.borrow().len();
        surface.click(first_slot_center(), |ui, view| {
            block.render(ui, view).unwrap()
        });
        let clicked = surface.output.iter().any(|event| matches!(event,
        egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == Some(row.copy_title.as_str())));
        assert!(
            clicked || copies == surface.effects.copies.borrow().len(),
            "{}: accessible name",
            row.id
        );
        settle().await;
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        assert_eq!(
            surface.effects.copies.borrow()[copies..],
            *std::slice::from_ref(&row.decoded),
            "{}",
            row.id
        );
        let shown = surface.painted.iter().any(|(text, _)| text == "Copied!");
        assert_eq!(shown, row.show_text, "{}: success text", row.id);
    }

    /// Base64 text is decoded when it is valid UTF-8 and kept whole otherwise.
    #[wasm_bindgen]
    pub async fn code_blocks_decode_base64_or_keep_original_input() {
        let mut surface = Surface::new();
        for row in code_rows("code_blocks_decode_base64_or_keep_original_input") {
            let mut block = mount(&mut surface, &row);
            observe(&mut surface, &row, &mut block).await;
        }
    }

    /// Records which languages the shared engine is asked to classify.
    struct Recording {
        /// The real engine.
        inner: maestro_theme::SyntectHighlighter,
        /// Labels passed to `highlight`.
        asked: RefCell<Vec<String>>,
    }
    impl SyntaxHighlighter for Recording {
        fn supports_language(&self, language: &str) -> bool {
            self.inner.supports_language(language)
        }
        fn highlight(
            &self,
            code: &str,
            language: &str,
        ) -> Result<Vec<SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
            self.asked.borrow_mut().push(language.to_owned());
            self.inner.highlight(code, language)
        }
    }

    /// Supported names use the supplied engine; unlabeled code is recognized and never lost.
    #[wasm_bindgen]
    pub async fn code_blocks_reuse_explicit_highlighting_and_plain_fallback() {
        let mut surface = Surface::new();
        let recording = Rc::new(Recording {
            inner: maestro_theme::SyntectHighlighter::new().unwrap(),
            asked: RefCell::default(),
        });
        surface.highlighter = recording.clone();
        for row in code_rows("code_blocks_reuse_explicit_highlighting_and_plain_fallback") {
            recording.asked.borrow_mut().clear();
            let supported = recording.supports_language(&row.language);
            let mut block = mount(&mut surface, &row);
            observe(&mut surface, &row, &mut block).await;
            check("document.querySelector('.maestro-code code tag') === null");
            if supported {
                assert!(
                    !recording.asked.borrow().is_empty(),
                    "{}: the supplied engine classifies a supported label",
                    row.id
                );
            }
            drop(block);
            check("document.querySelector('.maestro-code') === null");
        }
        let supported = [
            "javascript",
            "typescript",
            "python",
            "html",
            "css",
            "json",
            "bash",
            "sql",
        ]
        .into_iter()
        .filter(|name| recording.supports_language(name))
        .count();
        assert_eq!(
            supported, 8,
            "the named languages are supported by the shared engine"
        );
        let mut block = CodeBlock::new(surface.host());
        block.code = "let x = 1; // c".into();
        block.language = "javascript".into();
        for _ in 0..2 {
            surface.draw(|ui, view| block.render(ui, view).unwrap());
        }
        check("document.querySelector('.maestro-code code span[class^=\"hljs-\"]') !== null");
        block.code = "def total(items):\n    return sum(items)\n".into();
        block.language.clear();
        for _ in 0..2 {
            surface.draw(|ui, view| block.render(ui, view).unwrap());
        }
        check("document.querySelector('.maestro-code-label').textContent === 'plaintext'");
        check(
            "document.querySelector('.maestro-code code span[class^=\"hljs-\"]') !== null && document.querySelector('.maestro-code code').textContent === 'def total(items):\\n    return sum(items)\\n'",
        );
    }
}
