//! Install reporting policy and bounded request observations.
#![cfg(test)]
#![cfg(unix)]
use crate::service_observations as services;
use maestro_app::configuration::telemetry::is_install_telemetry_enabled;
use maestro_settings::SettingsManager;
#[test]
fn unset_telemetry_uses_the_existing_enabled_default() {
    let case = services::cases("telemetry").remove(0);
    let settings = SettingsManager::in_memory(maestro_settings::Settings::default());
    assert_eq!(
        is_install_telemetry_enabled(&settings, case.input["env"].as_str()),
        case.output.as_bool().unwrap()
    );
}
#[test]
fn telemetry_environment_presence_overrides_the_setting() {
    for case in services::cases("telemetry").into_iter().skip(1) {
        let mut settings = SettingsManager::in_memory(maestro_settings::Settings::default());
        if let Some(setting) = case.input["setting"].as_bool() {
            settings.set_enable_install_telemetry(setting);
        }
        assert_eq!(
            is_install_telemetry_enabled(&settings, case.input["env"].as_str()),
            case.output.as_bool().unwrap()
        );
    }
}
use maestro_app::configuration::telemetry::report_install_telemetry;
use services::{ORIGIN, Transport};
#[cfg(unix)]
#[test]
fn install_report_sends_only_version_and_user_agent() {
    services::run(install_report_sends_only_version_and_user_agent_run());
}
async fn install_report_sends_only_version_and_user_agent_run() {
    if services::isolated("install_report_sends_only_version_and_user_agent") {
        return;
    }
    for case in services::cases("report").into_iter().take(2) {
        let t = Transport::new(200, String::new(), "BODY_ERROR");
        let work = {
            let version = case.input["version"].as_str().unwrap().to_owned();
            let origin = ORIGIN.to_owned();
            let settings = SettingsManager::in_memory(maestro_settings::Settings::default());
            report_install_telemetry(&version, &settings, &origin, Some(t.fetch.clone()), None)
        };
        work.await;
        assert_eq!(t.calls(), services::expected_calls(&case.output));
        assert_eq!(
            t.body_polls.load(std::sync::atomic::Ordering::SeqCst),
            usize::try_from(case.output["jsonReads"].as_u64().unwrap()).unwrap()
        );
    }
}
#[cfg(unix)]
#[test]
fn install_report_gates_do_not_follow_the_version_skip_flag() {
    services::run(install_report_gates_do_not_follow_the_version_skip_flag_run());
}
async fn install_report_gates_do_not_follow_the_version_skip_flag_run() {
    let Some(case) = services::environment_cases(
        "install_report_gates_do_not_follow_the_version_skip_flag",
        "report",
        &[2, 3, 4, 5, 6, 7, 11, 12, 13, 14],
    ) else {
        return;
    };
    let mut settings = SettingsManager::in_memory(maestro_settings::Settings::default());
    if let Some(setting) = case.input["setting"].as_bool() {
        settings.set_enable_install_telemetry(setting);
    }
    let t = Transport::new(200, String::new(), "");
    let work = report_install_telemetry(
        case.input["version"].as_str().unwrap(),
        &settings,
        ORIGIN,
        Some(t.fetch.clone()),
        None,
    );
    if let Some(setting) = case.input["settingAfterPrepare"].as_bool() {
        settings.set_enable_install_telemetry(setting);
    }
    work.await;
    assert_eq!(t.calls(), services::expected_calls(&case.output));
    assert_eq!(
        t.body_polls.load(std::sync::atomic::Ordering::SeqCst),
        usize::try_from(case.output["jsonReads"].as_u64().unwrap()).unwrap()
    );
}
#[cfg(unix)]
#[test]
fn install_report_discards_all_request_outcomes() {
    services::run(install_report_discards_all_request_outcomes_run());
}
async fn install_report_discards_all_request_outcomes_run() {
    if services::isolated("install_report_discards_all_request_outcomes") {
        return;
    }
    for case in services::cases("report").into_iter().skip(8).take(3) {
        let t = Transport::new(
            u16::try_from(case.input["status"].as_u64().unwrap_or(200)).unwrap(),
            String::new(),
            case.input["response"].as_str().unwrap_or(""),
        );
        let settings = SettingsManager::in_memory(maestro_settings::Settings::default());
        report_install_telemetry(
            case.input["version"].as_str().unwrap(),
            &settings,
            ORIGIN,
            Some(t.fetch.clone()),
            None,
        )
        .await;
        assert_eq!(t.calls(), services::expected_calls(&case.output));
        assert_eq!(
            t.body_polls.load(std::sync::atomic::Ordering::SeqCst),
            usize::try_from(case.output["jsonReads"].as_u64().unwrap()).unwrap()
        );
    }
}
#[cfg(unix)]
#[test]
fn install_report_deadline_stops_waiting_at_headers() {
    crate::support::paused_io_runtime()
        .block_on(install_report_deadline_stops_waiting_at_headers_run());
}
async fn install_report_deadline_stops_waiting_at_headers_run() {
    if services::isolated("install_report_deadline_stops_waiting_at_headers") {
        return;
    }
    let settings = SettingsManager::in_memory(maestro_settings::Settings::default());
    let t = Transport::new(200, String::new(), "HEADERS_PENDING");
    let mut work =
        report_install_telemetry("1.2.3", &settings, ORIGIN, Some(t.fetch.clone()), None);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
    assert_eq!(t.count(), 1);
    tokio::time::advance(std::time::Duration::from_millis(4_999)).await;
    assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
    assert_eq!(t.dropped.load(std::sync::atomic::Ordering::SeqCst), 0);
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    assert!(std::future::Future::poll(work.as_mut(), &mut context).is_ready());
    assert_eq!(t.dropped.load(std::sync::atomic::Ordering::SeqCst), 1);
    headers_finish_without_a_body(&settings);
}
/// Ready headers complete without polling a withheld body.
fn headers_finish_without_a_body(settings: &SettingsManager) {
    let t = Transport::new(200, String::new(), "BODY_PENDING");
    let mut work = report_install_telemetry("1.2.3", settings, ORIGIN, Some(t.fetch.clone()), None);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(work.as_mut(), &mut context).is_ready());
    assert_eq!(t.count(), 1);
    assert_eq!(t.body_polls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(t.dropped.load(std::sync::atomic::Ordering::SeqCst), 1);
}
