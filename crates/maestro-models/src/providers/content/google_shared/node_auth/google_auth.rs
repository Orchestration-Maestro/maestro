//! Application-default file discovery and native credential adapters.

use super::CLOUD_SCOPE;
use super::base_external_client::{BaseExternalClient, Effects};
use super::external_account_authorized_user_client::ExternalAccountAuthorizedUserClient;
use crate::DiagnosticErrorInfo;
use google_cloud_auth::credentials::{
    Credentials, external_account, impersonated, service_account, user_account,
};
use serde_json::Value;

/// Read a nonempty environment variable without trimming authored text.
pub(super) fn environment(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// Locate the platform's well-known ADC file without interpreting authored paths.
fn well_known() -> Option<String> {
    let root = if cfg!(windows) {
        environment("APPDATA")
    } else {
        environment("HOME").map(|home| maestro_path::join(&[&home, ".config"]))
    }?;
    Some(maestro_path::join(&[
        &root,
        "gcloud",
        "application_default_credentials.json",
    ]))
}

/// Resolve only the selected source; a selected-file failure never falls through.
pub(super) async fn resolve() -> Result<Credentials, DiagnosticErrorInfo> {
    let selected = environment("GOOGLE_APPLICATION_CREDENTIALS")
        .or_else(|| environment("google_application_credentials"));
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
    if let Some(quota) = environment("GOOGLE_CLOUD_QUOTA_PROJECT") {
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
            return Ok(Credentials::from(BaseExternalClient::new(
                source, effects, &value,
            )));
        }
        Some("authorized_user") => user_account::Builder::new(value)
            .with_scopes([CLOUD_SCOPE])
            .build(),
        Some("service_account") => service_account::Builder::new(value)
            .with_access_specifier(service_account::AccessSpecifier::from_scopes([CLOUD_SCOPE]))
            .build(),
        Some("external_account")
            if value
                .get("credential_source")
                .and_then(|source| source.get("certificate"))
                .is_some() =>
        {
            let source = super::identity_pool_client::IdentityPoolClient::new(&value)
                .map_err(|_| configuration_error())?;
            return Ok(Credentials::from(BaseExternalClient::new(
                source, effects, &value,
            )));
        }
        Some("external_account") => external_account::Builder::new(value)
            .with_scopes([CLOUD_SCOPE])
            .build(),
        Some("impersonated_service_account") => return impersonated_from_json(&value, effects),
        _ => return Err(configuration_error()),
    };
    result.map_err(|_| configuration_error())
}

/// Fixed safe configuration diagnostic.
pub(super) fn configuration_error() -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error("Authentication: invalid credential configuration")
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
