//! Native observations for serverless and Linux compute residency.

use super::super::google_auth::environment;

/// Residency from documented serverless variables, DMI and Linux network addresses.
pub(in crate::providers::content::google_shared::node_auth) fn detect() -> bool {
    ["CLOUD_RUN_JOB", "FUNCTION_NAME", "K_SERVICE"]
        .iter()
        .any(|name| environment(name).is_some())
        || linux()
}

/// Read native Linux observations without adding an operating-system adapter.
#[cfg(target_os = "linux")]
fn linux() -> bool {
    let date = std::fs::metadata("/sys/class/dmi/id/bios_date").is_ok();
    let vendor = std::fs::read_to_string("/sys/class/dmi/id/bios_vendor").ok();
    if bios(date, vendor.as_deref()) {
        return true;
    }
    std::fs::read_dir("/sys/class/net").is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read_to_string(entry.path().join("address")).ok())
            .any(|address| mac(&address))
    })
}

/// Non-Linux host network observations are provided by a separate platform adapter.
#[cfg(not(target_os = "linux"))]
fn linux() -> bool {
    false
}

/// DMI requires both the date observation and case-sensitive vendor marker.
pub(in crate::providers::content::google_shared::node_auth) fn bios(
    date: bool,
    vendor: Option<&str>,
) -> bool {
    date && vendor.is_some_and(|vendor| vendor.contains("Google"))
}

/// Native MAC text is lowercased by the OS; the vendor prefix itself contains only digits.
pub(in crate::providers::content::google_shared::node_auth) fn mac(address: &str) -> bool {
    address.starts_with("42:01")
}
