//! Numbering output at the retained renderer boundary.
use super::HtmlRenderer;
use super::elements_fixture::{Input, cases, number, numberings, string};
use serde_json::json;

/// Replay each complete source-derived observation assigned to one behavior.
fn check(name: &str) {
    let cases = cases(name);
    assert!(!cases.is_empty());
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(case.outcome, "success");
        let observed = match case.input {
            Input::NumberingClass { args, namespace }
            | Input::NumberingCounter { args, namespace } => {
                let renderer = HtmlRenderer::new(namespace);
                let id = string(&args[0]);
                let level = number(&args[1]);
                let class = renderer.numbering_class(id, level);
                assert_eq!(class, renderer.numbering_counter(id, level));
                json!(class)
            }
            Input::Format { args } => {
                let value = HtmlRenderer::num_format_to_css_value(string(&args[0]));
                match value {
                    maestro_app::Presence::Missing => json!({"absent":true}),
                    maestro_app::Presence::Null => json!(null),
                    maestro_app::Presence::Present(value) => json!(value),
                }
            }
            Input::Content { args, namespace } => {
                json!(HtmlRenderer::new(namespace).level_text_to_content(
                    args[0].as_str().unwrap(),
                    args[1].as_str(),
                    string(&args[2]),
                    string(&args[3])
                ))
            }
            Input::Numbering {
                levels,
                namespace,
                root_selector,
            } => {
                let mut renderer = HtmlRenderer::new(namespace.unwrap_or_else(|| "docx".into()));
                let numberings = numberings(&levels, None);
                renderer.prodess_numberings(&numberings);
                json!({"css":renderer.render_numbering(&numberings, root_selector.as_deref().unwrap_or(".docx-wrapper")).replace("\r\n", "\n")})
            }
            Input::Picture { levels, picture_id } => {
                json!({"css":HtmlRenderer::new("docx".into()).render_numbering(&numberings(&levels, Some(&picture_id)), ".docx-wrapper").replace("\r\n", "\n")})
            }
            #[cfg(target_arch = "wasm32")]
            _ => unreachable!(),
        };
        assert_eq!(observed, case.expected, "{name} case {index}");
    }
}

/// Numbering identifiers preserve namespace and presence.
#[test]
fn numbering_identifiers_preserve_namespace_and_presence() {
    check("numbering_identifiers_preserve_namespace_and_presence");
}

/// Numbering formats keep every named css mapping.
#[test]
fn numbering_formats_keep_every_named_css_mapping() {
    check("numbering_formats_keep_every_named_css_mapping");
}

/// Unknown numbering formats keep the authored spelling.
#[test]
fn unknown_numbering_formats_keep_the_authored_spelling() {
    check("unknown_numbering_formats_keep_the_authored_spelling");
}

/// Level text keeps placeholder and suffix rules.
#[test]
fn level_text_keeps_placeholder_and_suffix_rules() {
    check("level_text_keeps_placeholder_and_suffix_rules");
}

/// Level text literals do not become css or phantom counters.
#[test]
fn level_text_literals_do_not_become_css_or_phantom_counters() {
    check("level_text_literals_do_not_become_css_or_phantom_counters");
}

/// Numbering css keeps counter levels resets and style order.
#[test]
fn numbering_css_keeps_counter_levels_resets_and_style_order() {
    check("numbering_css_keeps_counter_levels_resets_and_style_order");
}

/// Picture numbering prepares css without owning resource settlement.
#[test]
fn picture_numbering_prepares_css_without_owning_resource_settlement() {
    check("picture_numbering_prepares_css_without_owning_resource_settlement");
}
