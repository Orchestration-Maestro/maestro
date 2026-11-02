//! Recorded reference cases shared by the input test modules.

use serde::Deserialize;

/// Controlled package command harness.
#[cfg(unix)]
pub mod package_commands;

/// Presenter harness shared by the selector and configuration command tests.
pub mod presenters;

/// Controlled settings, catalogs and stored histories of the startup tests.
pub mod startup;

/// One recorded case: its group, its name, its input and its complete expected result.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recorded {
    /// The recorded case group.
    pub operation: String,
    /// The recorded case name.
    pub case: String,
    /// The recorded input.
    pub input: serde_json::Value,
    /// The complete recorded result.
    pub expected: serde_json::Value,
}

/// Read one recorded corpus by its fixture name.
pub fn group(name: &str) -> Vec<Recorded> {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("recorded corpus");
    serde_json::from_str(&text).expect("decodable recorded corpus")
}

/// Read every case of one group whose operation matches `operation`.
pub fn operation(name: &str, operation: &str) -> Vec<Recorded> {
    group(name)
        .into_iter()
        .filter(|case| case.operation == operation)
        .collect()
}

/// Read one recorded case by its name.
pub fn case_by_title(name: &str, title: &str) -> Recorded {
    group(name)
        .into_iter()
        .find(|case| case.case == title)
        .unwrap_or_else(|| panic!("missing recorded case {title}"))
}

/// The authored argument list of one case.
pub fn argv(case: &Recorded) -> Vec<String> {
    case.input
        .as_array()
        .expect("authored argument list")
        .iter()
        .map(|token| token.as_str().expect("authored token").to_owned())
        .collect()
}

/// Parse one case's authored arguments and compare the whole retained result.
pub fn assert_argv(case: &Recorded) {
    let actual = serde_json::to_value(maestro_cli::input::args::parse_args(&argv(case)))
        .expect("serializable arguments");
    assert_eq!(actual, case.expected, "{}", case.case);
}

/// Parse the case named `title` of the argument corpus.
pub fn assert_argv_title(title: &str) {
    assert_argv(&case_by_title("args", title));
}

/// Isolated missing-directory presentation harness.
pub mod missing_cwd;

/// Final dispatch recording presentations and streams.
pub mod mode_dispatch;

/// The startup entry over controlled effects.
#[cfg(unix)]
pub mod entry;
