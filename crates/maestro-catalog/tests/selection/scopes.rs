//! Recorded scope, initial-choice and restoration operations through the public catalog.
use crate::descriptors::descriptors;
use indexmap::IndexMap;
use maestro_catalog::{
    DEFAULT_MODEL_PER_PROVIDER, FindInitialModelOptions, ModelFileOperations, ModelRegistry,
    ModelResolverOutput, RestoreModelFromSessionOptions, ScopedModel, find_initial_model,
    model_id_comparator, resolve_model_scope, restore_model_from_session,
};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::{Model, ModelThinkingLevel, SharedHandle};
use serde::Deserialize;
use std::{
    cell::RefCell,
    io::Write,
    rc::Rc,
    sync::{Arc, RwLock},
};
/// Shared descriptor identity.
pub type Handle = Arc<RwLock<Model>>;
/// Strict recorded operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    op: String,
    #[serde(deserialize_with = "descriptors")]
    models: Vec<Model>,
    #[serde(default)]
    auth: Vec<String>,
    #[serde(default)]
    patterns: Vec<String>,
    color: Option<bool>,
    configured_keys: Option<IndexMap<String, String>>,
    options: Option<Options>,
    defaults: Option<IndexMap<String, String>>,
    saved_provider: Option<String>,
    saved_model_id: Option<String>,
    print: Option<bool>,
    current: Option<serde_json::Value>,
}
/// Strict initial-choice operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Options {
    scoped: Vec<ScopedInput>,
    is_continuing: bool,
    cli_provider: Option<String>,
    cli_model: Option<String>,
    default_provider: Option<String>,
    default_model_id: Option<String>,
    default_thinking_level: Option<ModelThinkingLevel>,
}
/// A scoped entry named by its catalog position.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ScopedInput {
    index: usize,
    thinking_level: Option<ModelThinkingLevel>,
}
/// One recorded scope result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Picked {
    #[serde(deserialize_with = "descriptor")]
    model: Model,
    model_index: usize,
    thinking_level: Option<ModelThinkingLevel>,
}
/// Strict recorded results; each operation reads only its own fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    #[serde(default)]
    selected: Vec<Picked>,
    #[serde(default, deserialize_with = "optional_descriptor")]
    model: Option<Model>,
    #[serde(default)]
    model_index: Option<usize>,
    #[serde(default)]
    thinking_level: Option<ModelThinkingLevel>,
    #[serde(default)]
    fallback_message: Option<String>,
    #[serde(default)]
    current_identity: Option<bool>,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
    #[serde(default)]
    stream_order: Vec<String>,
    error: Option<String>,
}
/// One recorded operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    input: Input,
    expected: Expected,
}
/// Decode one strict descriptor.
fn descriptor<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Model, D::Error> {
    use serde::de::IntoDeserializer;
    let value = serde_json::Value::deserialize(decoder)?;
    descriptors(serde_json::Value::Array(vec![value]).into_deserializer())
        .map_err(serde::de::Error::custom)
        .map(|mut models| models.remove(0))
}
/// Decode one strict descriptor or null.
fn optional_descriptor<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<Model>, D::Error> {
    Option::<serde_json::Value>::deserialize(decoder)?
        .map(|value| descriptor(value).map_err(serde::de::Error::custom))
        .transpose()
}
/// Recorded data, parsed once per process.
fn cases() -> &'static [Case] {
    static CASES: std::sync::LazyLock<Vec<Case>> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("scope-fixtures.json")).unwrap()
    });
    &CASES
}
/// A controlled descriptor derived from the recorded template.
pub fn model(provider: &str, id: &str) -> Handle {
    let mut model = cases()[0].input.models[0].clone();
    model.provider = provider.into();
    model.id = id.into();
    model.name = id.into();
    Arc::new(RwLock::new(model))
}
/// A writer whose every write fails, counting the attempts.
#[derive(Default)]
pub struct Broken(pub usize);
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        self.0 += 1;
        Err(std::io::Error::other("closed"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Catalog file text served from memory.
struct Json(String);
impl ModelFileOperations for Json {
    fn exists(&self, _: &str) -> bool {
        true
    }
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        Ok(self.0.clone())
    }
}
/// A real registry over the shared handles with presence-only credentials.
pub fn registry(handles: &[Handle], authenticated: &[String]) -> ModelRegistry {
    let storage = SharedHandle::new(AuthStorage::in_memory(AuthStorageData::new()));
    for provider in authenticated {
        storage.set_runtime_api_key(provider, "present");
    }
    let registry = ModelRegistry::in_memory(storage, crate::support::values()).unwrap();
    *registry.get_all().write().unwrap() = handles.to_vec();
    registry
}
/// Registry whose providers carry configured key commands that must never run.
fn registry_with_commands(
    handles: &[Handle],
    commands: &IndexMap<String, String>,
    marker: &std::path::Path,
) -> ModelRegistry {
    let providers: serde_json::Map<_, _> = commands
        .iter()
        .map(|(provider, label)| {
            assert_eq!(label, "!never-run-this");
            let key = format!("!touch {}", marker.display());
            let body = serde_json::json!({
                "api": "openai-completions", "baseUrl": "https://example.invalid",
                "apiKey": key, "models": [{"id": "seed"}]
            });
            (provider.clone(), body)
        })
        .collect();
    let text = serde_json::json!({ "providers": providers }).to_string();
    let storage = SharedHandle::new(AuthStorage::in_memory(AuthStorageData::new()));
    let registry = ModelRegistry::with_operations(
        storage,
        "<models>",
        Json(text),
        crate::support::forbidden(),
    )
    .unwrap();
    *registry.get_all().write().unwrap() = handles.to_vec();
    registry
}
/// Captured stream bytes and the order of the transitions between streams.
#[derive(Default)]
pub struct Captured {
    stdout: Rc<RefCell<Vec<u8>>>,
    stderr: Rc<RefCell<Vec<u8>>>,
    order: Rc<RefCell<Vec<&'static str>>>,
}
/// A writer feeding a shared capture.
pub struct Tap {
    name: &'static str,
    bytes: Rc<RefCell<Vec<u8>>>,
    order: Rc<RefCell<Vec<&'static str>>>,
}
impl Write for Tap {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if !buffer.is_empty() && self.order.borrow().last() != Some(&self.name) {
            self.order.borrow_mut().push(self.name);
        }
        self.bytes.borrow_mut().extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Captured {
    /// Writers for the output record.
    pub fn taps(&self) -> (Tap, Tap) {
        let tap = |name, bytes: &Rc<RefCell<Vec<u8>>>| Tap {
            name,
            bytes: Rc::clone(bytes),
            order: Rc::clone(&self.order),
        };
        (tap("stdout", &self.stdout), tap("stderr", &self.stderr))
    }
    /// Captured text of one stream.
    fn text(bytes: &Rc<RefCell<Vec<u8>>>) -> String {
        String::from_utf8(bytes.borrow().clone()).unwrap()
    }
}
/// Observed operation result.
struct Observed {
    model: Option<Handle>,
    thinking: Option<ModelThinkingLevel>,
    fallback: Option<String>,
    error: Option<String>,
    current: Option<Handle>,
}
/// Execute the named recorded operations and compare every recorded field.
pub fn replay(ids: &[&str]) {
    for id in ids {
        let case = cases().iter().find(|c| c.id == *id).unwrap();
        if let Some(defaults) = &case.input.defaults {
            let mut table = DEFAULT_MODEL_PER_PROVIDER.write().unwrap();
            table.clear();
            table.extend(defaults.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        let handles: Vec<Handle> = case
            .input
            .models
            .iter()
            .cloned()
            .map(|m| Arc::new(RwLock::new(m)))
            .collect();
        let captured = Captured::default();
        let mut observed = Observed {
            model: None,
            thinking: None,
            fallback: None,
            error: None,
            current: None,
        };
        let picked = run(case, &handles, &captured, &mut observed);
        compare(case, &handles, &captured, &observed, &picked);
    }
}
/// Dispatch on the recorded operation, returning scope picks.
fn run(
    case: &Case,
    handles: &[Handle],
    captured: &Captured,
    observed: &mut Observed,
) -> Vec<(Handle, Option<ModelThinkingLevel>)> {
    let input = &case.input;
    let marker = std::env::temp_dir().join(format!(
        "maestro-catalog-marker-{}-{}",
        std::process::id(),
        case.id
    ));
    let registry = match &input.configured_keys {
        Some(commands) => registry_with_commands(handles, commands, &marker),
        None => registry(handles, &input.auth),
    };
    let compare = model_id_comparator(&|_| None).unwrap();
    let (stdout, stderr) = captured.taps();
    let (mut stdout, mut stderr) = (stdout, stderr);
    let mut output = ModelResolverOutput {
        stdout: &mut stdout,
        stderr: &mut stderr,
        color: input.color.unwrap_or(false),
    };
    let mut picked = Vec::new();
    match input.op.as_str() {
        "scope" => {
            let scoped = resolve_model_scope(&input.patterns, &registry, &compare, &mut output);
            match scoped {
                Ok(scoped) => {
                    picked = scoped
                        .into_iter()
                        .map(|s| (s.model, s.thinking_level))
                        .collect();
                }
                Err(error) => observed.error = Some(error.to_string()),
            }
        }
        "initial" => initial(input, handles, &registry, &compare, observed),
        "restore" => restore(input, handles, &registry, &mut output, observed),
        _ => panic!("unknown recorded operation"),
    }
    assert!(!marker.exists(), "configured key command ran: {}", case.id);
    picked
}
/// Run the initial choice with recorded operands.
fn initial(
    input: &Input,
    handles: &[Handle],
    registry: &ModelRegistry,
    compare: &dyn Fn(&str, &str) -> std::cmp::Ordering,
    observed: &mut Observed,
) {
    let options = input.options.as_ref().unwrap();
    let scoped: Vec<_> = options
        .scoped
        .iter()
        .map(|s| ScopedModel {
            model: Arc::clone(&handles[s.index]),
            thinking_level: s.thinking_level.clone(),
        })
        .collect();
    let result = find_initial_model(
        FindInitialModelOptions {
            cli_provider: options.cli_provider.as_deref(),
            cli_model: options.cli_model.as_deref(),
            scoped_models: &scoped,
            is_continuing: options.is_continuing,
            default_provider: options.default_provider.as_deref(),
            default_model_id: options.default_model_id.as_deref(),
            default_thinking_level: options.default_thinking_level.clone(),
            model_registry: registry,
        },
        compare,
    );
    match result {
        Ok(result) => {
            observed.model = result.model;
            observed.thinking = Some(result.thinking_level);
            observed.fallback = result.fallback_message;
        }
        Err(error) => observed.error = Some(error.to_string()),
    }
}
/// Run a restoration with recorded operands.
fn restore(
    input: &Input,
    handles: &[Handle],
    registry: &ModelRegistry,
    output: &mut ModelResolverOutput<'_>,
    observed: &mut Observed,
) {
    observed.current = input.current.as_ref().map(|value| match value.as_u64() {
        Some(index) => Arc::clone(&handles[usize::try_from(index).unwrap()]),
        None => Arc::new(RwLock::new(descriptor(value).unwrap())),
    });
    let result = restore_model_from_session(
        RestoreModelFromSessionOptions {
            saved_provider: input.saved_provider.as_deref().unwrap(),
            saved_model_id: input.saved_model_id.as_deref().unwrap(),
            current_model: observed.current.as_ref(),
            should_print_messages: input.print.unwrap(),
            model_registry: registry,
        },
        output,
    )
    .unwrap();
    observed.model = result.model;
    observed.fallback = result.fallback_message;
}
/// Compare a descriptor and its position in the input list.
fn same_model(
    actual: &Handle,
    expected: &Model,
    index: Option<usize>,
    handles: &[Handle],
    id: &str,
) {
    let actual_value = serde_json::to_value(&*actual.read().unwrap()).unwrap();
    assert!(
        actual_value == serde_json::to_value(expected).unwrap(),
        "descriptor {id}"
    );
    let found = handles.iter().position(|h| Arc::ptr_eq(h, actual));
    assert_eq!(found, index, "identity {id}");
}
/// Compare each field the recorded operation owns.
fn compare(
    case: &Case,
    handles: &[Handle],
    captured: &Captured,
    observed: &Observed,
    picked: &[(Handle, Option<ModelThinkingLevel>)],
) {
    let (id, expected) = (case.id.as_str(), &case.expected);
    assert_eq!(observed.error, expected.error, "error {id}");
    if case.input.op == "scope" {
        assert_eq!(picked.len(), expected.selected.len(), "count {id}");
        for ((handle, level), want) in picked.iter().zip(&expected.selected) {
            same_model(handle, &want.model, Some(want.model_index), handles, id);
            assert_eq!(level, &want.thinking_level, "level {id}");
        }
    } else {
        compare_choice(case, handles, captured, observed);
    }
    assert_eq!(
        Captured::text(&captured.stdout),
        expected.stdout,
        "stdout {id}"
    );
    assert_eq!(
        Captured::text(&captured.stderr),
        expected.stderr,
        "stderr {id}"
    );
}
/// Compare an initial or restored choice and, for restoration, its provenance and stream order.
fn compare_choice(case: &Case, handles: &[Handle], captured: &Captured, observed: &Observed) {
    let (id, expected) = (case.id.as_str(), &case.expected);
    match (&observed.model, &expected.model) {
        (Some(actual), Some(want)) => {
            same_model(actual, want, expected.model_index, handles, id);
        }
        (None, None) => assert_eq!(expected.model_index, None, "index {id}"),
        _ => panic!("model presence {id}"),
    }
    assert_eq!(observed.thinking, expected.thinking_level, "thinking {id}");
    assert_eq!(
        observed.fallback, expected.fallback_message,
        "fallback {id}"
    );
    if case.input.op == "restore" {
        let same =
            matches!((&observed.model, &observed.current), (Some(a), Some(b)) if Arc::ptr_eq(a, b));
        assert_eq!(Some(same), expected.current_identity, "current {id}");
        assert_eq!(
            *captured.order.borrow(),
            expected.stream_order,
            "order {id}"
        );
    }
}
