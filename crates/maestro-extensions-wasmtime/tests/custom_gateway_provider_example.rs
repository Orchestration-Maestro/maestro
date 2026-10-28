//! The shipped gateway provider through the compiled component, the native catalog and
//! controlled GitLab and protocol endpoints.
#![cfg(test)]
use maestro_catalog::{ModelRegistry, ResolvedRequestAuth};
use maestro_credentials::AuthCredential;
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory, types::ProviderActions,
};
use maestro_extensions_wasmtime::native_io::{
    ChildProcess, FsWatcher, IoFuture, NativeIo, Readable, Request, Response, SpawnOptions,
};
use maestro_models::*;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

/// The model the probe asks for unless another is named.
const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";

/// What the controlled GitLab answers, and every request it was asked.
#[derive(Default)]
struct Gitlab {
    /// Statuses and texts, consumed in request order.
    replies: RefCell<VecDeque<(u16, String)>>,
    /// URL, method, headers and body of each request.
    requests: RefCell<Vec<Value>>,
}
/// One scripted HTTP response.
struct Reply(u16, String);
impl Response for Reply {
    fn json(&self) -> IoFuture<'_, Value> {
        Box::pin(async move { serde_json::from_str(&self.1).map_err(|error| error.to_string()) })
    }
    fn text(&self) -> IoFuture<'_, String> {
        Box::pin(async move { Ok(self.1.clone()) })
    }
    fn status(&self) -> u16 {
        self.0
    }
    fn status_text(&self) -> String {
        String::new()
    }
    fn headers(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    fn body(&self) -> Option<Rc<dyn Readable>> {
        None
    }
}
impl NativeIo for Gitlab {
    fn watch(&self, _: String, _: Rc<dyn Fn()>) -> Result<Rc<dyn FsWatcher>, String> {
        Err("the gateway watches no file".into())
    }
    fn fetch(&self, request: Request) -> IoFuture<'_, Rc<dyn Response>> {
        self.requests.borrow_mut().push(json!({
            "url": request.url, "method": request.method, "headers": request.headers,
            "body": request.body.map(|body| String::from_utf8(body).unwrap())
        }));
        Box::pin(async move {
            let (status, text) = self
                .replies
                .borrow_mut()
                .pop_front()
                .ok_or("no reply queued")?;
            Ok(Rc::new(Reply(status, text)) as Rc<dyn Response>)
        })
    }
    fn spawn(
        &self,
        _: String,
        _: Vec<String>,
        _: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>> {
        Box::pin(async { Err("the gateway starts no process".into()) })
    }
    fn kill(&self, _: i32, _: Option<String>) -> Result<(), String> {
        Err("the gateway signals no process".into())
    }
}

/// A built-in protocol replaced by a recording provider with a scripted reply.
#[derive(Default)]
struct Protocol {
    /// Model, context and simple options of each delegated request.
    calls: Mutex<Vec<Value>>,
    /// Whether the reply carries a reasoning block first.
    thinking: bool,
}
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Protocol
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let Some(StreamOptionsInput::Simple(options)) = call.options.map(Into::into) else {
            panic!("the gateway delegates through the simple route");
        };
        self.calls.lock().unwrap().push(json!({
            "model": call.model, "context": call.context, "common": options.common,
            "reasoning": options.reasoning, "budgets": options.thinking_budgets
        }));
        let stream = AssistantMessageEventStream::new();
        let mut content = vec![AssistantContent::Text(faux_text("Hello there friend"))];
        if self.thinking {
            content.insert(0, AssistantContent::Thinking(faux_thinking("greeting")));
        }
        let message = Arc::new(RwLock::new(AssistantMessage {
            api: call.model.api,
            ..faux_assistant_message(content, FauxAssistantMessageOptions::default())
        }));
        for event in scripted(&message, self.thinking) {
            stream.push(event);
        }
        Ok(stream)
    }
}

/// The ordered events a protocol emits for one reply, all sharing the live message.
fn scripted(message: &SharedAssistantMessage, thinking: bool) -> Vec<AssistantMessageEvent> {
    let partial = || Arc::clone(message);
    let mut events = vec![AssistantMessageEvent::Start { partial: partial() }];
    let mut index = 0;
    if thinking {
        events.extend([
            AssistantMessageEvent::ThinkingStart {
                content_index: 0,
                partial: partial(),
            },
            AssistantMessageEvent::ThinkingDelta {
                content_index: 0,
                delta: "greeting".into(),
                partial: partial(),
            },
            AssistantMessageEvent::ThinkingEnd {
                content_index: 0,
                content: "greeting".into(),
                partial: partial(),
            },
        ]);
        index = 1;
    }
    events.extend([
        AssistantMessageEvent::TextStart {
            content_index: index,
            partial: partial(),
        },
        AssistantMessageEvent::TextDelta {
            content_index: index,
            delta: "Hello there friend".into(),
            partial: partial(),
        },
        AssistantMessageEvent::TextEnd {
            content_index: index,
            content: "Hello there friend".into(),
            partial: partial(),
        },
        AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: Arc::clone(message),
        },
    ]);
    events
}

/// The compiled component over the native catalog, with controlled endpoints.
struct Harness {
    /// The catalog the component registered its provider in.
    registry: SharedHandle<ModelRegistry>,
    /// Keeps the loaded component and its event bus alive.
    _runner: ExtensionRunner<Local>,
    /// The controlled GitLab.
    gitlab: Rc<Gitlab>,
    /// The Anthropic protocol replacement.
    anthropic: Arc<Protocol>,
    /// The `OpenAI` Responses replacement.
    openai: Arc<Protocol>,
    /// Retires the event bus when the test ends.
    bus: maestro_extensions::EventBusController<Local>,
}
impl Harness {
    /// Loads the shipped component against the production host.
    async fn new(thinking: bool) -> Self {
        let (session, _) = super::session::runner_session("/gateway");
        let registry = session.model_registry.clone();
        let anthropic = replace_protocol("anthropic-messages", thinking);
        let openai = replace_protocol("openai-responses", thinking);
        let gitlab = Rc::new(Gitlab::default());
        let (runner, bus) = load(session, gitlab.clone()).await;
        Self {
            registry,
            _runner: runner,
            gitlab,
            anthropic,
            openai,
            bus,
        }
    }

    /// Queues what GitLab answers to the next request.
    fn reply(&self, status: u16, body: &Value) {
        let text = body
            .as_str()
            .map_or_else(|| body.to_string(), str::to_owned);
        self.gitlab.replies.borrow_mut().push_back((status, text));
    }

    /// The registered model, as the catalog describes it.
    fn model(&self, id: &str) -> Model {
        self.registry
            .find("gitlab-duo", id)
            .unwrap()
            .read()
            .unwrap()
            .clone()
    }

    /// The models the component registered, in catalog order.
    fn gateway_models(&self) -> Vec<Model> {
        let all = self.registry.get_all();
        let all = all.read().unwrap();
        all.iter()
            .map(|model| model.read().unwrap().clone())
            .filter(|model| model.provider == "gitlab-duo")
            .collect()
    }

    /// Streams one simple request for a registered model and drains it.
    async fn stream(&self, id: &str, options: SimpleStreamOptions<Local>) -> (Vec<Value>, Value) {
        self.stream_model(self.model(id), options).await
    }

    /// Streams one simple request through the catalog's registered protocol and drains it.
    async fn stream_model(
        &self,
        model: Model,
        options: SimpleStreamOptions<Local>,
    ) -> (Vec<Value>, Value) {
        let stream =
            stream_simple_for::<Local>(model, greeting(), Some(options), spawner()).unwrap();
        drain(&stream).await
    }
}

/// Replaces a built-in protocol with a recording provider.
fn replace_protocol(api: &str, thinking: bool) -> Arc<Protocol> {
    let protocol = Arc::new(Protocol {
        thinking,
        ..Protocol::default()
    });
    register_api_provider(
        ApiProvider {
            api: api.into(),
            stream: protocol.clone(),
            stream_simple: protocol.clone(),
        },
        Some(format!("gateway-{api}")),
    );
    protocol
}

/// Loads the shipped component over the production host, with GitLab's replies controlled.
async fn load(
    session: maestro_extensions::runner::ExtensionRunnerSession,
    gitlab: Rc<Gitlab>,
) -> (
    ExtensionRunner<Local>,
    maestro_extensions::EventBusController<Local>,
) {
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let runtime = create_extension_runtime::<Local>();
    let path = std::env::var("MAESTRO_GATEWAY_COMPONENT").expect("nextest prepares the gateway");
    let factory = maestro_extensions_wasmtime::artifact_factory_with_io(
        path.clone(),
        super::support::widgets(),
        maestro_extensions_wasmtime::Installation {
            version: "gateway-host".into(),
            agent_dir: "/controlled-agent".into(),
        },
        gitlab,
    );
    let owner = load_extension_from_factory(
        factory,
        "/gateway".into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        session,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    (runner, bus)
}

/// Runs a provider's producer on the test's local executor.
fn spawner() -> ProviderSpawner<Local> {
    Rc::new(|future| {
        tokio::task::spawn_local(future);
        true
    })
}

/// The probe's single request: a fixed greeting prompt.
fn greeting() -> Context {
    serde_json::from_value(json!({"messages": [{"role": "user", "content": "Say hello in exactly 3 words.", "timestamp": 1}]}))
        .unwrap()
}

/// Every event of a stream in order, then its final message.
async fn drain(stream: &AssistantMessageEventStream) -> (Vec<Value>, Value) {
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(serde_json::to_value(&event).unwrap());
    }
    (
        events,
        serde_json::to_value(&*stream.result().await.read().unwrap()).unwrap(),
    )
}

/// Stored sign-in that outlives every controlled run, in Unix epoch milliseconds.
const NEVER_EXPIRES: f64 = 4_102_444_800_000.0;

/// The headers GitLab hands out with a direct access token.
fn granted() -> Value {
    json!({"token": "direct-token", "headers": {"x-gitlab-realm": "saas", "x-gitlab-instance-id": "abc"}})
}

/// A request with an explicit key, as a caller passes the stored credential.
fn keyed(key: Option<&str>) -> SimpleStreamOptions<Local> {
    SimpleStreamOptions {
        common: StreamOptions {
            api_key: key.map(str::to_owned),
            max_tokens: Some(100.0),
            temperature: Some(0.5),
            headers: Some(
                [("x-caller".to_owned(), "dropped".to_owned())]
                    .into_iter()
                    .collect(),
            ),
            ..StreamOptions::default()
        },
        reasoning: Some(ThinkingLevel::Low),
        ..SimpleStreamOptions::default()
    }
}

/// The event types of one drained stream.
fn types(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .map(|event| event["type"].as_str().unwrap())
        .collect()
}

/// The Anthropic proxy address.
const ANTHROPIC_PROXY: &str = "https://cloud.gitlab.com/ai/v1/proxy/anthropic/";
/// The `OpenAI` Responses proxy address.
const OPENAI_PROXY: &str = "https://cloud.gitlab.com/ai/v1/proxy/openai/v1";

/// A gateway model: identifier, name, prices, context window, output limit, protocol, address.
type Row = (
    &'static str,
    &'static str,
    [f64; 4],
    f64,
    f64,
    &'static str,
    &'static str,
);

/// The six models the gateway serves.
const TABLE: [Row; 6] = [
    (
        "claude-opus-4-5-20251101",
        "Claude Opus 4.5",
        [15.0, 75.0, 1.5, 18.75],
        200_000.0,
        32_000.0,
        "anthropic-messages",
        ANTHROPIC_PROXY,
    ),
    (
        "claude-sonnet-4-5-20250929",
        "Claude Sonnet 4.5",
        [3.0, 15.0, 0.3, 3.75],
        200_000.0,
        16_384.0,
        "anthropic-messages",
        ANTHROPIC_PROXY,
    ),
    (
        "claude-haiku-4-5-20251001",
        "Claude Haiku 4.5",
        [1.0, 5.0, 0.1, 1.25],
        200_000.0,
        8_192.0,
        "anthropic-messages",
        ANTHROPIC_PROXY,
    ),
    (
        "gpt-5.1-2025-11-13",
        "GPT-5.1",
        [2.5, 10.0, 0.0, 0.0],
        128_000.0,
        16_384.0,
        "openai-responses",
        OPENAI_PROXY,
    ),
    (
        "gpt-5-mini-2025-08-07",
        "GPT-5 Mini",
        [0.15, 0.6, 0.0, 0.0],
        128_000.0,
        16_384.0,
        "openai-responses",
        OPENAI_PROXY,
    ),
    (
        "gpt-5-codex",
        "GPT-5 Codex",
        [2.5, 10.0, 0.0, 0.0],
        128_000.0,
        16_384.0,
        "openai-responses",
        OPENAI_PROXY,
    ),
];

#[test]
fn custom_gateway_provider_registers_the_reference_models_and_sign_in() {
    super::components::local(async {
        let harness = Harness::new(false).await;
        for (id, name, cost, window, max, ..) in TABLE {
            assert_eq!(
                serde_json::to_value(harness.model(id)).unwrap(),
                json!({
                    "id": id, "name": name, "api": "gitlab-duo-api", "provider": "gitlab-duo",
                    "baseUrl": "https://cloud.gitlab.com", "reasoning": true, "input": ["text", "image"],
                    "cost": {"input": cost[0], "output": cost[1], "cacheRead": cost[2], "cacheWrite": cost[3]},
                    "contextWindow": window, "maxTokens": max
                })
            );
        }
        assert_eq!(harness.gateway_models().len(), 6);
        let sign_in = get_oauth_provider("gitlab-duo").unwrap();
        assert_eq!((sign_in.id(), sign_in.name()), ("gitlab-duo", "GitLab Duo"));
        harness.bus.clear();
    });
}

/// The protocol call that carried one routed request, checked against its table row.
fn assert_delegated(call: &Value, row: &Row) {
    let (id, .., base_url) = *row;
    assert_eq!(
        (
            call["model"]["id"].as_str(),
            call["model"]["api"].as_str(),
            call["model"]["baseUrl"].as_str()
        ),
        (Some(id), Some("gitlab-duo-api"), Some(base_url))
    );
    assert_eq!(call["model"]["provider"], "gitlab-duo");
    assert_eq!(call["common"]["apiKey"], "gitlab-duo");
    assert_eq!(
        call["common"]["headers"],
        json!({"x-gitlab-realm": "saas", "x-gitlab-instance-id": "abc", "Authorization": "Bearer direct-token"})
    );
    assert_eq!(
        (
            &call["common"]["maxTokens"],
            &call["common"]["temperature"],
            &call["reasoning"]
        ),
        (&json!(100.0), &json!(0.5), &json!("low"))
    );
    assert_eq!(
        call["context"]["messages"][0]["content"],
        "Say hello in exactly 3 words."
    );
}

/// The one request GitLab received for every model.
fn assert_direct_access_request(request: &Value) {
    assert_eq!(
        request["url"],
        "https://gitlab.com/api/v4/ai/third_party_agents/direct_access"
    );
    assert_eq!(request["method"], "POST");
    assert_eq!(
        request["headers"],
        json!([
            ["Authorization", "Bearer glpat-user"],
            ["Content-Type", "application/json"]
        ])
    );
    assert_eq!(
        request["body"],
        r#"{"feature_flags":{"DuoAgentPlatformNext":true}}"#
    );
}

#[test]
fn custom_gateway_provider_routes_each_model_through_direct_access() {
    super::components::local(async {
        let harness = Harness::new(false).await;
        harness.reply(200, &granted());
        for row in &TABLE {
            let (events, result) = harness.stream(row.0, keyed(Some("glpat-user"))).await;
            let protocol = if row.5 == "anthropic-messages" {
                &harness.anthropic
            } else {
                &harness.openai
            };
            assert_delegated(protocol.calls.lock().unwrap().last().unwrap(), row);
            assert_eq!(
                types(&events),
                ["start", "text_start", "text_delta", "text_end", "done"]
            );
            for event in &events {
                let message = event
                    .get("partial")
                    .or_else(|| event.get("message"))
                    .unwrap();
                assert_eq!(message["api"], "gitlab-duo-api", "{}", row.0);
            }
            assert_eq!(result["api"], "gitlab-duo-api");
            assert_eq!(result["stopReason"], "stop");
            assert_eq!(
                result["content"],
                json!([{"type": "text", "text": "Hello there friend"}])
            );
        }
        assert_eq!(harness.anthropic.calls.lock().unwrap().len(), 3);
        assert_eq!(harness.openai.calls.lock().unwrap().len(), 3);
        let requests = harness.gitlab.requests.borrow();
        assert_eq!(
            requests.len(),
            1,
            "one direct access token serves every model"
        );
        assert_direct_access_request(&requests[0]);
        drop(requests);
        harness.bus.clear();
    });
}

#[test]
fn custom_gateway_provider_failures_arrive_as_error_messages() {
    super::components::local(async {
        let harness = Harness::new(false).await;
        for key in [None, Some("")] {
            let (events, result) = harness.stream(DEFAULT_MODEL, keyed(key)).await;
            assert_eq!(types(&events), ["error"]);
            assert_eq!(
                result["errorMessage"],
                "No GitLab access token. Run /login gitlab-duo or set GITLAB_TOKEN"
            );
        }
        let mut stranger = harness.model(DEFAULT_MODEL);
        stranger.id = "gpt-9".into();
        let (_, result) = harness
            .stream_model(stranger, keyed(Some("glpat-user")))
            .await;
        assert_eq!(result["errorMessage"], "Unknown model: gpt-9");
        assert!(harness.gitlab.requests.borrow().is_empty());
        harness.reply(403, &json!("forbidden"));
        harness.reply(500, &json!("boom"));
        for expected in [
            "GitLab Duo access denied. Ensure GitLab Duo is enabled for your account. Error: forbidden",
            "Failed to get direct access token: 500 boom",
        ] {
            let (events, result) = harness
                .stream(DEFAULT_MODEL, keyed(Some("glpat-user")))
                .await;
            assert_eq!(types(&events), ["error"]);
            assert_eq!(result["errorMessage"], expected);
            assert_eq!(result["stopReason"], "error");
            assert_eq!(result["content"], json!([]));
            assert_eq!(
                (&result["api"], &result["provider"], &result["model"]),
                (
                    &json!("gitlab-duo-api"),
                    &json!("gitlab-duo"),
                    &json!(DEFAULT_MODEL)
                )
            );
        }
        assert!(harness.anthropic.calls.lock().unwrap().is_empty());
        harness.bus.clear();
    });
}

#[test]
fn custom_gateway_provider_raw_requests_keep_open_reasoning_fields() {
    super::components::local(async {
        let harness = Harness::new(false).await;
        harness.reply(200, &granted());
        let options = ProviderStreamOptions::<Local> {
            common: StreamOptions {
                api_key: Some("glpat-user".into()),
                ..StreamOptions::default()
            },
            extra: serde_json::from_value(
                json!({"reasoning": "high", "thinkingBudgets": {"high": 19}, "other": 1}),
            )
            .unwrap(),
            ..ProviderStreamOptions::default()
        };
        let stream = stream_for::<Local>(
            harness.model(DEFAULT_MODEL),
            greeting(),
            Some(options),
            spawner(),
        )
        .unwrap();
        let (events, _) = drain(&stream).await;
        assert_eq!(types(&events).last(), Some(&"done"));
        let call = harness
            .anthropic
            .calls
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap();
        assert_eq!(
            (&call["reasoning"], &call["budgets"]),
            (&json!("high"), &json!({"high": 19.0}))
        );
        harness.bus.clear();
    });
}

/// Answers the callback prompt with the state the authorization address carried.
#[derive(Default)]
struct SignIn(Mutex<Vec<String>>);
impl OAuthLoginCallbacks for SignIn {
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError> {
        self.0.lock().unwrap().push(info.url);
        Ok(())
    }
    fn on_prompt(&self, prompt: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        assert_eq!(prompt.message, "Paste the callback URL:");
        let url = self.0.lock().unwrap().last().cloned().unwrap();
        let state = url.split("state=").nth(1).unwrap().to_owned();
        Box::pin(async move {
            Ok(format!(
                "http://127.0.0.1:8080/callback?code=granted&state={state}"
            ))
        })
    }
}

/// An unpadded URL-safe Base64 text of 32 bytes.
fn is_proof_text(text: &str) -> bool {
    text.len() == 43
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[test]
fn custom_gateway_provider_signs_in_and_refreshes_through_the_host() {
    super::components::local(async {
        let harness = Harness::new(false).await;
        harness.reply(200, &json!({"access_token": "access-1", "refresh_token": "refresh-1", "expires_in": 7200, "created_at": 1_700_000_000}));
        let sign_in = get_oauth_provider("gitlab-duo").unwrap();
        let callbacks = Arc::new(SignIn::default());
        let credentials = sign_in.login(callbacks.clone(), None).await.unwrap();
        let url = callbacks.0.lock().unwrap()[0].clone();
        let challenge = url
            .split("code_challenge=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        assert!(is_proof_text(challenge), "{url}");
        assert!(
            url.starts_with("https://gitlab.com/oauth/authorize?client_id=da4edff2"),
            "{url}"
        );
        let exchange = harness.gitlab.requests.borrow()[0].clone();
        let body = exchange["body"].as_str().unwrap();
        let verifier = body
            .split("code_verifier=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        assert!(is_proof_text(verifier), "{body}");
        assert_ne!(verifier, challenge);
        assert_eq!(
            (
                credentials.access.as_str(),
                credentials.refresh.as_str(),
                credentials.expires
            ),
            ("access-1", "refresh-1", 1_700_006_900_000.0)
        );
        harness.registry.auth_storage().set(
            "gitlab-duo",
            AuthCredential::OAuth(OAuthCredentials {
                expires: 1.0,
                ..credentials
            }),
        );
        harness.reply(200, &json!({"access_token": "access-2", "refresh_token": "refresh-2", "expires_in": 7200, "created_at": 1_700_000_000}));
        let model = harness.registry.find("gitlab-duo", DEFAULT_MODEL).unwrap();
        let ResolvedRequestAuth::Ok { api_key, .. } =
            harness.registry.get_api_key_and_headers(&model).await
        else {
            panic!("the stored sign-in refreshes");
        };
        assert_eq!(api_key.as_deref(), Some("access-2"));
        let refresh = harness.gitlab.requests.borrow()[1].clone();
        assert_eq!(
            refresh["body"],
            "client_id=da4edff2e6ebd2bc3208611e2768bc1c1dd7be791dc5ff26ca34ca9ee44f7d4b&grant_type=refresh_token&refresh_token=refresh-1"
        );
        harness.bus.clear();
    });
}

/// The standalone probe: one greeting from one model with the stored sign-in, printed as events arrive.
async fn probe(harness: &Harness, arguments: &[&str]) -> Result<Vec<String>, String> {
    let id = arguments
        .iter()
        .find(|argument| !argument.starts_with("--"))
        .copied()
        .unwrap_or(DEFAULT_MODEL);
    let Some(model) = harness.registry.find("gitlab-duo", id) else {
        let available: Vec<String> = harness
            .gateway_models()
            .into_iter()
            .map(|model| model.id)
            .collect();
        return Err(format!(
            "Unknown model: {id}\nAvailable: {}",
            available.join(", ")
        ));
    };
    let stored = harness
        .registry
        .auth_storage()
        .get_api_key("gitlab-duo", false, &**harness.registry.config_values())
        .await;
    let Ok(Some(api_key)) = stored else {
        return Err("No gitlab-duo credentials. Run /login gitlab-duo first.".into());
    };
    let options = SimpleStreamOptions {
        common: StreamOptions {
            api_key: Some(api_key),
            max_tokens: Some(100.0),
            ..StreamOptions::default()
        },
        reasoning: arguments
            .contains(&"--thinking")
            .then_some(ThinkingLevel::Low),
        ..SimpleStreamOptions::default()
    };
    let model = model.read().unwrap().clone();
    let (events, _) = harness.stream_model(model, options).await;
    Ok(events.iter().filter_map(printed).collect())
}

/// What the probe prints for one event, if anything.
fn printed(event: &Value) -> Option<String> {
    Some(match event["type"].as_str()? {
        "thinking_start" => "[Thinking]\n".to_owned(),
        "thinking_end" => "\n[/Thinking]\n\n".to_owned(),
        "thinking_delta" | "text_delta" => event["delta"].as_str()?.to_owned(),
        "error" => format!("\nError: {}\n", event["error"]["errorMessage"].as_str()?),
        "done" => format!("\n\nDone! {}\n", event["reason"].as_str()?),
        _ => return None,
    })
}

#[test]
fn custom_gateway_provider_probe_greets_with_the_stored_sign_in() {
    super::components::local(async {
        let harness = Harness::new(true).await;
        harness.reply(200, &granted());
        harness.registry.auth_storage().set(
            "gitlab-duo",
            AuthCredential::OAuth(OAuthCredentials {
                refresh: "refresh".into(),
                access: "stored-access".into(),
                expires: NEVER_EXPIRES,
                extra: JsonObject::default(),
            }),
        );
        assert_eq!(
            probe(&harness, &["gpt-5-codex"]).await.unwrap().concat(),
            "[Thinking]\ngreeting\n[/Thinking]\n\nHello there friend\n\nDone! stop\n"
        );
        let call = harness
            .openai
            .calls
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap();
        assert_eq!(
            (&call["common"]["maxTokens"], &call["reasoning"]),
            (&json!(100.0), &json!(null))
        );
        assert_eq!(
            harness.gitlab.requests.borrow()[0]["headers"][0],
            json!(["Authorization", "Bearer stored-access"])
        );
        probe(&harness, &["--thinking"]).await.unwrap();
        let call = harness
            .anthropic
            .calls
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap();
        assert_eq!(
            (&call["model"]["id"], &call["reasoning"]),
            (&json!(DEFAULT_MODEL), &json!("low"))
        );
        assert!(
            probe(&harness, &["gpt-9"])
                .await
                .unwrap_err()
                .starts_with("Unknown model: gpt-9\nAvailable: claude-opus-4-5-20251101, ")
        );
        harness.registry.auth_storage().remove("gitlab-duo");
        assert_eq!(
            probe(&harness, &[]).await.unwrap_err(),
            "No gitlab-duo credentials. Run /login gitlab-duo first."
        );
        harness.bus.clear();
    });
}
