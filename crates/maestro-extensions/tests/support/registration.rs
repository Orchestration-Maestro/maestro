#![cfg(test)]
//! Replay of the bounded registration corpus through public owners and queries.
use indexmap::IndexMap;
use maestro_extensions::types::*;
use maestro_extensions::{
    EventBusController, EventError, Extension, ExtensionAPI, ExtensionFlag, ExtensionRunner,
    ResolvedCommand, SourceInfo, create_event_bus,
};
use maestro_resources::ResourceDiagnostic;
use maestro_tui::{AutocompleteItem, KeybindingKeys, KeybindingsConfig};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: Input,
    expected: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    test: String,
    owners: Vec<Provenance>,
    steps: Vec<Step>,
    observe: Vec<String>,
    #[serde(default, deserialize_with = "binding_map")]
    bindings: IndexMap<String, Value>,
    #[serde(default)]
    unset_bindings: Vec<String>,
    #[serde(default)]
    warnings: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Provenance {
    path: String,
    source: String,
    scope: String,
    origin: String,
    base_dir: Option<String>,
}
impl Provenance {
    fn record(self) -> SourceInfo {
        serde_json::from_value(json!({"path":self.path,"source":self.source,"scope":self.scope,"origin":self.origin,"baseDir":self.base_dir})).unwrap()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    value: String,
    label: String,
    description: Option<String>,
}
struct CompletionList(Option<Vec<Completion>>);
fn binding_map<'de, D: Deserializer<'de>>(d: D) -> Result<IndexMap<String, Value>, D::Error> {
    let v = serde_json::Map::<String, Value>::deserialize(d)?;
    Ok(v.into_iter().collect())
}
fn completions<'de, D: Deserializer<'de>>(d: D) -> Result<Option<CompletionList>, D::Error> {
    Option::<Vec<Completion>>::deserialize(d).map(|items| Some(CompletionList(items)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    op: String,
    #[serde(default)]
    owner: usize,
    name: Option<String>,
    key: Option<String>,
    description: Option<String>,
    tag: Option<String>,
    fail: Option<String>,
    error: Option<String>,
    #[serde(default, deserialize_with = "completions")]
    completions: Option<CompletionList>,
    r#type: Option<String>,
    default: Option<Value>,
    data: Option<Value>,
    args: Option<String>,
    ctx: Option<Value>,
    prefix: Option<String>,
    value: Option<Value>,
    message: Option<String>,
    #[serde(default)]
    retain: bool,
    #[serde(default, deserialize_with = "binding_map")]
    bindings: IndexMap<String, Value>,
    #[serde(default)]
    installed: bool,
    path: Option<String>,
    source: Option<String>,
    source_info: Option<Provenance>,
}
struct Replay {
    owners: ExtensionCollection<Value, Value>,
    apis: Vec<ExtensionAPI<Value, Value>>,
    runtime: Arc<ExtensionRuntimeState>,
    runner: Arc<ExtensionRunner<Value, Value>>,
    bus: EventBusController,
    trace: Arc<Mutex<Vec<Value>>>,
    warnings: Arc<Mutex<Vec<String>>>,
    observations: Vec<Value>,
    snapshot: Option<IndexMap<String, FlagValue>>,
    diagnostics: Option<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
    flag: Option<Arc<RwLock<ExtensionFlag>>>,
    command: Option<ResolvedCommand<Value>>,
    has_ui: bool,
}
fn value(v: Value) -> FlagValue {
    match v {
        Value::Bool(v) => FlagValue::Boolean(v),
        Value::String(v) => FlagValue::String(v),
        _ => panic!("invalid fixture flag value"),
    }
}
fn flag_value(v: &FlagValue) -> Value {
    match v {
        FlagValue::Boolean(v) => json!(v),
        FlagValue::String(v) => json!(v),
    }
}
fn values(v: &IndexMap<String, FlagValue>) -> Value {
    json!(
        v.iter()
            .map(|(k, v)| json!([k, flag_value(v)]))
            .collect::<Vec<_>>()
    )
}
fn bindings(v: &IndexMap<String, Value>, unset: &[String]) -> KeybindingsConfig {
    v.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                Some(match v {
                    Value::String(s) => KeybindingKeys::Single(s.clone()),
                    Value::Array(a) => KeybindingKeys::Multiple(
                        a.iter().map(|s| s.as_str().unwrap().to_owned()).collect(),
                    ),
                    _ => panic!("invalid fixture binding"),
                }),
            )
        })
        .chain(unset.iter().map(|k| (k.clone(), None)))
        .collect()
}
fn command_view(c: &ResolvedCommand<Value>) -> Value {
    let registration: &maestro_extensions::RegisteredCommand<Value> = c;
    let mut v = json!({"name":registration.name,"invocation_name":c.invocation_name,"source_info":*c.source_info.read().unwrap(),"has_completions":c.get_argument_completions.is_some()});
    if let Some(d) = &c.description {
        v["description"] = json!(d);
    }
    v
}
fn completion_view(c: &AutocompleteItem) -> Value {
    let mut v = json!({"value":c.value,"label":c.label});
    if let Some(d) = &c.description {
        v["description"] = json!(d);
    }
    v
}
fn assert_error(error: &EventError, expected: Option<&str>) {
    match expected {
        Some("default") => assert!(error.to_string().starts_with("This extension ctx is stale")),
        Some(expected) => assert_eq!(error.to_string(), expected),
        None => (),
    }
}
fn shortcut_view(key: &str, record: &maestro_extensions::ExtensionShortcut<Value>) -> Value {
    let mut v =
        json!({"key":key,"shortcut":record.shortcut,"extension_path":record.extension_path});
    if let Some(d) = &record.description {
        v["description"] = json!(d);
    }
    v
}
fn diagnostic_view(d: &Arc<RwLock<Vec<ResourceDiagnostic>>>) -> Value {
    json!(
        d.read()
            .unwrap()
            .iter()
            .map(|d| {
                assert_eq!(d.r#type, maestro_resources::DiagnosticType::Warning);
                assert!(d.collision.is_none());
                json!({"type":"warning","message":d.message,"path":d.path})
            })
            .collect::<Vec<_>>()
    )
}
fn assert_registration_locks_released(owners: &ExtensionCollection<Value, Value>) {
    let owners = owners.try_write().unwrap();
    for owner in owners.iter() {
        let owner = owner.try_write().unwrap();
        for shortcut in owner.shortcuts.values() {
            assert!(shortcut.try_write().is_ok());
        }
    }
}
impl Replay {
    fn new(sources: Vec<Provenance>) -> Self {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let warnings = Arc::new(Mutex::new(Vec::new()));
        let runtime = Arc::new(ExtensionRuntimeState::new());
        let bus = create_event_bus(
            Arc::new(|tail| {
                tokio::spawn(tail);
            }),
            Arc::new(|_| panic!("unexpected bus error")),
        );
        let events = bus.for_extension();
        let output = Arc::clone(&trace);
        events.on(
            "registration-probe",
            Arc::new(move |data, _| {
                output.lock().unwrap().push(json!({"event":*data}));
                Ok(None)
            }),
        );
        let owners: ExtensionCollection<Value, Value> = Arc::new(RwLock::new(Vec::new()));
        let output = Arc::clone(&warnings);
        let runner = Arc::new_cyclic(|runner: &std::sync::Weak<ExtensionRunner<Value, Value>>| {
            let runner = runner.clone();
            let owners = Arc::clone(&owners);
            ExtensionRunner::new(
                Arc::clone(&owners),
                Arc::clone(&runtime),
                Arc::new(move |s| {
                    let runner = runner.upgrade().unwrap();
                    assert!(runner.get_shortcut_diagnostics().try_write().is_ok());
                    assert_registration_locks_released(&owners);
                    assert!(s.ends_with('\n'));
                    output
                        .lock()
                        .unwrap()
                        .push(s.strip_suffix('\n').unwrap().to_owned());
                }),
            )
        });
        let mut r = Self {
            owners,
            apis: Vec::new(),
            runtime,
            runner,
            bus,
            trace,
            warnings,
            observations: Vec::new(),
            snapshot: None,
            diagnostics: None,
            flag: None,
            command: None,
            has_ui: false,
        };
        for (i, source) in sources.into_iter().enumerate() {
            r.add(source.record(), &format!("/resolved/{i}"));
        }
        r
    }
    fn add(&mut self, source: SourceInfo, resolved: &str) {
        let owner = Arc::new(RwLock::new(Extension::new(
            source.path.clone(),
            resolved.into(),
            Arc::new(RwLock::new(source)),
        )));
        assert_eq!(owner.read().unwrap().resolved_path, resolved);
        self.apis.push(ExtensionAPI::new(
            &owner,
            Arc::clone(&self.runtime),
            self.bus.for_extension(),
        ));
        self.owners.write().unwrap().push(owner);
    }
    async fn step(&mut self, s: Step) -> Result<(), EventError> {
        match s.op.as_str() {
            "command" => self.register_command(s),
            "shortcut" => self.register_shortcut(s),
            "flag" => self.register_flag(s),
            "emit_event" | "set" | "get" | "invalidate" | "list_commands" | "lookup" => {
                self.access(s).await
            }
            "invoke_command" | "complete" | "invoke_shortcut" => self.callbacks(s).await,
            "snapshot_flags"
            | "read_flag_snapshot"
            | "query_shortcuts"
            | "read_diagnostic_snapshot"
            | "ui"
            | "append_owner" => {
                self.snapshots(s);
                Ok(())
            }
            "clear_commands"
            | "retain_flag"
            | "edit_retained_flag"
            | "retain_command"
            | "edit_source"
            | "replace_source"
            | "read_retained_command"
            | "change_path" => {
                self.edits(s);
                Ok(())
            }
            _ => panic!("unknown fixture operation"),
        }
    }
    fn completion_callback(&self, list: CompletionList) -> ArgumentCompletions {
        let items = list.0.map(|items| {
            items
                .into_iter()
                .map(|c| AutocompleteItem {
                    value: c.value,
                    label: c.label,
                    description: c.description,
                })
                .collect::<Vec<_>>()
        });
        let trace = Arc::clone(&self.trace);
        Arc::new(move |prefix| {
            trace
                .lock()
                .unwrap()
                .push(json!({"completion_prefix":prefix}));
            let items = items.clone();
            Box::pin(async move { Ok(items) })
        })
    }
    fn register_command(&self, s: Step) -> Result<(), EventError> {
        let api = &self.apis[s.owner];

        let name = s.name.unwrap();
        let tag = s.tag.unwrap_or_else(|| name.clone());
        let trace = Arc::clone(&self.trace);
        let fail = s.fail;
        let get_argument_completions = s.completions.map(|items| self.completion_callback(items));
        api.register_command(
            name,
            RegisteredCommandOptions {
                description: s.description,
                get_argument_completions,
                handler: Arc::new(move |args, ctx| {
                    let trace = Arc::clone(&trace);
                    let tag = tag.clone();
                    let fail = fail.clone();
                    Box::pin(async move {
                        trace
                            .lock()
                            .unwrap()
                            .push(json!({"tag":tag,"args":args,"ctx":ctx}));
                        fail.map_or(Ok(()), |message| Err(message.into()))
                    })
                }),
            },
        )?;

        Ok(())
    }
    fn register_shortcut(&self, s: Step) -> Result<(), EventError> {
        let api = &self.apis[s.owner];

        let key = s.key.unwrap();
        let tag = s.tag.unwrap_or_else(|| key.clone());
        let trace = Arc::clone(&self.trace);
        let fail = s.fail;
        api.register_shortcut(
            key,
            ExtensionShortcutOptions {
                description: s.description,
                handler: Arc::new(move |ctx| {
                    let trace = Arc::clone(&trace);
                    let tag = tag.clone();
                    let fail = fail.clone();
                    Box::pin(async move {
                        trace.lock().unwrap().push(json!({"tag":tag,"ctx":ctx}));
                        fail.map_or(Ok(()), |message| Err(message.into()))
                    })
                }),
            },
        )?;

        Ok(())
    }
    fn register_flag(&self, s: Step) -> Result<(), EventError> {
        let api = &self.apis[s.owner];
        api.register_flag(
            s.name.unwrap(),
            ExtensionFlagOptions {
                description: s.description,
                r#type: match s.r#type.as_deref() {
                    Some("boolean") => FlagType::Boolean,
                    Some("string") => FlagType::String,
                    _ => panic!("invalid fixture flag kind"),
                },
                default: s.default.map(value),
            },
        )?;
        Ok(())
    }
    async fn access(&mut self, s: Step) -> Result<(), EventError> {
        let api = &self.apis[s.owner];
        match s.op.as_str() {
            "emit_event" => api.events.emit("registration-probe", s.data.unwrap()).await,
            "set" => self
                .runner
                .set_flag_value(s.name.unwrap(), value(s.value.unwrap())),
            "get" => {
                let v = api.get_flag(s.name.as_deref().unwrap())?;
                self.observations.push(v.as_ref().map_or_else(
                    || json!({"absent":true}),
                    |v| json!({"value":flag_value(v)}),
                ));
            }
            "invalidate" => self.runtime.invalidate(s.message),
            "list_commands" => {
                let previous = self.runner.get_command_diagnostics();
                previous.write().unwrap().push(ResourceDiagnostic {
                    r#type: maestro_resources::DiagnosticType::Warning,
                    message: "retained diagnostic".into(),
                    path: None,
                    collision: None,
                });
                self.observations.push(json!(
                    self.runner
                        .get_registered_commands()
                        .iter()
                        .map(command_view)
                        .collect::<Vec<_>>()
                ));
                let current = self.runner.get_command_diagnostics();
                assert!(!Arc::ptr_eq(&previous, &current));
                assert!(current.read().unwrap().is_empty());
                assert_eq!(previous.read().unwrap().len(), 1);
                assert_eq!(previous.read().unwrap()[0].message, "retained diagnostic");
            }
            "lookup" => self.observations.push(
                self.runner
                    .get_command(s.name.as_deref().unwrap())
                    .as_ref()
                    .map_or(Value::Null, command_view),
            ),
            _ => panic!("unknown fixture operation"),
        }
        Ok(())
    }
    async fn callbacks(&mut self, s: Step) -> Result<(), EventError> {
        match s.op.as_str() {
            "invoke_command" => {
                let c = self
                    .runner
                    .get_command(s.name.as_deref().unwrap())
                    .expect("promised command");
                (c.handler)(s.args.unwrap(), s.ctx.unwrap()).await?;
                self.observations.push(json!({"invoked":true}));
            }
            "complete" => {
                let c = self
                    .runner
                    .get_command(s.name.as_deref().unwrap())
                    .expect("promised command");
                let items = (c
                    .get_argument_completions
                    .as_ref()
                    .expect("promised completions"))(s.prefix.unwrap())
                .await?;
                self.observations.push(items.map_or(Value::Null, |items| {
                    json!(items.iter().map(completion_view).collect::<Vec<_>>())
                }));
            }
            "invoke_shortcut" => {
                let selected = self
                    .runner
                    .get_shortcuts(&bindings(&s.bindings, &[]), self.has_ui);
                let handler =
                    Arc::clone(&selected[s.key.as_deref().unwrap()].read().unwrap().handler);
                handler(s.ctx.unwrap()).await?;
                self.observations.push(json!({"invoked":true}));
            }
            _ => panic!("unknown fixture operation"),
        }
        Ok(())
    }
    fn snapshots(&mut self, s: Step) {
        match s.op.as_str() {
            "snapshot_flags" => {
                let mut v = self.runner.get_flag_values();
                v.insert("private-copy".into(), FlagValue::Boolean(false));
                self.snapshot = Some(v);
            }
            "read_flag_snapshot" => self
                .observations
                .push(values(self.snapshot.as_ref().unwrap())),
            "query_shortcuts" => {
                let selected = self
                    .runner
                    .get_shortcuts(&bindings(&s.bindings, &[]), self.has_ui);
                if s.retain {
                    self.diagnostics = Some(self.runner.get_shortcut_diagnostics());
                }
                self.observations.push(json!({"keys":selected.keys().collect::<Vec<_>>(),"diagnostics":diagnostic_view(&self.runner.get_shortcut_diagnostics())}));
            }
            "read_diagnostic_snapshot" => self
                .observations
                .push(diagnostic_view(self.diagnostics.as_ref().unwrap())),
            "ui" => self.has_ui = s.installed,
            "append_owner" => {
                let path = s.path.unwrap();
                self.add(serde_json::from_value(json!({"path":path,"source":"late","scope":"temporary","origin":"top-level"})).unwrap(),&path);
            }
            _ => panic!("unknown fixture operation"),
        }
    }
    fn edits(&mut self, s: Step) {
        let owner = Arc::clone(&self.owners.read().unwrap()[s.owner]);
        match s.op.as_str() {
            "clear_commands" => {
                let removed = std::mem::take(&mut owner.write().unwrap().commands);
                drop(removed);
            }
            "retain_flag" => {
                self.flag = Some(Arc::clone(
                    &self.runner.get_flags()[s.name.as_deref().unwrap()],
                ));
            }
            "edit_retained_flag" => {
                self.flag.as_ref().unwrap().write().unwrap().description = s.description;
            }
            "retain_command" => {
                self.command = Some(
                    self.runner
                        .get_command(s.name.as_deref().unwrap())
                        .expect("promised command"),
                );
            }
            "edit_source" => {
                owner.read().unwrap().source_info.write().unwrap().source = s.source.unwrap();
            }
            "replace_source" => {
                let mut owner = owner.write().unwrap();
                owner.source_info = Arc::new(RwLock::new(s.source_info.unwrap().record()));
                for c in owner.commands.values() {
                    c.write().unwrap().source_info = Arc::clone(&owner.source_info);
                }
            }
            "read_retained_command" => self
                .observations
                .push(command_view(self.command.as_ref().unwrap())),
            "change_path" => owner.write().unwrap().path = s.path.unwrap(),
            _ => panic!("unknown fixture operation"),
        }
    }
    fn observe(&self, field: &str, config: &KeybindingsConfig) -> Value {
        match field {
            "commands" => json!(self.runner.get_registered_commands().iter().map(command_view).collect::<Vec<_>>()),
            "command_diagnostics" => diagnostic_view(&self.runner.get_command_diagnostics()),
            "flags" => json!(self.runner.get_flags().iter().map(|(key,f)| {let f=f.read().unwrap();let mut v=json!({"key":key,"name":f.name,"type":match f.r#type{FlagType::Boolean=>"boolean",FlagType::String=>"string"},"extension_path":f.extension_path});if let Some(d)=&f.description{v["description"]=json!(d);}
if let Some(d)=&f.default{v["default"]=flag_value(d);}v}).collect::<Vec<_>>()),
            "flag_values" => values(&self.runner.get_flag_values()),
            "shortcuts" => json!(self.runner.get_shortcuts(config,self.has_ui).iter().map(|(key,s)| shortcut_view(key, &s.read().unwrap())).collect::<Vec<_>>()),
            "diagnostics" => diagnostic_view(&self.runner.get_shortcut_diagnostics()),
            "observations" => json!(self.observations), "trace" => json!(*self.trace.lock().unwrap()),"paths" => json!(self.runner.get_extension_paths()),
            _ => panic!("unknown fixture observation"),
        }
    }
}
pub fn run(group: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/registration_queries.json")).unwrap();
    let cases = cases
        .into_iter()
        .filter(|c| c.input.test == group)
        .collect::<Vec<_>>();
    assert!(!cases.is_empty());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for case in cases {
        runtime.block_on(run_case(case));
    }
}
async fn run_case(case: Case) {
    let input = case.input;
    let mut r = Replay::new(input.owners);
    for step in input.steps {
        let expected_error = step.error.clone();
        match r.step(step).await {
            Ok(()) => assert!(
                expected_error.is_none(),
                "{} missing promised error",
                input.id
            ),
            Err(error) => {
                assert_error(&error, expected_error.as_deref());
                r.observations.push(json!({"error":error.to_string()}));
            }
        }
    }
    let config = bindings(&input.bindings, &input.unset_bindings);
    let mut output = json!({});
    for field in input.observe {
        output[&field] = r.observe(&field, &config);
    }
    if input.warnings {
        output["warnings"] = json!(*r.warnings.lock().unwrap());
    }
    assert_eq!(output, case.expected, "{}", input.id);
    r.bus.clear();
}
