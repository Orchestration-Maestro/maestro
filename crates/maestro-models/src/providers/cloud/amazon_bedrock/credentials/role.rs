//! Role assumption on top of source credentials that were already resolved.

use std::time::Duration;

use aws_config::SdkConfig;
use aws_config::profile::Profile;
use aws_config::sts::AssumeRoleProvider;
use aws_credential_types::Credentials;
use aws_credential_types::provider::error::CredentialsError;
use aws_credential_types::provider::{self, ProvideCredentials};
use aws_sdk_bedrockruntime::config::Region;

use super::profile::field;

/// Session length used when the profile names none.
const DEFAULT_SESSION_SECONDS: u32 = 3600;

/// Assume the role `profile` names, authorized by `source`.
///
/// # Errors
/// Fails with an invalid configuration when `duration_seconds` is not a whole number, and
/// otherwise with the role provider's own error, which is never a missing-credentials error.
pub(super) async fn assume(
    profile: &Profile,
    role_arn: &str,
    source: Credentials,
    shared: &SdkConfig,
) -> provider::Result {
    let seconds = match field(profile, "duration_seconds") {
        None => DEFAULT_SESSION_SECONDS,
        Some(text) => text.parse().map_err(|_| {
            CredentialsError::invalid_configuration(format!(
                "Profile {} has a duration_seconds that is not a whole number of seconds.",
                profile.name()
            ))
        })?,
    };
    let mut builder = AssumeRoleProvider::builder(role_arn)
        .configure(shared)
        .session_length(Duration::from_secs(u64::from(seconds)));
    if let Some(name) = field(profile, "role_session_name") {
        builder = builder.session_name(name);
    }
    if let Some(id) = field(profile, "external_id") {
        builder = builder.external_id(id);
    }
    if let Some(region) = field(profile, "region") {
        builder = builder.region(Region::new(region.to_owned()));
    }
    builder
        .build_from_provider(source)
        .await
        .provide_credentials()
        .await
}
