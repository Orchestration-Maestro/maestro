//! The model table observed through the public writer.

use crate::support;
use maestro_app::{
    AuthStorage, AuthStorageData, ModelCost, ModelFileOperations, ModelInput, ModelRegistry,
    ProviderModelInput,
};
use maestro_cli::input::list_models::{ListModelsOptions, list_models};
use maestro_theme::Collator;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

/// File effects serving one recorded models document or its recorded failure.
struct RecordedFile {
    /// The read result the case records.
    document: Result<String, String>,
}

impl ModelFileOperations for RecordedFile {
    fn exists(&self, path: &str) -> bool {
        !path.is_empty()
    }

    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        self.document.clone().map_err(std::io::Error::other)
    }
}

/// Read one recorded case's field.
fn field<'a>(case: &'a support::Recorded, name: &str) -> Option<&'a serde_json::Value> {
    case.input.get(name)
}

/// Build one dynamic model definition from a recorded descriptor.
fn definition(descriptor: &serde_json::Value) -> ProviderModelInput {
    ProviderModelInput {
        id: descriptor["id"]
            .as_str()
            .expect("recorded identifier")
            .to_owned(),
        name: descriptor["id"]
            .as_str()
            .expect("recorded identifier")
            .to_owned(),
        api: None,
        base_url: None,
        reasoning: descriptor["reasoning"]
            .as_bool()
            .expect("recorded reasoning"),
        thinking_level_map: None,
        input: descriptor["input"]
            .as_array()
            .expect("recorded input kinds")
            .iter()
            .map(|kind| match kind.as_str() {
                Some("image") => ModelInput::Image,
                _ => ModelInput::Text,
            })
            .collect(),
        cost: ModelCost::default(),
        context_window: descriptor["contextWindow"]
            .as_f64()
            .expect("recorded context"),
        max_tokens: descriptor["maxTokens"].as_f64().expect("recorded output"),
        headers: None,
        compat: None,
    }
}

/// Register every recorded provider with its models, keeping their authored order.
fn register(registry: &ModelRegistry, case: &support::Recorded) {
    let mut providers: Vec<(String, Vec<ProviderModelInput>)> = Vec::new();
    for descriptor in field(case, "models")
        .and_then(serde_json::Value::as_array)
        .expect("recorded models")
    {
        let provider = descriptor["provider"]
            .as_str()
            .expect("recorded provider")
            .to_owned();
        let definition = definition(descriptor);
        match providers.iter_mut().find(|(name, _)| *name == provider) {
            Some((_, definitions)) => definitions.push(definition),
            None => providers.push((provider, vec![definition])),
        }
    }
    for (provider, definitions) in providers {
        let options = Arc::new(RwLock::new(crate::support::startup::configuration(
            None,
            "recorded key",
            definitions,
        )));
        registry
            .register_provider(&provider, options)
            .expect("registered provider");
    }
}

/// Build the registry one case observes: its models and its recorded load failure.
fn registry(case: &support::Recorded) -> ModelRegistry {
    let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::new()));
    let commands = Arc::new(support::startup::NoCommands);
    let registry = match field(case, "error").and_then(serde_json::Value::as_str) {
        Some(message) => ModelRegistry::with_operations(
            auth,
            "models.json",
            RecordedFile {
                document: Err(message.to_owned()),
            },
            commands,
        )
        .expect("catalog with a recorded load failure"),
        None => ModelRegistry::in_memory(auth, commands).expect("offline catalog"),
    };
    register(&registry, case);
    registry
}

/// List one case and return its whole output and diagnostics.
fn listing(case: &support::Recorded) -> (String, String) {
    let locale = field(case, "locale")
        .and_then(serde_json::Value::as_str)
        .expect("recorded locale");
    let collator = Collator::new(&|key| (key == "LC_ALL").then(|| locale.to_owned()))
        .expect("prepared collator");
    let docs_path = field(case, "docs_path")
        .and_then(serde_json::Value::as_str)
        .expect("recorded documentation root");
    let read_docs = || Ok::<String, std::io::Error>(docs_path.to_owned());
    let options = ListModelsOptions {
        search_pattern: field(case, "search").and_then(serde_json::Value::as_str),
        docs_path: &read_docs,
        collator: &collator,
        styles_enabled: field(case, "styles_enabled")
            .and_then(serde_json::Value::as_bool)
            .expect("recorded style choice"),
    };
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    list_models(&registry(case), options, &mut output, &mut diagnostics)
        .expect("written model table");
    let output = String::from_utf8(output).expect("utf-8 table");
    // Windows joins the recorded POSIX documentation root natively; the recording uses `/`.
    let output = if cfg!(windows) {
        output.replace('\\', "/")
    } else {
        output
    };
    (
        output,
        String::from_utf8(diagnostics).expect("utf-8 diagnostics"),
    )
}

/// Compare one recorded case's complete table and diagnostics.
fn assert_case(title: &str) {
    let case = support::case_by_title("table", title);
    let (output, diagnostics) = listing(&case);
    assert_eq!(
        output,
        case.expected["stdout"].as_str().expect("recorded stdout"),
        "{}",
        case.case
    );
    assert_eq!(
        diagnostics,
        case.expected["stderr"].as_str().expect("recorded stderr"),
        "{}",
        case.case
    );
}

/// Compare every named recorded case.
fn assert_cases(titles: &[&str]) {
    for title in titles {
        assert_case(title);
    }
}

#[test]
fn model_listing_preserves_warning_empty_and_no_match_lines() {
    assert_cases(&["table-142", "table-143", "table-144", "table-145"]);
}

#[test]
fn model_guidance_resolves_documentation_only_for_empty_results() {
    assert_case("table-142");
    let case = support::case_by_title("table", "table-146");
    let reads = Rc::new(RefCell::new(0));
    let counter = Rc::clone(&reads);
    let failing_docs = move || {
        *counter.borrow_mut() += 1;
        Err::<String, std::io::Error>(std::io::Error::other("unread documentation"))
    };
    let collator = Collator::new(&|key| (key == "LC_ALL").then(|| "en-US".to_owned()))
        .expect("prepared collator");
    let options = ListModelsOptions {
        search_pattern: None,
        docs_path: &failing_docs,
        collator: &collator,
        styles_enabled: true,
    };
    let mut output = Vec::new();
    list_models(&registry(&case), options, &mut output, &mut Vec::new())
        .expect("listing without guidance");
    assert_eq!(*reads.borrow(), 0, "a nonempty listing reads no guidance");
    assert_eq!(
        String::from_utf8(output).expect("utf-8 table"),
        case.expected["stdout"].as_str().expect("recorded stdout")
    );
}

#[test]
fn model_search_reads_both_provider_and_identifier() {
    assert_cases(&["table-146", "table-152", "table-145"]);
}

#[test]
fn model_token_columns_keep_thresholds_and_rounding() {
    assert_cases(&[
        "table-149",
        "table-151",
        "table-154",
        "table-157",
        "table-158",
    ]);
}

#[test]
fn model_column_padding_keeps_its_code_unit_measure() {
    assert_cases(&["table-150"]);
}

#[test]
fn model_capability_columns_are_independent() {
    assert_cases(&["table-159", "table-149"]);
}

#[test]
fn model_table_orders_providers_and_identifiers_stably() {
    assert_cases(&[
        "table-147",
        "table-153",
        "table-155",
        "table-156",
        "table-160",
    ]);
}

#[test]
fn configured_locales_reach_the_consuming_table() {
    assert_cases(&[
        "table-197",
        "table-201",
        "table-205",
        "table-210",
        "table-211",
        "table-212",
        "table-213",
    ]);
}
