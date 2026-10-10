//! Cargo-compiled native source witnesses for boundary descriptors.
use super::{Case, Scratch};
use crate::Model;
use std::fmt::Write as _;

/// Generated boundary constructors compiled by the ordinary crate build.
mod boundary {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/catalog_boundary.rs"
    ));
}
/// Pair each selected boundary query with its typed input descriptor.
fn boundary_inputs() -> Vec<(String, Model)> {
    let cases: Vec<Case> = serde_json::from_str(include_str!("corpus.json")).unwrap();
    let mut records = Vec::new();
    for case in cases.into_iter().filter(|case| {
        matches!(
            case.test.as_str(),
            "emitted_strings_compile_without_changing_identity"
                | "emission_retains_optional_empty_and_ordered_fields"
                | "emitted_numbers_reconstruct_finite_values"
        )
    }) {
        let feeds: [Vec<Model>; 3] = serde_json::from_value(case.input).unwrap();
        for (index, model) in feeds.into_iter().flatten().enumerate() {
            records.push((format!("{}/{index}", case.id), model));
        }
    }
    let command: Model = serde_json::from_str(r#"{"id":"boundary/\"\\\n\u0000😀","name":"Name \"\\\n\u0000😀","api":"openai-completions","provider":"openrouter","baseUrl":"https://openrouter.ai/api/v1","reasoning":false,"input":["text","image"],"cost":{"input":1,"output":2,"cacheRead":0,"cacheWrite":0},"contextWindow":4096,"maxTokens":4096}"#).unwrap();
    records.push(("command-boundary".into(), command));
    let mut open = records.last().unwrap().1.clone();
    open.compat = Some(crate::ModelCompat(serde_json::from_str(r#"{"supportsStore":false,"sendSessionIdHeader":true,"openRouterRouting":{"unknown":{"values":["b","a","b"]}},"open":null,"precise":0.8455124082255701,"zero":-0.0}"#).unwrap()));
    records.push(("open-compatibility".into(), open));
    records
}
/// Emit a small constructor capsule, reusing identical zero-normalized expressions.
fn boundary_source(records: &[(String, Model)]) -> String {
    let mut source = String::from("// Generated typed boundary constructors.\n");
    let mut expressions = Vec::new();
    let mut table = String::from(
        "/// Controlled descriptor constructors.\nconst CASES: &[(&str, Constructor)] = &[\n",
    );
    for (key, model) in records {
        let expression = super::super::emit::model_expression(model);
        let index = expressions.iter().position(|known| known == &expression).unwrap_or_else(|| {
            let index = expressions.len();
            let _ = writeln!(source, "/// Construct a controlled descriptor.\nfn model_{index}() -> crate::Model {{\n{expression}\n}}");
            expressions.push(expression);
            index
        });
        writeln!(table, "({key:?}, model_{index}),").unwrap();
    }
    table.push_str("];\n");
    source.push_str("/// Construct one descriptor.\ntype Constructor = fn() -> crate::Model;\n");
    source.push_str(&table);
    source.push_str("/// Reconstruct the controlled descriptors through native expressions.\npub(super) fn cases() -> Vec<(&'static str, crate::Model)> {\nCASES.iter().map(|(key, constructor)| (*key, constructor())).collect()\n}\n");
    source
}
/// Assert every reconstructed field, ordered headers and normalized zero signs.
fn emission_cases(name: &str) {
    super::assembly_cases(name);
    let inputs = boundary_inputs();
    assert_eq!(
        boundary_source(&inputs),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/catalog_boundary.rs"
        ))
    );
    let compiled = boundary::cases();
    let cases: Vec<Case> = serde_json::from_str(include_str!("corpus.json")).unwrap();
    for case in cases.into_iter().filter(|case| case.test == name) {
        for (key, expected) in inputs
            .iter()
            .filter(|(key, _)| key.starts_with(&format!("{}/", case.id)))
        {
            let actual = &compiled.iter().find(|(id, _)| id == key).unwrap().1;
            if case.id == "number--0" {
                assert!(expected.cost.input.is_sign_negative());
            }
            assert_eq!(actual, expected, "{key}");
            assert_eq!(
                actual
                    .headers
                    .as_ref()
                    .map(|headers| headers.keys().collect::<Vec<_>>()),
                expected
                    .headers
                    .as_ref()
                    .map(|headers| headers.keys().collect::<Vec<_>>())
            );
            assert_positive_zeros(actual, key);
        }
    }
}
/// Verify numeric normalization separately from floating-point equality.
fn assert_positive_zeros(model: &Model, key: &str) {
    for number in [
        model.cost.input,
        model.cost.output,
        model.cost.cache_read,
        model.cost.cache_write,
        model.context_window,
        model.max_tokens,
    ] {
        assert!(
            number != 0.0 || !number.is_sign_negative(),
            "{key} zero sign"
        );
    }
}
#[test]
fn emitted_strings_compile_without_changing_identity() {
    emission_cases("emitted_strings_compile_without_changing_identity");
    assert_directory_local_filenames();
}
#[test]
fn emission_retains_optional_empty_and_ordered_fields() {
    emission_cases("emission_retains_optional_empty_and_ordered_fields");
}
#[test]
fn emitted_numbers_reconstruct_finite_values() {
    emission_cases("emitted_numbers_reconstruct_finite_values");
}
#[test]
fn complete_catalog_matches_all_descriptors() {
    super::assembly_cases("complete_catalog_matches_all_descriptors");
    let cases: Vec<Case> = serde_json::from_str(include_str!("corpus.json")).unwrap();
    let case = cases
        .into_iter()
        .find(|case| case.test == "complete_catalog_matches_all_descriptors")
        .unwrap();
    let catalog = super::super::assemble(serde_json::from_value(case.input).unwrap());
    let scratch = Scratch::new();
    super::super::emit::write_catalog(&catalog, &scratch.0.join("generated")).unwrap();
    let committed =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/catalog/models_generated");
    let actual_files = std::fs::read_dir(scratch.0.join("generated"))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(
        actual_files.len(),
        std::fs::read_dir(&committed).unwrap().count()
    );
    for file in actual_files {
        assert_eq!(
            std::fs::read(file.path()).unwrap(),
            std::fs::read(committed.join(file.file_name())).unwrap(),
            "{:?} emitted bytes",
            file.file_name()
        );
    }
    let expected = super::expected_models(serde_json::json!("offline_catalog"));
    let actual: Vec<_> = crate::get_providers()
        .iter()
        .flat_map(|provider| crate::get_models(provider))
        .collect();
    assert_eq!(actual, expected);
}

/// Check literal filename identities through actual filesystem publication.
fn assert_directory_local_filenames() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("corpus.json")).unwrap();
    let case = cases
        .into_iter()
        .find(|case| case.test == "emitted_strings_compile_without_changing_identity")
        .unwrap();
    let catalog = super::super::assemble(serde_json::from_value(case.input).unwrap());
    let scratch = Scratch::new();
    let destination = scratch.0.join("generated");
    super::super::emit::write_catalog(&catalog, &destination).unwrap();
    let registry = std::fs::read_to_string(destination.join("mod.rs")).unwrap();
    for filename in [
        "provider_.rs",
        "provider_con.rs",
        "provider_%43%4F%4E.rs",
        "provider_a%2Fb.rs",
        "provider_a%25b.rs",
        "provider_%41.rs",
        "provider_a.rs",
        "provider_%2E%2E%2Fescape.rs",
        "provider_provider%5F%0A.rs",
        "provider_provider%2F%2E%2E%2F%22%5C%0A%F0%9F%98%80.rs",
    ] {
        assert!(destination.join(filename).is_file(), "{filename}");
        assert!(
            registry.contains(&format!("#[path = {filename:?}]")),
            "{filename}"
        );
    }
    assert_eq!(
        std::fs::read_dir(&destination).unwrap().count(),
        catalog.len() + 1
    );
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 1);
}

#[test]
fn generated_open_compatibility_reconstructs_supplied_fields() {
    let inputs = boundary_inputs();
    let expected = &inputs
        .iter()
        .find(|(key, _)| key == "open-compatibility")
        .unwrap()
        .1;
    assert_eq!(
        boundary_source(&inputs),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/catalog_boundary.rs"
        ))
    );
    let compiled = boundary::cases();
    let actual = &compiled
        .iter()
        .find(|(key, _)| *key == "open-compatibility")
        .unwrap()
        .1;
    assert_eq!(actual, expected);
    let actual = &actual.compat.as_ref().unwrap().0;
    let expected = &expected.compat.as_ref().unwrap().0;
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        actual["zero"].as_f64().unwrap().to_bits(),
        (-0.0_f64).to_bits()
    );
}
