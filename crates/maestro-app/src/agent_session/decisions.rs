//! Admission of session selections before their state and persistence effects.

use super::{AgentSession, selection::saved_level};
use crate::utils::lock::read;
use maestro_extensions::ExtensionExecution;
use maestro_models::{DiagnosticErrorInfo, Model, ModelThinkingLevel, clamp_thinking_level};
use maestro_settings::{DecisionRequest, PreparedSettingsEdit, SettingsChange, SettingsScope};
use serde_json::json;
use std::sync::{Arc, RwLock};

/// Distinguishes an admitted-operation refusal from a later storage or listener failure.
pub(super) const REFUSAL_NAME: &str = "ConfigurationRefusal";

/// A refused operation in the existing diagnostic representation.
fn refusal(message: String) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::new(Some(REFUSAL_NAME.into()), message)
}

/// A derived top-level preference value.
fn preference(field: &str, value: serde_json::Value) -> SettingsChange {
    SettingsChange {
        path: vec![field.into()],
        value: Some(value),
    }
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// Admits a model and its derived saves before the first active-state change.
    pub(super) fn prepare_model_selection(
        &self,
        model: &Arc<RwLock<Model>>,
        requested: &ModelThinkingLevel,
    ) -> Result<PreparedSettingsEdit, DiagnosticErrorInfo> {
        let model = read(model);
        let mut changes = vec![
            preference("defaultProvider", json!(model.provider)),
            preference("defaultModel", json!(model.id)),
        ];
        let effective = clamp_thinking_level(&model, requested.clone());
        let settings = self.settings_manager.borrow();
        if !settings.is_key_path_locked("defaultThinkingLevel", &[]) {
            changes.extend(self.thinking_preferences(&effective, model.reasoning));
        }
        let saved = saved_level(requested);
        let request = DecisionRequest::Model {
            provider: &model.provider,
            model_id: &model.id,
            thinking_level: saved.as_str(),
        };
        settings
            .prepare_operation(&request, SettingsScope::Global, changes)
            .map_err(|message| {
                settings.report_refusal(SettingsScope::Global, message.clone());
                refusal(message)
            })
    }

    /// The thinking preference written only for an effective change the model can retain.
    fn thinking_preferences(
        &self,
        level: &ModelThinkingLevel,
        reasoning: bool,
    ) -> Vec<SettingsChange> {
        if *level == self.thinking_level() || (!reasoning && *level == ModelThinkingLevel::Off) {
            return Vec::new();
        }
        vec![preference(
            "defaultThinkingLevel",
            json!(saved_level(level).as_str()),
        )]
    }

    /// Admits requested thinking before capability clamping can erase its intent.
    pub(super) fn prepare_thinking_selection(
        &self,
        requested: &ModelThinkingLevel,
    ) -> Result<PreparedSettingsEdit, DiagnosticErrorInfo> {
        let effective = clamp_thinking_level(&read(&self.model()), requested.clone());
        let changes = self.thinking_preferences(&effective, self.supports_thinking());
        let settings = self.settings_manager.borrow();
        settings
            .prepare_operation(
                &DecisionRequest::ThinkingLevel(saved_level(requested).as_str()),
                SettingsScope::Global,
                changes,
            )
            .map_err(|message| {
                settings.report_refusal(SettingsScope::Global, message.clone());
                refusal(message)
            })
    }
}
