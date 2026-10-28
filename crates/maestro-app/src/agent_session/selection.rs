//! Model, thinking-level and queue-mode selection.
use super::{
    AgentSession, AgentSessionEvent, CycleDirection, ModelCycleResult, decisions::SelectionError,
    state::thrown,
};
use crate::utils::lock::{read, write};
use maestro_agent::QueueMode;
use maestro_catalog::DEFAULT_THINKING_LEVEL;
use maestro_extensions::{ExtensionExecution, ModelSelectEvent, ThinkingLevelSelectEvent};
use maestro_models::{
    DiagnosticErrorInfo, Model, ModelThinkingLevel, clamp_thinking_level,
    extension_events::selection_events::ModelSelectSource, get_supported_thinking_levels,
    models_are_equal,
};
use maestro_session::Presence;
use maestro_settings::{
    MessageDeliveryMode, PreparedSettingsEdit, ThinkingLevel as SavedThinkingLevel,
};
use std::sync::{Arc, RwLock};

/// A model to cycle to with its explicit thinking level.
type Candidate = (Arc<RwLock<Model>>, Option<ModelThinkingLevel>);

impl<E: ExtensionExecution> AgentSession<E> {
    /// Selects `model`, records it in the session and the settings and re-clamps thinking.
    ///
    /// The `model_select` notification starts at once and finishes on the session's task
    /// facility, so a caller that stops waiting does not cancel it, while a caller that
    /// keeps waiting returns after the handlers finish.
    ///
    /// # Errors
    /// Returns `No API key for <provider>/<id>` without changes when the model has no
    /// configured credentials, or a credential, recording or listener failure.
    /// An admission refusal returns before state, history or settings change.
    pub async fn set_model(&self, model: Arc<RwLock<Model>>) -> Result<(), DiagnosticErrorInfo> {
        self.set_model_checked(model).await.map_err(Into::into)
    }

    /// Selects a model while keeping admission refusals distinct from operation failures.
    pub(super) async fn set_model_checked(
        &self,
        model: Arc<RwLock<Model>>,
    ) -> Result<(), SelectionError> {
        if !self
            .model_registry
            .has_configured_auth(&model)
            .map_err(thrown)?
        {
            let model = read(&model);
            return Err(DiagnosticErrorInfo::error(format!(
                "No API key for {}/{}",
                model.provider, model.id
            ))
            .into());
        }
        let previous = self.model();
        let level = self.thinking_level_for_model_switch(None);
        let edits = self.prepare_model_selection(&model, &level)?;
        self.apply_model(&model, level, edits)?;
        self.emit_model_select(model, previous, ModelSelectSource::Set)
            .await
            .map_err(Into::into)
    }

    /// Selects `model`, records it and its provider, then applies `level` clamped to it.
    fn apply_model(
        &self,
        model: &Arc<RwLock<Model>>,
        level: ModelThinkingLevel,
        edits: PreparedSettingsEdit,
    ) -> Result<(), DiagnosticErrorInfo> {
        let (model_edits, thinking_edits) = edits.split_at(2);
        write(self.agent.state()).model = Arc::clone(model);
        let (provider, id) = {
            let model = read(model);
            (model.provider.clone(), model.id.clone())
        };
        self.session_manager
            .borrow_mut()
            .append_model_change(&provider, &id)
            .map_err(thrown)?;
        self.settings_manager
            .borrow_mut()
            .apply_prepared_edit(model_edits);
        self.apply_thinking_level(level, thinking_edits)
    }

    /// Selects the next or previous scoped model, or else the next or previous available
    /// model; `None` when fewer than two are candidates.
    ///
    /// The `model_select` notification runs as [`set_model`](Self::set_model)'s does.
    ///
    /// # Errors
    /// Returns an admission refusal before state, history or settings change,
    /// or a credential, recording or listener failure.
    pub async fn cycle_model(
        &self,
        direction: CycleDirection,
    ) -> Result<Option<ModelCycleResult>, DiagnosticErrorInfo> {
        let (mut candidates, is_scoped) = self.cycle_candidates()?;
        if candidates.len() <= 1 {
            return Ok(None);
        }
        let current = self.model();
        let index = candidates
            .iter()
            .position(|(model, _)| same_model(model, &current))
            .unwrap_or(0);
        let next = match direction {
            CycleDirection::Forward => (index + 1) % candidates.len(),
            CycleDirection::Backward => (index + candidates.len() - 1) % candidates.len(),
        };
        let (model, explicit) = candidates.swap_remove(next);
        let level = self.thinking_level_for_model_switch(explicit);
        let edits = self
            .prepare_model_selection(&model, &level)
            .map_err(DiagnosticErrorInfo::from)?;
        self.apply_model(&model, level, edits)?;
        self.emit_model_select(Arc::clone(&model), current, ModelSelectSource::Cycle)
            .await?;
        let thinking_level = read(self.agent.state()).thinking_level.clone();
        Ok(Some(ModelCycleResult {
            model,
            thinking_level,
            is_scoped,
        }))
    }

    /// The authenticated scoped models with their explicit levels, or else every available
    /// model, and whether they are the scoped ones.
    fn cycle_candidates(&self) -> Result<(Vec<Candidate>, bool), DiagnosticErrorInfo> {
        let state = self.state.borrow();
        if state.scoped_models.is_empty() {
            drop(state);
            let available = self.model_registry.get_available().map_err(thrown)?;
            return Ok((
                available.into_iter().map(|model| (model, None)).collect(),
                false,
            ));
        }
        let mut authenticated = Vec::new();
        for scoped in &state.scoped_models {
            if self
                .model_registry
                .has_configured_auth(&scoped.model)
                .map_err(thrown)?
            {
                authenticated.push((Arc::clone(&scoped.model), scoped.thinking_level.clone()));
            }
        }
        Ok((authenticated, true))
    }

    /// Applies `level`, clamped to the model; records, saves and announces an effective change.
    ///
    /// # Errors
    /// Returns an admission refusal before state, history or settings change,
    /// or a recording failure after the agent state changed.
    pub fn set_thinking_level(&self, level: ModelThinkingLevel) -> Result<(), DiagnosticErrorInfo> {
        let edits = self.prepare_thinking_selection(&level)?;
        self.apply_thinking_level(level, edits)
    }

    /// Applies an admitted level with the original history, settings and event ordering.
    fn apply_thinking_level(
        &self,
        level: ModelThinkingLevel,
        edits: PreparedSettingsEdit,
    ) -> Result<(), DiagnosticErrorInfo> {
        let effective = if self.get_available_thinking_levels().contains(&level) {
            level
        } else {
            clamp_thinking_level(&read(&self.model()), level)
        };
        let previous = std::mem::replace(
            &mut write(self.agent.state()).thinking_level,
            effective.clone(),
        );
        if effective == previous {
            return Ok(());
        }
        let saved = saved_level(&effective);
        self.session_manager
            .borrow_mut()
            .append_thinking_level_change(saved.as_str())
            .map_err(thrown)?;
        self.settings_manager
            .borrow_mut()
            .apply_prepared_edit(edits);
        self.emit(&AgentSessionEvent::ThinkingLevelChanged {
            level: effective.clone(),
        });
        self.emit_thinking_level_select(ThinkingLevelSelectEvent {
            level: effective,
            previous_level: previous,
        });
        Ok(())
    }

    /// Selects the next supported level; `None` when the model cannot reason.
    ///
    /// # Errors
    /// Returns an admission refusal before state, history or settings change,
    /// or a recording failure after the agent state changed.
    pub fn cycle_thinking_level(&self) -> Result<Option<ModelThinkingLevel>, DiagnosticErrorInfo> {
        if !self.supports_thinking() {
            return Ok(None);
        }
        let levels = self.get_available_thinking_levels();
        let current = read(self.agent.state()).thinking_level.clone();
        let next = levels
            .iter()
            .position(|level| *level == current)
            .map_or(0, |index| (index + 1) % levels.len());
        let Some(level) = levels.get(next).cloned() else {
            return Ok(None);
        };
        self.set_thinking_level(level.clone())?;
        Ok(Some(level))
    }

    /// The levels the selected model supports.
    #[must_use]
    pub fn get_available_thinking_levels(&self) -> Vec<ModelThinkingLevel> {
        get_supported_thinking_levels(&read(&self.model()))
    }

    /// Whether the selected model can reason.
    #[must_use]
    pub fn supports_thinking(&self) -> bool {
        read(&self.model()).reasoning
    }

    /// Sets and saves how steering messages are delivered.
    ///
    /// # Errors
    /// Returns the publisher's refusal before changing live delivery or preferences.
    pub fn set_steering_mode(&self, mode: QueueMode) -> Result<(), DiagnosticErrorInfo> {
        self.agent.set_steering_mode(mode);
        self.settings_manager
            .borrow_mut()
            .set_steering_mode(delivery_mode(mode));
        Ok(())
    }

    /// Sets and saves how follow-up messages are delivered.
    ///
    /// # Errors
    /// Returns the publisher's refusal before changing live delivery or preferences.
    pub fn set_follow_up_mode(&self, mode: QueueMode) -> Result<(), DiagnosticErrorInfo> {
        self.agent.set_follow_up_mode(mode);
        self.settings_manager
            .borrow_mut()
            .set_follow_up_mode(delivery_mode(mode));
        Ok(())
    }

    /// Announces a model change to extensions, unless the identity is unchanged.
    async fn emit_model_select(
        &self,
        next: Arc<RwLock<Model>>,
        previous: Arc<RwLock<Model>>,
        source: ModelSelectSource,
    ) -> Result<(), DiagnosticErrorInfo> {
        if same_model(&previous, &next) {
            return Ok(());
        }
        let mut event = ModelSelectEvent {
            model: next,
            previous_model: Presence::Present(previous),
            source,
        };
        let runner = self.extension_runner();
        self.start_operation(
            async move { runner.emit(&mut event).await.map_err(thrown).map(|_| ()) },
        )
        .await
        .map_err(thrown)?
    }

    /// Starts the selection notification now and lets it finish on the session's task
    /// facility.
    fn emit_thinking_level_select(&self, mut event: ThinkingLevelSelectEvent) {
        let runner = self.extension_runner();
        drop(self.start_operation(async move {
            drop(runner.emit(&mut event).await);
        }));
    }

    /// The level a model switch starts from: the explicit one, else the saved or default
    /// level when the current model cannot reason, else the current level.
    fn thinking_level_for_model_switch(
        &self,
        explicit: Option<ModelThinkingLevel>,
    ) -> ModelThinkingLevel {
        if let Some(level) = explicit {
            return level;
        }
        if !self.supports_thinking() {
            return self
                .settings_manager
                .borrow()
                .get_default_thinking_level()
                .map_or(DEFAULT_THINKING_LEVEL, |level| model_level(&level));
        }
        read(self.agent.state()).thinking_level.clone()
    }
}

/// Whether two shared models have one identity, reading each lock once.
fn same_model(a: &Arc<RwLock<Model>>, b: &Arc<RwLock<Model>>) -> bool {
    Arc::ptr_eq(a, b) || models_are_equal(Some(&read(a)), Some(&read(b)))
}

/// The saved form of a queue mode.
#[must_use]
pub fn delivery_mode(mode: QueueMode) -> MessageDeliveryMode {
    match mode {
        QueueMode::All => MessageDeliveryMode::All,
        QueueMode::OneAtATime => MessageDeliveryMode::OneAtATime,
    }
}

/// The saved form of a level.
pub(crate) fn saved_level(level: &ModelThinkingLevel) -> SavedThinkingLevel {
    match level {
        ModelThinkingLevel::Off => SavedThinkingLevel::Off,
        ModelThinkingLevel::Minimal => SavedThinkingLevel::Minimal,
        ModelThinkingLevel::Low => SavedThinkingLevel::Low,
        ModelThinkingLevel::Medium => SavedThinkingLevel::Medium,
        ModelThinkingLevel::High => SavedThinkingLevel::High,
        ModelThinkingLevel::Xhigh => SavedThinkingLevel::Xhigh,
    }
}

/// A saved level; an unrecognized one is off, which clamping turns into the model's first
/// supported level.
pub(crate) fn model_level(level: &SavedThinkingLevel) -> ModelThinkingLevel {
    level.as_str().parse().unwrap_or(ModelThinkingLevel::Off)
}
