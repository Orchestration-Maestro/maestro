//! Canonical API imports reach the original loader runtime and application effects.
#![cfg(test)]
use maestro_extensions::{
    ExtensionActions, ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local,
    types::{ExtensionHandle, ProviderActions},
};
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Mutable state owned by the controlled application, not by the adapter.
struct State {
    /// Selected tool names retain duplicates and order.
    tools: Vec<String>,
    /// Recorded delivery and model-selection operands.
    effects: Vec<Value>,
    /// Current application reasoning policy.
    thinking: maestro_models::ModelThinkingLevel,
}
/// Original native session owner.
type History = Rc<RefCell<maestro_session::SessionManager>>;
/// Original application effect state.
type SharedState = Rc<RefCell<State>>;
/// Spawned native tails waiting for caller-controlled admission.
pub(super) type Tails = Rc<RefCell<Vec<std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>>>>;

/// Credential effects are observed without ambient environment or shell access.
pub(super) struct Credentials(pub(super) std::sync::Arc<std::sync::Mutex<Vec<String>>>);
impl maestro_credentials::ConfigValueOperations for Credentials {
    fn execute(&self, name: &str) -> Option<Vec<u8>> {
        assert_eq!(name, "CANONICAL_API_KEY");
        self.0.lock().unwrap().push(name.into());
        Some(b" literal controlled key ".to_vec())
    }
}

/// Executes ordinary API methods through the same component used for context reads.
pub(super) async fn observe(
    owner: &ExtensionHandle<Local>,
    runner: &Rc<ExtensionRunner<Local>>,
    events: &maestro_extensions::EventBus<Local>,
    history: &History,
    tails: &Tails,
) {
    let state = Rc::new(RefCell::new(State {
        tools: Vec::new(),
        effects: Vec::new(),
        thinking: maestro_models::ModelThinkingLevel::Off,
    }));
    let bus_observed = Rc::new(RefCell::new(Vec::new()));
    let observer = bus_observed.clone();
    let _subscription = events.on(
        "canonical-observer",
        Rc::new(move |value, _| {
            observer.borrow_mut().push((*value).clone());
            Ok(None)
        }),
    );
    let prepared = Rc::new(RefCell::new(Vec::new()));
    let observer = prepared.clone();
    let _prepare = events.on(
        "canonical-prepare",
        Rc::new(move |value, _| {
            observer.borrow_mut().push((*value).clone());
            Ok(None)
        }),
    );
    bind(runner, history, &state);
    let root = history.borrow().get_entries()[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let answer = handler(
        format!("api imports:{root}"),
        runner.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    let answer: Value =
        serde_json::from_str(&answer).unwrap_or_else(|error| panic!("{error}: {answer}"));
    assert_eq!(
        answer,
        json!({"name":"literal API name", "tools":[" z ","a","a"], "all":[tool()], "commands":[command()], "thinking":"high", "selected":false, "flag":true, "auth":true})
    );
    check_effects(history, &root, &state);
    check_flag(owner);
    assert_eq!(*bus_observed.borrow(), [json!({"present":true})]);
    assert_eq!(owner.read().unwrap().handlers["session_start"].len(), 1);
    assert_eq!(owner.read().unwrap().handlers["user_bash"].len(), 1);
    super::canonical_callbacks::observe(runner.clone()).await;
    super::canonical_json::observe(owner, runner, events).await;
    assert_eq!(*prepared.borrow(), [json!({"n":2.5})]);
    renderer(runner, events, tails).await;
}

/// Binds the original loader runtime to controlled session and credential owners.
fn bind(runner: &ExtensionRunner<Local>, history: &History, state: &SharedState) {
    let model = super::model_support::model("literal API model");
    runner
        .create_context()
        .model_registry()
        .unwrap()
        .auth_storage()
        .set(
            "fixture",
            maestro_credentials::AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
                env: None,
                key: "!CANONICAL_API_KEY".into(),
            }),
        );
    runner
        .bind_core(
            bindings(history, state),
            ExtensionContextActions {
                get_model: Rc::new(move || Ok(Some(model.clone()))),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Checks application state directly after the authored action sequence.
fn check_effects(history: &History, root: &str, state: &SharedState) {
    assert_eq!(
        history.borrow().get_session_name().unwrap(),
        Some("literal API name")
    );
    assert_eq!(
        history.borrow().get_label(root),
        Some(&json!(" literal API label "))
    );
    let entries = history
        .borrow()
        .get_entries()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let entry = entries
        .iter()
        .find(|entry| entry["customType"] == "canonical-api")
        .unwrap();
    assert_eq!(entry["data"], json!({"nested":[2.5,null]}));
    assert_eq!(
        state.borrow().effects,
        [
            json!(["model", "literal API model"]),
            json!(["custom", {"customType":"canonical-api", "content":" literal custom ", "display":false,"details":{"n":2.5}}, {"triggerTurn":false,"deliverAs":"nextTurn"}]),
            json!(["user", [{"type":"text", "text":" literal user "}], {"deliverAs":"followUp"}]),
        ]
    );
}

/// Inspects the actual registered default, not the getter's report alone.
fn check_flag(owner: &ExtensionHandle<Local>) {
    let flag = owner.read().unwrap().flags["canonical-api"].clone();
    let flag = flag.read().unwrap();
    assert_eq!(flag.description.as_deref(), Some(" literal flag "));
    assert_eq!(
        flag.default,
        Some(maestro_extensions::types::FlagValue::Boolean(false))
    );
}

/// Populated schema metadata remains semantic JSON through the authored `ToolInfo` alias.
fn tool() -> Value {
    json!({"name":" literal tool ","description":"literal description", "parameters":{"type":"object","properties":{"n":{"type":"number"}}},"sourceInfo":{"path":"literal tool source","source":"custom","scope":"project","origin":"package","baseDir":"literal base"}})
}

/// Command metadata retains open source vocabulary and literal names.
fn command() -> Value {
    json!({"name":" literal command ","description":"literal description", "source":"literal-source","sourceInfo":{"path":"literal command source","source":"custom","scope":"project","origin":"package","baseDir":"literal base"}})
}

/// Supplies real mutable session operations behind the original API runtime.
fn bindings(history: &History, state: &SharedState) -> ExtensionActions<Local> {
    let mut actions = Local::unbound_actions();
    let append = history.clone();
    actions.append_entry = Rc::new(move |name, data| {
        append.borrow_mut().append_custom_entry(
            &name,
            data.map_or(
                maestro_session::Presence::Missing,
                maestro_session::Presence::Present,
            ),
        )?;
        Ok(())
    });
    let set_name = history.clone();
    actions.set_session_name = Rc::new(move |name| {
        set_name.borrow_mut().append_session_info(&name)?;
        Ok(())
    });
    let get_name = history.clone();
    actions.get_session_name =
        Rc::new(move || Ok(get_name.borrow().get_session_name()?.map(str::to_owned)));
    let label = history.clone();
    actions.set_label = Rc::new(move |id, text| {
        label.borrow_mut().append_label_change(
            &id,
            text.as_deref().map_or(
                maestro_session::Presence::Null,
                maestro_session::Presence::Present,
            ),
        )?;
        Ok(())
    });
    inventory(&mut actions, state);
    deliveries(&mut actions, state);
    actions
}

/// Reads application inventories and observes actual state changes, not fixed replies.
fn inventory(actions: &mut ExtensionActions<Local>, state: &SharedState) {
    let get = state.clone();
    actions.get_active_tools = Rc::new(move || Ok(get.borrow().tools.clone()));
    let set = state.clone();
    actions.set_active_tools = Rc::new(move |names| {
        set.borrow_mut().tools = names;
        Ok(())
    });
    actions.get_all_tools = Rc::new(|| Ok(vec![serde_json::from_value(tool()).unwrap()]));
    actions.get_commands = Rc::new(|| Ok(vec![serde_json::from_value(command()).unwrap()]));
    let get = state.clone();
    actions.get_thinking_level = Rc::new(move || Ok(get.borrow().thinking.clone()));
    let set = state.clone();
    actions.set_thinking_level = Rc::new(move |level| {
        set.borrow_mut().thinking = level;
        Ok(())
    });
}

/// Captures exact native model and message operands at their application consumers.
fn deliveries(actions: &mut ExtensionActions<Local>, state: &SharedState) {
    let selected = state.clone();
    actions.set_model = Rc::new(move |model| {
        selected
            .borrow_mut()
            .effects
            .push(json!(["model", model.read().unwrap().id]));
        Box::pin(async { Ok(false) })
    });
    let custom = state.clone();
    actions.send_message = Rc::new(move |message, options| {
        custom
            .borrow_mut()
            .effects
            .push(json!(["custom", message, options]));
        Ok(())
    });
    let user = state.clone();
    actions.send_user_message = Rc::new(move |content, options| {
        user.borrow_mut()
            .effects
            .push(json!(["user", content, options]));
        Ok(())
    });
}

/// Proves actual registration and callback-frame effects in this same canonical test.
async fn renderer(
    runner: &ExtensionRunner<Local>,
    events: &maestro_extensions::EventBus<Local>,
    tails: &Tails,
) {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let received = trace.clone();
    let subscription = events.on(
        "trace",
        Rc::new(move |value, _| {
            received.borrow_mut().push((*value).clone());
            Ok(None)
        }),
    );
    let view = super::retained_components::view(runner, events, "canonical-api")
        .unwrap()
        .unwrap();
    assert_eq!(view.try_render(47).unwrap(), ["canonical-api:47"]);
    assert_eq!(
        *trace.borrow(),
        [json!("g1:canonical-api"), json!("g2:canonical-api")]
    );
    let _tail_view = super::retained_components::view(runner, events, "listener-tail")
        .unwrap()
        .unwrap();
    assert_eq!(
        *trace.borrow(),
        [
            json!("g1:canonical-api"),
            json!("g2:canonical-api"),
            json!("g1:listener-tail"),
            json!("g2:listener-tail")
        ]
    );
    let pending = std::mem::take(&mut *tails.borrow_mut());
    assert_eq!(pending.len(), 2);
    for tail in pending {
        tail.await;
    }
    assert_eq!(
        *trace.borrow(),
        [
            json!("g1:canonical-api"),
            json!("g2:canonical-api"),
            json!("g1:listener-tail"),
            json!("g2:listener-tail"),
            json!("g1:tail"),
            json!("g2:tail")
        ]
    );
    subscription.unsubscribe();
}
