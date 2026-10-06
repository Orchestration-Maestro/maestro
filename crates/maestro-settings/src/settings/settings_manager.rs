mod value;
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Operation<'a> = dyn FnMut(Option<&str>) -> Result<Option<String>, Error> + 'a;
type Spawn = Arc<dyn Fn(Pin<Box<dyn Future<Output = ()> + Send + 'static>>) + Send + Sync>;
/// Raw scoped preference values, including unknown properties.
///
/// Very deep caller-owned trees must be dismantled iteratively before dropping
/// them: the underlying value's destructor is recursive. Manager-owned trees
/// use iterative teardown. Pretty saves retain two-space indentation at every
/// depth and are bounded by memory for their complete output string.
pub type Settings = Value;
/// A preference storage scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsScope {
    /// User-wide preferences.
    Global,
    /// Project preferences.
    Project,
}
impl SettingsScope {
    fn index(self) -> usize {
        match self {
            Self::Global => 0,
            Self::Project => 1,
        }
    }
}
/// An original storage or load error associated with its scope.
#[derive(Clone, Debug)]
pub struct SettingsError {
    /// The affected scope.
    pub scope: SettingsScope,
    /// The original error, shared without replacing its details.
    pub error: Arc<dyn std::error::Error + Send + Sync>,
}
/// Raw text storage. `None` returned by the callback leaves bytes unchanged.
/// Callbacks run synchronously without an internal state mutex held. File storage
/// holds a cooperating-writer lease for existing files; missing-file callbacks
/// and memory callbacks run without that lease.
pub trait SettingsStorage: Send + Sync {
    /// Reads and optionally replaces one scope, propagating original errors.
    fn with_lock<'a>(
        &self,
        scope: SettingsScope,
        operation: &'a mut Operation<'a>,
    ) -> Result<(), Error>;
}
/// In-memory raw-text storage with the same callback contract as file storage.
#[derive(Default)]
pub struct InMemorySettingsStorage {
    text: Mutex<[Option<String>; 2]>,
}
impl InMemorySettingsStorage {
    /// Creates two missing scopes.
    pub fn new() -> Self {
        Self::default()
    }
}
impl SettingsStorage for InMemorySettingsStorage {
    fn with_lock<'a>(
        &self,
        scope: SettingsScope,
        operation: &'a mut Operation<'a>,
    ) -> Result<(), Error> {
        let current = self.text.lock().unwrap()[scope.index()].clone();
        if let Some(next) = operation(current.as_deref())? {
            self.text.lock().unwrap()[scope.index()] = Some(next);
        }
        Ok(())
    }
}
#[derive(Default)]
struct Completion(Mutex<(bool, Vec<Waker>)>);
impl Completion {
    fn finish(&self) {
        let wakers = {
            let mut state = self.0.lock().unwrap();
            state.0 = true;
            std::mem::take(&mut state.1)
        };
        for waker in wakers {
            waker.wake();
        }
    }
}
struct Wait(Arc<Completion>);
impl Future for Wait {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0.0.lock().unwrap().0 {
            return Poll::Ready(());
        }
        let mut waker = Some(cx.waker().clone());
        {
            let mut state = self.0.0.lock().unwrap();
            if state.0 {
                Poll::Ready(())
            } else {
                if !state.1.iter().any(|w| w.will_wake(cx.waker())) {
                    state.1.push(waker.take().unwrap());
                }
                Poll::Pending
            }
        }
    }
}
#[derive(Clone, Default)]
struct Modified {
    fields: Vec<String>,
    nested: BTreeMap<String, Vec<String>>,
}
impl Modified {
    fn mark(&mut self, field: &str, child: Option<&str>) {
        if !self.fields.iter().any(|key| key == field) {
            self.fields.push(field.into());
        }
        if let Some(child) = child {
            let keys = self.nested.entry(field.into()).or_default();
            if !keys.iter().any(|key| key == child) {
                keys.push(child.into());
            }
        }
    }
}
struct State {
    effective_numbers: BTreeMap<String, f64>,
    effective_named: BTreeMap<(String, String), Value>,
    named: BTreeMap<(String, String), Value>,
    root_properties: [Map<String, Value>; 2],
    numbers: BTreeMap<String, f64>,
    scopes: [Value; 2],
    effective: Value,
    modified: [Modified; 2],
    failed: [bool; 2],
    errors: Vec<SettingsError>,
    tail: Arc<Completion>,
}
impl Drop for State {
    fn drop(&mut self) {
        for scope in &mut self.scopes {
            value::teardown(std::mem::take(scope));
        }
        value::teardown(std::mem::take(&mut self.effective));
        for map in &mut self.root_properties {
            value::teardown(Value::Object(std::mem::take(map)));
        }
        for (_, item) in std::mem::take(&mut self.named) {
            value::teardown(item);
        }
        for (_, item) in std::mem::take(&mut self.effective_named) {
            value::teardown(item);
        }
    }
}
/// Owns accepted preferences, immediate publication and an ordered write queue.
/// The supplied executor must defer polling until after the setter returns and
/// must drive jobs independently of `flush`. Clones share one cache and queue.
#[derive(Clone)]
pub struct SettingsManager {
    storage: Arc<dyn SettingsStorage>,
    state: Arc<Mutex<State>>,
    spawn: Spawn,
}
fn empty() -> Value {
    Value::Object(Map::new())
}
fn load(storage: &dyn SettingsStorage, scope: SettingsScope) -> Result<Value, Error> {
    let mut raw = None;
    storage.with_lock(scope, &mut |text| {
        raw = text.map(str::to_owned);
        Ok(None)
    })?;
    match raw.filter(|text| !text.is_empty()) {
        Some(text) => value::convert(value::parse(&text)?),
        None => Ok(empty()),
    }
}
impl SettingsManager {
    /// Loads global then project text independently through a replaceable adapter.
    pub fn from_storage(storage: Arc<dyn SettingsStorage>, spawn: Spawn) -> Self {
        let mut scopes = [empty(), empty()];
        let mut failed = [false; 2];
        let mut errors = Vec::new();
        for scope in [SettingsScope::Global, SettingsScope::Project] {
            match load(storage.as_ref(), scope) {
                Ok(value) => scopes[scope.index()] = value,
                Err(error) => {
                    failed[scope.index()] = true;
                    errors.push(SettingsError {
                        scope,
                        error: Arc::from(error),
                    });
                }
            }
        }
        let effective = value::merge(&scopes[0], &scopes[1]);
        let tail = Arc::new(Completion::default());
        tail.finish();
        Self {
            storage,
            spawn,
            state: Arc::new(Mutex::new(State {
                effective_numbers: Default::default(),
                effective_named: Default::default(),
                named: Default::default(),
                root_properties: Default::default(),
                numbers: BTreeMap::new(),
                scopes,
                effective,
                modified: Default::default(),
                failed,
                errors,
                tail,
            })),
        }
    }
    /// Clones a seed into raw in-memory storage before loading it.
    pub fn in_memory(settings: Settings, spawn: Spawn) -> Result<Self, Error> {
        let settings = value::Owned(value::convert(settings)?);
        let storage = Arc::new(InMemorySettingsStorage::new());
        storage.with_lock(SettingsScope::Global, &mut |_| {
            Ok(Some(value::stringify(&settings)))
        })?;
        Ok(Self::from_storage(storage, spawn))
    }
    /// Returns a detached global snapshot. Nonfinite setter values appear as null.
    /// See [`Settings`] for the teardown obligation of very deep raw snapshots.
    pub fn get_global_settings(&self) -> Settings {
        value::clone(&self.state.lock().unwrap().scopes[0])
    }
    /// Returns a detached project snapshot.
    /// See [`Settings`] for the teardown obligation of very deep raw snapshots.
    pub fn get_project_settings(&self) -> Settings {
        value::clone(&self.state.lock().unwrap().scopes[1])
    }
    /// Waits for the captured queue tail; does not start queued jobs.
    pub async fn flush(&self) {
        let tail = self.state.lock().unwrap().tail.clone();
        Wait(tail).await;
    }
    /// Waits for pending writes and independently reloads accepted scopes.
    pub async fn reload(&self) {
        self.flush().await;
        for scope in [SettingsScope::Global, SettingsScope::Project] {
            let loaded = load(self.storage.as_ref(), scope);
            let mut state = self.state.lock().unwrap();
            match loaded {
                Ok(value) => {
                    value::replace(&mut state.scopes[scope.index()], value);
                    value::teardown(Value::Object(std::mem::take(
                        &mut state.root_properties[scope.index()],
                    )));
                    if scope == SettingsScope::Global {
                        state.numbers.clear();
                        for (_, item) in std::mem::take(&mut state.named) {
                            value::teardown(item);
                        }
                    }
                    state.failed[scope.index()] = false;
                }
                Err(error) => {
                    state.failed[scope.index()] = true;
                    state.errors.push(SettingsError {
                        scope,
                        error: Arc::from(error),
                    });
                }
            }
        }
        let mut state = self.state.lock().unwrap();
        state.modified = Default::default();
        publish(&mut state);
    }
    fn read(&self, key: &str) -> Option<value::Owned> {
        self.state
            .lock()
            .unwrap()
            .effective
            .get(key)
            .map(|item| value::Owned(value::clone(item)))
    }
    fn set_nested(&self, field: &str, child: &str, value: Value) -> Result<(), Error> {
        let job = {
            let mut state = self.state.lock().unwrap();
            let map = if state.scopes[0].is_array() {
                &mut state.root_properties[0]
            } else {
                state.scopes[0].as_object_mut().unwrap()
            };
            let container = map.entry(field.to_owned()).or_insert_with(empty);
            if !value::truthy(container) {
                value::replace(container, empty());
            }
            if let Some(object) = container.as_object_mut() {
                value::insert(object, child.into(), value);
            } else if container.is_array() {
                if let Some(old) = state.named.insert((field.into(), child.into()), value) {
                    value::teardown(old);
                }
            } else {
                let kind = match container {
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    _ => unreachable!(),
                };
                return Err(std::io::Error::other(format!(
                    "Cannot create property '{child}' on {kind} '{}'",
                    value::primitive_text(container)
                ))
                .into());
            }
            state.modified[0].mark(field, Some(child));
            capture_write(&mut state, SettingsScope::Global)
        };
        if let Some(job) = job {
            self.schedule(SettingsScope::Global, job);
        }
        Ok(())
    }
    fn nested(&self, field: &str, child: &str) -> Option<value::Owned> {
        let state = self.state.lock().unwrap();
        if state.effective.get(field).is_some_and(Value::is_array)
            && state.scopes[1].get(field).is_none()
            && let Some(v) = state.effective_named.get(&(field.into(), child.into()))
        {
            return Some(value::Owned(value::clone(v)));
        }
        state
            .effective
            .get(field)
            .and_then(|v| v.get(child).map(|item| value::Owned(value::clone(item))))
    }
    fn set(&self, scope: SettingsScope, key: &str, value: Option<Value>) {
        self.set_value(scope, key, value, None);
    }
    fn set_value(
        &self,
        scope: SettingsScope,
        key: &str,
        value: Option<Value>,
        number: Option<f64>,
    ) {
        let job = {
            let mut state = self.state.lock().unwrap();
            let i = scope.index();
            if let Some(number) = number {
                state.numbers.insert(key.into(), number);
            }
            let map = if state.scopes[i].is_array() {
                &mut state.root_properties[i]
            } else {
                state.scopes[i].as_object_mut().unwrap()
            };
            if let Some(value) = value {
                value::insert(map, key.into(), value);
            } else {
                value::remove(map, key);
            }
            state.modified[i].mark(key, None);
            capture_write(&mut state, scope)
        };
        if let Some(job) = job {
            self.schedule(scope, job);
        }
    }
    fn schedule(
        &self,
        scope: SettingsScope,
        (snapshot, modified, previous, next): (Value, Modified, Arc<Completion>, Arc<Completion>),
    ) {
        let storage = self.storage.clone();
        let state = self.state.clone();
        let snapshot = value::Owned(snapshot);
        (self.spawn)(Box::pin(async move {
            Wait(previous).await;
            let result = storage.with_lock(scope, &mut |text| {
                let current = value::Owned(match text.filter(|s| !s.is_empty()) {
                    Some(s) => value::convert(value::parse(s)?)?,
                    None => empty(),
                });
                let mut current = value::Owned(Value::Object(value::spread(&current)));
                let object = current.as_object_mut().unwrap();
                for field in &modified.fields {
                    if let Some(children) = modified.nested.get(field)
                        && let Some(value) = snapshot
                            .get(field)
                            .filter(|v| v.is_object() || v.is_array())
                    {
                        let mut nested = object.get(field).map(value::spread).unwrap_or_default();
                        for child in children {
                            if let Some(v) = value.get(child) {
                                value::insert(&mut nested, child.clone(), value::clone(v));
                            } else {
                                value::remove(&mut nested, child);
                            }
                        }
                        value::insert(object, field.clone(), Value::Object(nested));
                    } else if let Some(value) = snapshot.get(field) {
                        value::insert(object, field.clone(), value::clone(value));
                    } else {
                        value::remove(object, field);
                    }
                }
                Ok(Some(value::stringify(&current)))
            });
            {
                let mut state = state.lock().unwrap();
                match result {
                    Ok(()) => state.modified[scope.index()] = Modified::default(),
                    Err(error) => state.errors.push(SettingsError {
                        scope,
                        error: Arc::from(error),
                    }),
                }
            }
            next.finish();
        }));
    }
    /// Returns the selected theme without materializing defaults.
    pub fn get_theme(&self) -> Option<String> {
        self.read("theme")
            .and_then(|v| v.as_str().map(str::to_owned))
    }
    /// Publishes a global theme and enqueues persistence.
    pub fn set_theme(&self, theme: String) {
        self.set(SettingsScope::Global, "theme", Some(Value::String(theme)));
    }
    /// Returns the selected model without materializing defaults.
    pub fn get_default_model(&self) -> Option<String> {
        self.read("defaultModel")
            .and_then(|v| v.as_str().map(str::to_owned))
    }
}
impl SettingsManager {
    /// Returns the requested thinking level as open string data.
    pub fn get_default_thinking_level(&self) -> Option<String> {
        self.read("defaultThinkingLevel")
            .and_then(|v| v.as_str().map(str::to_owned))
    }
    /// Publishes the requested thinking level and queues persistence.
    pub fn set_default_thinking_level(&self, level: String) {
        self.set(
            SettingsScope::Global,
            "defaultThinkingLevel",
            Some(Value::String(level)),
        );
    }
}
impl SettingsManager {
    /// Drains errors without clearing a failed-load persistence latch.
    pub fn drain_errors(&self) -> Vec<SettingsError> {
        std::mem::take(&mut self.state.lock().unwrap().errors)
    }
}

/// Provider-request retry preferences, with unknown properties retained.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProviderRetrySettings {
    /// Optional request timeout in milliseconds.
    pub timeout_ms: Option<f64>,
    /// Optional provider retry count.
    pub max_retries: Option<f64>,
    /// Maximum server-requested delay in milliseconds.
    pub max_retry_delay_ms: Option<f64>,
    /// Additional provider properties.
    pub extra: Map<String, Value>,
}
impl SettingsManager {
    /// Applies ephemeral one-level overrides without persisting them.
    ///
    /// A null root returns an immediate error without changing accepted or effective values.
    pub fn apply_overrides(&self, overrides: Settings) -> Result<(), Error> {
        if overrides.is_null() {
            return Err(std::io::Error::other("Cannot convert undefined or null to object").into());
        }
        let mut state = self.state.lock().unwrap();
        let keys = value::Owned(Value::Object(value::spread(&overrides)));
        for key in keys.as_object().unwrap().keys() {
            state.effective_numbers.remove(key);
            state.effective_named.retain(|(field, _), _| field != key);
        }
        let effective = value::merge(&state.effective, &overrides);
        value::replace(&mut state.effective, effective);
        value::teardown(overrides);
        Ok(())
    }
    /// Returns provider defaults without synthesizing them in raw storage.
    pub fn get_provider_retry_settings(&self) -> ProviderRetrySettings {
        let state = self.state.lock().unwrap();
        let p = &state.effective["retry"]["provider"];
        ProviderRetrySettings {
            timeout_ms: p["timeoutMs"].as_f64(),
            max_retries: p["maxRetries"].as_f64(),
            max_retry_delay_ms: Some(p["maxRetryDelayMs"].as_f64().unwrap_or(60000.0)),
            extra: Map::new(),
        }
    }
}
impl SettingsManager {
    /// Returns the effective last changelog version preference.
    pub fn get_last_changelog_version(&self) -> Option<String> {
        self.read("lastChangelogVersion")
            .and_then(|v| v.as_str().map(str::to_owned))
    }
    /// Publishes the global last changelog version preference.
    pub fn set_last_changelog_version(&self, value: String) {
        self.set(
            SettingsScope::Global,
            "lastChangelogVersion",
            Some(Value::String(value)),
        );
    }
}
impl SettingsManager {
    /// Returns the effective default provider preference.
    pub fn get_default_provider(&self) -> Option<String> {
        self.read("defaultProvider")
            .and_then(|v| v.as_str().map(str::to_owned))
    }
    /// Publishes the global default provider preference.
    pub fn set_default_provider(&self, value: String) {
        self.set(
            SettingsScope::Global,
            "defaultProvider",
            Some(Value::String(value)),
        );
    }
}
impl SettingsManager {
    /// Returns the effective steering mode preference.
    pub fn get_steering_mode(&self) -> String {
        self.read("steeringMode")
            .and_then(|v| v.as_str().map(str::to_owned))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "one-at-a-time".into())
    }
    /// Publishes the global steering mode preference.
    pub fn set_steering_mode(&self, value: String) {
        self.set(
            SettingsScope::Global,
            "steeringMode",
            Some(Value::String(value)),
        );
    }
}
impl SettingsManager {
    /// Returns the effective follow up mode preference.
    pub fn get_follow_up_mode(&self) -> String {
        self.read("followUpMode")
            .and_then(|v| v.as_str().map(str::to_owned))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "one-at-a-time".into())
    }
    /// Publishes the global follow up mode preference.
    pub fn set_follow_up_mode(&self, value: String) {
        self.set(
            SettingsScope::Global,
            "followUpMode",
            Some(Value::String(value)),
        );
    }
}
impl SettingsManager {
    /// Returns the effective transport preference.
    pub fn get_transport(&self) -> String {
        self.read("transport")
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "auto".into())
    }
    /// Publishes the global transport preference.
    pub fn set_transport(&self, value: String) {
        self.set(
            SettingsScope::Global,
            "transport",
            Some(Value::String(value)),
        );
    }
}
/// Model transport preference, kept as open string data.
pub type TransportSetting = String;
impl SettingsManager {
    /// Publishes a global model identity with one queued write.
    pub fn set_default_model_and_provider(&self, provider: String, model_id: String) {
        let job = {
            let mut state = self.state.lock().unwrap();
            let map = if state.scopes[0].is_array() {
                &mut state.root_properties[0]
            } else {
                state.scopes[0].as_object_mut().unwrap()
            };
            value::insert(map, "defaultProvider".into(), Value::String(provider));
            value::insert(map, "defaultModel".into(), Value::String(model_id));
            state.modified[0].mark("defaultProvider", None);
            state.modified[0].mark("defaultModel", None);
            capture_write(&mut state, SettingsScope::Global)
        };
        if let Some(job) = job {
            self.schedule(SettingsScope::Global, job);
        }
    }
    /// Publishes the global model preference.
    pub fn set_default_model(&self, model_id: String) {
        self.set(
            SettingsScope::Global,
            "defaultModel",
            Some(Value::String(model_id)),
        );
    }
}
/// Typed CompactionSettings accessor values; not a file admission schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompactionSettings {
    /// The enabled preference.
    pub enabled: Option<bool>,
    /// The reserve tokens preference.
    pub reserve_tokens: Option<f64>,
    /// The keep recent tokens preference.
    pub keep_recent_tokens: Option<f64>,
    /// Additional raw properties.
    pub extra: Map<String, Value>,
}
/// Typed BranchSummarySettings accessor values; not a file admission schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BranchSummarySettings {
    /// The reserve tokens preference.
    pub reserve_tokens: Option<f64>,
    /// The skip prompt preference.
    pub skip_prompt: Option<bool>,
    /// Additional raw properties.
    pub extra: Map<String, Value>,
}
/// Typed RetrySettings accessor values; not a file admission schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RetrySettings {
    /// The enabled preference.
    pub enabled: Option<bool>,
    /// The max retries preference.
    pub max_retries: Option<f64>,
    /// The base delay ms preference.
    pub base_delay_ms: Option<f64>,
    /// The provider preference.
    pub provider: Option<ProviderRetrySettings>,
    /// Additional raw properties.
    pub extra: Map<String, Value>,
}
impl SettingsManager {
    /// Returns the effective compaction enabled preference.
    pub fn get_compaction_enabled(&self) -> bool {
        self.nested("compaction", "enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
    /// Publishes one nested compaction preference; malformed truthy containers fail before queueing.
    pub fn set_compaction_enabled(&self, enabled: bool) -> Result<(), Error> {
        self.set_nested("compaction", "enabled", Value::Bool(enabled))
    }
    /// Returns the effective compaction reserve tokens preference.
    pub fn get_compaction_reserve_tokens(&self) -> f64 {
        self.nested("compaction", "reserveTokens")
            .and_then(|v| v.as_f64())
            .unwrap_or(16384.0)
    }
    /// Returns the effective compaction keep recent tokens preference.
    pub fn get_compaction_keep_recent_tokens(&self) -> f64 {
        self.nested("compaction", "keepRecentTokens")
            .and_then(|v| v.as_f64())
            .unwrap_or(20000.0)
    }
    /// Returns the effective branch summary skip prompt preference.
    pub fn get_branch_summary_skip_prompt(&self) -> bool {
        self.nested("branchSummary", "skipPrompt")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    /// Returns the effective retry enabled preference.
    pub fn get_retry_enabled(&self) -> bool {
        self.nested("retry", "enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
    /// Publishes one nested retry preference; malformed truthy containers fail before queueing.
    pub fn set_retry_enabled(&self, enabled: bool) -> Result<(), Error> {
        self.set_nested("retry", "enabled", Value::Bool(enabled))
    }
    /// Returns effective compaction defaults without storing them.
    pub fn get_compaction_settings(&self) -> CompactionSettings {
        CompactionSettings {
            enabled: Some(self.get_compaction_enabled()),
            reserve_tokens: Some(self.get_compaction_reserve_tokens()),
            keep_recent_tokens: Some(self.get_compaction_keep_recent_tokens()),
            extra: Map::new(),
        }
    }
    /// Returns effective branch summary defaults without storing them.
    pub fn get_branch_summary_settings(&self) -> BranchSummarySettings {
        BranchSummarySettings {
            reserve_tokens: Some(
                self.nested("branchSummary", "reserveTokens")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(16384.0),
            ),
            skip_prompt: Some(self.get_branch_summary_skip_prompt()),
            extra: Map::new(),
        }
    }
    /// Returns effective application retry defaults without a synthesized provider.
    pub fn get_retry_settings(&self) -> RetrySettings {
        RetrySettings {
            enabled: Some(self.get_retry_enabled()),
            max_retries: Some(
                self.nested("retry", "maxRetries")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(3.0),
            ),
            base_delay_ms: Some(
                self.nested("retry", "baseDelayMs")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(2000.0),
            ),
            provider: None,
            extra: Map::new(),
        }
    }
}
impl SettingsManager {
    /// Returns the effective shell path preference.
    pub fn get_shell_path(&self) -> Option<String> {
        self.read("shellPath")
            .and_then(|v| v.as_str().map(str::to_owned))
    }
    /// Publishes the global shell path preference.
    pub fn set_shell_path(&self, value: Option<String>) {
        self.set(SettingsScope::Global, "shellPath", value.map(Value::String));
    }
}
impl SettingsManager {
    /// Returns the effective shell command prefix preference.
    pub fn get_shell_command_prefix(&self) -> Option<String> {
        self.read("shellCommandPrefix")
            .and_then(|v| v.as_str().map(str::to_owned))
    }
    /// Publishes the global shell command prefix preference.
    pub fn set_shell_command_prefix(&self, value: Option<String>) {
        self.set(
            SettingsScope::Global,
            "shellCommandPrefix",
            value.map(Value::String),
        );
    }
}
impl SettingsManager {
    /// Returns the effective double escape action preference.
    pub fn get_double_escape_action(&self) -> String {
        self.read("doubleEscapeAction")
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "tree".into())
    }
    /// Publishes the global double escape action preference.
    pub fn set_double_escape_action(&self, value: String) {
        self.set(
            SettingsScope::Global,
            "doubleEscapeAction",
            Some(Value::String(value)),
        );
    }
}
impl SettingsManager {
    /// Returns the effective quiet startup preference.
    pub fn get_quiet_startup(&self) -> bool {
        self.read("quietStartup")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    /// Publishes the global quiet startup preference.
    pub fn set_quiet_startup(&self, value: bool) {
        self.set(
            SettingsScope::Global,
            "quietStartup",
            Some(Value::Bool(value)),
        );
    }
    /// Returns the effective hide thinking block preference.
    pub fn get_hide_thinking_block(&self) -> bool {
        self.read("hideThinkingBlock")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    /// Publishes the global hide thinking block preference.
    pub fn set_hide_thinking_block(&self, value: bool) {
        self.set(
            SettingsScope::Global,
            "hideThinkingBlock",
            Some(Value::Bool(value)),
        );
    }
    /// Returns the effective collapse changelog preference.
    pub fn get_collapse_changelog(&self) -> bool {
        self.read("collapseChangelog")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    /// Publishes the global collapse changelog preference.
    pub fn set_collapse_changelog(&self, value: bool) {
        self.set(
            SettingsScope::Global,
            "collapseChangelog",
            Some(Value::Bool(value)),
        );
    }
    /// Returns the effective enable install telemetry preference.
    pub fn get_enable_install_telemetry(&self) -> bool {
        self.read("enableInstallTelemetry")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
    /// Publishes the global enable install telemetry preference.
    pub fn set_enable_install_telemetry(&self, value: bool) {
        self.set(
            SettingsScope::Global,
            "enableInstallTelemetry",
            Some(Value::Bool(value)),
        );
    }
    /// Returns a detached command argument vector when all members are strings.
    pub fn get_npm_command(&self) -> Option<Vec<String>> {
        self.read("npmCommand").as_deref().and_then(value::strings)
    }
    /// Publishes a command argument vector or omits its raw property.
    pub fn set_npm_command(&self, command: Option<Vec<String>>) {
        self.set(
            SettingsScope::Global,
            "npmCommand",
            command.map(|v| Value::Array(v.into_iter().map(Value::String).collect())),
        );
    }
    /// Returns the configured code indentation or two spaces.
    pub fn get_code_block_indent(&self) -> String {
        self.nested("markdown", "codeBlockIndent")
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "  ".into())
    }
}
/// A package source or a source with ordered resource filters.
#[derive(Clone, Debug, PartialEq)]
pub enum PackageSource {
    /// A source with all resources enabled.
    String(String),
    /// A source with optional resource filters.
    Object {
        /// The source identifier.
        source: String,
        /// Extension filters.
        extensions: Option<Vec<String>>,
        /// Skill filters.
        skills: Option<Vec<String>>,
        /// Prompt filters.
        prompts: Option<Vec<String>>,
        /// Theme filters.
        themes: Option<Vec<String>>,
        /// Original property positions retained through typed round-trips.
        property_order: Vec<String>,
        /// Additional raw properties.
        extra: Map<String, Value>,
    },
}
impl PackageSource {
    fn read(v: &Value) -> Option<Self> {
        if let Some(s) = v.as_str() {
            return Some(Self::String(s.into()));
        }
        let mut owned = value::Owned(Value::Object(value::clone_map(v.as_object()?)));
        let extra = owned.as_object_mut().unwrap();
        let property_order = extra.keys().cloned().collect();
        let source = value::take(extra, "source")?.as_str()?.to_owned();
        fn list(map: &mut Map<String, Value>, key: &str) -> Option<Option<Vec<String>>> {
            match value::take(map, key) {
                None => Some(None),
                Some(v) if v.is_null() => Some(None),
                Some(v) => value::strings(&v).map(Some),
            }
        }
        Some(Self::Object {
            source,
            property_order,
            extensions: list(extra, "extensions")?,
            skills: list(extra, "skills")?,
            prompts: list(extra, "prompts")?,
            themes: list(extra, "themes")?,
            extra: std::mem::take(extra),
        })
    }
    fn raw(self) -> Value {
        match self {
            Self::String(s) => Value::String(s),
            Self::Object {
                source,
                extensions,
                skills,
                prompts,
                themes,
                property_order,
                mut extra,
            } => {
                value::insert(&mut extra, "source".into(), Value::String(source));
                for (key, list) in [
                    ("extensions", extensions),
                    ("skills", skills),
                    ("prompts", prompts),
                    ("themes", themes),
                ] {
                    if let Some(list) = list {
                        value::insert(
                            &mut extra,
                            key.into(),
                            Value::Array(list.into_iter().map(Value::String).collect()),
                        );
                    } else {
                        value::remove(&mut extra, key);
                    }
                }
                Value::Object(value::ordered(extra, &property_order))
            }
        }
    }
}
impl SettingsManager {
    /// Returns detached package sources; malformed lists act as unset.
    pub fn get_packages(&self) -> Vec<PackageSource> {
        self.read("packages")
            .and_then(|v| {
                let mut packages = Vec::new();
                for item in v.as_array()? {
                    if let Some(package) = PackageSource::read(item) {
                        packages.push(package);
                    } else {
                        for package in packages {
                            value::teardown(package.raw());
                        }
                        return None;
                    }
                }
                Some(packages)
            })
            .unwrap_or_default()
    }
    /// Replaces global package sources without installation or resolution.
    pub fn set_packages(&self, packages: Vec<PackageSource>) {
        self.set(
            SettingsScope::Global,
            "packages",
            Some(Value::Array(
                packages.into_iter().map(PackageSource::raw).collect(),
            )),
        );
    }
    /// Replaces project package sources without installation or resolution.
    pub fn set_project_packages(&self, packages: Vec<PackageSource>) {
        self.set(
            SettingsScope::Project,
            "packages",
            Some(Value::Array(
                packages.into_iter().map(PackageSource::raw).collect(),
            )),
        );
    }
    /// Returns detached effective extension paths.
    pub fn get_extension_paths(&self) -> Vec<String> {
        self.read("extensions")
            .as_deref()
            .and_then(value::strings)
            .unwrap_or_default()
    }
    /// Replaces global extension paths without resolving them.
    pub fn set_extension_paths(&self, paths: Vec<String>) {
        self.set(
            SettingsScope::Global,
            "extensions",
            Some(Value::Array(paths.into_iter().map(Value::String).collect())),
        );
    }
    /// Replaces project extension paths without resolving them.
    pub fn set_project_extension_paths(&self, paths: Vec<String>) {
        self.set(
            SettingsScope::Project,
            "extensions",
            Some(Value::Array(paths.into_iter().map(Value::String).collect())),
        );
    }
    /// Returns detached effective skill paths.
    pub fn get_skill_paths(&self) -> Vec<String> {
        self.read("skills")
            .as_deref()
            .and_then(value::strings)
            .unwrap_or_default()
    }
    /// Replaces global skill paths without resolving them.
    pub fn set_skill_paths(&self, paths: Vec<String>) {
        self.set(
            SettingsScope::Global,
            "skills",
            Some(Value::Array(paths.into_iter().map(Value::String).collect())),
        );
    }
    /// Replaces project skill paths without resolving them.
    pub fn set_project_skill_paths(&self, paths: Vec<String>) {
        self.set(
            SettingsScope::Project,
            "skills",
            Some(Value::Array(paths.into_iter().map(Value::String).collect())),
        );
    }
    /// Returns detached effective prompt template paths.
    pub fn get_prompt_template_paths(&self) -> Vec<String> {
        self.read("prompts")
            .as_deref()
            .and_then(value::strings)
            .unwrap_or_default()
    }
    /// Replaces global prompt template paths without resolving them.
    pub fn set_prompt_template_paths(&self, paths: Vec<String>) {
        self.set(
            SettingsScope::Global,
            "prompts",
            Some(Value::Array(paths.into_iter().map(Value::String).collect())),
        );
    }
    /// Replaces project prompt template paths without resolving them.
    pub fn set_project_prompt_template_paths(&self, paths: Vec<String>) {
        self.set(
            SettingsScope::Project,
            "prompts",
            Some(Value::Array(paths.into_iter().map(Value::String).collect())),
        );
    }
    /// Returns detached effective theme paths.
    pub fn get_theme_paths(&self) -> Vec<String> {
        self.read("themes")
            .as_deref()
            .and_then(value::strings)
            .unwrap_or_default()
    }
    /// Replaces global theme paths without resolving them.
    pub fn set_theme_paths(&self, paths: Vec<String>) {
        self.set(
            SettingsScope::Global,
            "themes",
            Some(Value::Array(paths.into_iter().map(Value::String).collect())),
        );
    }
    /// Replaces project theme paths without resolving them.
    pub fn set_project_theme_paths(&self, paths: Vec<String>) {
        self.set(
            SettingsScope::Project,
            "themes",
            Some(Value::Array(paths.into_iter().map(Value::String).collect())),
        );
    }
    /// Returns whether skills become commands, defaulting to true.
    pub fn get_enable_skill_commands(&self) -> bool {
        self.read("enableSkillCommands")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
    /// Publishes the skill-command preference.
    pub fn set_enable_skill_commands(&self, enabled: bool) {
        self.set(
            SettingsScope::Global,
            "enableSkillCommands",
            Some(Value::Bool(enabled)),
        );
    }
}
impl SettingsManager {
    /// Returns the effective show images preference.
    pub fn get_show_images(&self) -> bool {
        self.nested("terminal", "showImages")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
    /// Publishes one nested show images preference.
    pub fn set_show_images(&self, enabled: bool) -> Result<(), Error> {
        self.set_nested("terminal", "showImages", Value::Bool(enabled))
    }
    /// Returns the effective show terminal progress preference.
    pub fn get_show_terminal_progress(&self) -> bool {
        self.nested("terminal", "showTerminalProgress")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    /// Publishes one nested show terminal progress preference.
    pub fn set_show_terminal_progress(&self, enabled: bool) -> Result<(), Error> {
        self.set_nested("terminal", "showTerminalProgress", Value::Bool(enabled))
    }
    /// Returns the effective image auto resize preference.
    pub fn get_image_auto_resize(&self) -> bool {
        self.nested("images", "autoResize")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
    /// Publishes one nested image auto resize preference.
    pub fn set_image_auto_resize(&self, enabled: bool) -> Result<(), Error> {
        self.set_nested("images", "autoResize", Value::Bool(enabled))
    }
    /// Returns the effective block images preference.
    pub fn get_block_images(&self) -> bool {
        self.nested("images", "blockImages")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    /// Publishes one nested block images preference.
    pub fn set_block_images(&self, enabled: bool) -> Result<(), Error> {
        self.set_nested("images", "blockImages", Value::Bool(enabled))
    }
}
impl SettingsManager {
    /// Returns a finite floored image width of at least one, or sixty.
    pub fn get_image_width_cells(&self) -> f64 {
        self.nested("terminal", "imageWidthCells")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
            .map(|v| value::clamp(v.floor(), 1.0, f64::INFINITY))
            .unwrap_or(60.0)
    }
    /// Publishes a floored image width; NaN is not suppressed by the clamp.
    pub fn set_image_width_cells(&self, width: f64) -> Result<(), Error> {
        self.set_nested(
            "terminal",
            "imageWidthCells",
            Value::from(value::clamp(width.floor(), 1.0, f64::INFINITY)),
        )
    }
    /// Returns loaded padding unchanged, defaulting to zero.
    pub fn get_editor_padding_x(&self) -> f64 {
        self.number("editorPaddingX").unwrap_or(0.0)
    }
    /// Publishes floored padding clamped to zero through three.
    pub fn set_editor_padding_x(&self, padding: f64) {
        self.set_number("editorPaddingX", value::clamp(padding.floor(), 0.0, 3.0));
    }
    /// Returns a loaded autocomplete limit unchanged, defaulting to five.
    pub fn get_autocomplete_max_visible(&self) -> f64 {
        self.number("autocompleteMaxVisible").unwrap_or(5.0)
    }
    /// Publishes a floored autocomplete limit clamped to three through twenty.
    pub fn set_autocomplete_max_visible(&self, max_visible: f64) {
        self.set_number(
            "autocompleteMaxVisible",
            value::clamp(max_visible.floor(), 3.0, 20.0),
        );
    }
}

impl SettingsManager {
    fn number(&self, key: &str) -> Option<f64> {
        let state = self.state.lock().unwrap();
        state
            .effective_numbers
            .get(key)
            .copied()
            .or_else(|| state.effective.get(key).and_then(Value::as_f64))
    }
    fn set_number(&self, key: &str, number: f64) {
        self.set_value(
            SettingsScope::Global,
            key,
            Some(Value::from(number)),
            Some(number),
        );
    }
}
impl SettingsManager {
    /// Returns one recognized tree filter, otherwise the default filter.
    pub fn get_tree_filter_mode(&self) -> String {
        self.read("treeFilterMode")
            .and_then(|v| {
                v.as_str()
                    .filter(|s| {
                        ["default", "no-tools", "user-only", "labeled-only", "all"].contains(s)
                    })
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "default".into())
    }
    /// Publishes a tree filter without validating other string preferences.
    pub fn set_tree_filter_mode(&self, mode: String) {
        self.set(
            SettingsScope::Global,
            "treeFilterMode",
            Some(Value::String(mode)),
        );
    }
}
impl SettingsManager {
    /// Returns an explicit clear-on-shrink value; null is false, absence uses the environment.
    pub fn get_clear_on_shrink(&self) -> bool {
        match self.nested("terminal", "clearOnShrink").as_deref() {
            Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            _ => std::env::var("MAESTRO_CLEAR_ON_SHRINK").as_deref() == Ok("1"),
        }
    }
    /// Publishes one clear-on-shrink preference.
    pub fn set_clear_on_shrink(&self, enabled: bool) -> Result<(), Error> {
        self.set_nested("terminal", "clearOnShrink", Value::Bool(enabled))
    }
    /// Returns the hardware cursor preference, using the environment for null or absence.
    pub fn get_show_hardware_cursor(&self) -> bool {
        self.read("showHardwareCursor")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| std::env::var("MAESTRO_HARDWARE_CURSOR").as_deref() == Ok("1"))
    }
    /// Publishes the global hardware cursor preference.
    pub fn set_show_hardware_cursor(&self, enabled: bool) {
        self.set(
            SettingsScope::Global,
            "showHardwareCursor",
            Some(Value::Bool(enabled)),
        );
    }
}
impl SettingsManager {
    /// Expands only exact tilde or tilde-slash paths; other text is literal.
    pub fn get_session_dir(&self) -> Option<String> {
        let text = self.read("sessionDir")?.as_str()?.to_owned();
        if text == "~" || text.starts_with("~/") {
            // The standard host lookup includes the account database fallback.
            #[allow(deprecated)]
            let home = if cfg!(windows) {
                std::env::home_dir().map(|path| path.into_os_string())
            } else {
                std::env::var_os("HOME")
                    .or_else(|| std::env::home_dir().map(|path| path.into_os_string()))
            }
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
            if text == "~" {
                Some(home)
            } else {
                Some(
                    value::join(&[
                        std::path::Path::new(&home),
                        std::path::Path::new(&text[2..]),
                    ])
                    .to_string_lossy()
                    .into_owned(),
                )
            }
        } else {
            Some(text)
        }
    }
}
/// Typed TerminalSettings accessor values; not a file admission schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerminalSettings {
    /// The show images preference.
    pub show_images: Option<bool>,
    /// The image width cells preference.
    pub image_width_cells: Option<f64>,
    /// The clear on shrink preference.
    pub clear_on_shrink: Option<bool>,
    /// The show terminal progress preference.
    pub show_terminal_progress: Option<bool>,
    /// Additional raw properties.
    pub extra: Map<String, Value>,
}
/// Typed ImageSettings accessor values; not a file admission schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageSettings {
    /// The auto resize preference.
    pub auto_resize: Option<bool>,
    /// The block images preference.
    pub block_images: Option<bool>,
    /// Additional raw properties.
    pub extra: Map<String, Value>,
}
/// Typed ThinkingBudgetsSettings accessor values; not a file admission schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThinkingBudgetsSettings {
    /// The minimal preference.
    pub minimal: Option<f64>,
    /// The low preference.
    pub low: Option<f64>,
    /// The medium preference.
    pub medium: Option<f64>,
    /// The high preference.
    pub high: Option<f64>,
    /// Additional raw properties.
    pub extra: Map<String, Value>,
}
/// Typed MarkdownSettings accessor values; not a file admission schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarkdownSettings {
    /// The code block indent preference.
    pub code_block_indent: Option<String>,
    /// Additional raw properties.
    pub extra: Map<String, Value>,
}
/// Typed WarningSettings accessor values; not a file admission schema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WarningSettings {
    /// Original property positions retained through typed round-trips.
    pub property_order: Vec<String>,
    /// The anthropic extra usage preference.
    pub anthropic_extra_usage: Option<bool>,
    /// Additional raw properties.
    pub extra: Map<String, Value>,
}
impl SettingsManager {
    /// Returns detached model patterns when all members have the declared type.
    pub fn get_enabled_models(&self) -> Option<Vec<String>> {
        self.read("enabledModels")
            .as_deref()
            .and_then(value::strings)
    }
    /// Replaces or omits global model patterns.
    pub fn set_enabled_models(&self, patterns: Option<Vec<String>>) {
        self.set(
            SettingsScope::Global,
            "enabledModels",
            patterns.map(|v| Value::Array(v.into_iter().map(Value::String).collect())),
        );
    }
    /// Returns detached typed thinking budgets, leaving malformed raw members unchanged.
    pub fn get_thinking_budgets(&self) -> Option<ThinkingBudgetsSettings> {
        let value = self.read("thinkingBudgets")?;
        let mut extra = value::clone_map(value.as_object()?);
        Some(ThinkingBudgetsSettings {
            minimal: value::take(&mut extra, "minimal").and_then(|v| v.as_f64()),
            low: value::take(&mut extra, "low").and_then(|v| v.as_f64()),
            medium: value::take(&mut extra, "medium").and_then(|v| v.as_f64()),
            high: value::take(&mut extra, "high").and_then(|v| v.as_f64()),
            extra,
        })
    }
    /// Returns an owned typed warning record with additional properties.
    pub fn get_warnings(&self) -> WarningSettings {
        let mut extra = self
            .read("warnings")
            .and_then(|v| v.as_object().map(value::clone_map))
            .unwrap_or_default();
        WarningSettings {
            property_order: extra.keys().cloned().collect(),
            anthropic_extra_usage: value::take(&mut extra, "anthropicExtraUsage")
                .and_then(|v| v.as_bool()),
            extra,
        }
    }
    /// Replaces global warning preferences, preserving additional properties.
    pub fn set_warnings(&self, warnings: WarningSettings) {
        let mut raw = warnings.extra;
        if let Some(value) = warnings.anthropic_extra_usage {
            value::insert(&mut raw, "anthropicExtraUsage".into(), Value::Bool(value));
        } else {
            value::remove(&mut raw, "anthropicExtraUsage");
        }
        self.set(
            SettingsScope::Global,
            "warnings",
            Some(Value::Object(value::ordered(raw, &warnings.property_order))),
        );
    }
}

fn projected(state: &State, i: usize) -> Value {
    if state.scopes[i].is_array() {
        let mut properties = value::spread(&state.scopes[i]);
        for (key, item) in &state.root_properties[i] {
            value::insert(&mut properties, key.clone(), value::clone(item));
        }
        Value::Object(properties)
    } else {
        value::clone(&state.scopes[i])
    }
}
fn publish(state: &mut State) {
    let project = value::Owned(projected(state, 1));
    state.effective_numbers = state
        .numbers
        .iter()
        .filter(|(key, _)| project.get(*key).is_none())
        .map(|(key, v)| (key.clone(), *v))
        .collect();
    for (_, item) in std::mem::take(&mut state.effective_named) {
        value::teardown(item);
    }
    state.effective_named = state
        .named
        .iter()
        .filter(|((field, _), _)| project.get(field).is_none())
        .map(|(key, v)| (key.clone(), value::clone(v)))
        .collect();
    let global = value::Owned(projected(state, 0));
    let effective = value::merge(&global, &project);
    value::replace(&mut state.effective, effective);
}

fn captured(state: &State, i: usize) -> Value {
    let mut raw = projected(state, i);
    if i == 0 {
        for ((field, child), value) in &state.named {
            if raw.get(field).is_some_and(Value::is_array) {
                let properties = Value::Object(value::spread(&raw[field]));
                value::replace(&mut raw[field], properties);
            }
            if let Some(map) = raw.get_mut(field).and_then(Value::as_object_mut) {
                value::insert(map, child.clone(), value::clone(value));
            }
        }
    }
    raw
}
#[cfg(not(target_arch = "wasm32"))]
mod file_storage;
#[cfg(not(target_arch = "wasm32"))]
pub use file_storage::FileSettingsStorage;
#[cfg(not(target_arch = "wasm32"))]
impl SettingsManager {
    /// Loads native file preferences from caller-selected directories.
    pub fn create(
        cwd: &std::path::Path,
        agent_dir: &std::path::Path,
        configuration_dir_name: &str,
        spawn: Spawn,
    ) -> Self {
        Self::from_storage(
            Arc::new(FileSettingsStorage::new(
                cwd,
                agent_dir,
                configuration_dir_name,
            )),
            spawn,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod lease;

type CapturedWrite = (Value, Modified, Arc<Completion>, Arc<Completion>);
fn capture_write(state: &mut State, scope: SettingsScope) -> Option<CapturedWrite> {
    publish(state);
    let i = scope.index();
    if state.failed[i] {
        return None;
    }
    let previous = state.tail.clone();
    let next = Arc::new(Completion::default());
    state.tail = next.clone();
    Some((
        captured(state, i),
        state.modified[i].clone(),
        previous,
        next,
    ))
}
