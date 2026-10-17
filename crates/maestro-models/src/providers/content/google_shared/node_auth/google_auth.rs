//! Application-default file discovery and native credential adapters.

use super::base_external_client::{
    BaseExternalAccountClient, Effects, ServiceAccountImpersonation, TokenSource,
};
use super::external_account_authorized_user_client::ExternalAccountAuthorizedUserClient;
use super::identity_pool_client::IdentityPoolClient;
use super::{CLOUD_SCOPE, INVALID_CONFIGURATION};
use crate::DiagnosticErrorInfo;
use crate::builtins::env_value;
use google_cloud_auth::credentials::{
    Credentials, external_account, impersonated, service_account, user_account,
};
use serde_json::Value;

/// Locate the platform's well-known ADC file without interpreting authored paths.
fn well_known() -> Option<String> {
    let root = if cfg!(windows) {
        env_value("APPDATA")
    } else {
        env_value("HOME").map(|home| maestro_path::join(&[&home, ".config"]))
    }?;
    Some(maestro_path::join(&[
        &root,
        "gcloud",
        "application_default_credentials.json",
    ]))
}

/// Resolve only the selected source; a selected-file failure never falls through.
pub(super) async fn resolve() -> Result<Credentials, DiagnosticErrorInfo> {
    let selected = env_value("GOOGLE_APPLICATION_CREDENTIALS")
        .or_else(|| env_value("google_application_credentials"));
    let path = selected.or_else(|| well_known().filter(|path| std::path::Path::new(path).exists()));
    let Some(path) = path else {
        return super::metadata::credentials(Effects::default()).await;
    };
    let path = std::fs::canonicalize(path).map_err(|_| file_error())?;
    if !path.is_file() {
        return Err(file_error());
    }
    let text = std::fs::read_to_string(path).map_err(|_| file_error())?;
    let mut value: Value = serde_json::from_str(&text).map_err(|_| configuration_error())?;
    if let Some(quota) = env_value("GOOGLE_CLOUD_QUOTA_PROJECT") {
        value
            .as_object_mut()
            .ok_or_else(configuration_error)?
            .insert("quota_project_id".into(), Value::String(quota));
    }
    from_json(value, Effects::default())
}

/// Build supported credentials from one already-read object.
pub(super) fn from_json(
    value: Value,
    effects: Effects,
) -> Result<Credentials, DiagnosticErrorInfo> {
    let result = match value.get("type").and_then(Value::as_str) {
        Some("external_account_authorized_user") => {
            let source = ExternalAccountAuthorizedUserClient::new(&value)
                .map_err(|_| configuration_error())?;
            return Ok(Credentials::from(external_client(source, effects, &value)));
        }
        Some("authorized_user") => user_account::Builder::new(value).build(),
        Some("service_account") => service_account::Builder::new(value)
            .with_access_specifier(service_account::AccessSpecifier::from_scopes([CLOUD_SCOPE]))
            .build(),
        Some("external_account")
            if value
                .get("credential_source")
                .and_then(|source| source.get("certificate"))
                .is_some() =>
        {
            let source = IdentityPoolClient::new(&value).map_err(|_| configuration_error())?;
            let impersonation =
                ServiceAccountImpersonation::new(&value).map_err(|_| configuration_error())?;
            return Ok(Credentials::from(
                external_client(source, effects, &value).with_impersonation(impersonation),
            ));
        }
        Some("external_account") => external_account::Builder::new(value)
            .with_scopes([CLOUD_SCOPE])
            .build(),
        Some("impersonated_service_account") => return impersonated_from_json(&value, effects),
        _ => return Err(configuration_error()),
    };
    result.map_err(|_| configuration_error())
}

/// Owned client over `source`, with the quota and universe the configuration names.
fn external_client<S: TokenSource>(
    source: S,
    effects: Effects,
    value: &Value,
) -> BaseExternalAccountClient<S> {
    let text = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);
    BaseExternalAccountClient::new(
        source,
        effects,
        text("quota_project_id"),
        text("universe_domain"),
    )
}

/// Fixed safe configuration diagnostic.
pub(super) fn configuration_error() -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(INVALID_CONFIGURATION)
}

/// Fixed safe filesystem diagnostic.
fn file_error() -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error("Authentication: unable to read credential file")
}

/// Compose an outer impersonation over its retained typed source credentials.
fn impersonated_from_json(
    value: &Value,
    effects: Effects,
) -> Result<Credentials, DiagnosticErrorInfo> {
    let url = value
        .get("service_account_impersonation_url")
        .and_then(Value::as_str)
        .filter(|url| url.len() <= 256)
        .ok_or_else(configuration_error)?;
    let principal = url
        .strip_suffix(":generateAccessToken")
        .or_else(|| url.strip_suffix(":generateIdToken"))
        .and_then(|rest| rest.rsplit('/').next())
        .filter(|target| !target.is_empty())
        .ok_or_else(configuration_error)?;
    let source = from_json(
        value
            .get("source_credentials")
            .cloned()
            .ok_or_else(configuration_error)?,
        effects,
    )?;
    let mut builder = impersonated::Builder::from_source_credentials(source)
        .with_target_principal(principal)
        .with_scopes([CLOUD_SCOPE]);
    if let Some(delegates) = value.get("delegates").and_then(Value::as_array) {
        builder = builder.with_delegates(delegates.iter().filter_map(Value::as_str));
    }
    if let Some(quota) = value.get("quota_project_id").and_then(Value::as_str) {
        builder = builder.with_quota_project_id(quota);
    }
    builder.build().map_err(|_| configuration_error())
}
