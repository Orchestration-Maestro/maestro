//! The walk over a profile's role sources and the choice of its leaf credential source.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use aws_config::SdkConfig;
use aws_config::ecs::EcsCredentialsProvider;
use aws_config::environment::EnvironmentVariableCredentialsProvider;
use aws_config::imds::credentials::ImdsCredentialsProvider;
use aws_config::profile::{Profile, ProfileFileCredentialsProvider, ProfileSet};
use aws_config::provider_config::ProviderConfig;
use aws_config::sso::SsoCredentialsProvider;
use aws_config::web_identity_token::{StaticConfiguration, WebIdentityTokenCredentialsProvider};
use aws_credential_types::Credentials;
use aws_credential_types::provider::error::CredentialsError;
use aws_credential_types::provider::{self, ProvideCredentials};
use aws_sdk_bedrockruntime::config::Region;

use super::{process, role};

/// The profile fields that select an SSO source.
const SSO_FIELDS: [&str; 5] = [
    "sso_start_url",
    "sso_account_id",
    "sso_session",
    "sso_region",
    "sso_role_name",
];

/// The value of a profile field, absent when it is missing or empty.
pub(super) fn field<'a>(profile: &'a Profile, name: &str) -> Option<&'a str> {
    profile.get(name).filter(|value| !value.is_empty())
}

/// A remote credential source a profile can name.
#[derive(Clone, Copy)]
enum Named {
    /// Credentials from the process environment.
    Environment,
    /// Container credentials.
    EcsContainer,
    /// Instance metadata credentials.
    Ec2InstanceMetadata,
}

/// Where a role profile gets the credentials it assumes the role with.
enum Source<'a> {
    /// Another profile of the file.
    Profile(&'a str),
    /// A named remote source.
    Named(Named),
}

/// Complete static keys of a profile, with the optional token and account.
fn static_keys(profile: &Profile) -> Option<Credentials> {
    let mut keys = Credentials::builder()
        .access_key_id(field(profile, "aws_access_key_id")?)
        .secret_access_key(field(profile, "aws_secret_access_key")?)
        .provider_name("Profile");
    if let Some(token) = field(profile, "aws_session_token") {
        keys = keys.session_token(token);
    }
    if let Some(account) = field(profile, "aws_account_id") {
        keys = keys.account_id(account);
    }
    Some(keys.build())
}

/// Whether the profile assumes a role from exactly one of a source profile or a named source.
fn assumes_role(profile: &Profile) -> bool {
    field(profile, "role_arn").is_some()
        && field(profile, "source_profile")
            .xor(field(profile, "credential_source"))
            .is_some()
}

/// Whether the profile only names a credential source, supplying credentials to a parent role.
fn is_source_only(profile: &Profile) -> bool {
    field(profile, "role_arn").is_none() && field(profile, "credential_source").is_some()
}

/// The named source of a profile.
fn named(profile: &Profile) -> Result<Named, CredentialsError> {
    match field(profile, "credential_source") {
        Some("Environment") => Ok(Named::Environment),
        Some("EcsContainer") => Ok(Named::EcsContainer),
        Some("Ec2InstanceMetadata") => Ok(Named::Ec2InstanceMetadata),
        _ => Err(CredentialsError::not_loaded(format!(
            "Unsupported credential source in profile {}.",
            profile.name()
        ))),
    }
}

/// Turn a missing-credentials error of a source that is selected and configured into a final one.
fn stop(error: CredentialsError) -> CredentialsError {
    if matches!(error, CredentialsError::CredentialsNotLoaded(_)) {
        CredentialsError::provider_error(error)
    } else {
        error
    }
}

/// What a walk has collected so far.
#[derive(Default)]
struct State<'a> {
    /// Role profiles and their roles, nearest the requested profile first.
    roles: Vec<(&'a Profile, &'a str)>,
    /// Source profiles already followed.
    visited: HashSet<&'a str>,
    /// The region of the first profile that assumes or supplies a role; every role is assumed
    /// through one service client built for it.
    region: Option<&'a str>,
}

/// The outcome of one step of a walk.
enum Step<'a> {
    /// Credentials of the base source.
    Done(Credentials),
    /// The source profile to continue with, and whether it only names a credential source.
    Next(&'a str, bool),
}

/// A walk over the profiles of one loaded file.
pub(super) struct Walk<'a> {
    /// The loaded profiles.
    pub(super) profiles: &'a ProfileSet,
    /// Configuration of the native source providers.
    pub(super) conf: &'a ProviderConfig,
    /// Configuration roles are assumed with.
    pub(super) shared: &'a SdkConfig,
}

impl<'a> Walk<'a> {
    /// Resolve the credentials of `root`: follow role sources to a base source, then assume the
    /// roles from the base back toward `root`.
    ///
    /// # Errors
    /// A missing profile, incomplete keys, a missing or unsupported source and a source cycle
    /// are missing-credentials errors, and so is a failed credential process. The error of a
    /// native source or role provider is otherwise returned as it is, except that SSO turns a
    /// missing-credentials error into a final one.
    pub(super) async fn resolve(&self, root: &'a str) -> provider::Result {
        let mut state = State::default();
        let (mut current, mut supplies_parent) = (root, false);
        let mut credentials = loop {
            match self.advance(&mut state, current, supplies_parent).await? {
                Step::Done(base) => break base,
                Step::Next(name, supplies) => (current, supplies_parent) = (name, supplies),
            }
        };
        let region = state.region;
        for (profile, arn) in state.roles.into_iter().rev() {
            credentials = role::assume(profile, arn, credentials, self.shared, region).await?;
        }
        Ok(credentials)
    }

    /// Take one step from `current`: base credentials, or the profile to continue with.
    async fn advance(
        &self,
        state: &mut State<'a>,
        current: &'a str,
        supplies_parent: bool,
    ) -> Result<Step<'a>, CredentialsError> {
        let data = self.profiles.get_profile(current);
        if !state.visited.is_empty()
            && let Some(keys) = data.and_then(static_keys)
        {
            return Ok(Step::Done(keys));
        }
        let Some(profile) = data.filter(|p| supplies_parent || assumes_role(p)) else {
            return self.leaf(current, data).await.map(Step::Done);
        };
        if state.visited.is_empty() {
            state.region = field(profile, "region");
        }
        let source = Self::source_of(profile, &state.visited)?;
        if let Some(arn) = field(profile, "role_arn") {
            if field(profile, "mfa_serial").is_some() {
                return Err(CredentialsError::invalid_configuration(format!(
                    "Profile {current} requires multi-factor authentication, but no MFA code callback was provided."
                )));
            }
            state.roles.push((profile, arn));
        }
        match source {
            Source::Profile(name) => {
                state.visited.insert(name);
                let supplies = self.profiles.get_profile(name).is_some_and(is_source_only);
                Ok(Step::Next(name, supplies))
            }
            Source::Named(kind) => self.named_source(kind).await.map(Step::Done),
        }
    }

    /// The source of a role profile, refusing a profile already visited.
    fn source_of(
        profile: &'a Profile,
        visited: &HashSet<&str>,
    ) -> Result<Source<'a>, CredentialsError> {
        match field(profile, "source_profile") {
            Some(name) if visited.contains(name) => Err(CredentialsError::not_loaded(format!(
                "Detected a cycle attempting to resolve credentials for profile {}.",
                profile.name()
            ))),
            Some(name) => Ok(Source::Profile(name)),
            None => named(profile).map(Source::Named),
        }
    }

    /// Credentials of a named remote source; its own error kinds decide whether the chain goes on.
    async fn named_source(&self, kind: Named) -> provider::Result {
        match kind {
            Named::Environment => {
                EnvironmentVariableCredentialsProvider::new()
                    .provide_credentials()
                    .await
            }
            Named::EcsContainer => {
                EcsCredentialsProvider::builder()
                    .configure(self.conf)
                    .build()
                    .provide_credentials()
                    .await
            }
            Named::Ec2InstanceMetadata => {
                ImdsCredentialsProvider::builder()
                    .configure(self.conf)
                    .build()
                    .provide_credentials()
                    .await
            }
        }
    }

    /// The first source a profile that assumes no role configures: complete static keys, web
    /// identity, process, SSO, then login.
    async fn leaf(&self, name: &str, profile: Option<&Profile>) -> provider::Result {
        let Some(profile) = profile else {
            return Err(CredentialsError::not_loaded(format!(
                "Profile {name} is not configured."
            )));
        };
        if let Some(keys) = static_keys(profile) {
            return Ok(keys);
        }
        if let (Some(file), Some(arn)) = (
            field(profile, "web_identity_token_file"),
            field(profile, "role_arn"),
        ) {
            return self
                .web_identity(file, arn, field(profile, "role_session_name"))
                .await;
        }
        if let Some(command) = field(profile, "credential_process") {
            return process(command).await;
        }
        if SSO_FIELDS.iter().any(|key| field(profile, key).is_some()) {
            return self.sso(profile).await;
        }
        if field(profile, "login_session").is_some() {
            return ProfileFileCredentialsProvider::builder()
                .configure(self.conf)
                .profile_name(name)
                .build()
                .provide_credentials()
                .await;
        }
        Err(CredentialsError::not_loaded(format!(
            "Could not resolve credentials using profile: [{name}]."
        )))
    }

    /// Web identity credentials; a failed token read or exchange is final.
    async fn web_identity(
        &self,
        token_file: &str,
        role_arn: &str,
        session: Option<&str>,
    ) -> provider::Result {
        let session_name = session.map_or_else(
            || {
                let millis = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_millis());
                format!("maestro-web-identity-{millis}")
            },
            str::to_owned,
        );
        WebIdentityTokenCredentialsProvider::builder()
            .static_configuration(StaticConfiguration {
                web_identity_token_file: token_file.into(),
                role_arn: role_arn.to_owned(),
                session_name,
            })
            .configure(self.conf)
            .build()
            .provide_credentials()
            .await
    }

    /// SSO credentials; every failure once SSO is selected is final. A named session supplies
    /// the start URL and region and nothing else, and its token is renewed natively; a profile
    /// with only a start URL sends its cached token as it is, without renewing it.
    async fn sso(&self, profile: &Profile) -> provider::Result {
        let session_name = field(profile, "sso_session");
        let session = session_name
            .map(|name| {
                self.profiles.sso_session(name).ok_or_else(|| {
                    CredentialsError::invalid_configuration(format!("Missing SSO session: {name}."))
                })
            })
            .transpose()?;
        let value = |key: &str| match session {
            Some(section) if matches!(key, "sso_start_url" | "sso_region") => {
                section.get(key).filter(|text| !text.is_empty())
            }
            _ => field(profile, key),
        };
        let need = |key: &str| {
            value(key).ok_or_else(|| {
                CredentialsError::invalid_configuration(format!(
                    "Missing required SSO field: {key}."
                ))
            })
        };
        let (account, role) = (need("sso_account_id")?, need("sso_role_name")?);
        let (start_url, region) = (need("sso_start_url")?, need("sso_region")?);
        let mut builder = SsoCredentialsProvider::builder()
            .configure(self.conf)
            .account_id(account)
            .role_name(role)
            .start_url(start_url)
            .region(Region::new(region.to_owned()));
        if let Some(name) = session_name {
            builder = builder.session_name(name);
        }
        builder.build().provide_credentials().await.map_err(stop)
    }
}
