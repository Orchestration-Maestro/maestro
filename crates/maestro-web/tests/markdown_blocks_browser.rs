//! Real-browser native composition of Markdown, code and canvas copy controls.
#![cfg(all(test, target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use crate::markdown_support::{
        Surface, block_rows, check, first_slot_center, js, publish, settle, slot_center,
    };
    use maestro_theme::BrandMode;
    use maestro_web::{
        browser::attachment_content::{AttachmentHost as _, ReadingPhase},
        components::{code_block::CodeBlock, markdown_block::MarkdownBlock},
    };
    use std::fmt::Write as _;
    use wasm_bindgen::prelude::*;

    /// Text, a code block and a tail, so the document has one canvas control.
    const WITH_CODE: &str = "first paragraph\n\n```js\nlet x = 1;\n```\n\nlast paragraph";
    /// Keyboard Tab press.
    fn tab() -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    /// The first returned frame already matches the native height, placement and controls.
    #[wasm_bindgen]
    pub async fn markdown_native_first_frame_matches_reading_flow() {
        let mut surface = Surface::new();
        let mut block = MarkdownBlock::new(surface.host());
        block.content = WITH_CODE.into();
        let (mut response, mut following) = (egui::Rect::NOTHING, 0.);
        surface.draw(|ui, view| {
            ui.spacing_mut().item_spacing.y = 0.;
            response = block.render(ui, view).unwrap().rect;
            following = ui.allocate_space(egui::vec2(10., 10.)).1.top();
        });
        let native = js("maestro.root().parentElement.getBoundingClientRect().height")
            .as_f64()
            .unwrap();
        assert!(
            (f64::from(response.height()) - native).abs() < 0.5,
            "{} != {native}",
            response.height()
        );
        assert!((following - response.bottom()).abs() < 0.01);
        let phases: Vec<_> = surface
            .effects
            .phases
            .borrow()
            .iter()
            .map(|(p, v)| (*p, v.clone()))
            .collect();
        assert_eq!(
            phases,
            [
                (ReadingPhase::Measure, vec!["hidden".to_owned()]),
                (ReadingPhase::Publish, vec!["visible".to_owned()]),
            ]
        );
        surface.click(first_slot_center(), |ui, view| {
            block.render(ui, view).unwrap()
        });
        settle().await;
        assert_eq!(*surface.effects.copies.borrow(), ["let x = 1;\n"]);
        drop(block);

        modal_placements(&mut surface);
        narrow_placements(&mut surface);
        let mut block = MarkdownBlock::new(surface.host());
        block.content = WITH_CODE.into();
        for _ in 0..2 {
            surface.draw(|ui, view| {
                ui.horizontal(|ui| block.render(ui, view).unwrap());
            });
        }
        let _ = js("maestro.root().parentElement.parentElement.dispatchEvent(new Event('scroll'))");
        surface.draw(|ui, view| block.render(ui, view).unwrap());
    }

    /// One returned frame uses the retained viewport's clamped scroll after presentation changes.
    #[wasm_bindgen]
    pub fn markdown_first_reconciled_frame_uses_retained_scroll() {
        let mut surface = Surface::new();
        surface.size = (350., 170.);
        let mut block = MarkdownBlock::new(surface.host());
        block.escape_html = false;
        block.content = format!(
            "<div style=\"width:900px;height:1px\"></div>\n\n{}\n\n```js\nlet end;\n```",
            "filler paragraph\n\n".repeat(40)
        );
        surface.settle(&mut block);
        let _ = js(
            "globalThis.retainedViewport = maestro.root().closest('.maestro-reading-viewport'); retainedViewport.scrollTop = 1e6; retainedViewport.scrollLeft = 23.5; globalThis.oldTop = retainedViewport.scrollTop; globalThis.oldLeft = retainedViewport.scrollLeft",
        );
        check("oldTop > 0 && oldLeft > 0 && oldLeft % 1 !== 0");
        block.is_thinking = true;
        let response = surface.draw(|ui, view| block.render(ui, view).unwrap());
        check(
            "maestro.root().closest('.maestro-reading-viewport') === retainedViewport && retainedViewport.scrollTop < oldTop && retainedViewport.scrollLeft === oldLeft",
        );
        assert_first_control(&surface, response.rect);
    }

    /// The shared standalone-code route measures changed-height clamping before returning.
    #[wasm_bindgen]
    pub fn code_first_reconciled_frame_uses_retained_scroll() {
        let mut surface = Surface::new();
        surface.size = (350., 170.);
        let mut block = CodeBlock::new(surface.host());
        block.code = "line\n".repeat(80);
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        let _ = js(
            "globalThis.retainedViewport = document.querySelector('.maestro-reading-viewport'); retainedViewport.scrollTop = 10.5; globalThis.oldTop = retainedViewport.scrollTop",
        );
        check("oldTop > 0");
        block.code = "line\n".repeat(60);
        block.language = "text".into();
        let response = surface.draw(|ui, view| block.render(ui, view).unwrap());
        check(
            "document.querySelector('.maestro-reading-viewport') === retainedViewport && retainedViewport.scrollTop === oldTop",
        );
        assert_first_control(&surface, response.rect);
        block.code = "short".into();
        let response = surface.draw(|ui, view| block.render(ui, view).unwrap());
        check(
            "document.querySelector('.maestro-reading-viewport') === retainedViewport && retainedViewport.scrollTop === 0",
        );
        assert_first_control(&surface, response.rect);
    }

    /// Compare the first readback and canvas paint with the retained native slot, without settling.
    fn assert_first_control(surface: &Surface, response: egui::Rect) {
        let measured = surface.effects.controls.borrow()[0].translate(response.min.to_vec2());
        let native = first_slot_center();
        assert!(
            measured.center().distance(native) < 0.8,
            "measured {measured:?}, native {native:?}"
        );
        let painted = surface
            .painted
            .iter()
            .any(|(label, rect)| label == "image" && rect.contains(native));
        assert!(
            painted,
            "canvas copy glyph must paint inside the retained native slot"
        );
    }

    /// Modal identity is forwarded through both native phases.
    fn modal_placements(surface: &mut Surface) {
        let modal = egui::Id::new("modal");
        for (open, caller, visible) in [
            (false, None, true),
            (true, Some(modal), true),
            (true, None, false),
            (true, Some(egui::Id::new("other")), false),
        ] {
            surface.effects.phases.borrow_mut().clear();
            surface.effects.set_overlay(modal, open).unwrap();
            let mut block = MarkdownBlock::new(surface.host());
            block.content = WITH_CODE.into();
            let pack = surface.pack.clone();
            let presentation = pack.presentation(BrandMode::Dark).unwrap();
            for _ in 0..2 {
                draw_modal(surface, &mut block, &presentation, caller);
            }
            let visibility = if visible { "visible" } else { "hidden" };
            let seen = surface.effects.phases.borrow().last().cloned().unwrap();
            assert_eq!(
                seen,
                (ReadingPhase::Publish, vec![String::from(visibility)])
            );
            drop(block);
            surface.effects.set_overlay(modal, false).unwrap();
        }
    }

    /// One modal frame consumes its borrowed presentation without retaining it.
    fn draw_modal(
        surface: &mut Surface,
        block: &mut MarkdownBlock,
        presentation: &maestro_theme::BrandPresentation<'_>,
        caller: Option<egui::Id>,
    ) {
        surface.draw_with(presentation, (800., 600.), caller, |ui, view| {
            block.render(ui, view).unwrap()
        });
    }

    /// Narrow widths clip each native content form.
    fn narrow_placements(surface: &mut Surface) {
        for width in [0_f32, 1., 2., 200., 800.] {
            for content in [
                "",
                WITH_CODE,
                "one very long unbroken paragraph of words that must wrap",
            ] {
                narrow_content(surface, width, content);
            }
        }
    }

    /// One content width remains bounded after native readback.
    fn narrow_content(surface: &mut Surface, width: f32, content: &str) {
        let mut block = MarkdownBlock::new(surface.host());
        block.content = content.into();
        let pack = surface.pack.clone();
        let presentation = pack.presentation(BrandMode::Dark).unwrap();
        for _ in 0..2 {
            let rect = surface.draw_with(&presentation, (width, 400.), None, |ui, view| {
                block.render(ui, view).unwrap().rect
            });
            assert!(
                rect.width() <= width + 0.01,
                "width {width}: {}",
                rect.width()
            );
        }
    }

    /// Unchanged text keeps its root, selection and scroll; changed text replaces the owners.
    #[wasm_bindgen]
    pub async fn markdown_native_replacement_retains_or_releases_the_right_owner() {
        let mut surface = Surface::new();
        surface.size = (400., 150.);
        let tall = format!(
            "{}\n\n```js\nlet a;\n```\n\nselected text",
            "filler\n\n".repeat(60)
        );
        let mut block = surface.markdown(&tall);
        let frame = |surface: &mut Surface, block: &mut MarkdownBlock| {
            surface.draw(|ui, view| block.render(ui, view))
        };
        for _ in 0..2 {
            frame(&mut surface, &mut block).unwrap();
        }
        let _ = js(
            "globalThis.root = maestro.root(); globalThis.viewport = root.parentElement.parentElement; viewport.scrollTop = 23.5; globalThis.scrolled = viewport.scrollTop; const last = [...root.querySelectorAll('p')].at(-1); const range = document.createRange(); range.selectNodeContents(last); getSelection().removeAllRanges(); getSelection().addRange(range); globalThis.selected = getSelection().toString()",
        );
        check("scrolled > 0 && scrolled % 1 !== 0 && selected === 'selected text'");
        let same = "maestro.root() === root && root.isConnected && viewport.scrollTop === scrolled && getSelection().toString() === selected";
        frame(&mut surface, &mut block).unwrap();
        check(same);
        block.is_thinking = true;
        frame(&mut surface, &mut block).unwrap();
        check(same);
        check("getComputedStyle(root).fontStyle === 'italic'");
        check("globalThis.oldCode = root.querySelector('.maestro-code'); oldCode !== null");

        block.content = format!("{tall}\n\nappended");
        frame(&mut surface, &mut block).unwrap();
        check(
            "!root.isConnected && maestro.root() !== root && !oldCode.isConnected && maestro.root().querySelector('.maestro-code') !== oldCode",
        );
        check("maestro.root().textContent.trimEnd().endsWith('appended')");

        let _ = js("globalThis.kept = maestro.root();");
        block.content.push_str(" more");
        *surface.effects.fail_next.borrow_mut() = Some("controlled failure".into());
        assert!(frame(&mut surface, &mut block).is_err());
        check(
            "maestro.root() === kept && kept.isConnected && !kept.textContent.trimEnd().endsWith('more')",
        );
        frame(&mut surface, &mut block).unwrap();
        check("maestro.root() !== kept && maestro.root().textContent.trimEnd().endsWith('more')");

        release_feedback(surface, block).await;
    }

    /// Final release revokes the actual copy feedback and native root.
    async fn release_feedback(mut surface: Surface, mut block: MarkdownBlock) {
        let _ = js("document.querySelector('.control-slot').scrollIntoView()");
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        surface.click(first_slot_center(), |ui, view| {
            block.render(ui, view).unwrap()
        });
        settle().await;
        assert_eq!(surface.effects.copies.borrow().len(), 1);
        let live = surface
            .effects
            .timers
            .borrow()
            .iter()
            .filter(|t| t.get())
            .count();
        assert_eq!(live, 1, "the copy indication owns one live timer");
        drop(block);
        check("document.querySelector('.maestro-markdown') === null");
        assert_eq!(
            surface
                .effects
                .timers
                .borrow()
                .iter()
                .filter(|t| t.get())
                .count(),
            0,
            "dropping the owner revokes its feedback"
        );
    }

    /// A standalone listing keeps its header outside the local scroller and its owners across updates.
    #[wasm_bindgen]
    pub async fn code_native_header_stays_outside_local_scroll() {
        let mut surface = Surface::new();
        let mut block = CodeBlock::new(surface.host());
        block.code = (0..80).fold(String::new(), |mut code, i| {
            writeln!(code, "let value{i} = {i}; // {}", "wide ".repeat(40)).unwrap();
            code
        });
        block.language = "javascript".into();
        for _ in 0..2 {
            surface.draw(|ui, view| block.render(ui, view).unwrap());
        }
        check(
            "(() => { const s = document.querySelector('.maestro-code-scroll'); return s.scrollHeight > s.clientHeight && s.scrollWidth > s.clientWidth && s.clientHeight <= 384; })()",
        );
        check(
            "getComputedStyle(document.querySelector('.maestro-code-scroll')).maxHeight === '384px'",
        );
        let _ = js(
            "globalThis.header = document.querySelector('.maestro-code-header').getBoundingClientRect(); globalThis.scroller = document.querySelector('.maestro-code-scroll'); scroller.scrollTop = 41.5; scroller.scrollLeft = 17.5",
        );
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        check(
            "(() => { const h = document.querySelector('.maestro-code-header').getBoundingClientRect(); return h.top === header.top && h.height === header.height && h.bottom <= scroller.getBoundingClientRect().top + 0.5 && scroller.scrollTop > 0 && scroller.scrollLeft > 0; })()",
        );
        let slot = slot_center(0);
        let rect = js_sys::Float32Array::new(&js(
            "(() => { const r = document.querySelector('.control-slot').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]; })()",
        ));
        let rect = egui::Rect::from_min_max(
            egui::pos2(rect.get_index(0), rect.get_index(1)),
            egui::pos2(rect.get_index(2), rect.get_index(3)),
        );
        assert!(
            rect.contains(slot)
                && surface
                    .painted
                    .iter()
                    .any(|(label, r)| label == "image" && rect.contains(r.center()))
        );

        code_updates(&mut surface, &mut block, slot).await;
    }

    /// Body and language updates preserve selection and active control ownership.
    async fn code_updates(surface: &mut Surface, block: &mut CodeBlock, slot: egui::Pos2) {
        surface.click(slot, |ui, view| block.render(ui, view).unwrap());
        settle().await;
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        assert!(surface.painted.iter().any(|(label, _)| label == "Copied!"));
        let _ = js(
            "globalThis.slotNode = document.querySelector('.control-slot'); globalThis.codeNode = document.querySelector('.maestro-code code'); globalThis.codeText = codeNode.firstChild; scroller.scrollTop = 1e6;",
        );

        block.language = "zzz".into();
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        let _ = js(
            "const range = document.createRange(); range.selectNodeContents(codeNode); getSelection().removeAllRanges(); getSelection().addRange(range); globalThis.selected = getSelection().toString()",
        );
        block.language = "yyy".into();
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        check(
            "document.querySelector('.control-slot') === slotNode && document.querySelector('.maestro-code code') === codeNode && codeNode.firstChild === codeText && getSelection().toString() === selected && document.querySelector('.maestro-code-label').textContent === 'yyy'",
        );
        assert!(
            surface.painted.iter().any(|(label, _)| label == "Copied!"),
            "the active indication survives the update"
        );

        block.code = "short\n".into();
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        check(
            "document.querySelector('.control-slot') === slotNode && document.querySelector('.maestro-code code') === codeNode && !codeText.isConnected && codeNode.textContent === 'short\\n' && scroller.scrollTop === 0",
        );
    }

    /// Real fixtures keep link metadata, tasks, tables and exact code through focus and native scrolling.
    #[wasm_bindgen]
    pub async fn markdown_native_links_tables_and_code_keep_focus_and_selection() {
        let mut surface = Surface::new();
        let fixtures: Vec<crate::markdown_support::BlockRow> =
            serde_json::from_str(include_str!("fixtures/markdown/blocks.json")).unwrap();
        native_metadata(&mut surface, &fixtures);

        let table = fixtures.iter().find(|r| r.id == "table").unwrap();
        let wide = format!(
            "{}\n\n{}",
            table.content,
            "| wide | wider |\n| - | - |\n| unbreakable_cell_that_cannot_fit_a_narrow_viewport | another_unbreakable_cell_that_cannot_fit |"
        );
        surface.size = (220., 120.);
        let mut block = surface.markdown(&format!(
            "{wide}\n\n```rust\nfn main() {{ println!(\"雪\"); }}\n```\n\ntail text"
        ));
        check(
            "[...document.querySelectorAll('.maestro-markdown th')].slice(0, 2).map(c => c.align).join() === 'left,right'",
        );
        check(
            "(() => { const t = document.querySelector('.maestro-table-scroll'); return getComputedStyle(t).overflowX === 'auto' && [...document.querySelectorAll('.maestro-table-scroll')].some((t) => t.scrollWidth > t.clientWidth); })()",
        );
        let _ = js(
            "globalThis.viewport = maestro.root().parentElement.parentElement; const p = [...maestro.root().querySelectorAll('p')].at(-1); const range = document.createRange(); range.selectNodeContents(p); getSelection().removeAllRanges(); getSelection().addRange(range); globalThis.selected = getSelection().toString()",
        );
        assert_eq!(js("selected").as_string().unwrap(), "tail text");

        surface.events = tab();
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        check(
            "(() => { const s = document.querySelector('.control-slot').getBoundingClientRect(), v = viewport.getBoundingClientRect(); return s.top >= v.top - 0.5 && s.bottom <= v.bottom + 0.5; })()",
        );
        check("getSelection().toString() === selected");

        let _ = js("viewport.scrollTop = 0");
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        let _ = js("viewport.scrollTop = viewport.scrollHeight");
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        let visible = slot_center(0);
        surface.click(visible, |ui, view| block.render(ui, view).unwrap());
        settle().await;
        assert_eq!(
            *surface.effects.copies.borrow(),
            ["fn main() { println!(\"雪\"); }\n"]
        );
        let _ = js("viewport.scrollTop = 0");
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        surface.click(visible, |ui, view| block.render(ui, view).unwrap());
        settle().await;
        assert_eq!(
            surface.effects.copies.borrow().len(),
            1,
            "the stale position no longer admits a press"
        );
    }

    /// Native links, images and task markers consume retained metadata.
    fn native_metadata(surface: &mut Surface, fixtures: &[crate::markdown_support::BlockRow]) {
        let rows = block_rows("markdown_links_open_separately_with_safe_relation");
        let links = rows.iter().find(|r| r.id == "authored-links").unwrap();
        let block = surface.row(links);
        check(
            "document.querySelector('.maestro-markdown a').href === 'https://example.test/a?x=1&y=2' && document.querySelector('.maestro-markdown a').title === 'a title'",
        );
        check("document.querySelector('.maestro-markdown a strong').textContent === 'bold'");
        check(
            "document.querySelector('.maestro-markdown img').getAttribute('src') === 'https://example.test/image.png' && document.querySelector('.maestro-markdown img').alt === 'alt' && document.querySelector('.maestro-markdown img').title === 'image'",
        );
        drop(block);

        let blocks = fixtures.iter().find(|r| r.id == "blocks").unwrap();
        let block = surface.row(blocks);
        check(
            "[...document.querySelectorAll('.maestro-markdown input[type=checkbox]')].map(c => c.checked + ':' + c.disabled).join() === 'true:true,false:true'",
        );
        drop(block);
    }

    /// Ordinary, thinking, error, inline and display math, code, tables and controls paint with the selected pack.
    #[wasm_bindgen]
    pub async fn markdown_native_paints_selected_brand_and_loaded_math_fonts() {
        let mut surface = Surface::new();
        let source = "# one\n\n## two\n\n### three\n\n#### four\n\n##### five\n\n###### six\n\ntext [link](https://x.test) `code` $x^2$ $\\frac{$ and\n\n$$\\int_0^1 x\\,dx$$\n\n> quote\n\n- item\n\n  loose\n\n| a | b |\n| - | - |\n| c | d |\n\n```js\nlet x = 1;\n```\n\nlast";
        let mut block = surface.markdown(source);
        let packs = [
            (surface.pack.clone(), BrandMode::Dark),
            (surface.pack.clone(), BrandMode::Light),
            (surface.alternate.clone(), BrandMode::Dark),
        ];
        for (pack, mode) in packs {
            let presentation = pack.presentation(mode).unwrap();
            for thinking in [false, true] {
                paint_mode(&mut surface, &mut block, &presentation, thinking);
                crate::support::reading_paint::observe("markdown-brand").await;
            }
        }
        check(
            "[...maestro.root().querySelectorAll('h1,h2,h3,h4,h5,h6')].map(h => getComputedStyle(h).fontSize).join() === '24px,20px,18px,16px,14px,12px'",
        );
        check(
            "(() => { const p = [...maestro.root().querySelectorAll(':scope > p')]; return getComputedStyle(p[0]).marginTop === '16px' && getComputedStyle(maestro.root().querySelector('li > p:last-child')).marginBottom === '0px'; })()",
        );
        check(
            "(() => { const last = [...maestro.root().querySelectorAll('td')].at(-1); return getComputedStyle(last).borderRightWidth === '0px' && getComputedStyle(last).borderBottomWidth === '0px'; })()",
        );

        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&js(
        "Promise.all(['KaTeX_Main', 'KaTeX_Math', 'KaTeX_Size2'].map((f) => document.fonts.load(`1em ${f}`)))",
    )))
    .await
    .unwrap();
        check(
            "['KaTeX_Main', 'KaTeX_Math', 'KaTeX_Size2'].every((f) => document.fonts.check(`1em ${f}`)) && [...document.fonts].filter((f) => f.family.startsWith('KaTeX') && f.status === 'loaded').length >= 3",
        );
        check(
            "(() => { const display = maestro.root().querySelector('.katex-display .katex').getBoundingClientRect(), inline = maestro.root().querySelector('p .katex').getBoundingClientRect(); return display.width > 0 && display.height > inline.height && inline.width > 0; })()",
        );
        check(
            "(() => { const code = maestro.root().querySelector('.maestro-code code'); return getComputedStyle(code).fontFamily.length > 0 && code.getBoundingClientRect().height > 0; })()",
        );

        copy_glyph_paint(&mut surface, &mut block).await;
    }

    /// Copy and successful check feedback produce distinct visible raster samples.
    async fn copy_glyph_paint(surface: &mut Surface, block: &mut MarkdownBlock) {
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        let center = first_slot_center();
        let glyph = egui::Rect::from_center_size(center, egui::vec2(16., 16.));
        let before = surface
            .paint
            .region(&surface.context, &surface.shapes, glyph);
        assert!(
            before.iter().any(|p| p[3] > 0),
            "copy paint survives the final clip"
        );
        surface.click(center, |ui, view| block.render(ui, view).unwrap());
        settle().await;
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        let after = surface
            .paint
            .region(&surface.context, &surface.shapes, glyph);
        assert!(
            after.iter().any(|p| p[3] > 0),
            "check paint survives the final clip"
        );
        assert_ne!(before, after, "copy and check have distinct painted glyphs");
    }

    /// Each selected presentation resolves native paint without replacing content.
    fn paint_mode(
        surface: &mut Surface,
        block: &mut MarkdownBlock,
        presentation: &maestro_theme::BrandPresentation<'_>,
        thinking: bool,
    ) {
        block.is_thinking = thinking;
        for _ in 0..2 {
            surface.draw_with(presentation, (800., 600.), None, |ui, view| {
                block.render(ui, view).unwrap()
            });
        }
        let colors: serde_json::Map<String, serde_json::Value> =
            ["text", "muted", "accent", "border", "panel", "error"]
                .into_iter()
                .map(|role| (role.to_owned(), presentation.colors[role].into()))
                .collect();
        publish(
            "colors",
            &serde_json::Value::Object(colors).to_string().into(),
        );
        let failures = js(&format!(
            "maestro.paintFailures(JSON.parse(globalThis.colors), {thinking}).join('; ')"
        ));
        assert_eq!(failures.as_string().unwrap(), "", "thinking {thinking}");
    }
    /// Hover changes link text paint, never its image or inline-code background.
    #[wasm_bindgen]
    pub async fn markdown_hover_keeps_nontext_paint() {
        let mut surface = Surface::new();
        let mut block = MarkdownBlock::new(surface.host());
        block.escape_html = false;
        block.content = "<a href='#hover'><span>hover text</span><img width='20' height='20' src='data:image/svg+xml,%3Csvg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"20\"%3E%3Cpath fill=\"red\" d=\"M0 0h20v20H0z\"/%3E%3C/svg%3E'><code>code</code></a>".into();
        surface.settle(&mut block);
        crate::support::reading_paint::observe("markdown-hover").await;
    }
    /// Header text occupies its authored left, center or right cell position.
    #[wasm_bindgen]
    pub fn markdown_header_alignment_controls_measured_text() {
        let mut surface = Surface::new();
        let _block =
            surface.markdown("| left | center | right |\n| :--- | :---: | ---: |\n| x | y | z |");
        check(
            "(() => { const cells = [...maestro.root().querySelectorAll('th')]; return cells.every((cell, i) => { const r = document.createRange(); r.selectNodeContents(cell); const t = r.getBoundingClientRect(), b = cell.getBoundingClientRect(); return i === 0 ? t.left - b.left < 12 : i === 1 ? Math.abs((t.left + t.right - b.left - b.right) / 2) < 1 : b.right - t.right < 12; }); })()",
        );
    }

    /// Every failed phase preserves the published listing and unchanged-body updates keep selection.
    #[wasm_bindgen]
    pub fn code_failed_publication_keeps_text_selection_and_scroll() {
        let mut surface = Surface::new();
        let mut block = CodeBlock::new(surface.host());
        block.code = "selected line\n".repeat(100);
        block.language = "first-unknown".into();
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        let _ = js(
            "globalThis.codeRoot = document.querySelector('.maestro-code'); globalThis.body = codeRoot.querySelector('code'); globalThis.scroll = codeRoot.querySelector('.maestro-code-scroll'); scroll.scrollTop = 23; globalThis.offset = scroll.scrollTop; const r = document.createRange(); r.selectNodeContents(body); getSelection().removeAllRanges(); getSelection().addRange(r); globalThis.selectionText = getSelection().toString(); globalThis.originalText = body.textContent",
        );
        for phase in [
            Some(ReadingPhase::Measure),
            None,
            Some(ReadingPhase::Publish),
        ] {
            block.code = "changed text".into();
            block.language = "python".into();
            surface.effects.fail_phase.set(phase);
            surface.effects.fail_measure.set(phase.is_none());
            assert!(surface.draw(|ui, view| block.render(ui, view)).is_err());
            check(
                "codeRoot.isConnected && document.querySelector('.maestro-code') === codeRoot && body.textContent === originalText && getSelection().toString() === selectionText && scroll.scrollTop === offset && codeRoot.querySelector('.maestro-code-label').textContent === 'first-unknown'",
            );
        }
        block.code = "selected line\n".repeat(100);
        block.language = "second-unknown".into();
        surface.draw(|ui, view| block.render(ui, view).unwrap());
        check(
            "document.querySelector('.maestro-code') === codeRoot && codeRoot.querySelector('code') === body && getSelection().toString() === selectionText && scroll.scrollTop === offset && codeRoot.querySelector('.maestro-code-label').textContent === 'second-unknown'",
        );
    }
    /// A rejected candidate cannot steal the mounted presentation backedge.
    #[wasm_bindgen]
    pub fn markdown_failed_publication_keeps_mounted_presentation_owner() {
        let mut surface = Surface::new();
        let mut block = surface.markdown("[kept](#kept)");
        let _ = js(
            "globalThis.kept = maestro.root(); const a = kept.querySelector('a'); a.focus(); globalThis.focused = document.activeElement",
        );
        for phase in [
            Some(ReadingPhase::Measure),
            None,
            Some(ReadingPhase::Publish),
        ] {
            block.content = "replacement".into();
            surface.effects.fail_phase.set(phase);
            surface.effects.fail_measure.set(phase.is_none());
            assert!(surface.draw(|ui, view| block.render(ui, view)).is_err());
            check("maestro.root() === kept && document.activeElement === focused");
            block.content = "[kept](#kept)".into();
            block.is_thinking = !block.is_thinking;
            surface.draw(|ui, view| block.render(ui, view).unwrap());
            publish("thinking", &block.is_thinking.into());
            check(
                "maestro.root() === kept && (getComputedStyle(kept).fontStyle === 'italic') === thinking && document.activeElement === focused",
            );
        }
    }
    /// Each emitted syntax class resolves to actual pixels from the selected brand pack.
    #[wasm_bindgen]
    pub async fn markdown_native_paints_all_syntax_roles() {
        use maestro_theme::ThemeColor;
        let mut surface = Surface::new();
        surface.highlighter = std::rc::Rc::new(crate::markdown_support::Highlighter(vec![
            Some(ThemeColor::SyntaxComment),
            Some(ThemeColor::SyntaxKeyword),
            Some(ThemeColor::SyntaxFunction),
            Some(ThemeColor::SyntaxVariable),
            Some(ThemeColor::SyntaxString),
            Some(ThemeColor::SyntaxNumber),
            Some(ThemeColor::SyntaxType),
            Some(ThemeColor::SyntaxOperator),
            Some(ThemeColor::SyntaxPunctuation),
        ]));
        let mut block = surface.markdown("```js\ncomment\nkeyword\nfunction\nvariable\nstring\nnumber\ntype\noperator\npunctuation\n```");
        let packs = [surface.pack.clone(), surface.alternate.clone()];
        for (pack, mode) in packs
            .iter()
            .flat_map(|pack| [BrandMode::Dark, BrandMode::Light].map(|mode| (pack, mode)))
        {
            let presentation = pack.presentation(mode).unwrap();
            surface.draw_with(&presentation, (800., 600.), None, |ui, view| {
                block.render(ui, view).unwrap()
            });
            let colors: serde_json::Map<String, serde_json::Value> = presentation
                .colors
                .iter()
                .map(|(role, value)| ((*role).to_owned(), (*value).into()))
                .collect();
            publish(
                "colors",
                &serde_json::Value::Object(colors).to_string().into(),
            );
            crate::support::reading_paint::observe("markdown-syntax").await;
        }
    }
}
