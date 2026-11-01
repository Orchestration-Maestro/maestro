//! The agent a created session runs on: its message conversion, provider requests and the
//! extension hooks around them.
use super::conversion::convert_to_llm_with_block_images;
use super::creation::Services;
use crate::agent_session::{ExtensionRunnerRef, local_spawner, thrown};
use crate::configuration::telemetry::is_install_telemetry_enabled;
use crate::utils::lock::read;
use indexmap::IndexMap;
use maestro_agent::{
    Agent, AgentExecution, AgentInitialState, AgentOptions, AgentSpawner, QueueMode,
};
use maestro_catalog::ResolvedRequestAuth;
use maestro_extensions::{AfterProviderResponseEvent, ExtensionRunner};
use maestro_models::{
    DiagnosticErrorInfo, Local, Model, ModelThinkingLevel, PayloadHookInput, ThinkingBudgets,
    Transport,
};
use maestro_session::{
    SessionManager, messages::CustomAgentMessages, session_manager::ReadonlySessionManager,
};
use maestro_settings::{MessageDeliveryMode, SettingsManager, TransportSetting};
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};
use url::Url;

/// The agent over `model` at `thinking_level`, converting messages with images withheld when
/// the settings block them, requesting through the catalog's credentials and the provider
/// retry settings, and running the extensions' request, response and context hooks.
pub(super) fn build_agent(
    services: &Services,
    session_manager: &SessionManager,
    runner_ref: &ExtensionRunnerRef,
    model: Option<Arc<RwLock<Model>>>,
    thinking_level: ModelThinkingLevel,
) -> Rc<Agent<CustomAgentMessages, Local>> {
    let settings = services.settings_manager.borrow();
    let spawn = local_spawner();
    let converting = Rc::clone(&services.settings_manager);
    let retry = settings.get_provider_retry_settings();
    Rc::new(Agent::new_local(AgentOptions {
        initial_state: AgentInitialState {
            system_prompt: Some(String::new()),
            model,
            thinking_level: Some(thinking_level),
            ..AgentInitialState::default()
        },
        convert_to_llm: Some(Rc::new(move |history| {
            let block_images = converting.borrow().get_block_images();
            let messages = convert_to_llm_with_block_images(&read(&history), block_images);
            Box::pin(std::future::ready(Ok(messages)))
        })),
        stream_fn: Some(stream_fn(services, &spawn)),
        on_payload: Some(on_payload(runner_ref)),
        on_response: Some(on_response(runner_ref)),
        session_id: Some(session_manager.get_session_id().to_owned()),
        transform_context: Some(transform_context(runner_ref)),
        steering_mode: Some(queue_mode(&settings.get_steering_mode())),
        follow_up_mode: Some(queue_mode(&settings.get_follow_up_mode())),
        transport: transport(&settings.get_transport()),
        thinking_budgets: settings.get_thinking_budgets().map(|budgets| {
            Arc::new(RwLock::new(ThinkingBudgets {
                minimal: budgets.minimal(),
                low: budgets.low(),
                medium: budgets.medium(),
                high: budgets.high(),
            }))
        }),
        max_retry_delay_ms: Some(retry.max_retry_delay_ms),
        spawn: Some(spawn),
        ..AgentOptions::default()
    }))
}

/// The provider request: the catalog's key, endpoint and headers, the provider retry settings and then
/// the HTTP idle timeout where the request sets none, and the attribution, credential and
/// request headers merged in that order.
fn stream_fn(services: &Services, spawn: &AgentSpawner<Local>) -> maestro_agent::StreamFn<Local> {
    let registry = Arc::clone(&services.model_registry);
    let settings = Rc::clone(&services.settings_manager);
    let submission = Local::provider_submission(Rc::clone(spawn));
    Rc::new(move |model, context, mut options, _open| {
        let registry = Arc::clone(&registry);
        let settings = Rc::clone(&settings);
        let submission = submission.clone();
        Box::pin(async move {
            let shared = Arc::new(RwLock::new(model.clone()));
            let (api_key, auth_headers, base_url) =
                match registry.get_api_key_and_headers(&shared).await {
                    ResolvedRequestAuth::Ok {
                        api_key,
                        headers,
                        base_url,
                    } => (api_key, headers, base_url),
                    ResolvedRequestAuth::Error { error } => {
                        return Err(DiagnosticErrorInfo::error(error));
                    }
                };
            let (retry, idle_timeout, attribution) = {
                let settings = settings.borrow();
                (
                    settings.get_provider_retry_settings(),
                    request_timeout(&settings),
                    get_attribution_headers(
                        &model,
                        &settings,
                        options.common.session_id.as_deref(),
                    ),
                )
            };
            let common = &mut options.common;
            common.api_key = api_key;
            common.base_url = base_url;
            common.timeout_ms = common
                .timeout_ms
                .or(retry.timeout_ms)
                .or(Some(idle_timeout));
            common.max_retries = common.max_retries.or(retry.max_retries);
            common.max_retry_delay_ms =
                common.max_retry_delay_ms.or(Some(retry.max_retry_delay_ms));
            common.headers = merge_headers([attribution, auth_headers, common.headers.take()]);
            maestro_models::stream_simple_for::<Local>(model, context, Some(options), submission)
        })
    })
}

/// The request timeout the HTTP idle timeout setting supplies, in milliseconds. The setting's
/// zero disables the limit, which a provider timeout of zero would instead make immediate, so
/// it asks for the largest timeout the request carries, `i32::MAX` milliseconds.
fn request_timeout(settings: &SettingsManager) -> f64 {
    let idle = settings.get_http_idle_timeout_ms();
    if idle > 0.0 {
        idle
    } else {
        f64::from(i32::MAX)
    }
}

/// The headers of each present map, later ones replacing earlier values; absent when none
/// is present.
fn merge_headers<const N: usize>(
    maps: [Option<IndexMap<String, String>>; N],
) -> Option<IndexMap<String, String>> {
    maps.into_iter().flatten().reduce(|mut merged, next| {
        merged.extend(next);
        merged
    })
}

/// Report whether `base_url` parses to the host `host`; a path or query that merely names it
/// does not match.
fn has_host(base_url: &str, host: &str) -> bool {
    Url::parse(base_url).is_ok_and(|url| url.host_str() == Some(host))
}

/// The session headers `OpenCode` requests carry, and the attribution headers `OpenRouter`,
/// NVIDIA and Cloudflare requests carry while install telemetry is on, matched by provider
/// or by the base URL's host.
fn get_attribution_headers(
    model: &Model,
    settings: &SettingsManager,
    session_id: Option<&str>,
) -> Option<IndexMap<String, String>> {
    let telemetry = std::env::var("MAESTRO_TELEMETRY").ok();
    let attribution = is_install_telemetry_enabled(settings, telemetry.as_deref())
        .then(|| attribution_pairs(model))
        .flatten()
        .unwrap_or_default();
    let opencode = matches!(model.provider.as_str(), "opencode" | "opencode-go")
        || has_host(&model.base_url, "opencode.ai");
    let session = session_id
        .filter(|_| opencode)
        .into_iter()
        .flat_map(|id| [("x-opencode-session", id), ("x-opencode-client", "maestro")]);
    let headers: IndexMap<String, String> = session
        .chain(attribution.iter().copied())
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
    (!headers.is_empty()).then_some(headers)
}

/// The telemetry attribution headers of `model`'s provider or host, when it has any.
fn attribution_pairs(model: &Model) -> Option<&'static [(&'static str, &'static str)]> {
    if model.provider == "openrouter" || has_host(&model.base_url, "openrouter.ai") {
        return Some(&[
            (
                "HTTP-Referer",
                "https://github.com/Orchestration-Maestro/maestro",
            ),
            ("X-OpenRouter-Title", "maestro"),
            ("X-OpenRouter-Categories", "cli-agent"),
        ]);
    }
    if model.provider == "nvidia" || has_host(&model.base_url, "integrate.api.nvidia.com") {
        return Some(&[("X-BILLING-INVOKE-ORIGIN", "maestro")]);
    }
    let cloudflare = matches!(
        model.provider.as_str(),
        "cloudflare-workers-ai" | "cloudflare-ai-gateway"
    ) || has_host(&model.base_url, "api.cloudflare.com")
        || has_host(&model.base_url, "gateway.ai.cloudflare.com");
    cloudflare.then_some(&[("User-Agent", "maestro-coding-agent")])
}

/// The current runner, when it handles `event_type`.
fn handling(
    runner_ref: &ExtensionRunnerRef,
    event_type: &str,
) -> Option<Rc<ExtensionRunner<Local>>> {
    runner_ref
        .borrow()
        .clone()
        .filter(|runner| runner.has_handlers(event_type))
}

/// Lets the extensions replace each provider payload before it is sent.
fn on_payload(runner_ref: &ExtensionRunnerRef) -> maestro_models::OnPayload<Local> {
    let runner_ref = Rc::clone(runner_ref);
    Rc::new(move |input: PayloadHookInput, _model| {
        let runner = handling(&runner_ref, "before_provider_request");
        Box::pin(async move {
            let Some(runner) = runner else {
                return Ok(input);
            };
            let payload = runner
                .emit_before_provider_request(input.payload)
                .await
                .map_err(thrown)?;
            Ok(PayloadHookInput {
                payload,
                signal: input.signal,
            })
        })
    })
}

/// Tells the extensions each provider response's status and headers.
fn on_response(runner_ref: &ExtensionRunnerRef) -> maestro_models::OnResponse<Local> {
    let runner_ref = Rc::clone(runner_ref);
    Rc::new(move |response, _model| {
        let runner = handling(&runner_ref, "after_provider_response");
        Box::pin(async move {
            if let Some(runner) = runner {
                let mut event = AfterProviderResponseEvent {
                    status: response.status,
                    headers: response.headers,
                };
                runner.emit(&mut event).await.map_err(thrown)?;
            }
            Ok(())
        })
    })
}

/// Lets the extensions rewrite the messages of each request.
pub fn transform_context(
    runner_ref: &ExtensionRunnerRef,
) -> Rc<<Local as AgentExecution>::Transform<CustomAgentMessages>> {
    let runner_ref = Rc::clone(runner_ref);
    Rc::new(move |messages, _signal| {
        let runner = runner_ref.borrow().clone();
        Box::pin(async move {
            let Some(runner) = runner else {
                return Ok(messages);
            };
            let copied = read(&messages).clone();
            let transformed = runner.emit_context(&copied).await.map_err(thrown)?;
            Ok(Arc::new(RwLock::new(transformed)))
        })
    })
}

/// The agent's form of a saved delivery mode.
#[must_use]
pub fn queue_mode(mode: &MessageDeliveryMode) -> QueueMode {
    match mode {
        MessageDeliveryMode::All => QueueMode::All,
        _ => QueueMode::OneAtATime,
    }
}

/// The agent's form of a saved transport; an unrecognized one leaves the agent's default.
fn transport(setting: &TransportSetting) -> Option<Transport> {
    match setting {
        TransportSetting::Sse => Some(Transport::Sse),
        TransportSetting::Websocket => Some(Transport::Websocket),
        TransportSetting::WebsocketCached => Some(Transport::WebsocketCached),
        TransportSetting::Auto => Some(Transport::Auto),
        TransportSetting::Unknown(_) => None,
    }
}
