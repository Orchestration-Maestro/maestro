//! Configuration queries over controlled observations.
#![cfg(test)]

use maestro_app::configuration::config::paths;
use serde::Deserialize;
use std::io;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case<I, E> {
    input: I,
    expected: E,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TildeCase {
    input: TildeInput,
    home: String,
    expected: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TildeInput {
    args: [String; 1],
}
#[test]
fn tilde_expansion_changes_only_supported_prefixes() {
    let cases: Vec<TildeCase> = serde_json::from_str(include_str!(
        "fixtures/configuration/tilde_expansion_changes_only_supported_prefixes.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 11);
    for case in cases {
        let written = &case.input.args[0];
        assert_eq!(
            paths::expand_tilde_path(written, || Ok(case.home.clone())).unwrap(),
            case.expected,
            "{written:?} with home {:?}",
            case.home
        );
    }
}

use maestro_app::configuration::config::installation::{Installation, PackageLayout};
#[cfg(not(windows))]
use std::cell::RefCell;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Environment {
    #[serde(rename = "MAESTRO_PACKAGE_DIR")]
    package: Option<String>,
    #[serde(rename = "MAESTRO_CODING_AGENT_DIR")]
    agent: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum Layout {
    #[default]
    Package,
    Standalone,
}
impl Layout {
    fn native(&self) -> PackageLayout {
        match self {
            Self::Package => PackageLayout::Package,
            Self::Standalone => PackageLayout::Standalone,
        }
    }
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PathInput {
    #[serde(default)]
    env: Environment,
    #[serde(default)]
    layout: Layout,
    module_dir: Option<String>,
    exec_path: Option<String>,
    exists: Option<Vec<String>>,
    platform: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    home: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    asset_name: Option<String>,
}
impl PathInput {
    fn installation(&self) -> Installation {
        Installation {
            module_dir: self.module_dir.clone().unwrap_or_else(|| {
                if self.windows() {
                    "C:\\package\\src"
                } else {
                    "/package/src"
                }
                .to_owned()
            }),
            executable: self
                .exec_path
                .clone()
                .unwrap_or_else(|| "/usr/bin/node".to_owned()),
            layout: self.layout.native(),
        }
    }
    fn windows(&self) -> bool {
        self.platform.as_deref() == Some("win32")
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn home(&self) -> String {
        self.home.clone().unwrap_or_else(|| {
            if self.windows() {
                "C:\\users\\sample"
            } else {
                "/home/sample"
            }
            .to_owned()
        })
    }
    fn exists(&self, path: &str) -> bool {
        self.exists.as_ref().map_or(
            path == if self.windows() {
                "C:\\package\\package.json"
            } else {
                "/package/package.json"
            },
            |entries| entries.iter().any(|p| p == path),
        )
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservedCase<I, E, O> {
    input: I,
    expected: E,
    observations: O,
}
#[cfg(not(windows))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExistsLog {
    exists: Vec<String>,
}
#[cfg(not(windows))]
fn check_package_cases(text: &str, count: usize) {
    let cases: Vec<ObservedCase<PathInput, String, ExistsLog>> =
        serde_json::from_str(text).unwrap();
    assert_eq!(cases.len(), count);
    for case in cases {
        let probes = RefCell::new(Vec::new());
        let result = paths::get_package_dir(
            || Ok(case.input.installation()),
            case.input.env.package.as_deref(),
            || Ok("/home/sample".to_owned()),
            |path| {
                probes.borrow_mut().push(path.to_owned());
                case.input.exists(path)
            },
        )
        .unwrap();
        assert_eq!(result, case.expected);
        assert_eq!(*probes.borrow(), case.observations.exists);
    }
}
#[cfg(not(windows))]
#[test]
fn package_directory_keeps_override_and_walk() {
    check_package_cases(
        include_str!("fixtures/configuration/package_directory_keeps_override_and_walk.json"),
        15,
    );
}

#[test]
fn identity_constants_share_branding() {
    use maestro_app::configuration::config::{
        APP_NAME, APP_TITLE, CONFIG_DIR_NAME, ENV_AGENT_DIR, ENV_SESSION_DIR, PACKAGE_NAME,
        SERVICE_ORIGIN,
    };
    assert_eq!(
        (PACKAGE_NAME, APP_NAME, APP_TITLE, CONFIG_DIR_NAME),
        ("maestro", "maestro", "Maestro", ".maestro")
    );
    assert_eq!(maestro_app::VERSION, env!("CARGO_PKG_VERSION"));
    assert_eq!(&*ENV_AGENT_DIR, "MAESTRO_CODING_AGENT_DIR");
    assert_eq!(&*ENV_SESSION_DIR, "MAESTRO_CODING_AGENT_SESSION_DIR");
    assert_eq!(SERVICE_ORIGIN, "");
}

use std::cell::Cell;

/// Run the calling test in a child whose working directory is a fresh `work` directory,
/// or one that no longer exists when `removed`. Returns the child's working directory
/// in the child, `None` in the calling process once the child passed.
#[cfg(not(target_arch = "wasm32"))]
fn in_work_directory(test: &str, removed: bool) -> Option<String> {
    let case = if removed { "removed-work" } else { "work" };
    let selection = maestro_test_process::selection();
    if !selection.is_child() {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let work = scratch.path().join("work");
        std::fs::create_dir(&work).unwrap();
        let output = maestro_test_process::TestProcess::current(test)
            .unwrap()
            .nocapture()
            .case(case)
            .current_dir(&work)
            .output()
            .unwrap();
        assert!(output.status.success(), "{case}: {output:?}");
        return None;
    }
    if !selection.runs_case(case) {
        return None;
    }
    let work = std::env::current_dir().unwrap();
    if removed {
        std::fs::remove_dir(&work).unwrap();
    }
    Some(work.to_str().unwrap().to_owned())
}

/// Replace the recorded working directory at the start of quoted fixture paths with `cwd`.
#[cfg(not(target_arch = "wasm32"))]
fn at_work(text: &str, cwd: &str) -> String {
    let recorded = if cfg!(windows) {
        r#""C:\\work"#
    } else {
        r#""/work"#
    };
    let replacement = serde_json::to_string(cwd).unwrap();
    text.replace(recorded, &replacement[..replacement.len() - 1])
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Paths {
    #[serde(rename = "getPackageDir")]
    package_dir: String,
    #[serde(rename = "getThemesDir")]
    themes_dir: String,
    #[serde(rename = "getExportTemplateDir")]
    export_template_dir: String,
    #[serde(rename = "getReadmePath")]
    readme_path: String,
    #[serde(rename = "getDocsPath")]
    docs_path: String,
    #[serde(rename = "getExamplesPath")]
    examples_path: String,
    #[serde(rename = "getChangelogPath")]
    changelog_path: String,
    #[serde(rename = "getInteractiveAssetsDir")]
    interactive_assets_dir: String,
    #[serde(rename = "getAgentDir")]
    agent_dir: String,
    #[serde(rename = "getCustomThemesDir")]
    custom_themes_dir: String,
    #[serde(rename = "getModelsPath")]
    models_path: String,
    #[serde(rename = "getAuthPath")]
    auth_path: String,
    #[serde(rename = "getSettingsPath")]
    settings_path: String,
    #[serde(rename = "getToolsDir")]
    tools_dir: String,
    #[serde(rename = "getBinDir")]
    bin_dir: String,
    #[serde(rename = "getPromptsDir")]
    prompts_dir: String,
    #[serde(rename = "getSessionsDir")]
    sessions_dir: String,
    #[serde(rename = "getDebugLogPath")]
    debug_log_path: String,
    #[serde(rename = "getBundledInteractiveAssetPath")]
    bundled_interactive_asset_path: String,
}
#[cfg(not(target_arch = "wasm32"))]
fn observe_paths(input: &PathInput) -> Paths {
    let installation = input.installation();
    let package = paths::get_package_dir(
        || Ok(input.installation()),
        input.env.package.as_deref(),
        || Ok(input.home()),
        |p| input.exists(p),
    )
    .unwrap();
    let agent = paths::get_agent_dir(input.env.agent.as_deref(), || Ok(input.home())).unwrap();
    let assets =
        paths::get_interactive_assets_dir(&package, installation.layout, |p| input.exists(p));
    Paths {
        package_dir: package.clone(),
        themes_dir: paths::get_themes_dir(&package, installation.layout),
        export_template_dir: paths::get_export_template_dir(&package, installation.layout, |p| {
            input.exists(p)
        }),
        readme_path: paths::get_readme_path(&package).unwrap(),
        docs_path: paths::get_docs_path(&package).unwrap(),
        examples_path: paths::get_examples_path(&package).unwrap(),
        changelog_path: paths::get_changelog_path(&package).unwrap(),
        interactive_assets_dir: assets.clone(),
        agent_dir: agent.clone(),
        custom_themes_dir: paths::get_custom_themes_dir(&agent),
        models_path: paths::get_models_path(&agent),
        auth_path: paths::get_auth_path(&agent),
        settings_path: paths::get_settings_path(&agent),
        tools_dir: paths::get_tools_dir(&agent),
        bin_dir: paths::get_bin_dir(&agent),
        prompts_dir: paths::get_prompts_dir(&agent),
        sessions_dir: paths::get_sessions_dir(&agent),
        debug_log_path: paths::get_debug_log_path(&agent),
        bundled_interactive_asset_path: paths::get_bundled_interactive_asset_path(
            &assets,
            input.asset_name.as_deref().unwrap_or("badge.png"),
        ),
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn check_paths(name: &str, text: &str, expected_rows: usize) {
    let Some(cwd) = in_work_directory(name, false) else {
        return;
    };
    let cases: Vec<Case<PathInput, Paths>> = serde_json::from_str(&at_work(text, &cwd)).unwrap();
    let mut consumed = 0;
    for case in cases {
        if case.input.windows() != cfg!(windows) {
            continue;
        }
        assert_eq!(observe_paths(&case.input), case.expected);
        consumed += 1;
    }
    assert_eq!(consumed, expected_rows);
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn asset_and_user_paths_select_layout() {
    check_paths(
        "asset_and_user_paths_select_layout",
        include_str!("fixtures/configuration/asset_and_user_paths_select_layout.json"),
        if cfg!(windows) { 3 } else { 5 },
    );
}

#[cfg(all(not(windows), not(target_arch = "wasm32")))]
#[test]
fn agent_directory_keeps_override_precedence() {
    check_paths(
        "agent_directory_keeps_override_precedence",
        include_str!("fixtures/configuration/agent_directory_keeps_override_precedence.json"),
        9,
    );
}

#[cfg(not(windows))]
#[test]
fn rust_package_markers_preserve_package_walk() {
    check_package_cases(
        include_str!("fixtures/configuration/rust_package_markers_preserve_package_walk.json"),
        3,
    );
}

#[cfg(not(windows))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HomeInput {
    home: String,
}
#[cfg(not(windows))]
#[test]
fn home_observations_keep_present_empty_values() {
    let cases: Vec<Case<HomeInput, String>> = serde_json::from_str(include_str!(
        "fixtures/configuration/home_observations_keep_present_empty_values.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 2);
    for case in cases {
        assert_eq!(
            paths::get_agent_dir(None, || Ok(case.input.home)).unwrap(),
            case.expected
        );
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Expected {
    Path(String),
    Error(ErrorKind),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorKind {
    error_kind: String,
}
fn check_result(result: io::Result<String>, expected: Expected) {
    match expected {
        Expected::Path(path) => assert_eq!(result.unwrap(), path),
        Expected::Error(error) => {
            let actual = result.unwrap_err();
            assert_eq!(format!("{:?}", actual.kind()), error.error_kind);
            assert_eq!(actual.raw_os_error(), Some(2));
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct HomeEffectInput {
    op: HomeOperation,
    env: Environment,
    #[serde(default)]
    args: Vec<String>,
    home_error: bool,
}
#[derive(Deserialize)]
enum HomeOperation {
    #[serde(rename = "getPackageDir")]
    Package,
    #[serde(rename = "getAgentDir")]
    Agent,
    #[serde(rename = "expandTildePath")]
    Tilde,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HomeLog {
    home_reads: usize,
}
#[test]
fn directory_operations_read_home_only_when_needed() {
    let cases: Vec<ObservedCase<HomeEffectInput, Expected, HomeLog>> = serde_json::from_str(
        include_str!("fixtures/configuration/directory_operations_read_home_only_when_needed.json"),
    )
    .unwrap();
    assert_eq!(cases.len(), 5);
    for case in cases {
        let reads = Cell::new(0);
        let home = || {
            reads.set(reads.get() + 1);
            if case.input.home_error {
                Err(io::Error::from_raw_os_error(2))
            } else {
                Ok("/home/sample".to_owned())
            }
        };
        let result = match case.input.op {
            HomeOperation::Package => paths::get_package_dir(
                || panic!("override must skip the installation"),
                case.input.env.package.as_deref(),
                home,
                |_| panic!("override must skip marker probes"),
            ),
            HomeOperation::Agent => paths::get_agent_dir(case.input.env.agent.as_deref(), home),
            HomeOperation::Tilde => paths::expand_tilde_path(&case.input.args[0], home),
        };
        check_result(result, case.expected);
        assert_eq!(reads.get(), case.observations.home_reads);
    }
}

#[cfg(all(not(windows), not(target_arch = "wasm32")))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CwdInput {
    op: TextOperation,
    env: Environment,
    cwd_error: bool,
}
#[cfg(all(not(windows), not(target_arch = "wasm32")))]
#[derive(Deserialize)]
enum TextOperation {
    #[serde(rename = "getReadmePath")]
    Readme,
    #[serde(rename = "getDocsPath")]
    Docs,
    #[serde(rename = "getExamplesPath")]
    Examples,
    #[serde(rename = "getChangelogPath")]
    Changelog,
}
#[cfg(all(not(windows), not(target_arch = "wasm32")))]
#[test]
fn packaged_text_paths_read_cwd_only_when_needed() {
    let text =
        include_str!("fixtures/configuration/packaged_text_paths_read_cwd_only_when_needed.json");
    let name = "packaged_text_paths_read_cwd_only_when_needed";
    for removed in [false, true] {
        let Some(cwd) = in_work_directory(name, removed) else {
            continue;
        };
        let cases: Vec<Case<CwdInput, Expected>> =
            serde_json::from_str(&at_work(text, &cwd)).unwrap();
        assert_eq!(cases.len(), 8);
        for case in cases
            .into_iter()
            .filter(|case| case.input.cwd_error == removed)
        {
            let root = case.input.env.package.as_deref().unwrap();
            let result = match case.input.op {
                TextOperation::Readme => paths::get_readme_path(root),
                TextOperation::Docs => paths::get_docs_path(root),
                TextOperation::Examples => paths::get_examples_path(root),
                TextOperation::Changelog => paths::get_changelog_path(root),
            };
            check_result(result, case.expected);
        }
    }
}

#[cfg(unix)]
use crate::child;
#[cfg(unix)]
#[test]
fn native_agent_root_reads_disposable_environment() {
    if maestro_test_process::selection().is_child() {
        let matches_expected = maestro_app::get_agent_dir().unwrap()
            == std::env::var("MAESTRO_CONFIGURATION_EXPECTED").unwrap();
        assert!(matches_expected);
        println!("\nPROBE agent-directory-ok");
        println!("PROBE {}", maestro_app::VERSION);
        return;
    }
    let scratch = child::Scratch::new();
    let home = scratch.join("home");
    for (override_dir, expected) in [
        ("", format!("{home}/.maestro/agent")),
        ("~/state", format!("{home}/state")),
        ("../raw/./agent", "../raw/./agent".to_owned()),
    ] {
        let output = child::probe(
            maestro_test_process::TestProcess::current(
                "native_agent_root_reads_disposable_environment",
            )
            .unwrap()
            .case("*"),
            scratch.0.path(),
            &[
                ("MAESTRO_CONFIGURATION_EXPECTED", &expected),
                ("HOME", &home),
                ("MAESTRO_CODING_AGENT_DIR", override_dir),
            ],
            &[],
        );
        assert_eq!(output, ["agent-directory-ok", env!("CARGO_PKG_VERSION")]);
    }
}

/// A zero-match child must not qualify as an executed probe.
#[cfg(unix)]
#[test]
fn native_probe_rejects_missing_selection() {
    let scratch = child::Scratch::new();
    assert!(
        std::panic::catch_unwind(|| {
            child::probe(
                maestro_test_process::TestProcess::named("configuration_paths::missing_probe")
                    .unwrap(),
                scratch.0.path(),
                &[],
                &[],
            )
        })
        .is_err(),
        "unselected probe returned success"
    );
}

/// The shared probe keeps cwd, ordered edits and record framing.
#[cfg(unix)]
#[test]
fn native_probe_preserves_cwd_and_environment_order() {
    const KEY: &str = "MAESTRO_PROBE_ORDER";
    if let Ok(expected) = std::env::var("MAESTRO_PROBE_CWD") {
        assert_eq!(
            std::env::current_dir().unwrap(),
            std::path::Path::new(&expected)
        );
        assert_eq!(std::env::var(KEY).unwrap(), "replacement");
        assert_eq!(
            std::env::var("PATH").ok(),
            std::env::var("MAESTRO_PROBE_PATH").ok()
        );
        println!("\nPROBE first");
        println!("unrelated output");
        println!("PROBE second");
        return;
    }
    let before = std::env::var_os(KEY);
    let scratch = child::Scratch::new();
    let records = child::probe(
        maestro_test_process::TestProcess::current(
            "native_probe_preserves_cwd_and_environment_order",
        )
        .unwrap()
        .case("*"),
        scratch.0.path(),
        &[
            ("MAESTRO_PROBE_CWD", scratch.0.path().to_str().unwrap()),
            (KEY, "replacement"),
            ("MAESTRO_PROBE_PATH", &std::env::var("PATH").unwrap()),
        ],
        &[KEY],
    );
    assert_eq!(records, ["first", "second"]);
    assert_eq!(std::env::var_os(KEY), before);
}

#[test]
fn workspace_skill_path_resolves_the_shipped_safety_guide() {
    let root = if cfg!(windows) {
        r"C:\maestro"
    } else {
        "/opt/maestro"
    };
    assert_eq!(
        std::path::Path::new(&paths::get_workspace_skill_path(root).unwrap()),
        std::path::Path::new(root).join("docs/skills/workspace/SKILL.md")
    );
    let package = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let path = paths::get_workspace_skill_path(package).unwrap();
    let skill = std::fs::read_to_string(path).unwrap();
    for instruction in [
        "name: workspace",
        "MAESTRO_ENV",
        "--help",
        "--current",
        "maestro attach --daemon",
        "Do not answer an approval",
        "Do not use bare `maestro` for discovery",
    ] {
        assert!(skill.contains(instruction), "missing {instruction}");
    }
}
