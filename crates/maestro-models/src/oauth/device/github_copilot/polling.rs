//! Device-code validation and server-directed polling.
use super::token::{CLIENT_ID, fetch_json, response_record, string_field};
use crate::providers::http::{Raced, race};
use crate::providers::json_text::{member, raw_json, raw_number};
use crate::{Cancellation, Fetch, HttpRequest, OAuthError};
use std::future::pending;
use std::time::Duration;

/// Selected authorization fields and server timing operands.
pub(super) struct DeviceCodeResponse {
    /// Token grant operand.
    pub device_code: String,
    /// Human-readable verification code.
    pub user_code: String,
    /// Supplied verification location.
    pub verification_uri: String,
    /// Initial poll interval in seconds.
    pub interval: f64,
    /// Server lifetime in seconds.
    pub expires_in: f64,
}

/// The authored device-field validation failure.
fn invalid() -> OAuthError {
    OAuthError::message("Invalid device code response fields")
}

/// Require a finite consumed timing value.
fn finite(value: f64) -> Result<f64, OAuthError> {
    value.is_finite().then_some(value).ok_or_else(invalid)
}

/// Send an ordered form request using the device client identity.
async fn post_form(
    domain: &str,
    path: &str,
    fields: &[(&str, &str)],
    fetch: &Fetch,
) -> Result<String, OAuthError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().copied())
        .finish()
        .into_bytes();
    fetch_json(
        HttpRequest {
            method: "POST".into(),
            url: format!("https://{domain}/login/{path}"),
            headers: [
                ("accept".into(), "application/json".into()),
                (
                    "content-type".into(),
                    "application/x-www-form-urlencoded".into(),
                ),
                ("user-agent".into(), "GitHubCopilotChat/0.35.0".into()),
            ]
            .into(),
            body,
            signal: None,
        },
        fetch,
    )
    .await
}

/// Request a device code and select required fields before decoding them.
pub(super) async fn start_device_flow(
    domain: &str,
    fetch: &Fetch,
) -> Result<DeviceCodeResponse, OAuthError> {
    let text = post_form(
        domain,
        "device/code",
        &[("client_id", CLIENT_ID), ("scope", "read:user")],
        fetch,
    )
    .await?;
    let raw = response_record(&text, "device code")?;
    Ok(DeviceCodeResponse {
        device_code: string_field(raw, "device_code").ok_or_else(invalid)?,
        user_code: string_field(raw, "user_code").ok_or_else(invalid)?,
        verification_uri: string_field(raw, "verification_uri").ok_or_else(invalid)?,
        interval: finite(
            member(raw, "interval")
                .and_then(raw_number)
                .ok_or_else(invalid)?,
        )?,
        expires_in: finite(
            member(raw, "expires_in")
                .and_then(raw_number)
                .ok_or_else(invalid)?,
        )?,
    })
}

/// Poll result retaining only the branch's consumed values.
enum DeviceTokenResponse {
    /// The account token, including an empty token.
    Access(String),
    /// No terminal outcome was returned.
    Pending,
    /// Server-directed interval, when numeric.
    SlowDown(Option<f64>),
}

/// Select success first, then only the fields the selected error branch consumes.
fn decode_poll(text: &str) -> Result<DeviceTokenResponse, OAuthError> {
    let raw = raw_json(text).map_err(|error| OAuthError::message(error.to_string()))?;
    if let Some(access) = selected_string(raw, "access_token")? {
        return Ok(DeviceTokenResponse::Access(access));
    }
    let Some(error) = selected_string(raw, "error")? else {
        return Ok(DeviceTokenResponse::Pending);
    };
    match error.as_str() {
        "authorization_pending" => Ok(DeviceTokenResponse::Pending),
        "slow_down" => Ok(DeviceTokenResponse::SlowDown(
            member(raw, "interval").and_then(raw_number),
        )),
        _ => {
            let description = selected_string(raw, "error_description")?;
            let suffix = description
                .filter(|text| !text.is_empty())
                .map_or_else(String::new, |text| format!(": {text}"));
            Err(OAuthError::message(format!(
                "Device flow failed: {error}{suffix}"
            )))
        }
    }
}

/// Decode a consumed string, retaining native failures for unrepresentable string text.
fn selected_string(
    raw: &serde_json::value::RawValue,
    name: &str,
) -> Result<Option<String>, OAuthError> {
    member(raw, name)
        .filter(|raw| raw.get().starts_with('"'))
        .map(|raw| serde_json::from_str(raw.get()))
        .transpose()
        .map_err(|error| OAuthError::message(error.to_string()))
}

/// Observe cancellation without racing any HTTP request.
pub(super) fn check_cancelled(signal: Option<&Cancellation>) -> Result<(), OAuthError> {
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(OAuthError::message("Login cancelled"));
    }
    Ok(())
}

/// Wait with an owned cancellation observer, released on either outcome.
async fn abortable_sleep(
    milliseconds: f64,
    signal: Option<&Cancellation>,
) -> Result<(), OAuthError> {
    let duration = Duration::try_from_secs_f64(finite(milliseconds)?.max(0.0) / 1000.0)
        .map_err(|_| invalid())?;
    match race(pending::<()>(), Some(duration), signal).await {
        Raced::Cancelled => Err(OAuthError::message("Login cancelled")),
        Raced::TimedOut | Raced::Done(()) => Ok(()),
    }
}

/// Wait before each poll, retaining the admitted final poll after the deadline.
pub(super) async fn poll_for_github_access_token(
    domain: &str,
    device: &DeviceCodeResponse,
    fetch: &Fetch,
    signal: Option<&Cancellation>,
    clock: &(impl Fn() -> f64 + Sync),
) -> Result<String, OAuthError> {
    let deadline = finite(clock() + device.expires_in * 1000.0)?;
    let mut interval = (device.interval * 1000.0).floor().max(1000.0);
    let mut multiplier = 1.2;
    let mut slow_down = false;
    while clock() < deadline {
        check_cancelled(signal)?;
        let remaining = finite(deadline - clock())?;
        let wait = (interval * multiplier).ceil().min(remaining);
        abortable_sleep(wait, signal).await?;
        let text = post_form(
            domain,
            "oauth/access_token",
            &[
                ("client_id", CLIENT_ID),
                ("device_code", &device.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ],
            fetch,
        )
        .await?;
        match decode_poll(&text)? {
            DeviceTokenResponse::Access(access) => return Ok(access),
            DeviceTokenResponse::Pending => {}
            DeviceTokenResponse::SlowDown(supplied) => {
                slow_down = true;
                multiplier = 1.4;
                interval = match supplied.map(finite).transpose()? {
                    Some(seconds) if seconds > 0.0 => seconds * 1000.0,
                    _ => (interval + 5000.0).max(1000.0),
                };
            }
        }
    }
    Err(OAuthError::message(if slow_down {
        "Device flow timed out after one or more slow_down responses. This is often caused by clock drift in WSL or VM environments. Please sync or restart the VM clock and try again."
    } else {
        "Device flow timed out"
    }))
}
