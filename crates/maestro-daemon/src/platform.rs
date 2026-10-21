//! Safe native hostname and local calendar observations.

/// Reads the Linux kernel hostname, without invoking a shell.
#[must_use]
pub fn hostname() -> Option<String> {
    let value = std::fs::read_to_string("/proc/sys/kernel/hostname").ok()?;
    let value = value.trim_end_matches('\n');
    (!value.is_empty()).then(|| value.to_owned())
}

/// Observes the local wall clock through the native timezone database.
#[must_use]
pub fn local_datetime() -> Option<time::PrimitiveDateTime> {
    let local = chrono::Local::now();
    let offset = time::UtcOffset::from_whole_seconds(local.offset().local_minus_utc()).ok()?;
    let value = time::OffsetDateTime::from_unix_timestamp(local.timestamp())
        .ok()?
        .to_offset(offset);
    Some(time::PrimitiveDateTime::new(value.date(), value.time()))
}
