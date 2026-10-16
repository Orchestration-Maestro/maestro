//! Independently recorded stylesheet behavior.
mod fixtures;
mod records;
use super::{html_renderer::HtmlRenderer, utils::escape_class_name};
use fixtures::{Input, Pairs, cases};
use maestro_app::browser::docx_preview::{IDomStyle, StylesPart, parse_xml_string};
use maestro_app::markup::{EscapeMode, escape};
use serde_json::{Value, json};

/// Exercise every case assigned to one behavior.
fn check(name: &str) {
    let selected: Vec<_> = cases()
        .into_iter()
        .filter(|case| case.test == name)
        .collect();
    assert!(!selected.is_empty());
    for (index, case) in selected.into_iter().enumerate() {
        assert_eq!(case.outcome, "success");
        assert_eq!(
            observe(case.input),
            normalize(case.expected),
            "{name} case {index}"
        );
    }
}

/// Observe one operation through the renderer interface.
fn observe(input: Input) -> Value {
    match input {
        Input::Class { value } => json!(escape_class_name(&value)),
        Input::Prefix { value, prefix } => {
            json!(HtmlRenderer::new(prefix).process_style_name(value.as_deref()))
        }
        Input::Css {
            selectors,
            values,
            suffix,
        } => json!(HtmlRenderer::style_to_string(
            &selectors,
            &values.into_iter().collect(),
            suffix.as_deref()
        )),
        Input::Copy {
            input,
            output,
            attrs,
        } => observe_copy(input, output, attrs.as_deref()),
        Input::Styles { styles } => observe_styles(styles, "docx".into(), false),
        Input::Namespace { class_name, styles } => observe_styles(styles, class_name, true),
        Input::Inheritance {
            xml,
            ignore_width,
            lookups,
        } => observe_inheritance(parsed_styles(&xml, ignore_width), lookups),
        Input::Parsed { xml, ignore_width } => observe_parsed(parsed_styles(&xml, ignore_width)),
        Input::Default {
            hide_wrapper_on_print,
        } => json!(HtmlRenderer::new("docx".into()).render_default_style(hide_wrapper_on_print)),
        Input::Wrapper {
            class_name,
            children,
        } => {
            let children = children
                .iter()
                .map(|v| escape(v, EscapeMode::HtmlDecimal))
                .collect::<String>();
            json!(HtmlRenderer::new(class_name).render_wrapper(&children))
        }
        Input::WrapperHtml {
            class_name,
            children_html,
        } => json!(HtmlRenderer::new(class_name).render_wrapper(&children_html)),
    }
}

/// Observe optional-map presence and declaration insertion order.
fn observe_copy(input: Option<Pairs>, output: Option<Pairs>, attrs: Option<&[String]>) -> Value {
    let source = input.map(|v| v.into_iter().collect());
    let mut destination = output.map(|v| v.into_iter().collect());
    let refs = attrs.map(|a| a.iter().map(String::as_str).collect::<Vec<_>>());
    HtmlRenderer::copy_style_properties(source.as_ref(), &mut destination, refs.as_deref());
    json!(destination.map(|map| map.into_iter().collect::<Vec<_>>()))
}

/// Compare complete typed records, preserving declaration order and presence.
fn normalize(mut value: Value) -> Value {
    if let Some(styles) = value.get_mut("styles") {
        *styles = json!(
            styles
                .as_array()
                .unwrap()
                .iter()
                .map(|s| {
                    let (record, name) = records::style(s.clone());
                    json!({"record":format!("{record:?}"), "cssName":name.unwrap()})
                })
                .collect::<Vec<_>>()
        );
    }
    value
}

/// Prepare retained occurrences and observe ordered CSS and constructor forwarding.
fn observe_styles(styles: Vec<Value>, namespace: String, include_namespace: bool) -> Value {
    let mut renderer = HtmlRenderer::new(namespace);
    renderer.process_styles(
        styles
            .into_iter()
            .map(|s| {
                let (record, name) = records::style(s);
                assert!(name.is_none());
                record
            })
            .collect(),
    );
    let mut result = stylesheet(&renderer);
    if include_namespace {
        result["defaultCss"] = json!(renderer.render_default_style(false));
        result["wrapper"] = json!(renderer.render_wrapper(""));
    }
    result
}

/// Observe the same owned occurrences consumed by stylesheet emission.
fn observe_records(renderer: &HtmlRenderer) -> Value {
    json!(renderer.styles.iter().map(|style| json!({"record":format!("{style:?}"), "cssName":renderer.process_style_name(style.id.as_deref())})).collect::<Vec<_>>())
}

/// Keep exact raw bytes separate from the recorded DOM newline projection.
fn stylesheet(renderer: &HtmlRenderer) -> Value {
    let raw_css = renderer.render_styles();
    json!({"styles":observe_records(renderer), "domCss": raw_css.replace("\r\n", "\n"), "rawCss":raw_css})
}

/// Release both input and borrowed XML before returning owned factory output.
fn parsed_styles(input: &str, ignore_width: bool) -> Vec<IDomStyle> {
    let xml = parse_xml_string(input, false).unwrap();
    StylesPart::parse_xml(&xml, xml.root(), ignore_width)
        .unwrap()
        .styles
}

/// Observe inherited records, CSS and exact retained lookup identity.
fn observe_inheritance(definitions: Vec<IDomStyle>, lookups: Vec<Option<String>>) -> Value {
    let mut renderer = HtmlRenderer::new("docx".into());
    renderer.process_styles(definitions);
    let mut lookup = serde_json::Map::new();
    for name in lookups {
        let found = renderer.find_style(name.as_deref());
        if let Some(found) = found {
            let index = renderer.styles.iter().rposition(|s| s.id == name).unwrap();
            assert!(std::ptr::eq(
                std::ptr::from_ref(found),
                std::ptr::from_ref(&renderer.styles[index])
            ));
        }
        lookup.insert(
            name.as_deref().unwrap_or("null").into(),
            json!(found.and_then(|s| s.id.as_deref())),
        );
    }
    json!({"styles":observe_records(&renderer), "css": renderer.render_styles().replace("\r\n", "\n"), "lookup":lookup})
}

/// Match the parsed-document artifact and replace its retained state with an empty document.
fn observe_parsed(definitions: Vec<IDomStyle>) -> Value {
    let mut renderer = HtmlRenderer::new("docx".into());
    renderer.process_styles(definitions);
    let result = stylesheet(&renderer);
    let body = renderer.find_style(Some("Body.Text")).unwrap();
    assert_eq!(
        body.styles[0].values.get("text-align"),
        Some(&Some("center".into()))
    );
    assert_eq!(
        body.run_props.as_ref().unwrap().font_size,
        Some(Some("12.00pt".into()))
    );
    assert!(renderer.find_style(Some("Strong")).is_some());
    let empty = cases()
        .into_iter()
        .find_map(|case| match case.input {
            Input::Inheritance {
                xml, ignore_width, ..
            } if case.test == "idless_and_empty_documents_keep_default_style_output"
                && case.expected["styles"] == json!([]) =>
            {
                Some(parsed_styles(&xml, ignore_width))
            }
            _ => None,
        })
        .unwrap();
    renderer.process_styles(empty);
    assert!(renderer.find_style(Some("Body.Text")).is_none());
    assert!(renderer.find_style(Some("Strong")).is_none());
    assert!(renderer.render_styles().is_empty());
    result
}
#[test]
fn class_names_coalesce_only_literal_runs() {
    check("class_names_coalesce_only_literal_runs");
}

#[test]
fn class_prefix_keeps_null_empty_and_namespace() {
    check("class_prefix_keeps_null_empty_and_namespace");
}

#[test]
fn css_text_preserves_order_nullable_values_and_crlf() {
    check("css_text_preserves_order_nullable_values_and_crlf");
}

#[test]
fn css_text_omits_attribute_keys_and_appends_literal_suffix() {
    check("css_text_omits_attribute_keys_and_appends_literal_suffix");
}

#[test]
fn declarations_fill_only_absent_destination_keys() {
    check("declarations_fill_only_absent_destination_keys");
}

#[test]
fn missing_declaration_sources_keep_destination_absence() {
    check("missing_declaration_sources_keep_destination_absence");
}

#[test]
fn declaration_filters_keep_requested_order_and_skip_absent_keys() {
    check("declaration_filters_keep_requested_order_and_skip_absent_keys");
}

#[test]
fn linked_styles_follow_local_definitions_without_own_inheritance() {
    check("linked_styles_follow_local_definitions_without_own_inheritance");
}

#[test]
fn default_styles_choose_last_truthy_target_without_deduplication() {
    check("default_styles_choose_last_truthy_target_without_deduplication");
}

#[test]
fn style_selectors_keep_target_descendants_and_conditional_text() {
    check("style_selectors_keep_target_descendants_and_conditional_text");
}

#[test]
fn idless_and_empty_documents_keep_default_style_output() {
    check("idless_and_empty_documents_keep_default_style_output");
}

#[test]
fn style_inheritance_keeps_child_overrides() {
    check("style_inheritance_keeps_child_overrides");
}

#[test]
fn absent_metadata_groups_inherit_owned_values() {
    check("absent_metadata_groups_inherit_owned_values");
}

#[test]
fn style_metadata_inheritance_covers_every_producer_field() {
    check("style_metadata_inheritance_covers_every_producer_field");
}

#[test]
fn nested_metadata_retains_present_null_false_and_empty() {
    check("nested_metadata_retains_present_null_false_and_empty");
}

#[test]
fn forward_chains_keep_transitive_declarations() {
    check("forward_chains_keep_transitive_declarations");
}

#[test]
fn sibling_styles_resolve_from_unchanged_definitions() {
    check("sibling_styles_resolve_from_unchanged_definitions");
}

#[test]
fn cyclic_chains_keep_explicit_properties_until_revisit() {
    check("cyclic_chains_keep_explicit_properties_until_revisit");
}

#[test]
fn inherited_substyles_fill_first_target_without_changing_modifiers() {
    check("inherited_substyles_fill_first_target_without_changing_modifiers");
}

#[test]
fn duplicate_style_ids_use_last_lookup_keep_emission_order() {
    check("duplicate_style_ids_use_last_lookup_keep_emission_order");
}

#[test]
fn unresolved_and_empty_bases_leave_local_values() {
    check("unresolved_and_empty_bases_leave_local_values");
}

#[test]
fn style_lookup_distinguishes_absent_empty_and_literal_ids() {
    check("style_lookup_distinguishes_absent_empty_and_literal_ids");
}

#[test]
fn linked_styles_emit_resolved_bases_but_do_not_merge_metadata() {
    check("linked_styles_emit_resolved_bases_but_do_not_merge_metadata");
}

#[test]
fn default_stylesheet_retains_print_wrapper_and_document_rules() {
    check("default_stylesheet_retains_print_wrapper_and_document_rules");
}

#[test]
fn wrapper_markup_keeps_text_order_and_literal_class_attribute() {
    check("wrapper_markup_keeps_text_order_and_literal_class_attribute");
}

#[test]
fn renderer_namespace_reaches_styles_defaults_and_wrapper() {
    check("renderer_namespace_reaches_styles_defaults_and_wrapper");
}

#[test]
fn parsed_document_stylesheet_matches_recorded_artifact() {
    check("parsed_document_stylesheet_matches_recorded_artifact");
}
