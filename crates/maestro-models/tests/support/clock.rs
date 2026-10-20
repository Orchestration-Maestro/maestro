//! Wall-clock readings that bracket the timestamps a call records.
#![cfg(test)]

/// Current milliseconds since the Unix epoch, rounded down like recorded timestamps.
pub fn epoch_millis() -> f64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        * 1000.0)
        .floor()
}
