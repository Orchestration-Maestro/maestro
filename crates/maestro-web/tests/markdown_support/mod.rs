//! Real-document drawing harness for native Markdown and code through the public interface.
#![cfg(all(test, target_arch = "wasm32"))]

pub use tests::*;

#[cfg(test)]
mod tests {
    use crate::support::{self, brand::Pack};
    use maestro_theme::{BrandMode, BrandPack, SyntaxHighlighter, SyntectHighlighter};
    use maestro_web::{
        Attachment, I18n, TranslationFunction,
        browser::{
            attachment_content::{
                AttachmentContent, AttachmentHost, AttachmentSurface, AttachmentUiError,
                BrowserAttachmentHost, FocusTarget, ReadingLayout, ReadingPhase,
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
        /// Native frame visibilities observed after each reading phase.
        pub phases: RefCell<Vec<(ReadingPhase, Vec<String>)>>,
        /// Liveness of every admitted timer, cleared when its owner revokes it.
        pub timers: RefCell<Vec<Rc<Cell<bool>>>>,
        /// Failure delivered by the next presentation.
        pub fail_next: RefCell<Option<String>>,
        /// Phase rejected before native publication.
        pub fail_phase: Cell<Option<ReadingPhase>>,
        /// Reject the next native measurement readback.
        pub fail_measure: Cell<bool>,
    }
    impl AttachmentHost for Effects {
        fn copy_to_clipboard(&self, text: &str) -> Pin<Box<dyn Future<Output = bool>>> {
            self.copies.borrow_mut().push(text.to_owned());
            Box::pin(std::future::ready(true))
        }
        fn capture_focus(&self) -> Result<Option<Rc<FocusTarget>>, AttachmentUiError> {
            self.native.capture_focus()
        }
        fn restore_focus(&self, target: Rc<FocusTarget>) -> Result<Timer, AttachmentUiError> {
            self.native.restore_focus(target)
        }
        fn dialog_bounds(
            &self,
            id: egui::Id,
            dimensions: [&str; 2],
        ) -> Result<egui::Rect, AttachmentUiError> {
            self.native.dialog_bounds(id, dimensions)
        }
        fn native_key(&self, event: &web_sys::KeyboardEvent, context: &egui::Context) {
            self.native.native_key(event, context);
        }
        fn reading_layout(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentUiError> {
            if self.fail_measure.replace(false) {
                return Err(AttachmentUiError::Host(
                    "controlled measurement failure".into(),
                ));
            }
            self.native.reading_layout(id)
        }
        fn present(
            &self,
            surface: AttachmentSurface<'_>,
            presentation: &maestro_theme::BrandPresentation<'_>,
        ) -> Result<(), AttachmentUiError> {
            if let Some(message) = self.fail_next.borrow_mut().take() {
                return Err(AttachmentUiError::Presentation(message));
            }
            let phase = match &surface.content {
                AttachmentContent::Reading(reading) => Some(reading.phase),
                _ => None,
            };
            if phase.is_some() && self.fail_phase.get() == phase {
                self.fail_phase.set(None);
                return Err(AttachmentUiError::Host("controlled phase failure".into()));
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
        ) -> Result<(), AttachmentUiError> {
            self.native.set_retained_visibility(id, visible, modal)
        }
        fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentUiError> {
            self.native.set_overlay(id, visible)
        }
        fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
            self.native.hover_capable()
        }
        fn remove(&self, id: egui::Id) {
            self.native.remove(id);
        }
        fn take_escape(&self) -> bool {
            self.native.take_escape()
        }
        fn timer(
            &self,
            delay_ms: i32,
            repeat: bool,
            callback: Rc<dyn Fn()>,
        ) -> Result<Timer, AttachmentUiError> {
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
        ) -> Result<(), AttachmentUiError> {
            self.native.download_bytes(bytes, name, mime)
        }
        fn download(&self, attachment: Rc<RefCell<Attachment>>) -> Result<(), AttachmentUiError> {
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
        /// Bind to the runner's canvas and install the browser-side projections.
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        #[must_use]
        pub fn new() -> Self {
            std::panic::set_hook(Box::new(|info| {
                web_sys::console::error_1(&info.to_string().into());
            }));
            let _ = js(include_str!("dom.js"));
            publish(
                "formulasJson",
                &include_str!("../fixtures/markdown/formulas.json").into(),
            );
            let _ = js("maestro.formulas = JSON.parse(globalThis.formulasJson)");
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
                    phases: RefCell::default(),
                    timers: RefCell::default(),
                    fail_next: RefCell::default(),
                    fail_phase: Cell::default(),
                    fail_measure: Cell::default(),
                }),
                pack: Rc::new(maestro_theme::load_brand_pack("brand.json", &Pack).unwrap()),
                alternate: Rc::new(maestro_theme::load_brand_pack("signal.json", &Pack).unwrap()),
                highlighter: Rc::new(SyntectHighlighter::new().unwrap()),
                i18n: I18n::new(Rc::new(support::Host::default())).unwrap(),
                context: egui::Context::default(),
                events: vec![],
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
            publish("expectedHtml", &html.into());
            js("maestro.diff(maestro.root(), globalThis.expectedHtml)")
                .as_string()
                .unwrap()
        }
        /// First difference between the mounted structure and a recorded tree, or "".
        ///
        /// # Panics
        /// Panics if the test fixture or browser precondition is not satisfied.
        #[must_use]
        pub fn tree_difference(tree: &serde_json::Value) -> String {
            publish("expectedTree", &tree.to_string().into());
            js("maestro.treeDiff(maestro.root(), JSON.parse(globalThis.expectedTree))")
                .as_string()
                .unwrap()
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
