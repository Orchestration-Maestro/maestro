//! Literal viewer fragments and the common hosted-service origin.
#![cfg(test)]
#![cfg(unix)]
use crate::service_observations as services;
use maestro_app::configuration::config::{SERVICE_ORIGIN, get_share_viewer_url};
#[test]
fn share_links_keep_the_literal_fragment() {
    for case in services::cases("share") {
        assert_eq!(
            get_share_viewer_url(
                case.input["gist"].as_str().unwrap(),
                case.input["origin"].as_str().unwrap()
            )
            .as_deref(),
            case.output.as_str()
        );
    }
}
#[cfg(unix)]
#[test]
fn one_origin_controls_all_three_hosted_operations() {
    services::run(one_origin_controls_all_three_hosted_operations_run());
}
async fn one_origin_controls_all_three_hosted_operations_run() {
    if services::isolated("one_origin_controls_all_three_hosted_operations") {
        return;
    }
    assert_eq!(SERVICE_ORIGIN, "");
    for case in services::cases("origin") {
        let origin = case.input["origin"].as_str().unwrap();
        let t = services::Transport::new(200, case.input["releaseBody"].to_string(), "");
        let version = case.input["version"].as_str().unwrap();
        let options = maestro_app::configuration::version_check::VersionCheckOptions {
            fetch: Some(t.fetch.clone()),
            ..Default::default()
        };
        let release = maestro_app::configuration::version_check::get_latest_maestro_release(
            version, origin, &options,
        )
        .await
        .unwrap();
        assert_eq!(services::release_value(release), case.output["release"]);
        let settings =
            maestro_settings::SettingsManager::in_memory(maestro_settings::Settings::default());
        maestro_app::configuration::telemetry::report_install_telemetry(
            version,
            &settings,
            origin,
            Some(t.fetch.clone()),
            None,
        )
        .await;
        let urls: Vec<_> = t
            .calls()
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["url"].clone())
            .collect();
        assert_eq!(serde_json::json!(urls), case.output["urls"]);
        assert_eq!(
            get_share_viewer_url(case.input["gist"].as_str().unwrap(), origin).as_deref(),
            case.output["share"].as_str()
        );
    }
}
