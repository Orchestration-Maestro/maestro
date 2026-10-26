//! Session binding: request defaults, the event subscription and the editor callbacks.
use super::{BrowserSession, browser::Session};
use crate::{ModelSelector, components::message_editor::MessageEditorProps, dom::report};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::browser::utils::proxy_utils::create_local_stream_fn;
use maestro_app::{
    AgentEvent, AgentMessage, AgentSpawner, CustomAgentMessages, DiagnosticErrorInfo, Local, Model,
    ThinkingLevel, get_app_storage,
};
use std::{
    rc::{Rc, Weak},
    sync::{Arc, PoisonError, RwLock},
};

impl<C: CustomAgentMessages + 'static> Session<C> {
    /// Move the subscription to the current session, supplying its unset request defaults.
    pub(super) fn subscribe(self: &Rc<Self>) {
        self.unsubscribe();
        self.ended.set(false);
        let Some(session) = self.session() else {
            return;
        };
        supply_defaults(&session);
        if !self.root.is_connected() {
            return;
        }
        let weak = Rc::downgrade(self);
        let listener: maestro_app::AgentListener<BrowserMessages<C>, Local> =
            Rc::new(move |event, _| {
                if let Some(view) = weak.upgrade() {
                    let event = event.read().unwrap_or_else(PoisonError::into_inner);
                    view.observe(&event);
                }
                Box::pin(std::future::ready(Ok(())))
            });
        self.subscription.replace(Some(session.subscribe(listener)));
        let message = session
            .state()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .streaming_message()
            .cloned();
        report(self.stream.borrow().set_message(message, true));
    }
    /// End the current subscription.
    pub(super) fn unsubscribe(&self) {
        if let Some(subscription) = self.subscription.take() {
            subscription.unsubscribe();
        }
    }
    /// Show one event: completed messages leave the stream, updates stream on the next frame.
    fn observe(&self, event: &AgentEvent<BrowserMessages<C>>) {
        let cleared = match event {
            AgentEvent::MessageEnd { .. } => Some(None),
            AgentEvent::AgentEnd { .. } => {
                self.ended.set(true);
                self.stream.borrow_mut().is_streaming = false;
                Some(None)
            }
            AgentEvent::AgentStart => {
                self.ended.set(false);
                None
            }
            AgentEvent::MessageUpdate { message, .. } => {
                let streaming = self
                    .session()
                    .is_some_and(|session| session.state().read().is_ok_and(|s| s.is_streaming()));
                self.stream.borrow_mut().is_streaming = streaming;
                let message = AgentMessage::Assistant(Arc::clone(message));
                report(self.stream.borrow().set_message(Some(message), !streaming));
                None
            }
            AgentEvent::TurnStart
            | AgentEvent::TurnEnd { .. }
            | AgentEvent::MessageStart { .. } => None,
            _ => return,
        };
        if let Some(message) = cleared {
            report(self.stream.borrow().set_message(message, true));
        }
        report(self.render());
    }
    /// Give the editor this view's send, abort, model and level actions.
    pub(super) fn bind_editor(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let thinking = self.props.borrow().controls.thinking;
        let props = MessageEditorProps {
            on_send: Some(send_action(weak.clone())),
            on_abort: Some(abort_action(weak.clone())),
            on_model_select: Some(model_action(weak.clone())),
            on_thinking_change: thinking.then(|| level_change(weak.clone())),
            ..MessageEditorProps::default()
        };
        self.editor_props.replace(props);
    }
    /// Use the caller's model action, or pick a model with the built-in selector.
    fn choose_model(self: &Rc<Self>) {
        let custom = self.props.borrow().on_model_select.clone();
        if let Some(custom) = custom {
            custom();
            return;
        }
        let Some(session) = self.session() else {
            return;
        };
        let current = session
            .state()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .model
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let weak = Rc::downgrade(self);
        let chosen = Rc::new(move |model: Model| {
            session
                .state()
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .model = Arc::new(RwLock::new(model));
            if let Some(view) = weak.upgrade() {
                report(view.render());
            }
        });
        if let Err(error) = ModelSelector::open(&self.view, Some(current), chosen, None) {
            report(Err(error));
        }
    }
}
/// Send the editor's draft from the view while it exists.
fn send_action<C: CustomAgentMessages + 'static>(
    view: Weak<Session<C>>,
) -> crate::components::message_editor::SendCallback {
    Rc::new(move |input, attachments| {
        let Some(view) = view.upgrade() else {
            return;
        };
        wasm_bindgen_futures::spawn_local(async move {
            report(view.send(&input, attachments).await);
        });
    })
}
/// Abort the session's response while the view exists.
fn abort_action<C: CustomAgentMessages + 'static>(view: Weak<Session<C>>) -> Rc<dyn Fn()> {
    Rc::new(move || {
        if let Some(session) = view.upgrade().and_then(|view| view.session()) {
            session.abort();
        }
    })
}
/// Choose a model from the view while it exists.
fn model_action<C: CustomAgentMessages + 'static>(view: Weak<Session<C>>) -> Rc<dyn Fn()> {
    Rc::new(move || {
        if let Some(view) = view.upgrade() {
            view.choose_model();
        }
    })
}
/// Set the session's reasoning level while the view exists.
fn level_change<C: CustomAgentMessages + 'static>(
    view: Weak<Session<C>>,
) -> Rc<dyn Fn(ThinkingLevel)> {
    Rc::new(move |level| {
        if let Some(session) = view.upgrade().and_then(|view| view.session()) {
            session
                .state()
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .thinking_level = level;
        }
    })
}
/// Supply the browser's task spawner, proxy-aware stream and stored-key lookup where unset.
fn supply_defaults<C: CustomAgentMessages + 'static>(session: &BrowserSession<C>) {
    let spawn = {
        let mut spawn = session
            .spawn
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        spawn
            .get_or_insert_with(|| -> AgentSpawner<Local> {
                Rc::new(wasm_bindgen_futures::spawn_local)
            })
            .clone()
    };
    let mut stream = session
        .stream_fn
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    if stream.is_none() {
        *stream = Some(create_local_stream_fn(
            Rc::new(|| Box::pin(proxy_url())),
            spawn,
        ));
    }
    drop(stream);
    let mut key = session
        .get_api_key
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    if key.is_none() {
        *key = Some(Rc::new(|provider| {
            Box::pin(async move { Ok(stored_key(&provider).await) })
        }));
    }
}
/// The stored proxy address while the proxy is enabled.
async fn proxy_url() -> Result<Option<String>, DiagnosticErrorInfo> {
    let storage =
        get_app_storage().map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
    let enabled = storage
        .settings
        .get::<bool>("proxy.enabled")
        .await
        .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
    if enabled != Some(true) {
        return Ok(None);
    }
    storage
        .settings
        .get::<String>("proxy.url")
        .await
        .map(|url| url.filter(|url| !url.is_empty()))
        .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
}
/// The nonempty stored key for `provider`, else the key of the stored custom provider of that name.
pub(super) async fn stored_key(provider: &str) -> Option<String> {
    let storage = get_app_storage().ok()?;
    if let Some(key) = storage
        .provider_keys
        .get(provider)
        .await
        .ok()
        .flatten()
        .filter(|key| !key.is_empty())
    {
        return Some(key);
    }
    storage
        .custom_providers
        .get_all()
        .await
        .ok()?
        .into_iter()
        .find(|custom| custom.name == provider)
        .and_then(|custom| custom.api_key.present().cloned())
        .filter(|key| !key.is_empty())
}
