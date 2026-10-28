//! System prompt assembly against the recorded prompt texts.
#![cfg(not(target_arch = "wasm32"))]
use maestro_app::BuildSystemPromptOptions;
use maestro_app::configuration::config::{get_docs_path, get_readme_path, paths};
use maestro_app::resources::system_prompt::build_system_prompt;
use maestro_test_process::{TestProcess, selection};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

/// The package directory override the recorded prompts are checked under.
const PACKAGE_DIR: &str = "/package";
/// The environment variable that overrides the package directory.
const PACKAGE_OVERRIDE: &str = "MAESTRO_PACKAGE_DIR";

/// One recorded prompt.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PromptCase {
    name: String,
    options: Value,
    expected: String,
}

/// Every recorded prompt case.
fn cases() -> Vec<PromptCase> {
    let corpus: Value =
        serde_json::from_str(include_str!("fixtures/resources/system_prompt.json")).unwrap();
    serde_json::from_value(corpus["cases"].clone()).unwrap()
}

/// The prompt built from `options`.
fn prompt(options: Value) -> String {
    let options: BuildSystemPromptOptions = serde_json::from_value(options).unwrap();
    build_system_prompt(&options).unwrap()
}

/// The local date as the prompt writes it.
fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Runs the calling test again in a child with the package override set to `value`, or
/// removed.
fn in_child(test: &str, value: Option<&str>) {
    let child = TestProcess::current(test).unwrap().case("*");
    let child = match value {
        Some(value) => child.env(PACKAGE_OVERRIDE, value),
        None => child.env_remove(PACKAGE_OVERRIDE),
    };
    assert!(child.status().unwrap().success());
}

/// Every recorded prompt, with the documentation paths of `package_dir` and the local date
/// read around the build.
fn assert_recorded_prompts(package_dir: &str) {
    // Resolved as the prompt resolves them: `/package` is on the working drive on Windows.
    let readme = paths::get_readme_path(package_dir).unwrap();
    let docs = paths::get_docs_path(package_dir).unwrap();
    let examples = paths::get_examples_path(package_dir).unwrap();
    for case in cases() {
        let before = today();
        let built = prompt(case.options);
        let after = today();
        let expected = |date: &str| {
            (case.expected.replace("$README", &readme))
                .replace("$DOCS", &docs)
                .replace("$EXAMPLES", &examples)
                .replace("$DATE", date)
        };
        assert!(
            built == expected(&before) || built == expected(&after),
            "case {}:\n{built}",
            case.name
        );
    }
}

/// The package override names the documentation paths.
#[test]
fn prompt_sections_match_recorded_text() {
    if !selection().is_child() {
        return in_child("prompt_sections_match_recorded_text", Some(PACKAGE_DIR));
    }
    assert_recorded_prompts(PACKAGE_DIR);
}

/// Without the override the documentation paths come from the running installation. A
/// test build runs from its source checkout, whose workspace root holds the README and the
/// documentation folder the prompt names.
#[test]
fn prompt_names_the_running_installation_without_override() {
    if !selection().is_child() {
        return in_child(
            "prompt_names_the_running_installation_without_override",
            None,
        );
    }
    assert!(Path::new(&get_readme_path().unwrap()).is_file());
    assert!(Path::new(&get_docs_path().unwrap()).is_dir());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2);
    assert_recorded_prompts(workspace.unwrap().to_str().unwrap());
}

/// Every documentation and example path in the default prompt names shipped content.
#[test]
fn default_prompt_paths_exist_in_the_installation() {
    if !selection().is_child() {
        return in_child("default_prompt_paths_exist_in_the_installation", None);
    }
    let built = prompt(serde_json::json!({"cwd": "/work"}));
    let docs = get_docs_path().unwrap();
    let package = Path::new(&docs).parent().unwrap();
    let relative = built
        .split(|character: char| character.is_whitespace() || "(),".contains(character))
        .filter(|word| word.starts_with("docs/") || word.starts_with("examples/"));
    let absolute = built.lines().filter_map(|line| {
        [
            "- Main documentation: ",
            "- Additional docs: ",
            "- Examples: ",
        ]
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix))
        .and_then(|rest| rest.split(" (extensions,").next())
    });
    let paths: Vec<_> = relative.chain(absolute).collect();
    assert!(!paths.is_empty(), "the prompt must name its documentation");
    let missing: Vec<_> = paths
        .into_iter()
        .filter(|path| !package.join(path).exists())
        .collect();
    assert!(
        missing.is_empty(),
        "missing shipped prompt paths: {missing:?}"
    );
}

/// Only the default prompt resolves documentation paths, so a relative package override in
/// a removed working directory fails it and leaves a custom prompt working.
#[cfg(unix)]
#[test]
fn custom_prompt_never_reads_documentation_paths() {
    if selection().is_child() {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        scratch.enter_removed("cwd").unwrap();
        let options = |custom: Value| {
            let options = serde_json::json!({"customPrompt": custom, "cwd": "/work"});
            serde_json::from_value::<BuildSystemPromptOptions>(options).unwrap()
        };
        assert!(build_system_prompt(&options(serde_json::json!("Custom base"))).is_ok());
        let error = build_system_prompt(&options(Value::Null)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        return;
    }
    in_child(
        "custom_prompt_never_reads_documentation_paths",
        Some("relative"),
    );
}
