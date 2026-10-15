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
struct TildeInput {
    args: [String; 1],
}
#[test]
fn tilde_expansion_changes_only_supported_prefixes() {
    let cases: Vec<Case<TildeInput, String>> = serde_json::from_str(include_str!(
        "fixtures/configuration/tilde_expansion_changes_only_supported_prefixes.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        assert_eq!(
            paths::expand_tilde_path(&case.input.args[0], || Ok("/home/sample".to_owned()))
                .unwrap(),
            case.expected
        );
    }
}

use maestro_app::configuration::config::installation::{Installation, PackageLayout};
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
    home: Option<String>,
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExistsLog {
    exists: Vec<String>,
}
fn check_package_cases(text: &str, count: usize) {
    let cases: Vec<ObservedCase<PathInput, String, ExistsLog>> =
        serde_json::from_str(text).unwrap();
    assert_eq!(cases.len(), count);
    for case in cases {
        let probes = RefCell::new(Vec::new());
        let result = paths::get_package_dir(
            &case.input.installation(),
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

use maestro_app::{ResourceEntry, ResourceFileType, ResourceOperations};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
};
struct Directories {
    cwd: io::Result<String>,
    reads: Cell<usize>,
}
impl ResourceOperations for Directories {
    fn exists(&self, _: &Path) -> bool {
        panic!("unexpected exists")
    }
    fn read_dir(&self, _: &Path) -> io::Result<Vec<ResourceEntry>> {
        panic!("unexpected read_dir")
    }
    fn read_file(&self, _: &Path) -> io::Result<String> {
        panic!("unexpected read_file")
    }
    fn metadata(&self, _: &Path) -> io::Result<ResourceFileType> {
        panic!("unexpected metadata")
    }
    fn symlink_metadata(&self, _: &Path) -> io::Result<ResourceFileType> {
        panic!("unexpected symlink_metadata")
    }
    fn canonicalize(&self, _: &Path) -> io::Result<PathBuf> {
        panic!("unexpected canonicalize")
    }
    fn current_directory(&self) -> io::Result<String> {
        self.reads.set(self.reads.get() + 1);
        self.cwd
            .as_ref()
            .cloned()
            .map_err(|_| io::Error::from_raw_os_error(2))
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        Vec::new()
    }
}

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
fn observe_paths(input: &PathInput) -> Paths {
    let installation = input.installation();
    let package = paths::get_package_dir(
        &installation,
        input.env.package.as_deref(),
        || Ok(input.home()),
        |p| input.exists(p),
    )
    .unwrap();
    let agent = paths::get_agent_dir(input.env.agent.as_deref(), || Ok(input.home())).unwrap();
    let operations = Directories {
        cwd: Ok(if input.windows() { "C:\\work" } else { "/work" }.to_owned()),
        reads: Cell::new(0),
    };
    let assets =
        paths::get_interactive_assets_dir(&package, installation.layout, |p| input.exists(p));
    Paths {
        package_dir: package.clone(),
        themes_dir: paths::get_themes_dir(&package, installation.layout, |p| input.exists(p)),
        export_template_dir: paths::get_export_template_dir(&package, installation.layout, |p| {
            input.exists(p)
        }),
        readme_path: paths::get_readme_path(&package, &operations).unwrap(),
        docs_path: paths::get_docs_path(&package, &operations).unwrap(),
        examples_path: paths::get_examples_path(&package, &operations).unwrap(),
        changelog_path: paths::get_changelog_path(&package, &operations).unwrap(),
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
fn check_paths(text: &str, expected_rows: usize) {
    let cases: Vec<Case<PathInput, Paths>> = serde_json::from_str(text).unwrap();
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
#[test]
fn asset_and_user_paths_select_layout() {
    check_paths(
        include_str!("fixtures/configuration/asset_and_user_paths_select_layout.json"),
        if cfg!(windows) { 3 } else { 5 },
    );
}

#[cfg(not(windows))]
#[test]
fn agent_directory_keeps_override_precedence() {
    check_paths(
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
                &PathInput::default().installation(),
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CwdInput {
    op: TextOperation,
    env: Environment,
    cwd_error: bool,
}
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
#[cfg(not(windows))]
#[test]
fn packaged_text_paths_read_cwd_only_when_needed() {
    let cases: Vec<Case<CwdInput, Expected>> = serde_json::from_str(include_str!(
        "fixtures/configuration/packaged_text_paths_read_cwd_only_when_needed.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let operations = Directories {
            cwd: if case.input.cwd_error {
                Err(io::Error::from_raw_os_error(2))
            } else {
                Ok("/work".to_owned())
            },
            reads: Cell::new(0),
        };
        let root = case.input.env.package.as_deref().unwrap();
        let result = match case.input.op {
            TextOperation::Readme => paths::get_readme_path(root, &operations),
            TextOperation::Docs => paths::get_docs_path(root, &operations),
            TextOperation::Examples => paths::get_examples_path(root, &operations),
            TextOperation::Changelog => paths::get_changelog_path(root, &operations),
        };
        let needs_cwd = matches!(&case.expected, Expected::Error(_));
        check_result(result, case.expected);
        assert_eq!(operations.reads.get(), usize::from(needs_cwd));
    }
}

#[cfg(unix)]
#[path = "support/child.rs"]
mod child;
#[cfg(unix)]
#[test]
fn native_agent_root_reads_disposable_environment() {
    if std::env::var_os("MAESTRO_CONFIGURATION_CHILD").is_some() {
        println!("\nPROBE {}", maestro_app::get_agent_dir().unwrap());
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
            "native_agent_root_reads_disposable_environment",
            &scratch.0,
            &[
                ("MAESTRO_CONFIGURATION_CHILD", "1"),
                ("HOME", &home),
                ("MAESTRO_CODING_AGENT_DIR", override_dir),
            ],
        );
        assert_eq!(output, [expected, env!("CARGO_PKG_VERSION").to_owned()]);
    }
}
