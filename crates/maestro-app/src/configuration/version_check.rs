//! Package release comparison and native hosted release requests.
use crate::utils::changelog::{ChangelogEntry, compare_versions};
use maestro_text::{collation::Collator, is_whitespace_scalar};

/// Numeric components and the optional whole prerelease text.
struct ParsedVersion<'a> {
    /// Numeric release order with empty section content.
    version: ChangelogEntry,
    /// Prerelease compared under the caller's locale.
    prerelease: Option<&'a str>,
}

/// Read the package version grammar without interpreting build metadata.
fn parse_package_version(version: &str) -> Option<ParsedVersion<'_>> {
    let text = version.trim_matches(is_whitespace_scalar);
    let text = text.strip_prefix('v').unwrap_or(text);
    let (base, build) = text
        .split_once('+')
        .map_or((text, None), |(a, b)| (a, Some(b)));
    if build.is_some_and(|b| b.contains(['\n', '\r', '\u{2028}', '\u{2029}'])) {
        return None;
    }
    let (base, prerelease) = base
        .split_once('-')
        .map_or((base, None), |(a, b)| (a, Some(b)));
    if prerelease.is_some_and(|p| {
        p.is_empty()
            || !p
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c))
    }) {
        return None;
    }
    let mut parts = base.split('.');
    let mut numbers = [0.0; 3];
    for number in &mut numbers {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        *number = part.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    let [major, minor, patch] = numbers;
    Some(ParsedVersion {
        version: ChangelogEntry {
            major,
            minor,
            patch,
            content: String::new(),
        },
        prerelease,
    })
}

/// Compare package versions, returning the first numeric difference or locale order.
#[must_use]
pub fn compare_package_versions(left: &str, right: &str, collator: &Collator) -> Option<f64> {
    let left = parse_package_version(left)?;
    let right = parse_package_version(right)?;
    let difference = compare_versions(&left.version, &right.version);
    if difference != 0.0 {
        return Some(difference);
    }
    Some(match (left.prerelease, right.prerelease) {
        (None, None) => 0.0,
        (Some(_), None) => -1.0,
        (None, Some(_)) => 1.0,
        (Some(left), Some(right)) => f64::from(collator.compare(left, right) as i8),
    })
}

/// Whether a candidate is newer, falling back to unequal trimmed text if unparsed.
#[must_use]
pub fn is_newer_package_version(candidate: &str, current: &str, collator: &Collator) -> bool {
    compare_package_versions(candidate, current, collator).map_or_else(
        || {
            candidate.trim_matches(is_whitespace_scalar)
                != current.trim_matches(is_whitespace_scalar)
        },
        |difference| difference > 0.0,
    )
}

use super::{config::service_endpoint, maestro_user_agent::get_maestro_user_agent};
use futures_util::StreamExt;
use maestro_models::{
    Fetch, HttpRequest, default_fetch,
    providers::http::{Raced, race},
};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, io, time::Duration};

/// Selected release strings returned by the hosted service.
#[derive(Debug, PartialEq, Eq)]
pub struct LatestMaestroRelease {
    /// Package version.
    pub version: String,
    /// Distribution package name, when supplied.
    pub package_name: Option<String>,
}

/// Per-operation deadline, replaceable HTTP transport and startup selections.
#[derive(Default)]
pub struct VersionCheckOptions {
    /// Whole-operation deadline in milliseconds; absent means 10,000.
    pub timeout_ms: Option<u64>,
    /// Transport override; absent selects the shared default client.
    pub fetch: Option<Fetch>,
    /// A caller's offline selection, or `MAESTRO_OFFLINE` when absent.
    pub offline: Option<bool>,
    /// A caller's version-skip selection, or `MAESTRO_SKIP_VERSION_CHECK` when absent.
    pub skip_version_check: Option<bool>,
}

/// Fetch trimmed release fields; disabled origins or flags return no release.
///
/// # Errors
/// Propagates transport, body, JSON and deadline failures, including a null root.
pub async fn get_latest_maestro_release(
    current_version: &str,
    service_origin: &str,
    options: &VersionCheckOptions,
) -> io::Result<Option<LatestMaestroRelease>> {
    if options.skip_version_check.unwrap_or_else(|| {
        super::flag_is_set(std::env::var("MAESTRO_SKIP_VERSION_CHECK").ok().as_deref())
    }) || options
        .offline
        .unwrap_or_else(|| super::flag_is_set(std::env::var("MAESTRO_OFFLINE").ok().as_deref()))
    {
        return Ok(None);
    }
    let Some(url) = service_endpoint(service_origin, "/api/latest-version") else {
        return Ok(None);
    };
    let request = HttpRequest {
        method: "GET".into(),
        url,
        headers: [
            (
                "user-agent".into(),
                get_maestro_user_agent(
                    current_version,
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                ),
            ),
            ("accept".into(), "application/json".into()),
        ]
        .into(),
        body: Vec::new(),
        signal: None,
    };
    let fetch = options.fetch.clone().unwrap_or_else(default_fetch);
    let work = async {
        let mut response = fetch(request).await.map_err(io::Error::other)?;
        if !(200..300).contains(&response.status) {
            return Ok(None);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.body.next().await {
            body.extend(chunk.map_err(io::Error::other)?);
        }
        select_release(&maestro_text::decode_utf8(&body))
    };
    match race(
        work,
        Some(Duration::from_millis(options.timeout_ms.unwrap_or(10_000))),
        None,
    )
    .await
    {
        Raced::Done(result) => result,
        Raced::TimedOut | Raced::Cancelled => Err(io::ErrorKind::TimedOut.into()),
    }
}

/// Decode only selected strings, preserving last-member-wins object semantics.
fn select_release(text: &str) -> io::Result<Option<LatestMaestroRelease>> {
    let root: &RawValue = serde_json::from_str(text).map_err(io::Error::other)?;
    if root.get() == "null" {
        return Err(io::ErrorKind::InvalidData.into());
    }
    if !root.get().starts_with('{') {
        return Ok(None);
    }
    let fields: BTreeMap<String, &RawValue> =
        maestro_json::object::from_str(root.get()).map_err(io::Error::other)?;
    let selected = |key| {
        fields
            .get(key)
            .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok())
            .map(|s| s.trim_matches(is_whitespace_scalar).to_owned())
            .filter(|s| !s.is_empty())
    };
    Ok(selected("version").map(|version| LatestMaestroRelease {
        version,
        package_name: selected("packageName"),
    }))
}

/// Project a release to its version, retaining lower-operation failures.
///
/// # Errors
/// Propagates the release operation's errors.
pub async fn get_latest_maestro_version(
    current_version: &str,
    service_origin: &str,
    options: &VersionCheckOptions,
) -> io::Result<Option<String>> {
    Ok(
        get_latest_maestro_release(current_version, service_origin, options)
            .await?
            .map(|release| release.version),
    )
}

/// Return only a newer version, suppressing request and locale failures.
pub async fn check_for_new_maestro_version(
    current_version: &str,
    service_origin: &str,
    options: &VersionCheckOptions,
) -> Option<String> {
    let latest = get_latest_maestro_version(current_version, service_origin, options)
        .await
        .ok()??;
    let collator = Collator::new(&|key| std::env::var(key).ok()).ok()?;
    is_newer_package_version(&latest, current_version, &collator).then_some(latest)
}
