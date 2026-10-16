//! Recorded disclosure operands through complete header preparation.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisclosureCase {
    name: String,
    input: DisclosureInput,
    branch: String,
    output: Option<Value>,
    #[serde(rename = "errorKind")]
    error_kind: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisclosureInput {
    data: Value,
}
fn disclosure_cases() -> Vec<DisclosureCase> {
    serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/session_header_disclosures.json"
    ))
    .unwrap()
}
fn prompt_output(prompt: Option<&details::Prompt>) -> Value {
    match prompt {
        None => Value::Null,
        Some(details::Prompt::Full(full)) => {
            json!({"expandable":false,"heading":"System Prompt","preview":null,"hint":null,"full":full})
        }
        Some(details::Prompt::Preview {
            full,
            preview,
            hint,
        }) => {
            json!({"expandable":true,"heading":"System Prompt","preview":preview,"hint":hint,"full":full})
        }
    }
}
fn tool_output(tools: &[details::Tool]) -> Value {
    json!(tools.iter().map(|t| json!({"name":t.name,"description":t.description,"expandable":!t.parameters.is_empty(),"hint":if t.parameters.is_empty(){Value::Null}else{json!("[click to show parameters]")},"parameters":t.parameters.iter().map(|p|json!({"name":p.name,"type":p.kind,"required":p.required,"description":p.description})).collect::<Vec<_>>()})).collect::<Vec<_>>())
}

fn check_cases(names: &[&str]) {
    let cases = disclosure_cases();
    assert_eq!(cases.len(), 94);
    for name in names {
        let row = cases.iter().find(|r| r.name == *name).unwrap();
        let doc = document(&row.input.data);
        let result = prepare(&doc, &|_| unreachable!());
        if row.branch == "error" {
            assert!(row.output.is_none());
            assert_eq!(row.error_kind.as_deref(), Some("invalid_data"));
            let error = result.unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{}", row.name);
            let expected = match row.name.as_str() {
                "prompt-error-wins-over-tools-error" => Some("nonstring system prompt"),
                "required-error-wins-over-null-property" => Some("invalid required list"),
                "tool-name-validated-after-parameters" => Some("null property receiver"),
                _ => None,
            };
            if let Some(expected) = expected {
                assert_eq!(error.to_string(), expected);
            }
        } else {
            assert_eq!(row.branch, "success");
            assert!(row.error_kind.is_none());
            let p = result.unwrap();
            assert_eq!(
                json!({"prompt":prompt_output(p.prompt.as_ref()),"tools":tool_output(&p.tools)}),
                *row.output.as_ref().unwrap(),
                "{}",
                row.name
            );
            if row.name == "ignored-schema-fields" {
                assert_eq!(Value::Object(doc.data), row.input.data);
            }
        }
    }
}

#[test]
fn header_prompt_preserves_literal_text_and_blank_lines() {
    check_cases(&[
        "empty",
        "prompt-null",
        "prompt-empty",
        "prompt-zero",
        "prompt-false",
        "prompt-text",
        "prompt-number",
        "prompt-true",
        "prompt-object",
        "prompt-array",
    ]);
}

#[test]
fn header_prompt_previews_exactly_ten_split_lines() {
    check_cases(&[
        "prompt-lines-9",
        "prompt-lines-10",
        "prompt-lines-11",
        "prompt-lines-12",
        "prompt-trailing-newline",
        "prompt-crlf",
        "prompt-lone-cr",
        "prompt-other-separators",
    ]);
}

#[test]
fn header_tool_names_descriptions_and_order_stay_literal() {
    check_cases(&[
        "tool-name-missing",
        "tool-name-null",
        "tool-description-missing",
        "tool-description-null",
        "literal-tool-fields",
    ]);
}

#[test]
fn header_tools_without_properties_have_no_expansion() {
    check_cases(&[
        "tool-parameters-missing",
        "tool-parameters-null",
        "tool-parameters-string",
        "tool-parameters-number",
        "tool-parameters-false",
        "tool-parameters-object",
        "tool-parameters-array",
        "tool-parameters-no-properties",
        "tool-parameters-empty-properties",
        "tools-empty-string",
        "unreached-required-without-properties",
        "properties-missing",
        "properties-false",
        "properties-zero",
        "properties-number",
        "properties-true",
        "properties-empty",
        "properties-array-empty",
        "properties-array",
    ]);
}

#[test]
fn header_parameters_keep_canonical_keys_required_labels_and_union_types() {
    check_cases(&[
        "canonical-properties",
        "property-type-missing",
        "property-type-null",
        "property-type-empty",
        "property-type-zero",
        "property-type-false",
        "property-type-text",
        "property-type-array",
        "property-type-array-empty",
        "property-type-array-single",
        "boolean-schemas",
    ]);
}

#[test]
fn header_required_names_match_exact_strings() {
    check_cases(&[
        "required-undefined",
        "required-null",
        "required-0",
        "required-false",
        "required-\"\"",
        "required-[]",
        "required-[\"x\"]",
        "required-[\"xx\"]",
        "required-[1,\"other\"]",
        "empty-key-required",
        "required-exact-escaped",
        "array-required-nonstrings",
        "raw-key-identity",
    ]);
}

#[test]
fn header_parameter_descriptions_follow_truth_without_trimming() {
    check_cases(&[
        "property-description-missing",
        "property-description-null",
        "property-description-empty",
        "property-description-zero",
        "property-description-false",
        "property-description-text",
        "property-description-whitespace",
    ]);
}

#[test]
fn header_tool_domains_report_reached_failures() {
    check_cases(&[
        "expansions",
        "tools-null",
        "tools-empty",
        "tools-zero",
        "tools-false",
        "tools-text",
        "tools-object",
        "tools-array",
        "null-schema-property",
        "numeric-required",
        "normal-description-whitespace",
        "tools-positive-number",
        "tools-true",
        "tools-positive-length-object",
        "required-true",
        "required-object",
    ]);
}

#[test]
fn header_literal_nodes_match_parsed_text() {
    check_cases(&["literal-line-controls"]);
}

#[test]
fn header_preparation_preserves_error_order() {
    check_cases(&[
        "prompt-error-wins-over-tools-error",
        "required-error-wins-over-null-property",
        "tool-name-validated-after-parameters",
    ]);
}

#[test]
fn header_raw_disclosures_preserve_unknown_data() {
    check_cases(&["ignored-schema-fields"]);
}
