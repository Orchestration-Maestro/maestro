//! Retained status segments react only to current command output.
#![cfg(test)]
#![cfg(target_os = "linux")]

#[test]
fn local_datetime_observes_supplied_timezone_inside_runtime() {
    use maestro_test_process::TestProcess;
    if let Ok(seconds) = std::env::var("MAESTRO_TEST_ZONE_OFFSET") {
        let offset = time::UtcOffset::from_whole_seconds(seconds.parse().unwrap()).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let before = time::OffsetDateTime::now_utc().unix_timestamp();
                let local = maestro_daemon::platform::local_datetime().unwrap();
                let after = time::OffsetDateTime::now_utc().unix_timestamp();
                assert!((before..=after).contains(&local.assume_offset(offset).unix_timestamp()));
            });
        return;
    }
    for (zone, offset) in [("Etc/UTC", "0"), ("Etc/GMT+7", "-25200")] {
        let output =
            TestProcess::current("local_datetime_observes_supplied_timezone_inside_runtime")
                .unwrap()
                .env("TZ", zone)
                .env("MAESTRO_TEST_ZONE_OFFSET", offset)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{zone}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
