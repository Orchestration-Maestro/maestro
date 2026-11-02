//! Cases grouped into one cleared child per distinct environment.
#![cfg(test)]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use maestro_test_process::{TestProcess, run_per_environment};

/// Variable naming the file children append their observations to.
const MARKER: &str = "MTP_MARKER";

/// The marker file: created by the calling process, inherited by its children.
fn marker(test: &str) -> PathBuf {
    std::env::var_os(MARKER).map_or_else(
        || {
            let path = std::env::temp_dir().join(format!(
                "maestro-test-process-{}-{test}",
                std::process::id()
            ));
            std::fs::write(&path, "").unwrap();
            path
        },
        PathBuf::from,
    )
}

/// Each case's variables: the shared marker plus its own optional value.
fn variables<'a>(marker: &'a str, value: Option<&'a str>) -> BTreeMap<&'a str, &'a str> {
    let mut variables = BTreeMap::from([(MARKER, marker)]);
    variables.extend(value.map(|value| ("MTP_VALUE", value)));
    variables
}

#[test]
fn equal_environments_share_one_cleared_child() {
    assert_selection_owner();
    let test = "equal_environments_share_one_cleared_child";
    let path = marker(test);
    let marker = path.to_str().unwrap();
    let cases = [
        ("a", Some("1")),
        ("b", Some("2")),
        ("c", Some("1")),
        ("d", None),
    ];
    let selected = run_per_environment(test, cases, |case| variables(marker, case.1)).unwrap();
    if let Some(group) = selected {
        assert_group_selection(test, true);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        for (case, _) in group {
            let value = std::env::var("MTP_VALUE").unwrap_or_else(|_| "-".into());
            let ambient = std::env::var_os("PATH").is_some();
            writeln!(file, "{} {case} {value} {ambient}", std::process::id()).unwrap();
        }
        return;
    }
    assert_group_selection(test, false);
    let lines = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let rows: BTreeMap<&str, (&str, &str, &str)> = lines
        .lines()
        .map(|line| {
            let fields: Vec<_> = line.split(' ').collect();
            (fields[1], (fields[0], fields[2], fields[3]))
        })
        .collect();
    assert_eq!(rows.len(), 4, "{lines}");
    assert_eq!(rows["a"].0, rows["c"].0, "{lines}");
    assert_ne!(rows["a"].0, rows["b"].0, "{lines}");
    assert_ne!(rows["b"].0, rows["d"].0, "{lines}");
    assert_ne!(rows["a"].0, rows["d"].0, "{lines}");
    let values: Vec<_> = ["a", "b", "c", "d"].map(|case| rows[case].1).to_vec();
    assert_eq!(values, ["1", "2", "1", "-"], "{lines}");
    assert!(rows.values().all(|row| row.2 == "false"), "{lines}");
    let first_group: Vec<_> = lines
        .lines()
        .filter(|line| line.starts_with(rows["a"].0))
        .collect();
    assert_eq!(first_group.len(), 2, "{lines}");
    assert!(
        first_group[0].contains(" a ") && first_group[1].contains(" c "),
        "{lines}"
    );
}

#[test]
fn a_failed_child_reports_its_group_and_streams() {
    let test = "a_failed_child_reports_its_group_and_streams";
    let selected = run_per_environment(test, ["bad"], |_| BTreeMap::<&str, &str>::new());
    match selected {
        Ok(Some(group)) => panic!("case {} failed in its child", group[0]),
        Ok(None) => panic!("a failing child passed"),
        Err(error) => {
            let error = error.to_string();
            assert!(
                error.contains("environment group 0 failed")
                    && error.contains("case bad failed in its child"),
                "{error}"
            );
        }
    }
}

#[test]
fn an_unknown_group_is_refused() {
    let test = "an_unknown_group_is_refused";
    if std::env::var_os("MTP_CASE_PROBE").is_some() {
        let selection = maestro_test_process::selection();
        let wildcard = std::env::var("MTP_CASE_PROBE").unwrap() == "wildcard";
        assert!(selection.is_child());
        assert!(selection.runs_case("selected"));
        assert_eq!(selection.runs_case("different"), wildcard);
        assert_eq!(selection.runs_group(test), wildcard);
        assert_eq!(selection.runs_group("different_test"), wildcard);
        return;
    }
    if std::env::var_os("MTP_PROBE").is_some() {
        let error = run_per_environment(test, [1], |_| BTreeMap::<&str, &str>::new()).unwrap_err();
        assert!(error.to_string().contains("does not exist"), "{error}");
        return;
    }
    for (case, role) in [("selected", "specific"), ("*", "wildcard")] {
        let output = TestProcess::current(test)
            .unwrap()
            .env_clear()
            .env("MTP_CASE_PROBE", role)
            .case(case)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
    let output = TestProcess::current(test)
        .unwrap()
        .env("MTP_PROBE", "1")
        .env("MAESTRO_TEST_GROUP", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn cleared_children_keep_the_temporary_directory() {
    let test = "cleared_children_keep_the_temporary_directory";
    if !maestro_test_process::selection().is_child() {
        let root = maestro_test_directory::Scratch::new().unwrap();
        let output = TestProcess::current(test)
            .unwrap()
            .case("*")
            .temp_dir(root.path())
            .output();
        let output = output.unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    // The wildcard child runs under the directory its parent chose; its cleared group
    // child must see the same one.
    let expected = std::env::temp_dir();
    let selected = run_per_environment(test, [()], |()| {
        BTreeMap::from([("MTP_EXPECTED_TEMP", expected.clone())])
    });
    if selected.unwrap().is_some() {
        let expected = std::env::var_os("MTP_EXPECTED_TEMP").unwrap();
        assert_eq!(std::env::temp_dir(), PathBuf::from(expected));
    }
}

#[test]
fn cleared_children_open_sockets() {
    let test = "cleared_children_open_sockets";
    if maestro_test_process::selection().is_child() {
        std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        return;
    }
    let output = TestProcess::current(test)
        .unwrap()
        .env_clear()
        .case("*")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// Selection transport names that callers must never read or write.
const SELECTION_VARIABLES: &[&str] = &[
    "MAESTRO_CHILD_CASE",
    "MAESTRO_TEST_GROUP",
    "MAESTRO_ARTIFACT_CHILD",
    "MAESTRO_AUTH_CASE",
    "MAESTRO_AUTH_CHILD",
    "MAESTRO_BINDING_CHILD",
    "MAESTRO_BROWSER_CHILD",
    "MAESTRO_CACHE_CHILD",
    "MAESTRO_CLIPBOARD_CHILD",
    "MAESTRO_COMPLETION_HOME_CHILD",
    "MAESTRO_CONFIGURATION_CHILD",
    "MAESTRO_CONFIG_CHILD",
    "MAESTRO_CONTEXT_DATE_CHILD",
    "MAESTRO_CONVERSION_TIME_CHILD",
    "MAESTRO_COUNT_ENTRY_CHILD",
    "MAESTRO_DATE_CHILD",
    "MAESTRO_DEFAULT_CASE",
    "MAESTRO_DIRECTORY_CHILD",
    "MAESTRO_DISPLAY_CHILD",
    "MAESTRO_DYNAMIC_CHILD",
    "MAESTRO_EDITOR_CHILD",
    "MAESTRO_ENV_CHILD",
    "MAESTRO_EXTRACTION_CHILD",
    "MAESTRO_HISTORY_CWD_CHILD",
    "MAESTRO_HISTORY_UTC_CHILD",
    "MAESTRO_INHERITED_STDOUT_CHILD",
    "MAESTRO_KEYS_CHILD",
    "MAESTRO_LINK_COLLISION_CHILD",
    "MAESTRO_LOCALE_CASE",
    "MAESTRO_METADATA_CHILD",
    "MAESTRO_OUTPUT_PROBE",
    "MAESTRO_PACKAGE_CASE",
    "MAESTRO_RAW_FAILURE_CHILD",
    "MAESTRO_READ_CHILD",
    "MAESTRO_REMOVED_CWD_CHILD",
    "MAESTRO_REQUEST_CASE",
    "MAESTRO_RESOURCE_DIRECTORY_CHILD",
    "MAESTRO_RESPONSE_STDERR_CHILD",
    "MAESTRO_SELECTOR_KEYS_CHILD",
    "MAESTRO_SERVICE_CASE",
    "MAESTRO_SERVICE_PROBE",
    "MAESTRO_SHELL_SEQUENCE_CHILD",
    "MAESTRO_CAPTURE_STAGE",
    "MAESTRO_NATIVE_MODE",
    "MAESTRO_PROFILE_STAGE",
    "MAESTRO_NATIVE_STAGE",
];

/// Selection variables belong to the process owner, not its callers.
fn assert_selection_owner() {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let mut pending = vec![crates];
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() == "maestro-test-process" {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !SELECTION_VARIABLES
                        .iter()
                        .any(|marker| source.contains(marker)),
                    "{} reads or writes the private child-selection protocol",
                    path.display()
                );
            }
        }
    }
}

/// Parent and grouped-child roles dispatch groups without dispatching named cases.
fn assert_group_selection(test: &str, child: bool) {
    let selection = maestro_test_process::selection();
    assert_eq!(selection.is_child(), child);
    assert_eq!(selection.runs_case("any case"), !child);
    assert!(selection.runs_group(test));
    assert_eq!(selection.runs_group("different_test"), !child);
}
