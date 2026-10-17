//! Real-browser Markdown rendering through the public native component.
#![cfg(all(test, target_arch = "wasm32"))]
#[path = "attachment_support/canvas_paint.rs"]
pub mod canvas_paint;
pub mod markdown_support;
pub mod support;

#[cfg(test)]
mod tests {
    use crate::markdown_support::{
        BlockRow, Surface, block_rows, check, context_rows, first_slot_center, js, publish, settle,
    };
    use maestro_web::components::markdown_block::MarkdownBlock;
    use wasm_bindgen::prelude::*;

    /// The markup a row must produce, given the recorded library observations.
    fn target(row: &BlockRow) -> &str {
        match (&row.current_html, &row.original_html) {
            (Some(current), _) => {
                assert_ne!(
                    current, &row.html,
                    "{}: the tracked gap must still differ",
                    row.id
                );
                current
            }
            (None, Some(original)) => {
                assert_ne!(
                    original, &row.html,
                    "{}: the corrected row must differ",
                    row.id
                );
                &row.html
            }
            (None, None) => &row.html,
        }
    }
    /// Mount every row owned by `test` and require its recorded markup.
    fn rows_match(test: &str) {
        let mut surface = Surface::new();
        for row in block_rows(test) {
            let block = surface.row(&row);
            let difference = Surface::difference(target(&row));
            assert_eq!(difference, "", "row {}", row.id);
            drop(block);
            check("document.querySelector('.maestro-markdown') === null");
        }
    }
    /// Mount every contextual row owned by `test` and require its recorded structure.
    fn contexts_match(test: &str) {
        let mut surface = Surface::new();
        for row in context_rows(test) {
            let block = surface.markdown(&row.source);
            assert_eq!(Surface::tree_difference(&row.tree), "", "row {}", row.id);
            drop(block);
        }
    }

    /// Empty, whitespace, headings, lists, tasks, breaks and nested positions keep their content.
    #[wasm_bindgen]
    pub fn markdown_empty_and_structural_blocks_preserve_content() {
        let surface = Surface::new();
        let block = MarkdownBlock::new(surface.host());
        assert!(block.content.is_empty() && !block.is_thinking && block.escape_html);
        drop(block);
        rows_match("markdown_empty_and_structural_blocks_preserve_content");
    }

    /// Line endings, tabs, NUL and Unicode reach the native text unchanged; tabs stay tabs.
    #[wasm_bindgen]
    pub fn markdown_preserves_browser_whitespace_and_replacement() {
        rows_match("markdown_preserves_browser_whitespace_and_replacement");
        let mut surface = Surface::new();
        let _block = surface.markdown("```\n\tkept\tinside\n```");
        check("document.querySelector('.maestro-code code').textContent === '\\tkept\\tinside\\n'");
    }

    /// Tag-like text is shown literally and Unicode survives.
    #[wasm_bindgen]
    pub fn markdown_escapes_html_without_losing_unicode() {
        rows_match("markdown_escapes_html_without_losing_unicode");
    }

    /// Raw markup is kept when escaping is off, independently of the thinking style.
    #[wasm_bindgen]
    pub fn markdown_raw_html_option_preserves_native_markup() {
        rows_match("markdown_raw_html_option_preserves_native_markup");
        let mut surface = Surface::new();
        let mut block = surface.markdown("<b id=\"kept\">x</b>");
        check("document.querySelector('.maestro-markdown b') === null");
        for (escape, thinking) in [(false, false), (false, true), (true, true), (true, false)] {
            block.escape_html = escape;
            block.is_thinking = thinking;
            surface.settle(&mut block);
            let raw = js("document.querySelector('.maestro-markdown b#kept') !== null")
                .as_bool()
                .unwrap();
            assert_eq!(raw, !escape, "escape {escape}, thinking {thinking}");
            let italic = js("getComputedStyle(maestro.root()).fontStyle === 'italic'")
                .as_bool()
                .unwrap();
            assert_eq!(italic, thinking, "escape {escape}, thinking {thinking}");
        }
    }

    /// Code text reaches the listing and the clipboard as the original, decoded once.
    #[wasm_bindgen]
    pub async fn markdown_code_entities_decode_exactly_once() {
        rows_match("markdown_code_entities_decode_exactly_once");
        let mut surface = Surface::new();
        let mut block = surface.markdown("```html\n<script>&amp;\"'</script> 雪\n```");
        check(
            "document.querySelector('.maestro-code code').textContent === '<script>&amp;\"\\'</script> 雪\\n'",
        );
        surface.click(first_slot_center(), |ui, view| {
            block.render(ui, view).unwrap()
        });
        settle().await;
        assert_eq!(
            *surface.effects.copies.borrow(),
            ["<script>&amp;\"'</script> 雪\n"]
        );
    }

    /// Marker-looking text, dollars and backticks stay literal around code.
    #[wasm_bindgen]
    pub fn markdown_code_preservation_keeps_literal_placeholder_text() {
        rows_match("markdown_code_preservation_keeps_literal_placeholder_text");
        let mut surface = Surface::new();
        let _block =
            surface.markdown("__CODE_BLOCK_0__ `$&` `$1` ``a`b``\n\n    __CODE_BLOCK_0__ $'");
        check(
            "[...document.querySelectorAll('.maestro-markdown code:not(.hljs)')].map(c => c.textContent).join('|') === '$&|$1|a`b'",
        );
        check(
            "document.querySelector('.maestro-code code').textContent === '__CODE_BLOCK_0__ $\\'\\n'",
        );
    }

    /// Every anchor opens separately with the safe relation, formatted labels and images included.
    #[wasm_bindgen]
    pub fn markdown_links_open_separately_with_safe_relation() {
        rows_match("markdown_links_open_separately_with_safe_relation");
        let mut surface = Surface::new();
        let _block = surface.markdown(
            "[a **b**](https://x.test \"t\") https://y.test ![i](https://z.test/i.png \"c\")",
        );
        check(
            "[...document.querySelectorAll('.maestro-markdown a')].length === 2 && [...document.querySelectorAll('.maestro-markdown a')].every(a => a.target === '_blank' && a.rel === 'noopener noreferrer')",
        );
        check(
            "document.querySelector('.maestro-markdown a strong').textContent === 'b' && document.querySelector('.maestro-markdown a').title === 't'",
        );
        check(
            "document.querySelector('.maestro-markdown img').getAttribute('src') === 'https://z.test/i.png' && document.querySelector('.maestro-markdown img').title === 'c'",
        );
    }

    /// Relative, fragment, mail and script destinations keep their authored meaning.
    #[wasm_bindgen]
    pub fn markdown_link_targets_keep_authored_behavior() {
        rows_match("markdown_link_targets_keep_authored_behavior");
    }

    /// Tables keep order, alignment, cell content and their own horizontal scroller.
    #[wasm_bindgen]
    pub fn markdown_tables_keep_native_overflow_and_cell_content() {
        rows_match("markdown_tables_keep_native_overflow_and_cell_content");
        let mut surface = Surface::new();
        let _block = surface.markdown("| a | b |\n| :- | -: |\n| `c` | $d$ |\n");
        check(
            "getComputedStyle(document.querySelector('.maestro-table-scroll')).overflowX === 'auto'",
        );
        check(
            "[...document.querySelectorAll('.maestro-markdown td')].map(c => c.align).join() === 'left,right'",
        );
        check(
            "document.querySelector('.maestro-markdown td:last-child .katex') !== null && document.querySelector('.maestro-markdown td code').textContent === 'c'",
        );
    }

    /// Nested links, ordered-list interruption and label folding follow the standard grammar.
    #[wasm_bindgen]
    pub fn markdown_commonmark_corrections_keep_valid_structure() {
        rows_match("markdown_commonmark_corrections_keep_valid_structure");
    }

    /// Reasoning only changes muted, italic and small presentation.
    #[wasm_bindgen]
    pub fn markdown_thinking_changes_only_text_presentation() {
        rows_match("markdown_thinking_changes_only_text_presentation");
        let mut surface = Surface::new();
        let mut block = surface.markdown("**t** $x$ [l](https://x.test)\n\n```js\nlet a;\n```");
        publish("beforeHtml", &js("maestro.root().innerHTML"));
        check("getComputedStyle(maestro.root()).fontStyle === 'normal'");
        publish(
            "before",
            &js(
                "(() => { const s = getComputedStyle(maestro.root()); return [s.color, s.fontSize].join('|'); })()",
            ),
        );
        block.is_thinking = true;
        surface.settle(&mut block);
        check("maestro.root().innerHTML === globalThis.beforeHtml");
        check("getComputedStyle(maestro.root()).fontStyle === 'italic'");
        check(
            "(() => { const s = getComputedStyle(maestro.root()); return [s.color, s.fontSize].join('|') !== globalThis.before; })()",
        );
    }

    /// Labels, unknown and non-word names, empty, tilde, indented and unclosed fences keep their copy slots.
    #[wasm_bindgen]
    pub fn markdown_code_fences_keep_labels_text_and_copy_slots() {
        rows_match("markdown_code_fences_keep_labels_text_and_copy_slots");
        let mut surface = Surface::new();
        let _block = surface.markdown(
            "```c++\nint x;\n```\n\n```objective-c\nvoid f();\n```\n\nplain\n\n    indented\n",
        );
        check(
            "[...document.querySelectorAll('.maestro-code-label')].map(l => l.textContent).join() === 'c++,objective-c,text'",
        );
        check("document.querySelectorAll('.maestro-code .control-slot').length === 3");
        check("document.querySelectorAll('.maestro-code').length === 3");
    }

    /// Every delimiter keeps its mode, trimming and reached error output.
    #[wasm_bindgen]
    pub fn markdown_math_delimiters_keep_modes_trimming_and_errors() {
        rows_match("markdown_math_delimiters_keep_modes_trimming_and_errors");
    }

    /// Empty, unclosed, embedded, escaped and adjacent delimiters stay ordinary text.
    #[wasm_bindgen]
    pub fn markdown_math_recognition_keeps_nonmath_neighbors() {
        rows_match("markdown_math_recognition_keeps_nonmath_neighbors");
    }

    /// The approved renderer's output is used unchanged, with its tracked gaps kept visible.
    #[wasm_bindgen]
    pub fn markdown_math_tracks_qualified_library_gaps() {
        rows_match("markdown_math_tracks_qualified_library_gaps");
    }

    /// Tight, loose and multiline list items keep their formula modes.
    #[wasm_bindgen]
    pub fn markdown_math_keeps_display_context_in_lists() {
        contexts_match("markdown_math_keeps_display_context_in_lists");
    }

    /// Quoted paragraphs split around display formulas and keep nested continuation.
    #[wasm_bindgen]
    pub fn markdown_math_keeps_display_context_in_quotes() {
        contexts_match("markdown_math_keeps_display_context_in_quotes");
    }

    /// Table cells admit inline formulas only and keep their neighbors and alignment.
    #[wasm_bindgen]
    pub fn markdown_math_keeps_display_context_in_tables() {
        contexts_match("markdown_math_keeps_display_context_in_tables");
    }

    /// A display formula inside a paragraph separates its neighbors without trimming their spaces.
    #[wasm_bindgen]
    pub fn markdown_display_math_preserves_neighbor_whitespace() {
        contexts_match("markdown_display_math_preserves_neighbor_whitespace");
    }

    /// Link labels keep math while destinations, plain URLs and angle autolinks stay literal.
    #[wasm_bindgen]
    pub fn markdown_link_labels_keep_math_but_destinations_stay_literal() {
        contexts_match("markdown_link_labels_keep_math_but_destinations_stay_literal");
    }
    /// Formula preparation retains document-wide reference destinations and titles.
    #[wasm_bindgen]
    pub fn markdown_math_preserves_document_references() {
        let mut surface = Surface::new();
        let _block = surface.markdown(
            "[r][r] $x$ ![alt][image]\n\n[r]: /docs \"tip\"\n[image]: /favicon.ico \"image tip\"",
        );
        check(
            "maestro.root().querySelector('a').getAttribute('href') === '/docs' && maestro.root().querySelector('a').title === 'tip'",
        );
        check(
            "maestro.root().querySelector('img').getAttribute('src') === '/favicon.ico' && maestro.root().querySelector('img').title === 'image tip'",
        );
        check("maestro.root().querySelector('.katex') !== null");
    }

    /// Block-looking label punctuation stays literal beside a formula inside its anchor.
    #[wasm_bindgen]
    pub fn markdown_formula_labels_use_inline_grammar() {
        let mut surface = Surface::new();
        for (label, href, prefix) in [
            ("1. $x$", "#equation", "1. "),
            ("# $x$", "u", "# "),
            ("> $x$", "u", "> "),
            ("# Results $x$", "/docs", "# Results "),
            ("$a[b$ c]", "/docs", ""),
            ("a $x$\nb", "/docs", "a "),
            (r"\[ $x$ \]", "/docs", "[ "),
            (r"\\[ $x$ ]", "/docs", r"\[ "),
            (r"\\\[ $x$ \]", "/docs", r"\[ "),
            (" $x$ ", "/docs", " "),
        ] {
            let block = surface.markdown(&format!("[{label}]({href} \"tip\")"));
            publish("labelHref", &href.into());
            publish("labelPrefix", &prefix.into());
            assert_eq!(
                js("maestro.root().querySelectorAll('a').length").as_f64(),
                Some(1.0),
                "label {label}"
            );
            check("maestro.root().querySelector('a').getAttribute('href') === labelHref");
            check("maestro.root().querySelector('a').title === 'tip'");
            check("maestro.root().querySelector('a').textContent.startsWith(labelPrefix)");
            check("maestro.root().querySelectorAll('a .katex').length === 1");
            check("maestro.root().querySelector('a h1, a ol, a ul, a blockquote') === null");
            if label == "$a[b$ c]" {
                check("maestro.root().querySelector('a').textContent.endsWith(' c]')");
            }
            if label == " $x$ " {
                check("maestro.root().querySelector('a').firstChild.textContent === ' '");
                check("maestro.root().querySelector('a').lastChild.textContent === ' '");
            }
            if label.contains('\n') {
                check("maestro.root().querySelector('a').textContent.endsWith('\\nb')");
            }
            drop(block);
        }
    }

    /// Resolved shortcut and collapsed references retain their identity while labels render math.
    #[wasm_bindgen]
    pub fn markdown_formula_labels_keep_reference_identity() {
        let mut surface = Surface::new();
        for label in ["[$x$]", "[$x$][]", "[$x$][r]"] {
            let block = surface.markdown(&format!(
                "{label}\n\n[$x$]: /docs \"tip\"\n[r]: /docs \"tip\""
            ));
            check("maestro.root().querySelectorAll('a[href=\"/docs\"]').length === 1");
            check("maestro.root().querySelector('a').title === 'tip'");
            check("maestro.root().querySelectorAll('a .katex').length === 1");
            check(
                "!maestro.root().textContent.includes('[]') && !maestro.root().textContent.includes('[r]')",
            );
            drop(block);
        }
    }

    /// A formula beginning before link syntax wins; a formula inside a label keeps its anchor.
    #[wasm_bindgen]
    pub fn markdown_formula_order_keeps_resolved_label_scope() {
        let mut surface = Surface::new();
        for (source, anchors) in [
            ("$[f](x)$", 0),
            ("$[f][r]$\n\n[r]: x", 0),
            ("[$x$][r]\n\n[r]: /docs \"tip\"", 1),
            ("$f(x)$", 0),
        ] {
            let block = surface.markdown(source);
            assert_eq!(
                js("maestro.root().querySelectorAll('a').length").as_f64(),
                Some(f64::from(anchors))
            );
            check("maestro.root().querySelectorAll('.katex').length === 1");
            drop(block);
        }
        let _block = surface.markdown("$x `y$`");
        check(
            "maestro.root().querySelector('.katex') === null && maestro.root().querySelector('code').textContent === 'y$'",
        );
    }

    /// A formula cannot close inside an opaque code span.
    #[wasm_bindgen]
    pub fn markdown_math_rejects_opaque_overlap() {
        let mut surface = Surface::new();
        let _block = surface.markdown("$x `y$`");
        check(
            "maestro.root().querySelector('.katex') === null && maestro.root().querySelector('code').textContent === 'y$'",
        );
    }

    /// Display math keeps native block grammar in the following source slice.
    #[wasm_bindgen]
    pub fn markdown_display_math_keeps_following_native_blocks() {
        let mut surface = Surface::new();
        for (source, selector, text, math) in [
            ("$$x$$ # heading", "h1", "heading", 1),
            ("$$x$$ - item", "li", "item", 1),
            ("$$x$$ # heading $y$", "h1", "heading", 2),
        ] {
            let block = surface.markdown(source);
            publish("blockSelector", &selector.into());
            publish("blockText", &text.into());
            check(
                "maestro.root().querySelector(blockSelector)?.textContent.startsWith(blockText) === true",
            );
            assert_eq!(
                js("maestro.root().querySelectorAll('.katex').length").as_f64(),
                Some(f64::from(math))
            );
            drop(block);
        }
    }

    /// Authored slot-like markup never receives a generated listing.
    #[wasm_bindgen]
    pub fn markdown_generated_listings_ignore_authored_slots() {
        let mut surface = Surface::new();
        let mut block = MarkdownBlock::new(surface.host());
        block.escape_html = false;
        block.content =
            "<div class=\"maestro-code-slot\" data-code=\"0\">authored</div>\n\n```js\nlet x;\n```"
                .into();
        surface.settle(&mut block);
        check(
            "maestro.root().firstElementChild.textContent === 'authored' && !maestro.root().firstElementChild.querySelector('.maestro-code')",
        );
        check("maestro.root().lastElementChild.querySelector('code').textContent === 'let x;\\n'");
    }

    /// Replacement engines reclassify retained listings without editing Markdown.
    #[wasm_bindgen]
    pub fn markdown_replacement_highlighter_updates_existing_listings() {
        let mut surface = Surface::new();
        surface.highlighter = std::rc::Rc::new(crate::markdown_support::Highlighter(vec![Some(
            maestro_theme::ThemeColor::SyntaxKeyword,
        )]));
        let mut block = surface.markdown("```js\nlet x;\n```");
        check("maestro.root().querySelector('.hljs-keyword') !== null");
        surface.highlighter = std::rc::Rc::new(crate::markdown_support::Highlighter(vec![None]));
        surface.settle(&mut block);
        check(
            "maestro.root().querySelector('.hljs-keyword') === null && maestro.root().querySelector('code').textContent === 'let x;\\n'",
        );
    }
}
