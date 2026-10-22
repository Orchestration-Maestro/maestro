//! Default resource routing through real native consumers.
#![cfg(test)]
#![cfg(unix)]
use crate::default_configuration as fixture;
use fixture::{Group, child};
use maestro_app::configuration::config::{CONFIG_DIR_NAME, paths};
use maestro_app::{AuthStorage, ModelRegistry, SettingsManager};
use maestro_resources::{LoadPromptTemplatesOptions, load_prompt_templates};
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use serde::Deserialize;
use serde_json::Value;
use std::{path::Path, sync::Arc};

/// The environment override's selected source branch.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Override {
    Absent,
    Empty,
    Absolute,
    Relative,
    Tilde,
    Spaces,
    UnicodeBoundary,
    BackslashTilde,
    TildeDot,
}
/// Files consumed by selected constructors.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Files {
    settings: Value,
    auth: Value,
    models: Value,
    prompt: String,
}
/// Root selection operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RootInput {
    #[serde(rename = "override")]
    branch: Option<Override>,
    env: Option<String>,
    explicit_root: Option<String>,
    cwd: String,
    files: Files,
}
/// Selected prompt projection.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Prompt {
    name: String,
    content: String,
}
/// Custom model projection.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Model {
    id: String,
    provider: String,
    base_url: String,
}
/// Root selection and constructor observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RootExpected {
    agent: String,
    #[serde(rename = "getSettingsPath", alias = "settingsPath")]
    settings: String,
    #[serde(rename = "getAuthPath", alias = "authPath")]
    auth: String,
    #[serde(rename = "getModelsPath", alias = "modelsPath")]
    models: String,
    #[serde(rename = "getBinDir")]
    bin: Option<String>,
    #[serde(rename = "getPromptsDir")]
    prompts: Option<String>,
    #[serde(rename = "getSessionsDir")]
    sessions: Option<String>,
    theme: String,
    credential: Value,
    model: Model,
    prompt: Option<Prompt>,
    session_directory: String,
    session_cwd: String,
}
/// A package path and the marker files it must reach.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PackageInput {
    package_override: Option<String>,
    readme_content: String,
    tool_content: String,
}
/// Package and agent-bin observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PackageExpected {
    package_directory: String,
    readme: String,
    readme_text: String,
    bin: String,
    tool_text: String,
}

/// Observe selected settings, auth, model, prompt and session owners.
fn consumers(input: &RootInput, expected: RootExpected, agent: &str) {
    root_paths(agent, &expected);
    for (path, value) in [
        (&expected.settings, &input.files.settings),
        (&expected.auth, &input.files.auth),
        (&expected.models, &input.files.models),
    ] {
        fixture::write(path, &serde_json::to_string_pretty(value).unwrap());
    }
    fixture::write(
        format!("{}/selected-root.md", paths::get_prompts_dir(agent)),
        &input.files.prompt,
    );
    let manager = SettingsManager::create(
        std::env::current_dir().unwrap().to_str().unwrap(),
        agent,
        CONFIG_DIR_NAME,
    );
    assert_eq!(
        manager.get_theme().as_deref(),
        Some(expected.theme.as_str())
    );
    catalog(agent, expected.credential, &expected.model);
    let cwd = std::env::current_dir().unwrap();
    let home = std::env::var("HOME").unwrap();
    let prompts = load_prompt_templates(LoadPromptTemplatesOptions {
        cwd: cwd.to_str().unwrap(),
        home: &home,
        agent_dir: agent,
        config_dir_name: CONFIG_DIR_NAME,
        prompt_paths: &[],
        include_defaults: true,
    })
    .unwrap();
    let prompt = prompts
        .into_iter()
        .find(|p| p.name == "selected-root")
        .map(|p| Prompt {
            name: p.name,
            content: p.content,
        });
    assert_eq!(prompt, expected.prompt);
    let session = SessionManager::create(&input.cwd, None, agent).unwrap();
    assert_eq!(session.get_cwd(), expected.session_cwd);
    assert_eq!(session.get_session_dir(), expected.session_directory);
    assert!(Path::new(session.get_session_dir()).is_dir());
}
/// Select the same environment root through native and controlled observations.
fn roots(group: Group, input: &Value, expected: &Value, root: &str) {
    let input: RootInput = fixture::instantiate(input, root, "");
    let expected: RootExpected = fixture::instantiate(expected, root, "");
    let native = maestro_app::get_agent_dir().unwrap();
    let controlled =
        paths::get_agent_dir(input.env.as_deref(), || Ok(std::env::var("HOME").unwrap())).unwrap();
    assert_eq!(native, controlled);
    let agent = if group == Group::ExplicitRootPresenceOverridesEnvironment {
        input.explicit_root.as_ref().expect("present root").clone()
    } else {
        assert!(input.explicit_root.is_none());
        native
    };
    assert!(
        if group == Group::ExplicitRootPresenceOverridesEnvironment {
            input.branch.is_none()
        } else {
            input.branch.is_some()
        }
    );
    let decoy = if agent == format!("{root}/decoy") {
        format!("{root}/other")
    } else {
        format!("{root}/decoy")
    };
    fixture::write(
        format!("{decoy}/settings.json"),
        "{\"theme\":\"decoy-root\"}",
    );
    fixture::write(format!("{decoy}/auth.json"), "{}");
    fixture::write(format!("{decoy}/models.json"), "{}");
    fixture::write(
        format!("{decoy}/prompts/selected-root.md"),
        "Decoy prompt body",
    );
    consumers(&input, expected, &agent);
}
/// Compare native and controlled package observations, then read actual assets.
fn package(input: &Value, expected: &Value, root: &str) {
    use maestro_app::configuration::config::installation::{Installation, PackageLayout};
    let package = format!("{root}/distribution");
    let input: PackageInput = fixture::instantiate(input, root, &package);
    let expected: PackageExpected = fixture::instantiate(expected, root, &package);
    fixture::write(
        format!("{package}/Cargo.toml"),
        "[package]\nname = 'fixture'\n",
    );
    let installation = Installation {
        module_dir: format!("{package}/src"),
        executable: format!("{package}/bin/maestro"),
        layout: PackageLayout::Package,
    };
    let native_override = std::env::var("MAESTRO_PACKAGE_DIR").ok();
    assert_eq!(native_override, input.package_override);
    let home = std::env::var("HOME").unwrap();
    let native = paths::get_package_dir(
        &installation,
        native_override.as_deref(),
        || Ok(home.clone()),
        |p| Path::new(p).exists(),
    )
    .unwrap();
    let controlled = paths::get_package_dir(
        &installation,
        input.package_override.as_deref(),
        || Ok(home),
        |p| p == format!("{package}/Cargo.toml"),
    )
    .unwrap();
    assert_eq!(native, expected.package_directory);
    assert_eq!(controlled, expected.package_directory);
    let readme = paths::get_readme_path(&native).unwrap();
    assert_eq!(readme, expected.readme);
    fixture::write(&readme, &input.readme_content);
    assert_eq!(
        std::fs::read_to_string(&readme).unwrap(),
        expected.readme_text
    );
    let bin = paths::get_bin_dir(&maestro_app::get_agent_dir().unwrap());
    assert_eq!(bin, expected.bin);
    fixture::write(format!("{bin}/tool"), &input.tool_content);
    assert_eq!(
        std::fs::read_to_string(format!("{bin}/tool")).unwrap(),
        expected.tool_text
    );
}
/// Run a row in a disposable child with explicitly supplied ambient state.
fn child_case(name: &str, group: Group, index: usize) {
    let scratch = child::Scratch::new();
    let root = scratch.0.path().to_str().unwrap();
    let home = scratch.join("home");
    std::fs::create_dir(&home).unwrap();
    let rows = fixture::cases::<Value, Value>(group);
    let row = &rows[index];
    let mut envs = vec![
        ("HOME".to_owned(), home),
        ("MAESTRO_DEFAULT_ROOT".into(), root.into()),
    ];
    if group == Group::PackageAndBinaryPathsReachRealFiles {
        let input: PackageInput = fixture::instantiate(&row.input, root, "");
        if let Some(value) = input.package_override {
            envs.push(("MAESTRO_PACKAGE_DIR".into(), value));
        }
        envs.push(("MAESTRO_CODING_AGENT_DIR".into(), format!("{root}/decoy")));
    } else {
        let input: RootInput = fixture::instantiate(&row.input, root, "");
        match input.branch {
            Some(Override::Absent) => assert!(input.env.is_none()),
            _ => envs.push((
                "MAESTRO_CODING_AGENT_DIR".into(),
                input.env.expect("present environment operand"),
            )),
        }
    }
    let refs: Vec<_> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    assert_eq!(
        child::probe(
            maestro_test_process::TestProcess::current(name)
                .unwrap()
                .case(index.to_string()),
            scratch.0.path(),
            &refs,
            fixture::REMOVED
        ),
        ["routes-ok"]
    );
}
/// Exercise removal from an outer child with competing environment overrides.
fn qualify(name: &str, group: Group) {
    if maestro_test_process::selection().is_child()
        && !maestro_test_process::selection().runs_case("outer")
    {
        let root = std::env::var("MAESTRO_DEFAULT_ROOT").unwrap();
        let rows = fixture::cases::<Value, Value>(group);
        let row = rows
            .iter()
            .enumerate()
            .find(|(index, _)| maestro_test_process::selection().runs_case(&index.to_string()))
            .map(|(_, row)| row)
            .expect("selected route row");
        if group == Group::PackageAndBinaryPathsReachRealFiles {
            package(&row.input, &row.expected, &root);
        } else {
            roots(group, &row.input, &row.expected, &root);
        }
        println!("\nPROBE routes-ok");
    } else if std::env::var_os("MAESTRO_DEFAULT_OUTER").is_some() {
        for index in 0..fixture::cases::<Value, Value>(group).len() {
            child_case(name, group, index);
        }
        println!("\nPROBE outer-ok");
    } else {
        let parent = (
            std::env::current_dir().unwrap(),
            std::env::vars_os().collect::<Vec<_>>(),
        );
        let scratch = child::Scratch::new();
        assert_eq!(
            child::probe(
                maestro_test_process::TestProcess::current(name)
                    .unwrap()
                    .case("outer"),
                scratch.0.path(),
                &[
                    ("MAESTRO_DEFAULT_OUTER", "1"),
                    ("MAESTRO_CODING_AGENT_DIR", "inherited-decoy"),
                    ("MAESTRO_PACKAGE_DIR", "inherited-decoy")
                ],
                &[]
            ),
            ["outer-ok"]
        );
        assert_eq!(
            parent,
            (
                std::env::current_dir().unwrap(),
                std::env::vars_os().collect::<Vec<_>>()
            )
        );
    }
}
#[test]
fn resolved_roots_feed_native_and_controlled_consumers() {
    qualify(
        "resolved_roots_feed_native_and_controlled_consumers",
        Group::ResolvedRootsFeedNativeAndControlledConsumers,
    );
}

#[test]
fn explicit_root_presence_overrides_environment() {
    qualify(
        "explicit_root_presence_overrides_environment",
        Group::ExplicitRootPresenceOverridesEnvironment,
    );
}

#[test]
fn package_and_binary_paths_reach_real_files() {
    qualify(
        "package_and_binary_paths_reach_real_files",
        Group::PackageAndBinaryPathsReachRealFiles,
    );
}

/// Observe credentials and the custom model loaded from the selected files.
fn catalog(agent: &str, credential: Value, expected: &Model) {
    let auth = Arc::new(AuthStorage::create(&paths::get_auth_path(agent)));
    let credential: maestro_app::AuthCredential = serde_json::from_value(credential).unwrap();
    assert!(
        auth.get("qualification").as_ref() == Some(&credential),
        "selected dummy credential"
    );
    let registry = ModelRegistry::create(auth, paths::get_models_path(agent)).unwrap();
    assert!(registry.get_error().is_none());
    let model = registry
        .find(&expected.provider, &expected.id)
        .expect("selected custom model");
    let model = model.read().unwrap();
    assert_eq!(
        (
            model.id.as_str(),
            model.provider.as_str(),
            model.base_url.as_str()
        ),
        (
            expected.id.as_str(),
            expected.provider.as_str(),
            expected.base_url.as_str()
        )
    );
    drop(model);
}

/// Compare each recorded suffix without rediscovering the root.
fn root_paths(agent: &str, expected: &RootExpected) {
    assert_eq!(agent, expected.agent);
    assert_eq!(paths::get_settings_path(agent), expected.settings);
    assert_eq!(paths::get_auth_path(agent), expected.auth);
    assert_eq!(paths::get_models_path(agent), expected.models);
    if let Some(bin) = &expected.bin {
        assert_eq!(paths::get_bin_dir(agent), *bin);
    }
    if let Some(prompts) = &expected.prompts {
        assert_eq!(paths::get_prompts_dir(agent), *prompts);
    }
    if let Some(sessions) = &expected.sessions {
        assert_eq!(paths::get_sessions_dir(agent), *sessions);
    }
}
