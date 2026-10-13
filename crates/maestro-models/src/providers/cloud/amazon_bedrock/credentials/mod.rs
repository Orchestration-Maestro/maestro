//! The credential sources tried when a profile is selected: that profile's own source, its
//! process once more, web identity from the environment, then container or instance metadata.
//!
//! The environment's own keys are not a step: selecting a profile replaces them. A profile may
//! still name the environment as the source of a role. A step that fails with a
//! missing-credentials error lets the next one run and any other error ends the chain; the
//! profile walk and the process step decide which of their failures are missing-credentials
//! errors.

mod profile;
mod role;

use std::sync::Arc;
use std::time::SystemTime;

use aws_config::SdkConfig;
use aws_config::credential_process::CredentialProcessProvider;
use aws_config::ecs::EcsCredentialsProvider;
use aws_config::imds::credentials::ImdsCredentialsProvider;
use aws_config::meta::credentials::CredentialsProviderChain;
use aws_config::profile::ProfileSet;
use aws_config::provider_config::ProviderConfig;
use aws_config::web_identity_token::WebIdentityTokenCredentialsProvider;
use aws_credential_types::provider::error::CredentialsError;
use aws_credential_types::provider::{self, ProvideCredentials, future};

use self::profile::{Walk, field};

/// Container credential locations; either one selects the container source.
const CONTAINER_URIS: [&str; 2] = [
    "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
];

/// Credentials from a `credential_process` command. A failed command, unusable output and
/// credentials that are already expired all count as a source that did not apply.
async fn process(command: &str) -> provider::Result {
    let credentials = CredentialProcessProvider::new(command.to_owned())
        .provide_credentials()
        .await
        .map_err(CredentialsError::not_loaded)?;
    if credentials
        .expiry()
        .is_some_and(|expiry| expiry < SystemTime::now())
    {
        return Err(CredentialsError::not_loaded(
            "The credential process returned expired credentials.",
        ));
    }
    Ok(credentials)
}

/// The real value of a shim type the profile loader takes by reference and this crate cannot name.
fn ambient<T: Default>() -> T {
    T::default()
}

/// Whether the environment names `variable` with a nonempty value.
fn is_set(variable: &str) -> bool {
    std::env::var_os(variable).is_some_and(|value| !value.is_empty())
}

/// One step of the chain.
#[derive(Clone, Copy)]
enum Kind {
    /// The selected profile's own source.
    Profile,
    /// The selected profile's process, on its own.
    Process,
    /// Container credentials when configured, otherwise instance metadata.
    Remote,
}

/// What every step reads: the selected profile and the configuration of the native providers.
struct Selected {
    /// The profile name.
    profile: String,
    /// Configuration of the native source providers.
    conf: ProviderConfig,
    /// Configuration roles are assumed with, before any provider was composed from it.
    shared: SdkConfig,
    /// The container source.
    ecs: EcsCredentialsProvider,
    /// The instance metadata source.
    imds: ImdsCredentialsProvider,
}

impl Selected {
    /// Load the profile files when the first credential is looked up.
    async fn profiles(&self) -> Result<ProfileSet, CredentialsError> {
        aws_config::profile::load(
            &ambient(),
            &ambient(),
            &ambient(),
            Some(self.profile.clone().into()),
        )
        .await
        .map_err(CredentialsError::invalid_configuration)
    }

    /// Run one step.
    async fn run(&self, kind: Kind) -> provider::Result {
        match kind {
            Kind::Profile => {
                let profiles = self.profiles().await?;
                Walk {
                    profiles: &profiles,
                    conf: &self.conf,
                    shared: &self.shared,
                }
                .resolve(&self.profile)
                .await
            }
            Kind::Process => {
                let profiles = self.profiles().await?;
                let command = profiles
                    .get_profile(&self.profile)
                    .and_then(|profile| field(profile, "credential_process"));
                match command {
                    Some(command) => process(command).await,
                    None => Err(CredentialsError::not_loaded(
                        "The profile has no credential process.",
                    )),
                }
            }
            Kind::Remote if CONTAINER_URIS.into_iter().any(is_set) => {
                self.ecs.provide_credentials().await
            }
            Kind::Remote => match std::env::var("AWS_EC2_METADATA_DISABLED") {
                Ok(value) if !value.is_empty() && value != "false" => Err(
                    CredentialsError::not_loaded("EC2 Instance Metadata Service access disabled"),
                ),
                _ => self.imds.provide_credentials().await,
            },
        }
    }
}

/// A step bound to the selection it reads.
struct Step(Arc<Selected>, Kind);

impl std::fmt::Debug for Step {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Step")
    }
}

impl ProvideCredentials for Step {
    fn provide_credentials<'a>(&'a self) -> future::ProvideCredentials<'a>
    where
        Self: 'a,
    {
        future::ProvideCredentials::new(self.0.run(self.1))
    }
}

/// The chain for a selected profile. Nothing is read until the first credential is looked up.
pub(super) fn selected_profile(profile: &str, shared: &SdkConfig) -> CredentialsProviderChain {
    let conf = ProviderConfig::default().with_region(shared.region().cloned());
    let selected = Arc::new(Selected {
        profile: profile.to_owned(),
        shared: shared.clone(),
        ecs: EcsCredentialsProvider::builder().configure(&conf).build(),
        imds: ImdsCredentialsProvider::builder().configure(&conf).build(),
        conf: conf.clone(),
    });
    let step = |kind| Step(Arc::clone(&selected), kind);
    CredentialsProviderChain::first_try("Profile", step(Kind::Profile))
        .or_else("Process", step(Kind::Process))
        .or_else(
            "WebIdentityToken",
            WebIdentityTokenCredentialsProvider::builder()
                .configure(&conf)
                .build(),
        )
        .or_else("Remote", step(Kind::Remote))
}
