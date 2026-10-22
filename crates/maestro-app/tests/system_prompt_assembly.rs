//! System prompt assembly against the recorded prompt texts.
#![cfg(not(target_arch = "wasm32"))]
use maestro_app::BuildSystemPromptOptions;
use maestro_app::resources::system_prompt::build_system_prompt;
use serde::Deserialize;
use serde_json::{Value, json};

/// The package directory the documentation paths are taken from.
const PACKAGE_DIR: &str = "/package";

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
    build_system_prompt(&options, PACKAGE_DIR).unwrap()
}

/// The local date as the prompt writes it.
fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Every recorded prompt, with the documentation paths of the package directory and the
/// local date read around the build.
#[test]
fn prompt_sections_match_recorded_text() {
    let readme = format!("{PACKAGE_DIR}/README.md");
    let docs = format!("{PACKAGE_DIR}/docs");
    let examples = format!("{PACKAGE_DIR}/examples");
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

/// Only the default prompt resolves documentation paths, so a relative package directory
/// in a removed working directory fails it and leaves a custom prompt working.
#[cfg(unix)]
#[test]
fn custom_prompt_never_reads_documentation_paths() {
    if maestro_test_process::selection().is_child() {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        scratch.enter_removed("cwd").unwrap();
        let options = |custom: Value| {
            let options = json!({"customPrompt": custom, "cwd": "/work"});
            serde_json::from_value::<BuildSystemPromptOptions>(options).unwrap()
        };
        assert!(build_system_prompt(&options(json!("Custom base")), "relative").is_ok());
        let error = build_system_prompt(&options(Value::Null), "relative").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        return;
    }
    let status =
        maestro_test_process::TestProcess::current("custom_prompt_never_reads_documentation_paths")
            .unwrap()
            .case("*")
            .status()
            .unwrap();
    assert!(status.success());
}
