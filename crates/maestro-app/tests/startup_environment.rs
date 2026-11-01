//! Startup offline and version-skip selections reaching the hosted-service gates.
#![cfg(test)]
#![cfg(unix)]
use crate::resource_fixtures::{Root, module_loader, with_isolated_home};
use crate::service_observations as services;
use crate::support::session_harness::on_task_set;
use maestro_app::configuration::telemetry::{
    is_install_telemetry_enabled, report_install_telemetry,
};
use maestro_app::configuration::version_check::{
    VersionCheckOptions, check_for_new_maestro_version, get_latest_maestro_release,
    get_latest_maestro_version,
};
use maestro_app::resources::resource_loader::{
    DefaultResourceLoader, DefaultResourceLoaderOptions, ResourceLoader, ResourceLoaderOptions,
};
use maestro_app::{
    AgentSession, AgentSessionServices, CreateAgentSessionFromServicesOptions,
    CreateAgentSessionServicesOptions, create_agent_session_from_services,
    create_agent_session_services,
};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::Local;
use maestro_session::SessionManager;
use maestro_settings::{Settings, SettingsManager};
use serde_json::json;
use services::{ORIGIN, Transport};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::task::LocalSet;

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

/// One ambient environment the release selection matrix runs under, with whether its
/// nonempty flags still disable the operations they belong to.
#[derive(Clone, Copy)]
struct Ambient {
    /// The variable values the child process receives.
    env: &'static [(&'static str, &'static str)],
    /// Whether the ambient offline flag disables its operation.
    offline: bool,
    /// Whether the ambient version-skip flag disables its operation.
    skip: bool,
}

/// The ambient environments: no flags, empty values, and each flag set on its own.
const AMBIENTS: [Ambient; 4] = [
    Ambient {
        env: &[],
        offline: false,
        skip: false,
    },
    Ambient {
        env: &[("MAESTRO_OFFLINE", ""), ("MAESTRO_SKIP_VERSION_CHECK", "")],
        offline: false,
        skip: false,
    },
    Ambient {
        env: &[("MAESTRO_OFFLINE", "1")],
        offline: true,
        skip: false,
    },
    Ambient {
        env: &[("MAESTRO_SKIP_VERSION_CHECK", "1")],
        offline: false,
        skip: true,
    },
];

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
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let home = scratch.path().display().to_string();
    let selected = maestro_test_process::run_per_environment(
        "startup_selection_skips_all_release_entrypoints",
        AMBIENTS,
        |ambient| {
            let mut environment =
                std::collections::BTreeMap::from([("HOME".to_owned(), home.clone())]);
            environment.extend(
                ambient
                    .env
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
            );
            environment
        },
    )
    .unwrap();
    let Some(selected) = selected else {
        return;
    };
    let ambient = selected[0];
    for selection in SELECTIONS {
        let suppressed =
            selection.0.unwrap_or(ambient.offline) || selection.1.unwrap_or(ambient.skip);
        let expected = usize::from(!suppressed);
        assert_eq!(
            services::run(release_gates(selection)),
            (expected, expected, expected),
            "{selection:?} under {:?}",
            ambient.env
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

/// A tree whose project settings configure one missing pinned package, with a controlled
/// package command that records each acquisition.
fn package_tree(base: &str, name: &str) -> (Root, std::path::PathBuf) {
    let root = Root::new(base, name);
    let marker = std::path::PathBuf::from(&root.path).join("acquired");
    std::fs::create_dir_all(format!("{}/.maestro", root.project())).unwrap();
    std::fs::write(
        format!("{}/.maestro/settings.json", root.project()),
        json!({
            "packages": ["npm:missing@1.0.0"],
            "npmCommand": ["/bin/sh", "-c", format!("printf acquired >> {}", marker.display())],
        })
        .to_string(),
    )
    .unwrap();
    (root, marker)
}

/// Reloads the real default loader over `root` and reports whether it acquired the package.
fn loader_acquires(root: &Root, offline: Option<bool>) -> bool {
    let local = Rc::new(LocalSet::new());
    let mut resources: ResourceLoaderOptions<Local> =
        ResourceLoaderOptions::new(Rc::clone(&local), module_loader());
    resources.offline = offline;
    let loader = DefaultResourceLoader::<Local>::new(DefaultResourceLoaderOptions {
        cwd: root.project(),
        agent_dir: root.agent(),
        settings_manager: None,
        resources,
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(local.run_until(async { loader.reload().await.unwrap() }));
    std::path::Path::new(&format!("{}/acquired", root.path)).exists()
}

#[test]
fn loader_startup_selection_reaches_configured_packages() {
    with_isolated_home(
        "loader_startup_selection_reaches_configured_packages",
        |base| {
            let (root, marker) = package_tree(base, "loader");
            assert!(!loader_acquires(&root, Some(true)));
            assert!(!marker.exists());
            assert!(loader_acquires(&root, None));
            assert!(marker.exists());
        },
    );
}

/// Creates the services over `root` with the two independent offline selections.
async fn services_for(
    root: &Root,
    local: &Rc<LocalSet>,
    packages: Option<bool>,
    search: Option<bool>,
) -> Rc<AgentSessionServices<Local>> {
    let mut resources: ResourceLoaderOptions<Local> =
        ResourceLoaderOptions::new(Rc::clone(local), module_loader());
    resources.offline = packages;
    let options = CreateAgentSessionServicesOptions {
        cwd: root.project(),
        agent_dir: Some(root.agent()),
        auth_storage: Some(Arc::new(AuthStorage::in_memory(AuthStorageData::default()))),
        settings_manager: None,
        model_registry: None,
        extension_flag_values: None,
        resource_loader_options: resources,
        offline: search,
    };
    create_agent_session_services(options)
        .await
        .map(Rc::new)
        .unwrap()
}

/// Creates the session the services configure, carrying their retained search selection.
async fn session_from(services: &Rc<AgentSessionServices<Local>>) -> Rc<AgentSession<Local>> {
    create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
        services: Rc::clone(services),
        session_manager: Rc::new(RefCell::new(
            SessionManager::in_memory(Some(&services.cwd)).unwrap(),
        )),
        session_start_event: None,
        model: None,
        thinking_level: None,
        scoped_models: Vec::new(),
        tools: None,
        no_tools: None,
        custom_tools: Vec::new(),
        theme: maestro_theme::LiveTheme::default(),
    })
    .await
    .expect("the session starts")
    .session
}

/// Runs both search tools inside `session` and reports the failure each missing program
/// causes.
async fn search_failures(session: &Rc<AgentSession<Local>>) -> [String; 2] {
    let mut failures = [String::new(), String::new()];
    for (slot, name) in failures.iter_mut().zip(["find", "grep"]) {
        let definition = session.get_tool_definition(name).expect("a search tool");
        let tool =
            maestro_extensions::wrap_registered_tool(&definition, session.extension_runner());
        *slot = (tool.execute)("call".to_owned(), json!({"pattern": "*"}), None, None)
            .await
            .expect_err("no search program is installed")
            .message;
    }
    failures
}

/// Builds the session over `services` and runs both search tools inside it.
async fn session_search(services: &Rc<AgentSessionServices<Local>>) -> [String; 2] {
    search_failures(&session_from(services).await).await
}

/// A session whose search selection is offline admits no program: both tools fail before
/// running one, and no staging entry or program appears in the binary directory.
fn assert_search_offline(services: &AgentSessionServices<Local>, failures: &[String; 2]) {
    assert_eq!(
        failures[0],
        "fd is not available and could not be downloaded"
    );
    assert_eq!(
        failures[1],
        "ripgrep (rg) is not available and could not be downloaded"
    );
    assert_no_search_program(services);
}

#[test]
fn session_startup_selection_reaches_find_and_grep() {
    with_isolated_home("session_startup_selection_reaches_find_and_grep", |base| {
        let (root, marker) = package_tree(base, "session");
        on_task_set(|local| async move {
            let services = services_for(&root, &local, Some(true), Some(false)).await;
            assert!(
                !marker.exists(),
                "the search selection must not change packages"
            );
            assert_eq!(services.offline, Some(false));
            assert_no_search_program(&services);
        });
    });
    with_isolated_home("session_startup_selection_reaches_find_and_grep", |base| {
        let (root, marker) = package_tree(base, "session-online");
        on_task_set(|local| async move {
            let services = services_for(&root, &local, None, Some(true)).await;
            assert!(marker.exists(), "the package selection stays independent");
            assert_eq!(services.offline, Some(true));
            assert_search_offline(&services, &session_search(&services).await);
        });
    });
}

/// A missing search program stays absent with no staging entry, because no download started.
fn assert_no_search_program(services: &AgentSessionServices<Local>) {
    let bin = maestro_app::configuration::config::paths::get_bin_dir(&services.agent_dir);
    let entries: Vec<String> = std::fs::read_dir(&bin)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        entries
            .iter()
            .all(|name| !(name == "fd" || name == "rg" || name.starts_with("extract_tmp_"))),
        "{entries:?}"
    );
}

/// A second session over the same services keeps the retained search selection, and a
/// separate service keeps its own.
#[test]
fn startup_selection_survives_service_replacement() {
    with_isolated_home("startup_selection_survives_service_replacement", |base| {
        let (root, marker) = package_tree(base, "replace");
        on_task_set(|local| async move {
            let selected = services_for(&root, &local, Some(true), Some(true)).await;
            assert!(!marker.exists());
            assert_search_offline(&selected, &session_search(&selected).await);
            assert_search_offline(&selected, &session_search(&selected).await);
            let online = services_for(&root, &local, None, Some(false)).await;
            assert_eq!(online.offline, Some(false));
            assert!(
                marker.exists(),
                "a separate service keeps its own selection"
            );
        });
    });
}
