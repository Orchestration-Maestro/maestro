//! Native observations for serverless and compute-engine residency.

use crate::builtins::env_value;

/// Residency from documented serverless variables, DMI and network addresses.
pub(in super::super) fn detect_gcp_residency() -> bool {
    is_google_cloud_serverless() || is_google_compute_engine()
}

/// Serverless runtimes announce themselves through documented variables.
fn is_google_cloud_serverless() -> bool {
    ["CLOUD_RUN_JOB", "FUNCTION_NAME", "K_SERVICE"]
        .iter()
        .any(|name| env_value(name).is_some())
}

/// Read the Linux DMI and network observations; other platforms report none.
#[cfg(target_os = "linux")]
fn is_google_compute_engine() -> bool {
    let date = std::fs::metadata("/sys/class/dmi/id/bios_date").is_ok();
    let vendor = std::fs::read_to_string("/sys/class/dmi/id/bios_vendor").ok();
    if is_google_compute_engine_linux(date, vendor.as_deref()) {
        return true;
    }
    std::fs::read_dir("/sys/class/net").is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read_to_string(entry.path().join("address")).ok())
            .any(|address| is_google_compute_engine_mac_address(&address))
    })
}

/// Other platforms report no compute-engine observation.
#[cfg(not(target_os = "linux"))]
fn is_google_compute_engine() -> bool {
    false
}

/// DMI requires both the date observation and case-sensitive vendor marker.
pub(in super::super) fn is_google_compute_engine_linux(date: bool, vendor: Option<&str>) -> bool {
    date && vendor.is_some_and(|vendor| vendor.contains("Google"))
}

/// Native MAC text is lowercased by the OS; the vendor prefix itself contains only digits.
pub(in super::super) fn is_google_compute_engine_mac_address(address: &str) -> bool {
    address.starts_with("42:01")
}
