//! Offline Markdown and output formatting through their public entry points.
#![cfg(test)]
mod export_support;
#[cfg(test)]
mod tests {
    use crate::export_support;
    use maestro_web::export_html::template::{format_expandable_output, safe_marked_parse};

    /// Literal constructs do not swallow following Markdown operands.
    /// # Panics
    /// Panics when literal markup changes suffix emphasis or links.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_literal_constructs_preserve_suffix_operands() {
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        for (source, expected) in [
            (
                "<!-- note --> **bold** [go](/docs)",
                "<p>&lt;!-- note --&gt; <strong>bold</strong> <a href=\"/docs\">go</a></p>\n",
            ),
            (
                "<!DOCTYPE html> **bold** [go](/docs)",
                "<p>&lt;!DOCTYPE html&gt; <strong>bold</strong> <a href=\"/docs\">go</a></p>\n",
            ),
            (
                "<![CDATA[note]]> **bold** [go](/docs)",
                "<p>&lt;![CDATA[note]]&gt; <strong>bold</strong> <a href=\"/docs\">go</a></p>\n",
            ),
        ] {
            assert_eq!(safe_marked_parse(source, &engine), expected, "{source}");
        }
    }

    /// Exercise export markdown attributes cannot create handlers.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_markdown_attributes_cannot_create_handlers() {
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        let html = safe_marked_parse("[label](</safe> '\" onmouseover=\"run()')", &engine);
        assert!(
            html.contains("title=\"&quot; onmouseover=&quot;run()\""),
            "{html}"
        );
        assert!(!html.contains(" title=\"\" onmouseover="));
        check_rows(
            "export_markdown_attributes_cannot_create_handlers",
            "markdown",
        );
    }

    /// Exercise export markdown requires strict double tilde delimiters.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_markdown_requires_strict_double_tilde_delimiters() {
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        assert_eq!(
            safe_marked_parse("before ~~~ab~~ after", &engine),
            "<p>before ~~~ab~~ after</p>\n"
        );
        check_rows(
            "export_markdown_requires_strict_double_tilde_delimiters",
            "markdown",
        );
    }

    /// Exercise expandable output keeps lines previews and independent code bodies.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn expandable_output_keeps_lines_previews_and_independent_code_bodies() {
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        let html = maestro_web::export_html::template::format_expandable_output(
            "one\n\nthree\t",
            1,
            None,
            &engine,
        );
        assert_eq!(
            html,
            "<div class=\"tool-output expandable\"><div class=\"output-preview\"><div>one</div><div class=\"expand-hint\">... (2 more lines)</div></div><div class=\"output-full\"><div>one</div><div></div><div>three   </div></div></div>"
        );
        check_rows(
            "expandable_output_keeps_lines_previews_and_independent_code_bodies",
            "output",
        );
    }

    /// Exercise export markdown treats authored markup and math literally.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_markdown_treats_authored_markup_and_math_literally() {
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        assert_eq!(
            safe_marked_parse("</div >", &engine),
            "<p>&lt;/div &gt;</p>\n"
        );
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        assert_eq!(
            safe_marked_parse("<div>**literal** [r][r]</div>\n\n[r]: /docs", &engine),
            "<p>&lt;div&gt;<strong>literal</strong> <a href=\"/docs\">r</a>&lt;/div&gt;</p>\n"
        );
        check_rows(
            "export_markdown_treats_authored_markup_and_math_literally",
            "markdown",
        );
    }

    /// Literal markup leaves escapes, definitions and code operands intact.
    /// # Panics
    /// Panics if a parser operand or literal declaration changes.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_markdown_keeps_operands_when_markup_is_literal() {
        check_rows(
            "export_markdown_keeps_operands_when_markup_is_literal",
            "markdown",
        );
    }

    /// Exercise complete corpus operations through the public formatting boundary.
    fn check_rows(witness: &str, operation: &str) {
        #[cfg(target_arch = "wasm32")]
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into());
        }));
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        for row in export_support::rows(witness, operation) {
            assert_eq!(row.branch, "success");
            let text = row.input["text"].as_str().unwrap();
            let html = match row.operation.as_str() {
                "markdown" => safe_marked_parse(text, &engine),
                "output" => format_expandable_output(
                    text,
                    row.input["maxLines"].as_u64().unwrap().try_into().unwrap(),
                    row.input["lang"].as_str(),
                    &engine,
                ),
                _ => panic!("unknown corpus operation"),
            };
            export_support::assert_markup(&html, row.expected["html"].as_str().unwrap(), &row.id);
            if let Some(consumer) = row.expected.get("consumer") {
                check_consumer(&html, consumer, &row.id);
            }
        }
    }

    /// Require each URI row's actual element kind and literal accepted operand.
    fn check_consumer(html: &str, consumer: &serde_json::Value, id: &str) {
        assert!(consumer["unsafe_scheme"].is_null(), "{id}");
        if let Some(tag) = consumer["tag"].as_str() {
            assert!(
                html.contains(&format!("<{} ", tag.to_ascii_lowercase())),
                "{id}"
            );
            let attribute = maestro_app::markup::escape(
                consumer["attribute"].as_str().unwrap(),
                maestro_app::markup::EscapeMode::HtmlDecimal,
            );
            assert!(
                html.replace('\0', "\u{fffd}")
                    .contains(&format!("=\"{attribute}\"")),
                "{id}"
            );
        } else {
            assert!(consumer["attribute"].is_null(), "{id}");
            assert!(!html.contains("<a ") && !html.contains("<img "), "{id}");
        }
    }

    /// Exercise export markdown blocks unsafe links and images.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_markdown_blocks_unsafe_links_and_images() {
        check_rows("export_markdown_blocks_unsafe_links_and_images", "markdown");
    }

    /// Exercise export markdown keeps allowed destinations.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_markdown_keeps_allowed_destinations() {
        check_rows("export_markdown_keeps_allowed_destinations", "markdown");
    }

    /// Exercise export markdown preserves structure and hard breaks.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_markdown_preserves_structure_and_hard_breaks() {
        check_rows(
            "export_markdown_preserves_structure_and_hard_breaks",
            "markdown",
        );
    }

    /// Exercise export code uses viewer policy without chat controls.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_code_uses_viewer_policy_without_chat_controls() {
        let code = "{\"enabled\": true, \"value\": 42}";
        for (label, grammar) in [
            ("javascript", "javascript"),
            ("js", "javascript"),
            ("", "json"),
            ("missing-language", "json"),
            ("javascript metadata", "json"),
        ] {
            let engine = Faults {
                calls: std::cell::RefCell::default(),
                fail: 0,
            };
            let html = safe_marked_parse(&format!("```{label}\n{code}\n```"), &engine);
            assert_eq!(
                &*engine.calls.borrow(),
                &[(code.to_owned(), grammar.to_owned())]
            );
            assert!(
                html.contains("<span class=\"hljs-comment\">"),
                "{label}: {html}"
            );
        }
        check_rows(
            "export_code_uses_viewer_policy_without_chat_controls",
            "markdown",
        );
    }

    /// Failure injection at the actual shared highlighter operation.
    struct Faults {
        /// Full calls, in invocation order.
        calls: std::cell::RefCell<Vec<(String, String)>>,
        /// One-based operation to reject.
        fail: usize,
    }
    impl maestro_theme::SyntaxHighlighter for Faults {
        fn supports_language(&self, _: &str) -> bool {
            true
        }
        fn highlight(
            &self,
            code: &str,
            language: &str,
        ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>>
        {
            let ordinal = {
                let mut calls = self.calls.borrow_mut();
                calls.push((code.to_owned(), language.to_owned()));
                calls.len()
            };
            if ordinal == self.fail {
                return Err(std::io::Error::other("controlled highlighting failure").into());
            }
            Ok(vec![maestro_theme::SyntaxSpan {
                range: 0..code.len(),
                color: Some(maestro_theme::ThemeColor::SyntaxComment),
            }])
        }
    }

    /// Exercise export highlighter failures preserve the other body.
    /// # Panics
    /// Panics if a corpus observation differs.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn export_highlighter_failures_preserve_the_other_body() {
        #[cfg(target_arch = "wasm32")]
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into());
        }));
        for row in export_support::rows(
            "export_highlighter_failures_preserve_the_other_body",
            "highlight-fault",
        ) {
            assert_eq!(row.branch, "success");
            let engine = Faults {
                calls: std::cell::RefCell::default(),
                fail: row.input["failCall"].as_u64().unwrap().try_into().unwrap(),
            };
            let text = row.input["text"].as_str().unwrap();
            let html = if row.input["surface"] == "output" {
                format_expandable_output(
                    text,
                    row.input["maxLines"].as_u64().unwrap().try_into().unwrap(),
                    row.input["lang"].as_str(),
                    &engine,
                )
            } else {
                assert_eq!(row.input["surface"], "markdown");
                safe_marked_parse(text, &engine)
            };
            export_support::assert_markup(&html, row.expected["html"].as_str().unwrap(), &row.id);
            assert_eq!(
                html.replace("\n              ", ""),
                row.expected["html"]
                    .as_str()
                    .unwrap()
                    .replace("\n              ", ""),
                "{}",
                row.id
            );
            check_calls(&row, &engine.calls.borrow());
        }
    }
    /// Require complete ordered highlighter operands, including explicit-language identity.
    fn check_calls(row: &export_support::Row, calls: &[(String, String)]) {
        let expected = row.expected["calls"].as_array().unwrap();
        assert_eq!(calls.len(), expected.len(), "{}", row.id);
        for ((text, lang), expected) in calls.iter().zip(expected) {
            assert_eq!(text, expected[0].as_str().unwrap(), "{}", row.id);
            if row.input["method"] == "highlight" {
                assert_eq!(
                    lang,
                    expected[1]["language"].as_str().unwrap(),
                    "{}",
                    row.id
                );
            } else {
                assert_eq!(row.input["method"], "highlightAuto");
                assert_eq!(expected.as_array().unwrap().len(), 1);
                assert_eq!(lang, "bash", "{}", row.id);
            }
        }
    }
}
