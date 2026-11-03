//! Session-aware page operations serviced through one CDP dispatcher.

use super::{assets::Cache, connection::Dispatcher, inputs::ExpectedError, scripts};
use chromiumoxide::{
    cdp::browser_protocol::{browser::BrowserContextId, target::SessionId},
    types::{CallId, CdpJsonEventMessage},
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io,
};

/// A default-world execution context, including isolated-frame routing.
#[derive(Clone, Debug)]
pub(super) struct Context {
    /// Renderer session owning this context.
    pub session: SessionId,
    /// Runtime context identifier within that renderer.
    pub id: i64,
    /// Native frame identifier.
    pub frame: String,
}

/// Host calls waiting for attached renderers to finish their setup.
#[derive(Default)]
struct Deferral {
    /// Attached renderers still awaiting their setup answers.
    initializing: HashSet<SessionId>,
    /// Host calls received during that setup or while earlier ones are released, in arrival order.
    calls: VecDeque<CdpJsonEventMessage>,
}

/// One fresh page's retained state.
pub struct Page<'a> {
    /// Shared browser transport, also used for attached iframe targets.
    pub(super) dispatcher: &'a mut Dispatcher,
    /// Root renderer session.
    pub(super) session: SessionId,
    /// Root and automatically attached sessions owned by this page.
    sessions: HashSet<SessionId>,
    /// Host calls held back while attached renderers finish their setup.
    deferral: Deferral,
    /// Disposable browser context providing fresh cookies and storage for this page.
    context: BrowserContextId,
    /// Root target identifier used for cleanup.
    pub(super) target: String,
    /// Main document frame identifier after initialization.
    main_frame: Option<String>,
    /// Current export's diagnostic name.
    pub(super) name: String,
    /// Default execution contexts by frame identifier.
    pub(super) contexts: HashMap<String, Context>,
    /// Frame URLs used to report the origin of native bindings.
    pub(super) urls: HashMap<String, String>,
    /// Exactly matching designated policy errors observed on this page.
    pub(super) policy_errors: usize,
    /// Parsed policy exception admitted for the designated export.
    expected_error: &'a ExpectedError,
    /// Optional verified report assets and request-confinement observations.
    pub(super) cache: Option<Cache>,
    /// Whether native target teardown is expected.
    closing: bool,
    /// Ordered user-facing diagnostic output.
    pub(super) lines: &'a mut Vec<String>,
}

impl<'a> Page<'a> {
    /// Create an isolated fresh target before navigating to any fixture.
    pub(super) async fn open(
        dispatcher: &'a mut Dispatcher,
        name: &str,
        lines: &'a mut Vec<String>,
        expected_error: &'a ExpectedError,
    ) -> io::Result<Self> {
        let created = raw(
            dispatcher,
            None,
            "Target.createBrowserContext",
            json!({"disposeOnDetach":true}),
        )
        .await?;
        let context = BrowserContextId::from(string(&created, "browserContextId")?.to_owned());
        let result = async {
            let created = raw(
                dispatcher,
                None,
                "Target.createTarget",
                json!({"url":"about:blank","browserContextId":context}),
            )
            .await?;
            let target = string(&created, "targetId")?.to_owned();
            let attached = raw(
                dispatcher,
                None,
                "Target.attachToTarget",
                json!({"targetId":target,"flatten":true}),
            )
            .await?;
            let session = SessionId::from(string(&attached, "sessionId")?.to_owned());
            Ok((target, session))
        }
        .await;
        match result {
            Ok((target, session)) => Ok(Self {
                dispatcher,
                sessions: HashSet::from([session.clone()]),
                deferral: Deferral::default(),
                session,
                context,
                target,
                main_frame: None,
                name: name.to_owned(),
                contexts: HashMap::new(),
                urls: HashMap::new(),
                policy_errors: 0,
                expected_error,
                cache: None,
                closing: false,
                lines,
            }),
            Err(error) => {
                let disposed = raw(
                    dispatcher,
                    None,
                    "Target.disposeBrowserContext",
                    json!({"browserContextId":context}),
                )
                .await
                .map(|_| ());
                super::cleanup(Err(error), disposed)
            }
        }
    }

    /// Enable events, bindings and automatic attachment before running any page script.
    ///
    /// # Errors
    /// Returns renderer setup or event failures.
    pub async fn initialize(&mut self) -> io::Result<()> {
        let session = self.session.clone();
        self.initialize_session(&session).await?;
        let tree = self
            .command(Some(&session), "Page.getFrameTree", json!({}))
            .await?;
        self.main_frame = Some(string(&tree["frameTree"]["frame"], "id")?.to_owned());
        self.command(
            Some(&session),
            "Emulation.setDeviceMetricsOverride",
            json!({"width":1280,"height":720,"deviceScaleFactor":1.25,"mobile":false}),
        )
        .await?;
        if self.name == "tile_dom_remove_does_not_open_and_uses_current_callback" {
            self.command(
                Some(&session),
                "Emulation.setTouchEmulationEnabled",
                json!({"enabled":true}),
            )
            .await?;
        }
        Ok(())
    }

    /// Install the same binding boundary in a page or an isolated child renderer.
    pub(super) async fn initialize_session(&mut self, session: &SessionId) -> io::Result<()> {
        for method in ["Page.enable", "Runtime.enable", "Inspector.enable"] {
            self.command(Some(session), method, json!({})).await?;
        }
        self.command(
            Some(session),
            "Runtime.addBinding",
            json!({"name":"__maestroDriver"}),
        )
        .await?;
        self.command(
            Some(session),
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source":scripts::BINDINGS}),
        )
        .await?;
        self.command(
            Some(session),
            "Runtime.evaluate",
            json!({"expression":scripts::BINDINGS}),
        )
        .await?;
        self.command(
            Some(session),
            "Target.setAutoAttach",
            json!({"autoAttach":true,"waitForDebuggerOnStart":true,"flatten":true}),
        )
        .await?;
        Ok(())
    }

    /// Navigate and wait for this document's own complete-state signal.
    ///
    /// # Errors
    /// Returns navigation, page or disconnection failures.
    pub async fn navigate(&mut self, url: &str) -> io::Result<()> {
        let session = self.session.clone();
        let response = self
            .command(Some(&session), "Page.navigate", json!({"url":url}))
            .await?;
        if let Some(error) = response.get("errorText") {
            return Err(io::Error::other(format!("navigation failed: {error}")));
        }
        self.evaluate("new Promise(resolve => document.readyState === 'complete' ? resolve() : addEventListener('load', resolve, {once:true}))").await?;
        self.phase("page loaded");
        Ok(())
    }

    /// The root default context remains the owner of page-level host bindings.
    pub(super) fn root_context(&self) -> io::Result<Context> {
        self.main_frame
            .as_ref()
            .and_then(|frame| self.contexts.get(frame))
            .cloned()
            .ok_or_else(|| io::Error::other("root page context unavailable"))
    }

    /// Evaluate in the root default context, retaining promise and exception semantics.
    ///
    /// # Errors
    /// Returns JavaScript exceptions, native binding or renderer failures.
    pub async fn evaluate(&mut self, expression: &str) -> io::Result<Value> {
        let session = self.session.clone();
        let response = self
            .command(
                Some(&session),
                "Runtime.evaluate",
                json!({"expression":expression,"awaitPromise":true,"returnByValue":true}),
            )
            .await?;
        evaluation(&response)
    }

    /// Evaluate in the selected default context rather than a containing renderer.
    pub(super) async fn evaluate_in(
        &mut self,
        context: &Context,
        expression: &str,
        by_value: bool,
    ) -> io::Result<Value> {
        let response = self.command(Some(&context.session), "Runtime.evaluate", json!({"expression":expression,"contextId":context.id,"awaitPromise":true,"returnByValue":by_value})).await?;
        if response.get("exceptionDetails").is_some() {
            return evaluation(&response);
        }
        Ok(if by_value {
            response["result"]["value"].clone()
        } else {
            response["result"].clone()
        })
    }

    /// Execute a native protocol operation in this page's root renderer.
    ///
    /// # Errors
    /// Returns protocol rejection, page, binding or disconnection failures.
    pub async fn execute(&mut self, method: &'static str, params: Value) -> io::Result<Value> {
        let session = self.session.as_ref().to_owned();
        self.send(Some(&session), method, params).await
    }

    /// Execute an operation on the browser or an explicitly attached renderer session.
    ///
    /// # Errors
    /// Returns protocol rejection, renderer, binding or disconnection failures.
    pub async fn send(
        &mut self,
        session: Option<&str>,
        method: &'static str,
        params: Value,
    ) -> io::Result<Value> {
        let session = session.map(|session| SessionId::from(session.to_owned()));
        self.command(session.as_ref(), method, params).await
    }

    /// Close the target and verify exactly-once policy-error accounting.
    ///
    /// # Errors
    /// Returns target-close failures, outstanding page failures or policy-count differences.
    pub async fn close(mut self) -> io::Result<()> {
        self.closing = true;
        let closed = self
            .command(None, "Target.closeTarget", json!({"targetId":self.target}))
            .await
            .map(|_| ());
        let disposed = self
            .command(
                None,
                "Target.disposeBrowserContext",
                json!({"browserContextId":self.context}),
            )
            .await
            .map(|_| ());
        super::cleanup(closed, disposed)?;
        let expected = if self.name == self.expected_error.export {
            self.expected_error.count
        } else {
            0
        };
        if self.policy_errors != expected {
            return Err(io::Error::other(format!(
                "{}: expected policy error count {expected}, observed {}",
                self.name, self.policy_errors
            )));
        }
        if expected != 0 {
            self.log(&format!("POLICY {} {expected}/{expected}", self.name));
        }
        Ok(())
    }

    /// Submit and correlate while servicing binding and renderer-completion events.
    pub(super) async fn command(
        &mut self,
        session: Option<&SessionId>,
        method: &'static str,
        params: Value,
    ) -> io::Result<Value> {
        let renderer = session.filter(|session| self.sessions.contains(*session));
        let context = params["contextId"]
            .as_i64()
            .filter(|id| renderer.is_some_and(|session| self.context_exists(session, *id)));
        let id = self.dispatcher.submit(session, method, params)?;
        self.response(id, renderer, context).await
    }

    /// Wait on the connection itself; nested host calls may finish before this response.
    async fn response(
        &mut self,
        id: CallId,
        renderer: Option<&SessionId>,
        context: Option<i64>,
    ) -> io::Result<Value> {
        loop {
            if let Some(response) = self.dispatcher.response(id) {
                return response;
            }
            if let Some(renderer) = renderer
                && (!self.sessions.contains(renderer)
                    || context.is_some_and(|id| !self.context_exists(renderer, id)))
            {
                return Err(io::Error::other("evaluation renderer was removed"));
            }
            if let Some(event) = self.dispatcher.next().await? {
                Box::pin(self.event(event)).await?;
            }
        }
    }

    /// Route one browser event to its actual renderer session.
    async fn event(&mut self, event: CdpJsonEventMessage) -> io::Result<()> {
        if !self.owns_event(&event) {
            return Ok(());
        }
        match event.method.as_ref() {
            "Runtime.executionContextCreated" => self.context_created(&event)?,
            "Runtime.executionContextDestroyed" => {
                let id = event.params["executionContextId"].as_i64();
                self.contexts.retain(|_, context| {
                    Some(context.id) != id
                        || Some(context.session.as_ref()) != event.session_id.as_deref()
                });
            }
            "Runtime.executionContextsCleared" => self
                .contexts
                .retain(|_, context| Some(context.session.as_ref()) != event.session_id.as_deref()),
            "Page.frameNavigated" => {
                self.urls.insert(
                    string(&event.params["frame"], "id")?.into(),
                    string(&event.params["frame"], "url")?.into(),
                );
            }
            "Target.attachedToTarget" => self.attached(&event.params).await?,
            "Target.detachedFromTarget" => self.detached(string(&event.params, "sessionId")?)?,
            "Inspector.targetCrashed" | "Target.targetCrashed" => {
                return Err(io::Error::other(format!("page crashed: {}", self.name)));
            }
            "Inspector.detached" => self.detached(
                event
                    .session_id
                    .as_deref()
                    .ok_or_else(|| io::Error::other("detached inspector without session"))?,
            )?,
            "Runtime.exceptionThrown" => self.exception(&event.params)?,
            "Runtime.consoleAPICalled" => self.console(&event.params)?,
            "Runtime.bindingCalled"
                if !self.deferral.initializing.is_empty() || !self.deferral.calls.is_empty() =>
            {
                self.deferral.calls.push_back(event);
            }
            "Runtime.bindingCalled" => self.binding(&event).await?,
            "Fetch.requestPaused" => self.route(&event.params).await?,
            _ => {}
        }
        Ok(())
    }

    /// Ignore retired or unrelated sessions without losing browser-scoped detach events.
    fn owns_event(&self, event: &CdpJsonEventMessage) -> bool {
        let known = |session: &str| self.sessions.contains(&SessionId::from(session.to_owned()));
        match event.session_id.as_deref() {
            Some(session) => known(session),
            None => {
                event.method.as_ref() == "Target.detachedFromTarget"
                    && event.params["sessionId"].as_str().is_some_and(known)
            }
        }
    }

    /// Track an automatically attached renderer, then release the host calls its setup deferred.
    async fn attached(&mut self, params: &Value) -> io::Result<()> {
        let session = SessionId::from(string(params, "sessionId")?.to_owned());
        self.sessions.insert(session.clone());
        self.deferral.initializing.insert(session.clone());
        let ready = self.attach_session(&session, params).await;
        self.deferral.initializing.remove(&session);
        ready?;
        while self.deferral.initializing.is_empty()
            && let Some(event) = self.deferral.calls.pop_front()
        {
            self.binding(&event).await?;
        }
        Ok(())
    }

    /// Enable iframe events even when scripts are already running, then resume the renderer.
    async fn attach_session(&mut self, session: &SessionId, params: &Value) -> io::Result<()> {
        if params["targetInfo"]["type"] == "iframe" {
            self.initialize_session(session).await?;
        }
        self.command(Some(session), "Runtime.runIfWaitingForDebugger", json!({}))
            .await
            .map(|_| ())
    }

    /// Retire one renderer without making an unrelated frame's removal fail the page.
    fn detached(&mut self, session: &str) -> io::Result<()> {
        if self.session.as_ref() == session && !self.closing {
            return Err(io::Error::other("page closed unexpectedly"));
        }
        self.sessions.retain(|owned| owned.as_ref() != session);
        self.contexts
            .retain(|_, context| context.session.as_ref() != session);
        Ok(())
    }

    /// Match the default world within its renderer, whose identifiers may overlap others.
    fn context_exists(&self, session: &SessionId, id: i64) -> bool {
        self.contexts
            .values()
            .any(|context| context.session == *session && context.id == id)
    }

    /// Remember only default-world contexts, not developer-tool isolated worlds.
    fn context_created(&mut self, event: &CdpJsonEventMessage) -> io::Result<()> {
        let value = &event.params["context"];
        if value["auxData"]["isDefault"] != true {
            return Ok(());
        }
        let frame = string(&value["auxData"], "frameId")?.to_owned();
        let session = event
            .session_id
            .as_ref()
            .ok_or_else(|| io::Error::other("context without session"))?;
        self.contexts.insert(
            frame.clone(),
            Context {
                session: SessionId::from(session.clone()),
                id: value["id"]
                    .as_i64()
                    .ok_or_else(|| io::Error::other("context identifier"))?,
                frame,
            },
        );
        Ok(())
    }

    /// Accept the one designated srcdoc policy error and fail every other page error.
    fn exception(&mut self, params: &Value) -> io::Result<()> {
        let details = &params["exceptionDetails"];
        let exception = &details["exception"];
        let description = exception["description"]
            .as_str()
            .unwrap_or("page exception");
        let source = details["url"] == self.expected_error.source
            || details["stackTrace"]["callFrames"]
                .as_array()
                .is_some_and(|frames| {
                    frames
                        .iter()
                        .any(|frame| frame["url"] == self.expected_error.source)
                });
        if self.name == self.expected_error.export
            && exception["className"] == self.expected_error.name
            && description.lines().next()
                == Some(
                    format!(
                        "{}: {}",
                        self.expected_error.name, self.expected_error.message
                    )
                    .as_str(),
                )
            && source
        {
            self.policy_errors += 1;
            return Ok(());
        }
        Err(io::Error::other(format!("{}: {description}", self.name)))
    }

    /// Preserve browser console observations, treating runtime panics as failures.
    fn console(&mut self, params: &Value) -> io::Result<()> {
        let text = params["args"]
            .as_array()
            .map(|args| {
                args.iter()
                    .map(|arg| {
                        arg["value"]
                            .as_str()
                            .map_or_else(|| arg["value"].to_string(), str::to_owned)
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        self.log(&text);
        if text.contains("panicked at") {
            return Err(io::Error::other(format!("{}: {text}", self.name)));
        }
        Ok(())
    }

    /// Record a page-specific completion phase.
    pub(super) fn phase(&mut self, message: &str) {
        self.log(&format!("PHASE {} {message}", self.name));
    }

    /// Emit the same ordered observations to the command and its caller.
    pub(super) fn log(&mut self, message: &str) {
        println!("{message}");
        self.lines.push(message.to_owned());
    }
}

/// Send a browser-scoped operation outside any page lifetime.
pub(super) async fn raw(
    dispatcher: &mut Dispatcher,
    session: Option<&SessionId>,
    method: &'static str,
    params: Value,
) -> io::Result<Value> {
    let id = dispatcher.submit(session, method, params)?;
    loop {
        if let Some(response) = dispatcher.response(id) {
            return response;
        }
        // No target is running authored code while page creation or teardown owns the connection.
        dispatcher.next().await?;
    }
}

/// Require one string in a browser protocol record.
pub(super) fn string<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other(format!("missing CDP {key}")))
}

/// Preserve synchronous and asynchronous evaluation failures without losing their description.
fn evaluation(response: &Value) -> io::Result<Value> {
    if let Some(details) = response.get("exceptionDetails") {
        return Err(io::Error::other(
            details["exception"]["description"]
                .as_str()
                .or_else(|| details["text"].as_str())
                .unwrap_or("browser evaluation failed")
                .to_owned(),
        ));
    }
    Ok(response["result"]["value"].clone())
}
