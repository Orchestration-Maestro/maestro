//! Offline header formatting and drawing through the public document route.
#![cfg(test)]
use maestro_web::export_html::template::format_tokens;

#[test]
fn header_tokens_keep_thousand_and_million_rounding() {
    for c in cases(|n| n.starts_with("tokens-")) {
        assert_eq!(c.input.operation.as_deref(), Some("tokens"));
        assert_eq!(
            format_tokens(session_header_support::number(
                c.input.count.as_ref().unwrap()
            )),
            c.output.unwrap().as_str().unwrap(),
            "{}",
            c.name
        );
    }
}
mod session_header_support;
use maestro_web::export_html::template::HeaderState;
use serde_json::json;
use session_header_support::{Frame, document};

#[test]
fn header_empty_uses_exact_fallbacks() {
    let doc = document(&json!({"entries": []}));
    let mut frame = Frame::default();
    assert!(
        frame
            .draw(&doc, &mut HeaderState::default(), vec![], &|_| panic!(
                "unreached date"
            ))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        frame.texts(),
        [
            "Session: unknown",
            "T toggle thinking · O toggle tools",
            "Toggle thinking",
            "Toggle tools",
            "↓ JSONL",
            "Date:",
            "unknown",
            "Models:",
            "unknown",
            "Messages:",
            "0",
            "Tool Calls:",
            "0",
            "Tokens:",
            "0",
            "Cost:",
            "$0.000"
        ]
    );
}
use session_header_support::{cases, expected, metadata, prepared};
#[test]
fn header_document_route_draws_all_metadata_and_controls() {
    for c in cases(|n| n == "populated") {
        let doc = prepared(&c);
        let mut f = Frame::default();
        let mut state = HeaderState::default();
        let localize = |v: &serde_json::Value| {
            assert_eq!(v, &doc.data["header"]["timestamp"]);
            Ok("1/2/2025, 3:04:05 AM".into())
        };
        f.draw(&doc, &mut state, vec![], &localize).unwrap();
        let e = expected(&c);
        metadata(&f, &e);
        let prompt = e.prompt.unwrap();
        assert!(f.texts().contains(&prompt.heading));
        assert!(f.texts().contains(&prompt.full));
        session_header_support::tool_texts(&f, &e.tools, &[false, false]);
        let pos = f.at(e.tools[1].hint.as_ref().unwrap());
        f.draw(&doc, &mut state, click(pos), &localize).unwrap();
        f.draw(&doc, &mut state, vec![], &localize).unwrap();
        session_header_support::tool_texts(&f, &e.tools, &[false, true]);
    }
}
/// Replay metadata-only inputs through the actual document draw.
fn replay_metadata(select: impl Fn(&str) -> bool) {
    for c in cases(select) {
        let doc = prepared(&c);
        let mut f = Frame::default();
        f.draw(&doc, &mut HeaderState::default(), vec![], &|_| {
            panic!("unexpected date read")
        })
        .unwrap();
        metadata(&f, &expected(&c));
    }
}
#[test]
fn header_categories_keep_order_and_omit_only_zero() {
    replay_metadata(|n| {
        n.starts_with("counter-")
            || [
                "all-categories",
                "no-models",
                "empty-model-name",
                "all-negative-tokens",
            ]
            .contains(&n)
    });
    let mut doc = document(&json!({"entries": []}));
    doc.global_stats.tokens.input = f64::NAN;
    let mut f = Frame::default();
    f.plain(&doc, &mut HeaderState::default(), vec![]);
    assert_eq!(&f.texts()[13..15], ["Tokens:", "0"]);
}
#[test]
fn header_uses_prepared_statistics_without_recounting() {
    replay_metadata(|n| n == "stats-snapshot");
    let mut doc = document(&json!({"entries": []}));
    doc.data["entries"] = json!([{"type": "message", "message": {"role": "user"}}]);
    let mut f = Frame::default();
    f.plain(&doc, &mut HeaderState::default(), vec![]);
    assert_eq!(&f.texts()[9..11], ["Messages:", "0"]);
}
#[test]
fn header_identity_stays_literal_and_uses_truthy_fallback() {
    replay_metadata(|n| n.starts_with("id-"));
    for id in [json!(7), json!(true), json!({}), json!([])] {
        let doc = document(&json!({"entries": [], "header": {"id": id}}));
        assert_eq!(
            Frame::default()
                .draw(
                    &doc,
                    &mut HeaderState::default(),
                    vec![],
                    &|_| unreachable!()
                )
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}
#[test]
fn header_cost_sums_four_components_before_three_place_rounding() {
    replay_metadata(|n| {
        [
            "cost-cancellation",
            "negative-cost",
            "fixed-half",
            "large-cost",
        ]
        .contains(&n)
    });
}
#[test]
fn header_dates_forward_values_lazily_and_keep_localized_text() {
    for c in cases(|n| n.starts_with("date-")) {
        let native = c.input.operation.as_deref() == Some("date");
        let data = if native {
            json!({"entries": [], "header": {"timestamp": c.input.timestamp}})
        } else {
            c.input.data.clone().unwrap()
        };
        let doc = document(&data);
        let timestamp = &doc.data["header"]["timestamp"];
        let count = std::cell::Cell::new(0);
        let localized = if native {
            c.output.as_ref().unwrap().as_str().unwrap().to_owned()
        } else {
            expected(&c).info[0][1].clone()
        };
        let mut f = Frame::default();
        f.draw(&doc, &mut HeaderState::default(), vec![], &|v| {
            count.set(count.get() + 1);
            assert_eq!(v, timestamp);
            Ok(localized.clone())
        })
        .unwrap();
        assert_eq!(&f.texts()[5..7], ["Date:", &localized]);
        assert_eq!(count.get(), usize::from(maestro_app::is_truthy(timestamp)));
        if count.get() != 0 {
            let error = f
                .draw(&doc, &mut HeaderState::default(), vec![], &|_| {
                    Err(std::io::Error::from_raw_os_error(73))
                })
                .unwrap_err();
            assert_eq!(error.raw_os_error(), Some(73));
        }
    }
}
#[test]
fn header_prompt_preserves_literal_text_and_blank_lines() {
    for c in cases(|n| {
        n.starts_with("prompt-")
            && !n.starts_with("prompt-lines-")
            && ![
                "prompt-trailing-newline",
                "prompt-crlf",
                "prompt-lone-cr",
                "prompt-other-separators",
            ]
            .contains(&n)
    }) {
        let doc = prepared(&c);
        let mut f = Frame::default();
        let result = f.draw(
            &doc,
            &mut HeaderState::default(),
            vec![],
            &|_| unreachable!(),
        );
        if c.branch == "error" {
            assert_eq!(c.error_kind.as_deref(), Some("invalid_data"));
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        } else {
            result.unwrap();
            let e = expected(&c);
            if let Some(prompt) = e.prompt {
                assert!(f.texts().contains(&prompt.heading));
                assert!(f.texts().contains(&prompt.full));
            } else {
                assert!(!f.texts().contains(&"System Prompt".into()));
            }
        }
    }
}
use session_header_support::click;
#[test]
fn header_prompt_previews_exactly_ten_split_lines() {
    for c in cases(|n| {
        n.starts_with("prompt-lines-")
            || [
                "prompt-trailing-newline",
                "prompt-crlf",
                "prompt-lone-cr",
                "prompt-other-separators",
            ]
            .contains(&n)
    }) {
        let doc = prepared(&c);
        let e = expected(&c).prompt.unwrap();
        let mut f = Frame::default();
        let mut state = HeaderState::default();
        f.plain(&doc, &mut state, vec![]);
        assert!(
            f.texts().contains(e.preview.as_ref().unwrap_or(&e.full)),
            "{}",
            c.name
        );
        assert_eq!(e.expandable, e.hint.is_some());
        if let Some(hint) = &e.hint {
            assert!(f.texts().contains(hint));
            if e.preview.as_ref() != Some(&e.full) {
                assert!(!f.texts().contains(&e.full));
            }
            let pos = f.at(hint);
            f.plain(&doc, &mut state, click(pos));
            f.plain(&doc, &mut state, vec![]);
            assert!(f.texts().contains(&e.full));
            assert!(!f.texts().contains(hint));
            let pos = f.at(&e.heading);
            f.plain(&doc, &mut state, click(pos));
            f.plain(&doc, &mut state, vec![]);
            assert!(f.texts().contains(hint));
        } else {
            let pos = f.at(&e.heading);
            f.plain(&doc, &mut state, click(pos));
            f.plain(&doc, &mut state, vec![]);
            assert!(f.texts().contains(&e.full));
        }
    }
}
use session_header_support::replay_tools;
#[test]
fn header_parameters_keep_canonical_keys_required_labels_and_union_types() {
    let ordered = maestro_app::canonical_entries([("b", 0), ("2", 1), ("01", 2), ("1", 3)]);
    assert_eq!(ordered, [("1", 3), ("2", 1), ("b", 0), ("01", 2)]);
    for c in cases(|n| n == "canonical-properties" || n.starts_with("property-type-")) {
        replay_tools(&c);
    }
}
#[test]
fn header_tool_names_descriptions_and_order_stay_literal() {
    for c in cases(|n| {
        n.starts_with("tool-name-")
            || n.starts_with("tool-description-")
            || n == "literal-tool-fields"
    }) {
        replay_tools(&c);
    }
}
#[test]
fn header_tools_without_properties_have_no_expansion() {
    for c in cases(|n| {
        n.starts_with("tool-parameters-")
            || [
                "tools-null",
                "tools-empty",
                "tools-zero",
                "tools-false",
                "tools-object",
                "tools-empty-string",
                "unreached-required-without-properties",
            ]
            .contains(&n)
    }) {
        replay_tools(&c);
    }
}
#[test]
fn header_required_names_match_exact_strings() {
    for c in cases(|n| n.starts_with("required-")) {
        replay_tools(&c);
    }
}
#[test]
fn header_parameter_descriptions_follow_truth_without_trimming() {
    for c in cases(|n| n.starts_with("property-description-")) {
        replay_tools(&c);
    }
    let doc = document(
        &json!({"entries":[],"tools":[{"name":"plain", "description":"d", "parameters":{"description":"not properties"}}]}),
    );
    let mut f = Frame::default();
    f.plain(&doc, &mut HeaderState::default(), vec![]);
    assert!(!f.texts().iter().any(|s| s.contains("parameters]")));
}
#[test]
fn header_tool_domains_report_reached_failures() {
    for c in cases(|n| {
        [
            "tools-text",
            "tools-array",
            "null-schema-property",
            "numeric-required",
        ]
        .contains(&n)
    }) {
        let doc = prepared(&c);
        let error = Frame::default()
            .draw(
                &doc,
                &mut HeaderState::default(),
                vec![],
                &|_| unreachable!(),
            )
            .unwrap_err();
        assert_eq!(c.branch, "error");
        assert_eq!(c.error_kind.as_deref(), Some("invalid_data"));
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    for data in [
        json!({"entries":[],"tools":[{"parameters":{"properties":{"x":{}},"required":{}}}]}),
        json!({"entries":[],"tools":[{"parameters":{"properties":{"x":{"description":true}}}}]}),
    ] {
        let doc = document(&data);
        assert_eq!(
            Frame::default()
                .draw(
                    &doc,
                    &mut HeaderState::default(),
                    vec![],
                    &|_| unreachable!()
                )
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}
use maestro_web::export_html::template::HeaderAction;
#[test]
fn header_controls_emit_ordered_typed_actions() {
    let c = cases(|n| n == "empty").pop().unwrap();
    let e = expected(&c);
    let doc = prepared(&c);
    let mut f = Frame::default();
    let mut state = HeaderState::default();
    assert!(f.plain(&doc, &mut state, vec![]).is_empty());
    for (button, action) in e.buttons.iter().zip([
        HeaderAction::ToggleThinking,
        HeaderAction::ToggleTools,
        HeaderAction::DownloadSessionJson,
    ]) {
        let pos = f.at(&button.text);
        assert!(
            f.plain(&doc, &mut state, vec![egui::Event::PointerMoved(pos)])
                .is_empty()
        );
        f.time += 1.;
        f.plain(&doc, &mut state, vec![]);
        f.time += 1.;
        f.plain(&doc, &mut state, vec![]);
        assert!(
            f.texts().contains(&button.title),
            "tooltip {} {:?}",
            button.text,
            f.texts()
        );
        assert!(
            f.plain(
                &doc,
                &mut state,
                vec![session_header_support::pointer(pos, true)]
            )
            .is_empty()
        );
        assert_eq!(
            f.plain(
                &doc,
                &mut state,
                vec![session_header_support::pointer(pos, false)]
            ),
            [action]
        );
        assert!(f.plain(&doc, &mut state, vec![]).is_empty());
    }
    metadata(&f, &e);
}
/// Copy text through the toolkit's actual selection output.
fn copied(frame: &Frame) -> String {
    frame
        .output
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}
#[test]
fn header_expansion_is_independent_and_selection_safe() {
    let c = cases(|n| n == "expansions").pop().unwrap();
    assert_eq!(c.input.interaction, Some(true));
    let interactions = c.interactions.as_ref().unwrap();
    let doc = prepared(&c);
    let e = expected(&c);
    let mut f = Frame::default();
    let mut state = HeaderState::default();
    f.plain(&doc, &mut state, vec![]);
    let observe = |f: &Frame| {
        json!([
            f.texts().contains(&e.prompt.as_ref().unwrap().full),
            f.texts().contains(&"x".into()),
            f.texts().contains(&"y".into())
        ])
    };
    let hints = |f: &Frame| {
        json!(
            f.texts()
                .into_iter()
                .filter(|s| s.starts_with("[click") || s.starts_with("[hide"))
                .collect::<Vec<_>>()
        )
    };
    assert_eq!(observe(&f), interactions[0]);
    assert_eq!(hints(&f), interactions[1]);
    for (index, label) in ["System Prompt", "one", "two"].iter().enumerate() {
        let pos = f.at(label);
        f.plain(&doc, &mut state, click(pos));
        f.plain(&doc, &mut state, vec![]);
        assert_eq!(observe(&f), interactions[index + 2]);
        session_header_support::tool_texts(&f, &e.tools, &[index >= 1, index >= 2]);
    }
    assert_eq!(hints(&f), interactions[5]);
    f.plain(&doc, &mut state, vec![]);
    assert_eq!(observe(&f), interactions[6]);
    for label in ["System Prompt", "one"] {
        let pos = f.at(label);
        f.time += 1.;
        f.plain(&doc, &mut state, click(pos));
        f.plain(&doc, &mut state, vec![]);
    }
    assert_eq!(observe(&f), interactions[7]);
    selection();
}
/// Drag and double click on the public prompt label, not a fake selection flag.
fn selection() {
    use session_header_support::pointer;
    let prompt = format!(
        "select these words for the prompt\n{}",
        ["line"; 10].join("\n")
    );
    let doc = document(&json!({"entries": [], "systemPrompt": prompt}));
    let mut f = Frame::default();
    let mut state = HeaderState::default();
    f.plain(&doc, &mut state, vec![]);
    let rect = f
        .labels()
        .into_iter()
        .find(|(s, _)| s.starts_with("select these"))
        .unwrap()
        .1;
    let a = rect.left_top() + egui::vec2(3., 6.);
    let b = a + egui::vec2(85., 0.);
    f.plain(&doc, &mut state, click(a));
    f.plain(&doc, &mut state, vec![]);
    assert!(f.texts().contains(&prompt));
    f.time += 1.;
    f.plain(
        &doc,
        &mut state,
        vec![egui::Event::PointerMoved(a), pointer(a, true)],
    );
    f.plain(&doc, &mut state, vec![egui::Event::PointerMoved(b)]);
    f.plain(&doc, &mut state, vec![pointer(b, false)]);
    f.plain(&doc, &mut state, vec![egui::Event::Copy]);
    assert!(!copied(&f).is_empty());
    assert!(f.texts().contains(&prompt));
    f.time += 1.;
    f.plain(&doc, &mut state, click(a));
    f.plain(&doc, &mut state, vec![]);
    assert!(!f.texts().contains(&prompt));
    f.time += 1.;
    f.plain(&doc, &mut state, click(a));
    f.plain(&doc, &mut state, click(a));
    f.plain(&doc, &mut state, vec![egui::Event::Copy]);
    assert_eq!(copied(&f), "select");
    assert!(f.texts().contains(&prompt));
}
#[test]
fn header_scope_uses_shared_theme_and_preserves_caller_style() {
    session_header_support::theme::check();
}
