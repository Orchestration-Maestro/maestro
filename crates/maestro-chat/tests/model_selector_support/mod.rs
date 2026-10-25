//! Strict model-picker corpus and controlled owner operations.
use crate::bindings::Binding;
use maestro_app::{
    AuthStorage, ConfigValueOperations, ConfigValues, Model, ModelFileOperations, ModelRegistry,
    ProviderConfigInput, ProviderModelInput, SharedHandle,
};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Configured-value commands that selector fixtures never execute.
pub struct UnusedCommands;

impl ConfigValueOperations for UnusedCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}

/// Credential command handle for selector fixtures.
pub fn no_commands() -> ConfigValues {
    SharedHandle::new(UnusedCommands)
}

/// Every accepted environment authentication route, excluded from each child.
#[cfg(not(target_arch = "wasm32"))]
pub const AUTH_ENVIRONMENT: &[(&str, Option<&str>)] = &[
    ("COPILOT_GITHUB_TOKEN", None),
    ("GH_TOKEN", None),
    ("GITHUB_TOKEN", None),
    ("ANTHROPIC_OAUTH_TOKEN", None),
    ("ANTHROPIC_API_KEY", None),
    ("OPENAI_API_KEY", None),
    ("AZURE_OPENAI_API_KEY", None),
    ("DEEPSEEK_API_KEY", None),
    ("GEMINI_API_KEY", None),
    ("GOOGLE_CLOUD_API_KEY", None),
    ("GROQ_API_KEY", None),
    ("CEREBRAS_API_KEY", None),
    ("XAI_API_KEY", None),
    ("OPENROUTER_API_KEY", None),
    ("AI_GATEWAY_API_KEY", None),
    ("ZAI_API_KEY", None),
    ("MISTRAL_API_KEY", None),
    ("MINIMAX_API_KEY", None),
    ("MINIMAX_CN_API_KEY", None),
    ("MOONSHOT_API_KEY", None),
    ("HF_TOKEN", None),
    ("FIREWORKS_API_KEY", None),
    ("OPENCODE_API_KEY", None),
    ("KIMI_API_KEY", None),
    ("CLOUDFLARE_API_KEY", None),
    ("XIAOMI_API_KEY", None),
    ("XIAOMI_TOKEN_PLAN_CN_API_KEY", None),
    ("XIAOMI_TOKEN_PLAN_AMS_API_KEY", None),
    ("XIAOMI_TOKEN_PLAN_SGP_API_KEY", None),
    ("GOOGLE_CLOUD_PROJECT", None),
    ("GCLOUD_PROJECT", None),
    ("GOOGLE_CLOUD_LOCATION", None),
    ("AWS_PROFILE", None),
    ("AWS_ACCESS_KEY_ID", None),
    ("AWS_SECRET_ACCESS_KEY", None),
    ("AWS_BEARER_TOKEN_BEDROCK", None),
    ("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", None),
    ("AWS_CONTAINER_CREDENTIALS_FULL_URI", None),
    ("AWS_WEB_IDENTITY_TOKEN_FILE", None),
];

/// One minimal model observation.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    /// Provider identity.
    pub provider: String,
    /// Model identity.
    pub id: String,
    /// Display name.
    pub name: String,
}
impl Descriptor {
    /// Builds a full record using the existing owner's default metadata.
    pub fn model(&self) -> Model {
        let mut model = maestro_app::ModelRegistry::in_memory(
            SharedHandle::new(AuthStorage::in_memory(std::iter::empty().collect())),
            no_commands(),
        )
        .unwrap()
        .get_all()
        .read()
        .unwrap()[0]
            .read()
            .unwrap()
            .clone();
        model.provider.clone_from(&self.provider);
        model.id.clone_from(&self.id);
        model.name.clone_from(&self.name);
        model
    }
    /// Builds an owned shared descriptor.
    pub fn handle(&self) -> Arc<RwLock<Model>> {
        Arc::new(RwLock::new(self.model()))
    }
    /// Converts only the fields required by provider registration.
    pub fn definition(&self) -> ProviderModelInput {
        let model = self.model();
        ProviderModelInput {
            id: self.id.clone(),
            name: self.name.clone(),
            api: Some(model.api),
            base_url: Some(model.base_url),
            reasoning: model.reasoning,
            thinking_level_map: model.thinking_level_map,
            input: model.input,
            cost: model.cost,
            context_window: model.context_window,
            max_tokens: model.max_tokens,
            headers: None,
            compat: model.compat,
        }
    }
}
/// Standard recorded input order.
fn basic() -> Vec<Descriptor> {
    [
        ("zeta", "z", "Zed"),
        ("alpha", "a2", "Two"),
        ("alpha", "a1", "One"),
    ]
    .into_iter()
    .map(|(provider, id, name)| Descriptor {
        provider: provider.into(),
        id: id.into(),
        name: name.into(),
    })
    .collect()
}
/// Controlled read failure.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadFailure {
    /// Native error text.
    pub message: String,
    /// Authored file spelling.
    pub path: String,
}
/// Complete constructor query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Query {
    /// Available descriptors in source order.
    #[serde(default = "basic")]
    pub available: Vec<Descriptor>,
    /// Ordered scoped records.
    #[serde(default)]
    pub scoped: Vec<Descriptor>,
    /// Current descriptor, independent unless explicitly shared.
    pub current: Option<Descriptor>,
    /// Alias the first scoped fallback descriptor.
    #[serde(default)]
    pub current_shared: bool,
    /// Initial query.
    pub initial: Option<String>,
    /// Initial binding overrides.
    #[serde(default)]
    pub bindings: BTreeMap<String, Binding>,
    /// Synchronous selection reentry.
    #[serde(default)]
    pub reenter: bool,
    /// Subsequent operations.
    #[serde(default)]
    pub ops: Vec<Operation>,
    /// Render width.
    #[serde(default = "width")]
    pub width: usize,
    /// Controlled diagnostic source.
    pub read_failure: Option<ReadFailure>,
}
/// Ordinary frame width.
fn width() -> usize {
    120
}
/// Native model edit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mutation {
    /// Original descriptor position.
    pub index: usize,
    /// Edits to metadata.
    pub changes: Changes,
}
/// Editable metadata fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Changes {
    /// New provider.
    pub provider: Option<String>,
    /// New ID.
    pub id: Option<String>,
    /// New name.
    pub name: Option<String>,
}
/// One operation, preserving exact sequence and action identity.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Operation {
    /// Raw picker dispatch.
    Input(String),
    /// Structured public action.
    Action(Action),
}
/// Structured public action.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// Publish bindings.
    Bindings(BTreeMap<String, Binding>),
    /// Publish theme.
    Theme(u8),
    /// Set outer focus.
    Focus(bool),
    /// Set Input focus.
    InputFocus(bool),
    /// Direct Input dispatch.
    InputSubmit(String),
    /// Direct text replacement.
    SetSearch(String),
    /// Clear outer children.
    ClearChildren(bool),
    /// Edit shared model and dispatch filtering.
    MutateModel(Mutation),
}
/// Recorded complete frame.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Styled lines, without normalization.
    pub lines: Vec<String>,
    /// Ordered notifications.
    pub events: Vec<Vec<Value>>,
    /// Independent outer/Input focus when observed.
    pub focus: Option<[bool; 2]>,
}
/// Recorded outputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Expected {
    /// Frames before and after every operation.
    #[serde(default)]
    pub frames: Vec<Frame>,
    /// Constructor refresh count.
    pub refresh_count: Option<usize>,
    /// Retained Input rendering.
    pub input_lines: Option<Vec<String>>,
    /// Retained Input notifications.
    pub events: Option<Vec<Vec<Value>>>,
}
/// One named corpus row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Owning witness.
    pub test: String,
    /// Complete input.
    pub input: Query,
    /// Complete expected observations.
    pub expected: Expected,
}
/// Decode every field and reject repeated query ownership.
pub fn cases(name: &str) -> Vec<Case> {
    let text = include_str!("../fixtures/model_selection.json");
    let raw: Vec<Value> = serde_json::from_str(text).unwrap();
    let mut seen = std::collections::HashSet::new();
    for row in &raw {
        assert!(seen.insert((row["test"].to_string(), row["input"].to_string())));
    }
    let all: Vec<Case> = serde_json::from_str(text).unwrap();
    let selected: Vec<_> = all.into_iter().filter(|case| case.test == name).collect();
    assert!(!selected.is_empty());
    selected
}
/// Distinct palette values used by the oracle.
pub fn colors(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, 31),
                (ThemeColor::Accent, 32),
                (ThemeColor::Muted, 33),
                (ThemeColor::Success, 34),
                (ThemeColor::Error, 35),
                (ThemeColor::Warning, 36),
                (ThemeColor::Dim, 37),
            ]
            .into_iter()
            .map(|(key, index)| (key, ColorValue::Index(index + shift))),
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Read counter and optional diagnostic owned by a real file boundary.
struct Files {
    /// Number of reads.
    reads: Arc<std::sync::atomic::AtomicUsize>,
    /// Failure message.
    failure: Option<String>,
    /// Controlled JSON document.
    text: String,
}
impl ModelFileOperations for Files {
    fn exists(&self, _: &str) -> bool {
        true
    }
    fn read_to_string(&self, _: &str) -> io::Result<String> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.failure.as_ref().map_or_else(
            || Ok(self.text.clone()),
            |message| Err(io::Error::other(message.clone())),
        )
    }
}
/// Registry configured through its real retained provider records.
pub fn registry(query: &Query) -> (ModelRegistry, Arc<std::sync::atomic::AtomicUsize>) {
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let registry = ModelRegistry::with_operations(
        SharedHandle::new(AuthStorage::in_memory(std::iter::empty().collect())),
        query
            .read_failure
            .as_ref()
            .map_or("<models-file>", |f| f.path.as_str()),
        Files {
            text: "{\"providers\":{}}".into(),
            reads: reads.clone(),
            failure: query.read_failure.as_ref().map(|f| f.message.clone()),
        },
        no_commands(),
    )
    .unwrap();
    let mut providers: Vec<(String, Vec<ProviderModelInput>)> = vec![];
    for model in &query.available {
        if let Some((_, models)) = providers
            .iter_mut()
            .find(|(provider, _)| *provider == model.provider)
        {
            models.push(model.definition());
        } else {
            providers.push((model.provider.clone(), vec![model.definition()]));
        }
    }
    for (provider, models) in providers {
        registry
            .register_provider(
                &provider,
                SharedHandle::new(RwLock::new(ProviderConfigInput {
                    base_url: Some("https://example.invalid".into()),
                    api_key: Some("fixture-key".into()),
                    api: Some("openai-completions".into()),
                    models: Some(models),
                    ..Default::default()
                })),
            )
            .unwrap();
    }
    reads.store(0, std::sync::atomic::Ordering::SeqCst);
    (registry, reads)
}

/// Malformed data is parsed by the real catalog loader.
pub fn malformed_registry() -> ModelRegistry {
    let registry = ModelRegistry::with_operations(
        SharedHandle::new(AuthStorage::in_memory(std::iter::empty().collect())),
        "<malformed-models-file>",
        Files {
            reads: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            failure: None,
            text: "{".into(),
        },
        no_commands(),
    )
    .unwrap();
    assert!(
        registry
            .get_error()
            .unwrap()
            .contains("Failed to parse models.json")
    );
    registry
}
