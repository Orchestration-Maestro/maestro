//! Startup offline and version-skip selections reaching the hosted-service gates.
#![cfg(test)]
#![cfg(unix)]
use crate::service_observations as services;
use maestro_app::configuration::telemetry::{
    is_install_telemetry_enabled, report_install_telemetry,
};
use maestro_app::configuration::version_check::{
    VersionCheckOptions, check_for_new_maestro_version, get_latest_maestro_release,
    get_latest_maestro_version,
};
use maestro_settings::{Settings, SettingsManager};
use services::{ORIGIN, Transport};

/// The truth-value selections a startup decides, in both unequal directions.
const SELECTIONS: [(Option<bool>, Option<bool>); 9] = [
    (None, None),
    (None, Some(false)),
    (None, Some(true)),
    (Some(false), None),
    (Some(false), Some(false)),
    (Some(false), Some(true)),
    (Some(true), None),
    (Some(true), Some(false)),
    (Some(true), Some(true)),
];

/// Nonempty ambient values that still disable the operation they belong to.
const NONEMPTY: [&str; 4] = ["0", "false", " ", "\u{feff}false"];

/// One ambient row: the flag it sets, the value, and whether it disables the operation.
type AmbientRow = (&'static str, Option<&'static str>, bool);

/// Every ambient row of both fields, where an absent or empty value permits the request.
fn ambient_rows() -> Vec<AmbientRow> {
    let mut rows = Vec::new();
    for key in ["MAESTRO_OFFLINE", "MAESTRO_SKIP_VERSION_CHECK"] {
        rows.push((key, None, false));
        rows.push((key, Some(""), false));
        rows.extend(NONEMPTY.into_iter().map(|value| (key, Some(value), true)));
    }
    rows
}

/// Options carrying only a fetch override and the two selections.
fn options(
    fetch: &maestro_models::Fetch,
    selection: (Option<bool>, Option<bool>),
) -> VersionCheckOptions {
    VersionCheckOptions {
        timeout_ms: None,
        fetch: Some(fetch.clone()),
        offline: selection.0,
        skip_version_check: selection.1,
    }
}

/// One release body the controlled transport answers with.
fn transport() -> Transport {
    Transport::new(200, r#"{"version":"1.2.4"}"#.into(), "")
}

/// Runs the three release entry points and reports their transport call counts.
async fn release_gates(selection: (Option<bool>, Option<bool>)) -> (usize, usize, usize) {
    let release = transport();
    let version = transport();
    let newer = transport();
    assert_eq!(
        get_latest_maestro_release("1.2.3", ORIGIN, &options(&release.fetch, selection))
            .await
            .unwrap()
            .map(|release| release.version),
        (release.count() == 1).then(|| "1.2.4".to_owned())
    );
    assert_eq!(
        get_latest_maestro_version("1.2.3", ORIGIN, &options(&version.fetch, selection))
            .await
            .unwrap(),
        (version.count() == 1).then(|| "1.2.4".to_owned())
    );
    assert_eq!(
        check_for_new_maestro_version("1.2.3", ORIGIN, &options(&newer.fetch, selection)).await,
        (newer.count() == 1).then(|| "1.2.4".to_owned())
    );
    (release.count(), version.count(), newer.count())
}

#[test]
fn startup_selection_skips_all_release_entrypoints() {
    if services::isolated("startup_selection_skips_all_release_entrypoints") {
        return;
    }
    for selection in SELECTIONS {
        let suppressed = selection.0 == Some(true) || selection.1 == Some(true);
        let expected = (
            usize::from(!suppressed),
            usize::from(!suppressed),
            usize::from(!suppressed),
        );
        assert_eq!(
            services::run(release_gates(selection)),
            expected,
            "{selection:?}"
        );
    }
}

/// Whether an ambient service flag is set to any nonempty text.
fn ambient_flag_is_set(key: &str) -> bool {
    std::env::var(key).is_ok_and(|value| !value.is_empty())
}

#[test]
fn release_reads_nonempty_flags_without_trimming() {
    if maestro_test_process::selection().is_child() {
        let disabled = ambient_flag_is_set("MAESTRO_OFFLINE")
            || ambient_flag_is_set("MAESTRO_SKIP_VERSION_CHECK");
        let t = transport();
        let version = services::run(get_latest_maestro_version(
            "1.2.3",
            ORIGIN,
            &options(&t.fetch, (None, None)),
        ))
        .unwrap();
        assert_eq!(version, (t.count() == 1).then(|| "1.2.4".to_owned()));
        assert_eq!(t.count(), usize::from(!disabled));
        return;
    }
    for (key, value, _disabled) in ambient_rows() {
        let envs: Vec<(&str, &str)> = value.map(|value| (key, value)).into_iter().collect();
        services::native_probe(
            "release_reads_nonempty_flags_without_trimming",
            &envs,
            services::FLAGS,
        );
    }
}

#[test]
fn startup_selection_skips_install_reporting() {
    if services::isolated("startup_selection_skips_install_reporting") {
        return;
    }
    services::run(async {
        let mut settings = SettingsManager::in_memory(Settings::default());
        settings.set_enable_install_telemetry(true);
        let suppressed = transport();
        let work = report_install_telemetry(
            "1.2.3",
            &settings,
            ORIGIN,
            Some(suppressed.fetch.clone()),
            Some(true),
        );
        settings.set_enable_install_telemetry(true);
        work.await;
        assert_eq!(suppressed.count(), 0);

        let prepared = transport();
        let work = report_install_telemetry(
            "1.2.3",
            &settings,
            ORIGIN,
            Some(prepared.fetch.clone()),
            Some(false),
        );
        settings.set_enable_install_telemetry(false);
        work.await;
        assert_eq!(prepared.count(), 1);
    });
}

#[test]
fn install_reporting_keeps_telemetry_and_skip_independent() {
    if maestro_test_process::selection().is_child() {
        assert_eq!(
            report_with_skip_flag(),
            1,
            "a version-skip flag does not suppress reporting"
        );
        return;
    }
    let skip = [("MAESTRO_SKIP_VERSION_CHECK", "1")];
    services::native_probe(
        "install_reporting_keeps_telemetry_and_skip_independent",
        &skip,
        &["MAESTRO_OFFLINE"],
    );
    let mut settings = SettingsManager::in_memory(Settings::default());
    for setting in [false, true] {
        settings.set_enable_install_telemetry(setting);
        for (value, expected) in [
            (None, setting),
            (Some(""), false),
            (Some("0"), false),
            (Some("false"), false),
            (Some("1"), true),
            (Some("TRUE"), true),
            (Some("yes"), true),
            (Some(" yes"), false),
        ] {
            assert_eq!(
                is_install_telemetry_enabled(&settings, value),
                expected,
                "setting {setting}, telemetry {value:?}"
            );
        }
    }
}

/// Sends one report under the child's ambient version-skip flag, returning its call count.
fn report_with_skip_flag() -> usize {
    services::run(async {
        let mut settings = SettingsManager::in_memory(Settings::default());
        settings.set_enable_install_telemetry(true);
        let t = transport();
        report_install_telemetry("1.2.3", &settings, ORIGIN, Some(t.fetch.clone()), None).await;
        t.count()
    })
}
