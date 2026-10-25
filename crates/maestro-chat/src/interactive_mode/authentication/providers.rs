//! Which providers `/login` offers for each sign-in method.

use maestro_app::{BUILT_IN_PROVIDER_DISPLAY_NAMES, get_providers};
use std::collections::HashSet;
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
