//! A supplied host that runs the phases around a replacement or reload.
#![cfg(test)]
use maestro_extensions::{
    CommandContextOperations, EventError, ExtensionCommandContextActions, ExtensionRunner,
    ForkPosition, Local, Presence, ReplacedSessionContext, ResourcesDiscoverReason,
    SessionBeforeForkEvent, SessionBeforeSwitchEvent, SessionBeforeSwitchReason,
    SessionShutdownEvent, SessionShutdownReason, SessionStartEvent, SessionStartReason,
    create_event_bus_for, create_extension_runtime,
    runner::emit_session_shutdown_event,
    types::{
        FlagValue, ForkOptions, NewSessionCommandOptions, SessionChangeResult,
        SwitchSessionOptions, WithSession,
    },
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// The channel the authored handlers announce their phases on.
const PHASE: &str = "retention-phase";

/// A point where the supplied host fails an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    /// Before any extension hears about the operation.
    Preparation,
    /// After the old extensions heard the shutdown.
    Shutdown,
    /// While the host releases the old user interface.
    Disposal,
    /// While the host activates the fresh generation.
    Activation,
    /// While the host prepares the new history.
    Setup,
    /// While the host runs the operation's continuation.
    Continuation,
}

impl Phase {
    /// The literal failure the host reports for this phase.
    pub(super) fn failure(self) -> String {
        format!("{} failed in the supplied host", self.label())
    }

    /// The name the host logs when it enters this phase.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Preparation => "preparation",
            Self::Shutdown => "shutdown",
            Self::Disposal => "disposal",
            Self::Activation => "activation",
            Self::Setup => "setup",
            Self::Continuation => "continuation",
        }
    }
}

/// The host a bound operation serves, which outlives every operation it supplied.
fn serving(host: &Weak<Host>) -> Rc<Host> {
    host.upgrade().expect("the host outlives its operations")
}

/// The ordered observations and controls of one supplied host.
pub(super) struct Host {
    /// Host phases and authored announcements, in the order they happened.
    pub(super) log: Rc<RefCell<Vec<Value>>>,
    /// Every generation this host served, oldest first.
    pub(super) generations: RefCell<Vec<Rc<ExtensionRunner<Local>>>>,
    /// The announcements and failures every generation reports.
    controller: maestro_extensions::EventBusController<Local>,
    /// The phase where the next operation fails, when a case injects one.
    pub(super) failing: Cell<Option<Phase>>,
}

impl Host {
    /// Loads the first generation, binds the host's operations and starts the log.
    pub(super) async fn new(cwd: &str) -> Rc<Self> {
        let log = Rc::new(RefCell::new(Vec::new()));
        let controller = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
        let announced = log.clone();
        drop(controller.for_extension().on(
            PHASE,
            Rc::new(move |data, _| {
                announced.borrow_mut().push((*data).clone());
                Ok(None)
            }),
        ));
        let host = Rc::new(Self {
            log,
            generations: RefCell::default(),
            controller,
            failing: Cell::new(None),
        });
        host.activate(cwd, &[]).await;
        host
    }

    /// The generation the host currently serves.
    pub(super) fn current(&self) -> Rc<ExtensionRunner<Local>> {
        self.generations
            .borrow()
            .last()
            .expect("a generation")
            .clone()
    }

    /// Records one host phase.
    fn phase(&self, name: &str) {
        self.log.borrow_mut().push(json!(name));
    }

    /// Fails with the phase's literal failure when the case injects it there.
    fn inject(&self, phase: Phase) -> Result<(), EventError> {
        if self.failing.get() == Some(phase) {
            return Err(phase.failure().into());
        }
        Ok(())
    }

    /// Logs the phase, then fails when the case injects a failure there.
    fn enter(&self, phase: Phase) -> Result<(), EventError> {
        self.phase(phase.label());
        self.inject(phase)
    }

    /// Loads a fresh generation over the captured flags, binds it and keeps it.
    async fn activate(self: &Rc<Self>, cwd: &str, flags: &[(String, FlagValue)]) {
        let runtime = create_extension_runtime::<Local>();
        for (name, value) in flags {
            runtime
                .flag_values
                .write()
                .unwrap()
                .insert(name.clone(), value.clone());
        }
        let session = super::session::runner_session(cwd).0;
        let (_, runner, _) = super::retained_components::loaded_with_runtime(
            self.controller.for_extension(),
            session,
            "/fixture",
            runtime,
        )
        .await;
        let observed = self.log.clone();
        drop(runner.on_error(Rc::new(move |error| {
            observed
                .borrow_mut()
                .push(json!({ "error": error.event, "message": error.error }));
            Ok(())
        })));
        runner.bind_command_context(Some(self.operations()));
        self.generations.borrow_mut().push(Rc::new(runner));
    }

    /// The command operations every generation binds; they hold the host weakly, so the
    /// generations the host keeps do not keep the host alive in turn.
    pub(super) fn operations(self: &Rc<Self>) -> ExtensionCommandContextActions {
        let weak = Rc::downgrade(self);
        let (new, fork, switch, reload) = (weak.clone(), weak.clone(), weak.clone(), weak);
        ExtensionCommandContextActions::new(CommandContextOperations {
            new_session: Rc::new(move |options| {
                let host = serving(&new);
                Box::pin(async move { host.new_session(options.unwrap_or_default()).await })
            }),
            fork: Rc::new(move |entry, options| {
                let host = serving(&fork);
                Box::pin(async move { host.fork(&entry, options.unwrap_or_default()).await })
            }),
            switch_session: Rc::new(move |path, options| {
                let host = serving(&switch);
                Box::pin(async move { host.switch(&path, options.unwrap_or_default()).await })
            }),
            reload: Rc::new(move || {
                let host = serving(&reload);
                Box::pin(async move { host.reload().await })
            }),
            ..super::context_actions::silent_command_operations()
        })
    }

    /// Asks the old extensions whether to go on; true when one vetoed.
    async fn vetoed<E: maestro_extensions::runner::RunnerEmitEvent>(
        &self,
        event: &mut E,
        cancelled: impl Fn(&E::Result) -> bool,
    ) -> Result<bool, EventError> {
        let runner = self.current();
        let result = runner.emit(event).await?;
        let vetoed = result.as_ref().is_some_and(cancelled);
        if vetoed {
            self.phase("vetoed");
        }
        Ok(vetoed)
    }

    /// Starts a new session unless an extension vetoes it.
    async fn new_session(
        self: &Rc<Self>,
        options: NewSessionCommandOptions,
    ) -> Result<SessionChangeResult, EventError> {
        let mut before = SessionBeforeSwitchEvent {
            reason: SessionBeforeSwitchReason::New,
            target_session_file: Presence::Missing,
        };
        if self
            .vetoed(&mut before, |result| {
                matches!(result.cancel, Presence::Present(true))
            })
            .await?
        {
            return Ok(SessionChangeResult { cancelled: true });
        }
        self.enter(Phase::Preparation)?;
        self.replace(
            SessionShutdownReason::New,
            Some("/sessions/new-target.jsonl"),
        )
        .await?;
        self.start(
            SessionStartReason::New,
            "/fresh-new",
            Some("/sessions/previous.jsonl"),
        )
        .await?;
        self.enter(Phase::Setup)?;
        if let Some(setup) = options.setup {
            let history = super::session::runner_session("/setup-target").1;
            setup(history).await?;
        }
        self.continue_with(options.with_session).await?;
        Ok(SessionChangeResult { cancelled: false })
    }

    /// Forks the session unless an extension vetoes it.
    async fn fork(
        self: &Rc<Self>,
        entry: &str,
        options: ForkOptions,
    ) -> Result<SessionChangeResult, EventError> {
        let mut before = SessionBeforeForkEvent {
            entry_id: entry.to_owned(),
            position: options.position.unwrap_or(ForkPosition::Before),
        };
        if self
            .vetoed(&mut before, |result| {
                matches!(result.cancel, Presence::Present(true))
            })
            .await?
        {
            return Ok(SessionChangeResult { cancelled: true });
        }
        self.enter(Phase::Preparation)?;
        self.replace(
            SessionShutdownReason::Fork,
            Some("/sessions/fork-target.jsonl"),
        )
        .await?;
        self.start(
            SessionStartReason::Fork,
            "/fresh-fork",
            Some("/sessions/previous.jsonl"),
        )
        .await?;
        self.continue_with(options.with_session).await?;
        Ok(SessionChangeResult { cancelled: false })
    }

    /// Resumes another session file unless an extension vetoes it.
    async fn switch(
        self: &Rc<Self>,
        path: &str,
        options: SwitchSessionOptions,
    ) -> Result<SessionChangeResult, EventError> {
        let mut before = SessionBeforeSwitchEvent {
            reason: SessionBeforeSwitchReason::Resume,
            target_session_file: Presence::Present(path.to_owned()),
        };
        if self
            .vetoed(&mut before, |result| {
                matches!(result.cancel, Presence::Present(true))
            })
            .await?
        {
            return Ok(SessionChangeResult { cancelled: true });
        }
        self.enter(Phase::Preparation)?;
        self.replace(SessionShutdownReason::Resume, Some(path))
            .await?;
        self.start(
            SessionStartReason::Resume,
            "/fresh-switch",
            Some("/sessions/previous.jsonl"),
        )
        .await?;
        self.continue_with(options.with_session).await?;
        Ok(SessionChangeResult { cancelled: false })
    }

    /// Rebuilds the extensions, keeping the flags, and resumes the calling frame.
    async fn reload(self: &Rc<Self>) -> Result<(), EventError> {
        let flags: Vec<_> = self.current().get_flag_values().into_iter().collect();
        self.phase("flags captured");
        self.shutdown(SessionShutdownReason::Reload, None).await?;
        self.phase("settings reloaded");
        self.phase("resources reloaded");
        self.phase("providers reset");
        self.retire();
        let fresh = self.fresh("/fresh-reload", &flags).await?;
        let mut start = SessionStartEvent {
            reason: SessionStartReason::Reload,
            previous_session_file: Presence::Missing,
        };
        fresh.emit(&mut start).await?;
        fresh
            .emit_resources_discover("/fresh-reload".into(), ResourcesDiscoverReason::Reload)
            .await?;
        Ok(())
    }

    /// Tells the serving generation it shuts down for `reason`.
    async fn shutdown(
        &self,
        reason: SessionShutdownReason,
        target: Option<&str>,
    ) -> Result<(), EventError> {
        let mut shutdown = SessionShutdownEvent {
            reason,
            target_session_file: target
                .map_or(Presence::Missing, |path| Presence::Present(path.to_owned())),
        };
        self.phase("shutdown");
        emit_session_shutdown_event(&self.current(), &mut shutdown).await?;
        self.inject(Phase::Shutdown)
    }

    /// Shuts the serving generation down, releases its interface and invalidates it.
    async fn replace(
        &self,
        reason: SessionShutdownReason,
        target: Option<&str>,
    ) -> Result<(), EventError> {
        self.shutdown(reason, target).await?;
        self.enter(Phase::Disposal)?;
        self.retire();
        Ok(())
    }

    /// Invalidates the serving generation's contexts.
    fn retire(&self) {
        self.phase("invalidated");
        self.current().invalidate(None);
    }

    /// Activates the fresh generation over the captured flags and binds it.
    async fn fresh(
        self: &Rc<Self>,
        cwd: &str,
        flags: &[(String, FlagValue)],
    ) -> Result<Rc<ExtensionRunner<Local>>, EventError> {
        self.enter(Phase::Activation)?;
        self.activate(cwd, flags).await;
        self.phase("bound");
        Ok(self.current())
    }

    /// Activates the fresh generation and tells it a session started.
    async fn start(
        self: &Rc<Self>,
        reason: SessionStartReason,
        cwd: &str,
        previous: Option<&str>,
    ) -> Result<(), EventError> {
        let fresh = self.fresh(cwd, &[]).await?;
        let mut start = SessionStartEvent {
            reason,
            previous_session_file: previous
                .map_or(Presence::Missing, |path| Presence::Present(path.to_owned())),
        };
        fresh.emit(&mut start).await?;
        Ok(())
    }

    /// Runs the operation's continuation against the fresh generation's own context.
    async fn continue_with(&self, with_session: Option<WithSession>) -> Result<(), EventError> {
        self.enter(Phase::Continuation)?;
        if let Some(with_session) = with_session {
            with_session(self.replaced()).await?;
        }
        Ok(())
    }

    /// The context a continuation receives for the generation the host now serves.
    fn replaced(&self) -> ReplacedSessionContext {
        ReplacedSessionContext::new(
            self.current().create_command_context(),
            Rc::new(|_, _| Box::pin(async { Ok(()) })),
            Rc::new(|_, _| Box::pin(async { Ok(()) })),
        )
    }
}
