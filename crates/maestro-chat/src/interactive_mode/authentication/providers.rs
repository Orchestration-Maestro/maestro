//! Which providers `/login` offers for each sign-in method.

use super::{InteractiveMode, sorted_by_name};
use crate::{AuthSelectorProvider, AuthType};
use maestro_app::{BUILT_IN_PROVIDER_DISPLAY_NAMES, get_providers};
use std::collections::HashSet;
use std::error::Error;
use std::hash::BuildHasher;
use std::sync::{LazyLock, PoisonError};

/// Every provider the shipped catalog defines.
static BUILT_IN_MODEL_PROVIDERS: LazyLock<HashSet<String>> =
    LazyLock::new(|| get_providers().into_iter().collect());

/// Whether `provider_id` may sign in with an API key.
///
/// A provider with a nonempty shipped display label always may. Otherwise a member of
/// `built_in_provider_ids` (every shipped catalog provider when `None`) may not, and any
/// other provider may unless it is one of `oauth_provider_ids`. Identifiers are compared
/// exactly, so case differences name different providers.
#[must_use]
pub fn is_api_key_login_provider<S: BuildHasher>(
    provider_id: &str,
    oauth_provider_ids: &HashSet<String, S>,
    built_in_provider_ids: Option<&HashSet<String, S>>,
) -> bool {
    let labelled = BUILT_IN_PROVIDER_DISPLAY_NAMES
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(provider_id)
        .is_some_and(|name| !name.is_empty());
    labelled
        || !(built_in_provider_ids.map_or_else(
            || BUILT_IN_MODEL_PROVIDERS.contains(provider_id),
            |built_in| built_in.contains(provider_id),
        ) || oauth_provider_ids.contains(provider_id))
}

impl InteractiveMode {
    /// The choices `/login` offers for `auth_type`, or for both kinds when absent, ordered
    /// by display name under the current locale.
    ///
    /// Every registered subscription comes first, then each catalog provider, in its first
    /// appearance, that may sign in with an API key.
    ///
    /// # Errors
    /// Returns the collator's failure to read the locale environment.
    pub(in crate::interactive_mode) fn get_login_provider_options(
        &self,
        auth_type: Option<AuthType>,
    ) -> Result<Vec<AuthSelectorProvider>, Box<dyn Error>> {
        let session = self.runtime_host.session();
        let registry = session.model_registry();
        let subscriptions = registry.auth_storage().get_oauth_providers();
        let oauth_provider_ids: HashSet<String> = subscriptions
            .iter()
            .map(|provider| provider.id().to_owned())
            .collect();
        let mut options: Vec<_> = subscriptions
            .iter()
            .map(|provider| AuthSelectorProvider {
                id: provider.id().to_owned(),
                name: provider.name().to_owned(),
                auth_type: AuthType::OAuth,
            })
            .collect();
        let model_providers: Vec<String> = registry
            .get_all()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|model| {
                let model = model.read().unwrap_or_else(PoisonError::into_inner);
                model.provider.clone()
            })
            .collect();
        let mut seen = HashSet::new();
        options.extend(
            model_providers
                .iter()
                .filter(|id| seen.insert(id.as_str()))
                .filter(|id| is_api_key_login_provider(id, &oauth_provider_ids, None))
                .map(|id| AuthSelectorProvider {
                    id: id.clone(),
                    name: registry.get_provider_display_name(id),
                    auth_type: AuthType::ApiKey,
                }),
        );
        options.retain(|option| auth_type.is_none_or(|kind| option.auth_type == kind));
        sorted_by_name(options)
    }
}
