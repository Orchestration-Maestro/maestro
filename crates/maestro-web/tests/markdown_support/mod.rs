//! Real-document drawing harness for native Markdown and code through the public interface.
#![cfg(all(test, target_arch = "wasm32"))]

pub use tests::*;
pub mod walk;

#[cfg(test)]
mod tests {
    use super::walk;
    use crate::support::{self, brand::pack};
    use maestro_theme::{BrandMode, BrandPack, SyntaxHighlighter, SyntectHighlighter};
    use maestro_web::{
        I18n, TranslationFunction,
        browser::{
            attachment_content::{
                AttachmentContent, AttachmentHost, AttachmentHostError, AttachmentSurface,
                BrowserAttachmentHost, ReadingLayout, ReadingPhase,
            },
            timer::Timer,
        },
        components::{code_block::CodeBlockView, markdown_block::MarkdownBlock},
    };
    use serde::Deserialize;
    use std::{
        cell::{Cell, RefCell},
        future::Future,
        pin::Pin,
        rc::Rc,
    };
    use wasm_bindgen::{JsCast as _, JsValue};

    /// Supplied engine assigning one known syntax role to each complete input line.
    pub struct Highlighter(pub Vec<Option<maestro_theme::ThemeColor>>);
    impl SyntaxHighlighter for Highlighter {
        fn supports_language(&self, _: &str) -> bool {
            true
        }
        fn highlight(
            &self,
            code: &str,
            _: &str,
        ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>>
        {
            let mut start = 0;
            Ok(code
                .split_inclusive('\n')
                .zip(&self.0)
                .map(|(line, color)| {
                    let range = start..start + line.len();
                    start = range.end;
                    maestro_theme::SyntaxSpan {
                        range,
                        color: color.clone(),
                    }
                })
                .collect())
        }
    }

    /// One recorded Markdown render.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct BlockRow {
        pub id: String,
        pub test: String,
        pub content: String,
        pub escape_html: bool,
        pub thinking: bool,
        pub html: String,
        /// Observation of the approved math library where it differs from the recorded one.
        #[serde(default)]
        pub current_html: Option<String>,
        /// Recorded observation replaced by a reference-defect correction.
        #[serde(default)]
        pub original_html: Option<String>,
    }
    /// One independently measured formula and its owning browser test.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct MathRow {
        pub id: String,
        pub expression: String,
        pub display: bool,
        pub branch: String,
        #[serde(default)]
        pub observed: Option<serde_json::Value>,
        #[serde(default)]
        pub fallbacks: Vec<MathFallback>,
        pub test: String,
    }

    /// One delimiter-specific fallback from the mounted source recorder.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct MathFallback {
        pub delimiter: String,
        pub html: String,
    }

    /// Select the recorded geometry or reached delimiter fallback.
    fn math_expected(row: &MathRow, open: &str) -> serde_json::Value {
        if row.branch != "throw" {
            assert!(row.fallbacks.is_empty());
            return row.observed.as_ref().unwrap().clone();
        }
        let delimiter = match open {
            "$" => "dollar-inline",
            "$$" => "dollar-display",
            "\\(" => "backslash-inline",
            "\\[" => "backslash-display",
            _ => unreachable!(),
        };
        assert!(row.observed.is_none());
        assert_eq!(row.fallbacks.len(), 2);
        let fallback = row
            .fallbacks
            .iter()
            .find(|f| f.delimiter == delimiter)
            .unwrap();
        serde_json::Value::String(fallback.html.clone())
    }

    /// Mount both delimiter spellings of the rows `test` owns, or of every row, and compare
    /// the recorded formula observations.
    ///
    /// # Panics
    /// Panics if a formula differs from its recorded browser observations.
    pub async fn math_rows_match(mounter: &mut dyn walk::Mounter, test: Option<&str>) {
        let all: Vec<MathRow> =
            serde_json::from_str(include_str!("../fixtures/markdown/math.json")).unwrap();
        assert_eq!(all.len(), 101);
        let inputs: std::collections::BTreeSet<_> =
            all.iter().map(|r| (&r.expression, r.display)).collect();
        assert_eq!(inputs.len(), all.len());
        let mut count = 0;
        for row in all
            .iter()
            .filter(|row| test.is_none_or(|test| row.test == test))
        {
            count += 1;
            for (open, close) in if row.display {
                [("$$", "$$"), ("\\[", "\\]")]
            } else {
                [("$", "$"), ("\\(", "\\)")]
            } {
                let content = format!("{open}{}{close}", row.expression);
                let block = mounter.mount(&content, (false, false));
                let expected = math_expected(row, open);
                publish("mathExpected", &expected.to_string().into());
                publish("mathBranch", &row.branch.clone().into());
                publish("mathSource", &content.into());
                let promise = walk::call("measureMath", &js_sys::Array::new())
                    .dyn_into::<js_sys::Promise>()
                    .unwrap();
                let result = wasm_bindgen_futures::JsFuture::from(promise)
                    .await
                    .unwrap()
                    .as_string()
                    .unwrap();
                assert_eq!(
                    result, "",
                    "{} display={} delimiter={open}",
                    row.id, row.display
                );
                drop(block);
            }
        }
        let Some(test) = test else {
            assert_eq!(count, all.len());
            return;
        };
        let expected = match test {
            "math_roots_pad_bare_symbols_without_changing_groups"
            | "math_macro_failures_keep_inline_error_spans" => 22,
            "math_scaled_roots_preserve_display_style" => 4,
            "math_unchanged_size_preserves_superscript_spacing"
            | "math_unchanged_size_preserves_subscript_spacing"
            | "math_unchanged_size_preserves_nested_script_spacing" => 1,
            "math_arrows_keep_measured_label_and_svg_alignment" => 20,
            "math_error_paths_keep_fallback_and_escaped_text" => 14,
            "math_macro_overrides_keep_valid_expansions" => 16,
            _ => panic!("unknown formula group"),
        };
        assert_eq!(count, expected);
    }

    /// One recorded standalone code render.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct CodeRow {
        pub id: String,
        pub test: String,
        pub code: String,
        pub language: String,
        pub decoded: String,
        pub display_language: String,
        pub copy_title: String,
        pub show_text: bool,
        pub css_language: String,
    }
    /// One recorded contextual structure.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct ContextRow {
        pub id: String,
        pub test: String,
        pub source: String,
        pub tree: serde_json::Value,
    }
    /// Rows of one recorded file belonging to a test.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    pub fn rows<T: for<'a> Deserialize<'a>>(json: &str, owns: impl Fn(&T) -> bool) -> Vec<T> {
        let all: Vec<T> = serde_json::from_str(json).unwrap();
        let kept: Vec<T> = all.into_iter().filter(|row| owns(row)).collect();
        assert!(
            !kept.is_empty(),
            "a test must own at least one recorded row"
        );
        kept
    }
    /// Markdown rows owned by `test`.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    #[must_use]
    pub fn block_rows(test: &str) -> Vec<BlockRow> {
        rows(
            include_str!("../fixtures/markdown/blocks.json"),
            |r: &BlockRow| r.test == test,
        )
    }
    /// Code rows owned by `test`.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    #[must_use]
    pub fn code_rows(test: &str) -> Vec<CodeRow> {
        rows(
            include_str!("../fixtures/markdown/code.json"),
            |r: &CodeRow| r.test == test,
        )
    }
    /// Contextual rows owned by `test`.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    #[must_use]
    pub fn context_rows(test: &str) -> Vec<ContextRow> {
        rows(
            include_str!("../fixtures/markdown/contextual.json"),
            |r: &ContextRow| r.test == test,
        )
    }

    /// Evaluate an observation in the runner's document.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    #[must_use]
    pub fn js(code: &str) -> JsValue {
        js_sys::eval(code).unwrap()
    }
    /// Assert a native browser predicate without reconstructing its result in Rust.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    pub fn check(code: &str) {
        assert!(js(code).as_bool().unwrap(), "browser predicate: {code}");
    }
    /// Publish a value to the page under `name`.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    pub fn publish(name: &str, value: &JsValue) {
        js_sys::Reflect::set(&js_sys::global(), &name.into(), value).unwrap();
    }
    /// Yield so queued browser-local work runs.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    pub async fn settle() {
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL))
            .await
            .unwrap();
    }

    /// Real platform effects recording the exact text of each started copy.
    pub struct Effects {
        /// Real owner of every native node.
        pub native: BrowserAttachmentHost,
        /// Texts passed to the clipboard, in order.
        pub copies: RefCell<Vec<String>>,
        /// Native surface releases observed at the original platform seam.
        pub removed: RefCell<Vec<egui::Id>>,
        /// Native frame visibilities observed after each reading phase.
        pub phases: RefCell<Vec<(ReadingPhase, Vec<String>)>>,
        /// Liveness of every admitted timer, cleared when its owner revokes it.
        pub timers: RefCell<Vec<Rc<Cell<bool>>>>,
        /// Failure delivered by the next presentation.
        pub fail_next: RefCell<Option<String>>,
        /// Phase rejected before native publication.
        pub fail_phase: Cell<Option<ReadingPhase>>,
        /// Last measured slot rectangles, before publication.
        pub controls: RefCell<Vec<egui::Rect>>,
        /// Reject the next native measurement readback.
        pub fail_measure: Cell<bool>,
    }
    impl AttachmentHost for Effects {
        fn enter_placement_scope(
            &self,
            scope: egui::Id,
        ) -> maestro_web::browser::attachment_content::PlacementScope {
            self.native.enter_placement_scope(scope)
        }
        fn hide_placement_scope(&self, scope: egui::Id) {
            self.native.hide_placement_scope(scope);
        }

        fn copy_to_clipboard(
            &self,
            text: &str,
            _: maestro_web::browser::attachment_content::ClipboardMode,
        ) -> Pin<Box<dyn Future<Output = bool>>> {
            self.copies.borrow_mut().push(text.to_owned());
            Box::pin(std::future::ready(true))
        }
        fn reading_layout(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentHostError> {
            if self.fail_measure.replace(false) {
                return Err(AttachmentHostError::Host(
                    "controlled measurement failure".into(),
                ));
            }
            let layout = self.native.reading_layout(id)?;
            *self.controls.borrow_mut() = layout.controls.iter().map(|(_, rect)| *rect).collect();
            Ok(layout)
        }
        fn present(
            &self,
            surface: AttachmentSurface<'_>,
            presentation: &maestro_theme::BrandPresentation<'_>,
        ) -> Result<(), AttachmentHostError> {
            if let Some(message) = self.fail_next.borrow_mut().take() {
                return Err(AttachmentHostError::Presentation(message));
            }
            let phase = match &surface.content {
                AttachmentContent::Reading(reading) => Some(reading.phase),
                _ => None,
            };
            if phase.is_some() && self.fail_phase.get() == phase {
                self.fail_phase.set(None);
                return Err(AttachmentHostError::Host("controlled phase failure".into()));
            }
            let result = self.native.present(surface, presentation);
            if let Some(phase) = phase {
                let seen = js_sys::Array::from(&js(
                    "[...document.querySelectorAll('.maestro-reading-frame')].map(f => getComputedStyle(f).visibility)",
                ));
                let seen = seen.iter().filter_map(|v| v.as_string()).collect();
                self.phases.borrow_mut().push((phase, seen));
            }
            result
        }
        fn set_retained_visibility(
            &self,
            id: egui::Id,
            visible: bool,
            modal: Option<egui::Id>,
        ) -> Result<(), AttachmentHostError> {
            self.native.set_retained_visibility(id, visible, modal)
        }
        fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentHostError> {
            self.native.set_overlay(id, visible)
        }
        fn hover_capable(&self) -> Result<bool, AttachmentHostError> {
            self.native.hover_capable()
        }
        fn remove(&self, id: egui::Id) {
            self.removed.borrow_mut().push(id);
            self.native.remove(id);
        }
        fn timer(
            &self,
            delay_ms: i32,
            repeat: bool,
            callback: Rc<dyn Fn()>,
        ) -> Result<Timer, AttachmentHostError> {
            let inner = self.native.timer(delay_ms, repeat, callback)?;
            let live = Rc::new(Cell::new(true));
            self.timers.borrow_mut().push(Rc::clone(&live));
            Ok(Timer::new(move || {
                live.set(false);
                drop(inner);
            }))
        }
        fn spawn_local(&self, future: Pin<Box<dyn Future<Output = ()>>>) {
            self.native.spawn_local(future);
        }
        fn download_bytes(
            &self,
            bytes: &[u8],
            name: &str,
            mime: &str,
        ) -> Result<(), AttachmentHostError> {
            self.native.download_bytes(bytes, name, mime)
        }
        fn download(
            &self,
            attachment: maestro_web::SharedAttachment,
        ) -> Result<(), AttachmentHostError> {
            self.native.download(attachment)
        }
    }

    /// Real host, selected brand packs and frame state shared by a test's components.
    pub struct Surface {
        /// Real platform owner with copy recording.
        pub effects: Rc<Effects>,
        /// Checked-in brand pack.
        pub pack: Rc<BrandPack>,
        /// Alternate pack.
        pub alternate: Rc<BrandPack>,
        /// Shared syntax engine.
        pub highlighter: Rc<dyn SyntaxHighlighter>,
        /// Localization.
        pub i18n: I18n,
        /// Drawing context retained across frames.
        pub context: egui::Context,
        /// Events delivered by the next frame.
        pub events: Vec<egui::Event>,
        /// Optional controlled frame time for painted animation observations.
        pub time: Option<f64>,
        /// Cursor selected by the completed frame.
        pub cursor: egui::CursorIcon,
        /// Accessibility output of the last frame.
        pub output: Vec<egui::output::OutputEvent>,
        /// Available size of every frame drawn without an explicit one.
        pub size: (f32, f32),
        /// Painted text and images of the last frame with their rectangles.
        pub painted: Vec<(String, egui::Rect)>,
        /// Shared final-clip raster sampler and the most recent submitted paint.
        pub paint: crate::canvas_paint::Paint,
        pub shapes: Vec<egui::epaint::ClippedShape>,
    }
    impl Default for Surface {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Surface {
        /// Bind the shared test canvas and install the browser-side projections.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        #[must_use]
        pub fn new() -> Self {
            std::panic::set_hook(Box::new(|info| {
                web_sys::console::error_1(&info.to_string().into());
            }));
            let _ = js(
                "if (!document.getElementById('canvas')) { const c=document.createElement('canvas'); c.id='canvas'; c.width=800; c.height=600; document.body.appendChild(c); }",
            );
            let _ = js(include_str!("dom.js"));
            walk::set_formulas();
            let canvas = web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into()
                .unwrap();
            Self {
                effects: Rc::new(Effects {
                    native: BrowserAttachmentHost::new(canvas).unwrap(),
                    copies: RefCell::default(),
                    removed: RefCell::default(),
                    phases: RefCell::default(),
                    timers: RefCell::default(),
                    fail_next: RefCell::default(),
                    fail_phase: Cell::default(),
                    fail_measure: Cell::default(),
                    controls: RefCell::default(),
                }),
                pack: Rc::new(pack("brand.json").unwrap()),
                alternate: Rc::new(pack("signal.json").unwrap()),
                highlighter: Rc::new(SyntectHighlighter::new().unwrap()),
                i18n: I18n::new(Rc::new(support::Host::default())).unwrap(),
                context: egui::Context::default(),
                events: vec![],
                time: None,
                cursor: egui::CursorIcon::Default,
                output: vec![],
                size: (800., 600.),
                painted: vec![],
                paint: crate::canvas_paint::Paint::default(),
                shapes: vec![],
            }
        }
        /// Platform effects handed to components.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        #[must_use]
        pub fn host(&self) -> Rc<dyn AttachmentHost> {
            self.effects.clone()
        }
        /// Draw one frame in the dark mode of the selected pack.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        pub fn draw<R>(
            &mut self,
            draw: impl FnOnce(&mut egui::Ui, CodeBlockView<'_, TranslationFunction>) -> R,
        ) -> R {
            let pack = Rc::clone(&self.pack);
            let presentation = pack.presentation(BrandMode::Dark).unwrap();
            let size = self.size;
            self.draw_with(&presentation, size, None, draw)
        }
        /// Draw one frame with an explicit presentation, available size and modal.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        pub fn draw_with<R>(
            &mut self,
            presentation: &maestro_theme::BrandPresentation<'_>,
            size: (f32, f32),
            modal: Option<egui::Id>,
            draw: impl FnOnce(&mut egui::Ui, CodeBlockView<'_, TranslationFunction>) -> R,
        ) -> R {
            let mut result = None;
            let mut draw = Some(draw);
            let events = std::mem::take(&mut self.events);
            let mut output = self.context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(size.0, size.1),
                    )),
                    events,
                    time: self.time,
                    ..Default::default()
                },
                |ui| {
                    ui.set_max_size(egui::vec2(size.0, size.1));
                    let view = CodeBlockView {
                        i18n: &self.i18n,
                        presentation,
                        highlighter: &*self.highlighter,
                        modal,
                    };
                    result = draw.take().map(|draw| draw(ui, view));
                },
            );
            self.paint.update(&output);
            self.shapes.clone_from(&output.shapes);
            output.textures_delta.clear();
            self.cursor = output.platform_output.cursor_icon;
            self.output = output.platform_output.events;
            self.painted = output
                .shapes
                .iter()
                .map(|clipped| self.painted_shape(clipped))
                .collect();
            result.unwrap()
        }
        /// Name only text or images that actually survive the final paint clip.
        fn painted_shape(&self, clipped: &egui::epaint::ClippedShape) -> (String, egui::Rect) {
            let label = match &clipped.shape {
                egui::Shape::Text(text) => text.galley.text().to_owned(),
                egui::Shape::Mesh(_) => "image".to_owned(),
                _ => return (String::new(), egui::Rect::NOTHING),
            };
            let rect = clipped
                .shape
                .visual_bounding_rect()
                .intersect(clipped.clip_rect);
            let visible = self
                .paint
                .region(&self.context, std::slice::from_ref(clipped), rect)
                .iter()
                .any(|p| p[3] > 0);
            (if visible { label } else { String::new() }, rect)
        }
        /// A block with `content`, drawn until its native root is placed.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        pub fn markdown(&mut self, content: &str) -> MarkdownBlock {
            let mut block = MarkdownBlock::new(self.host());
            content.clone_into(&mut block.content);
            self.settle(&mut block);
            block
        }
        /// Draw a block for the two-phase placement and one stable repaint.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        pub fn settle(&mut self, block: &mut MarkdownBlock) {
            for _ in 0..2 {
                self.draw(|ui, view| block.render(ui, view).unwrap());
            }
        }
        /// Mount a recorded row with its recorded options.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        pub fn row(&mut self, row: &BlockRow) -> MarkdownBlock {
            let mut block = MarkdownBlock::new(self.host());
            block.content.clone_from(&row.content);
            block.escape_html = row.escape_html;
            block.is_thinking = row.thinking;
            self.settle(&mut block);
            block
        }
        /// First difference between the mounted document and reference markup, or "".
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        #[must_use]
        pub fn difference(html: &str) -> String {
            walk::difference(html)
        }
        /// First difference between the mounted structure and a recorded tree, or "".
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        #[must_use]
        pub fn tree_difference(tree: &serde_json::Value) -> String {
            walk::tree_difference(tree)
        }
        /// Press and release the primary button at `at` over the next three frames.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        pub fn click<R>(
            &mut self,
            at: egui::Pos2,
            mut draw: impl FnMut(&mut egui::Ui, CodeBlockView<'_, TranslationFunction>) -> R,
        ) {
            let button = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            for events in [
                vec![egui::Event::PointerMoved(at)],
                vec![button(true)],
                vec![button(false)],
            ] {
                self.events = events;
                self.draw(&mut draw);
            }
        }
    }
    impl walk::Mounter for Surface {
        fn mount(
            &mut self,
            content: &str,
            (escape_html, thinking): (bool, bool),
        ) -> Box<dyn std::any::Any> {
            let mut block = MarkdownBlock::new(self.host());
            content.clone_into(&mut block.content);
            block.escape_html = escape_html;
            block.is_thinking = thinking;
            self.settle(&mut block);
            Box::new(block)
        }
    }
    /// Center of the first native control slot in canvas points.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    #[must_use]
    pub fn first_slot_center() -> egui::Pos2 {
        slot_center(0)
    }
    /// Center of the `index`th native control slot in canvas points.
    ///
    /// # Panics
    /// Panics if the test fixture or browser precondition is not satisfied.
    #[must_use]
    pub fn slot_center(index: usize) -> egui::Pos2 {
        let rect = js(&format!(
            "(() => {{ const r = document.querySelectorAll('.control-slot')[{index}].getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; }})()"
        ));
        let point = js_sys::Float32Array::new(&rect);
        egui::pos2(point.get_index(0), point.get_index(1))
    }
}
