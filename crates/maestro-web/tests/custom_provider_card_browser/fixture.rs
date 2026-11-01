//! Typed semantic cases recorded from the provider card's public output.
use maestro_app::{Model, Presence};
use maestro_web::{
    CustomProvider, CustomProviderCard, CustomProviderType,
    components::custom_provider_card::ProviderStatus, dom::DomView,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Complete recorded card corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Fixture {
    /// Localized status cases.
    pub rows: Vec<Row>,
    /// Action masks and exact invocation order.
    pub callbacks: Vec<CallbackRow>,
    /// Current operand activation result.
    pub updates: Updates,
    /// Independent flag/protocol controls.
    pub route_controls: Vec<Route>,
}
/// One labeled status case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Row {
    /// Stable case label.
    pub id: String,
    /// Caller operands.
    pub input: Input,
    /// Visible status output.
    pub expected: StatusText,
}
/// Visible status output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StatusText {
    /// Exact text including dot and space.
    pub text: String,
}
/// Caller operands shared by status and action records.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Input {
    /// Language; controls without it use English.
    pub language: Option<String>,
    /// Missing/null/empty/one/two label where models vary.
    pub model_case: Option<String>,
    /// Provider strings and model identities.
    pub provider: Provider,
    /// Independent discovery switch.
    pub is_auto_discovery: bool,
    /// Supplied discovery result.
    pub status: Option<Status>,
    /// Independent callback-presence bits.
    pub mask: Option<u8>,
}
/// Recorded provider values with model identities rather than duplicate model schemas.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Provider {
    /// Caller identity.
    pub id: String,
    /// Literal display name.
    pub name: String,
    /// Existing provider protocol.
    pub r#type: CustomProviderType,
    /// Literal server URL.
    pub base_url: String,
    /// Optional supplied model identities.
    #[serde(default)]
    pub models: Presence<Vec<ModelId>>,
}
/// Identity applied to the existing controlled model fixture.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModelId {
    /// Model identifier.
    pub id: String,
}
/// Discovery result from the reference producer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Status {
    /// Count read only for a connected result.
    pub model_count: usize,
    /// Selected result branch.
    pub status: State,
}
/// The producer's three states.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum State {
    /// Successful discovery.
    Connected,
    /// Pending discovery.
    Checking,
    /// Failed connection.
    Disconnected,
}
/// One action-presence case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CallbackRow {
    /// Caller operands.
    pub input: Input,
    /// Exact labels and calls.
    pub expected: Actions,
}
/// Exact action labels and call order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Actions {
    /// Visible labels in native order.
    pub labels: Vec<String>,
    /// Calls in activation order.
    pub callbacks: Vec<Call>,
    /// Original provider identity preserved by every call.
    pub provider_identity: bool,
}
/// One observed callback operand.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Call {
    /// Callback field selected.
    pub name: String,
    /// Provider identifier read on activation.
    pub provider_id: String,
}
/// Current-operand update witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Updates {
    /// Replacement reads both current operands.
    pub replaced_provider_and_callback: bool,
    /// In-place edits are visible.
    pub mutated_original: bool,
    /// Removed callback does nothing.
    pub removed_callback_noop: bool,
    /// Calls after replacement/removal.
    pub calls: Vec<String>,
}
/// One unequal flag/protocol control.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Route {
    /// Caller operands.
    pub input: Input,
    /// Exact selected status text.
    pub expected: String,
}
/// Decode the complete retained corpus without ignored fields.
pub(super) fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/custom_provider_card.json")).unwrap()
}
impl Input {
    /// Construct real caller operands and preserve model absence.
    pub fn card(&self) -> CustomProviderCard {
        let mut card = CustomProviderCard::new(Rc::new(RefCell::new(self.provider.record())));
        card.is_auto_discovery = self.is_auto_discovery;
        card.status = self.status.as_ref().map(|status| match status.status {
            State::Connected => ProviderStatus::Connected {
                model_count: status.model_count,
            },
            State::Checking => ProviderStatus::Checking,
            State::Disconnected => ProviderStatus::Disconnected,
        });
        card
    }
    /// Supplied language view using the shared controlled host.
    pub fn view(&self) -> Rc<DomView> {
        localized(self.language.as_deref().unwrap_or("en"))
    }
}
impl Provider {
    /// Expand model identities using the existing complete controlled model.
    fn record(&self) -> CustomProvider {
        let models = match &self.models {
            Presence::Missing => Presence::Missing,
            Presence::Null => Presence::Null,
            Presence::Present(models) => Presence::Present(
                models
                    .iter()
                    .map(|id| {
                        let mut model: Model = crate::provider_exports::model();
                        model.id.clone_from(&id.id);
                        model
                    })
                    .collect(),
            ),
        };
        CustomProvider {
            id: self.id.clone(),
            name: self.name.clone(),
            r#type: self.r#type,
            base_url: self.base_url.clone(),
            api_key: Presence::Missing,
            models,
        }
    }
}
/// A view with the selected language and the ordinary supplied pack.
pub(super) fn localized(language: &str) -> Rc<DomView> {
    crate::contributions_support::localized_view(&crate::contributions_support::view(), language)
}
