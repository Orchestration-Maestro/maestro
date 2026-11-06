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
            let path = maestro_test_directory::temporary_root().join(format!(
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
fn variables<'a>(marker: &'a str, value: Option<&'a str>) -> BTreeMap<&'a str, Option<&'a str>> {
    let mut variables = BTreeMap::from([(MARKER, Some(marker))]);
    variables.extend(value.map(|value| ("MTP_VALUE", Some(value))));
    variables
}

#[test]
fn equal_environments_share_one_cleared_child() {
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
    let selected = run_per_environment(test, ["bad"], |_| BTreeMap::<&str, Option<&str>>::new());
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
fn a_passing_child_reports_the_premise_it_skipped_a_case_for() {
    /// What the group child states before it leaves its only case out.
    const PREMISE: &str = "premise not met: this host keeps no such record";
    let test = "a_passing_child_reports_the_premise_it_skipped_a_case_for";
    if maestro_test_process::selection().is_child() {
        let selected =
            run_per_environment(test, ["skipped"], |_| BTreeMap::<&str, Option<&str>>::new())
                .unwrap();
        if selected.is_some() {
            eprintln!("{PREMISE}");
        }
        return;
    }
    let output = TestProcess::current(test)
        .unwrap()
        .case("*")
        .nocapture()
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(PREMISE),
        "a passing group child's premise never reached the caller: {output:?}"
    );
}

#[test]
fn an_unknown_group_is_refused() {
    let test = "an_unknown_group_is_refused";
    if std::env::var_os("MTP_CASE_PROBE").is_some() {
        let selection = maestro_test_process::selection();
        let wildcard = std::env::var("MTP_CASE_PROBE").unwrap() == "wildcard";
        assert!(selection.is_child());
        assert_eq!(selection.case(), (!wildcard).then_some("selected"));
        assert!(selection.runs_case("selected"));
        assert_eq!(selection.runs_case("different"), wildcard);
        assert_eq!(selection.runs_group(test), wildcard);
        assert_eq!(selection.runs_group("different_test"), wildcard);
        return;
    }
    if std::env::var_os("MTP_PROBE").is_some() {
        let error =
            run_per_environment(test, [1], |_| BTreeMap::<&str, Option<&str>>::new()).unwrap_err();
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
fn a_case_removes_a_kept_variable_from_its_child() {
    let test = "a_case_removes_a_kept_variable_from_its_child";
    if !maestro_test_process::selection().is_child() {
        // The raw child carries the compiled-code cache home the run's setup exports, so a
        // cleared group child would keep it unless its case removes it.
        let output = TestProcess::current(test)
            .unwrap()
            .case("*")
            .env("XDG_CACHE_HOME", "/controlled/cache")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    let selected = run_per_environment(test, ["absent"], |_| {
        BTreeMap::<&str, Option<&str>>::from([("XDG_CACHE_HOME", None)])
    })
    .unwrap();
    if selected.is_some() {
        assert_eq!(std::env::var_os("XDG_CACHE_HOME"), None);
    }
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
    let expected = maestro_test_directory::temporary_root();
    let selected = run_per_environment(test, [()], |()| {
        BTreeMap::from([("MTP_EXPECTED_TEMP", Some(expected.clone()))])
    });
    if selected.unwrap().is_some() {
        let expected = std::env::var_os("MTP_EXPECTED_TEMP").unwrap();
        assert_eq!(
            maestro_test_directory::temporary_root(),
            PathBuf::from(expected)
        );
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

/// Every child-selection name starts after an opening quote with this prefix.
const QUOTED_PREFIX: &[u8] = b"\"MAESTRO_";

/// The suffixes that make a quoted name a child-selection scheme.
const SCHEME_SUFFIXES: [&[u8]; 2] = [b"_CHILD", b"_CASE"];

/// A quoted child-selection scheme belongs to the process owner, not to its callers.
///
/// The guard reports every quoted name that starts with `MAESTRO_` and ends with `_CHILD`
/// or `_CASE`, in any file under `crates/` outside this crate, whatever its type and
/// whether the name sits in an expression or behind a `const`.
#[test]
fn selection_schemes_belong_to_the_process_owner() {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let mut pending = vec![crates];
    let mut reported = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() == "maestro-test-process" {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let source = std::fs::read(&path).unwrap();
            for (line, name) in scheme_literals(&source) {
                reported.push(format!("{}:{line} names {name}", path.display()));
            }
        }
    }
    reported.sort();
    assert!(
        reported.is_empty(),
        "{} sites read or write the private child-selection protocol:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

/// The line and name of every quoted child-selection scheme in `source`.
fn scheme_literals(source: &[u8]) -> Vec<(usize, String)> {
    let mut literals = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let from = cursor;
        let Some(found) = source[from..]
            .windows(QUOTED_PREFIX.len())
            .position(|window| window == QUOTED_PREFIX)
        else {
            break;
        };
        let start = from + found + 1;
        let length = source[start..]
            .iter()
            .take_while(|byte| name_byte(**byte))
            .count();
        let end = start + length;
        let name = &source[start..end];
        if source.get(end) == Some(&b'"') && is_scheme(name) {
            let line = source[..start].split(|byte| *byte == b'\n').count();
            literals.push((line, String::from_utf8_lossy(name).into_owned()));
        }
        cursor = end;
    }
    literals
}

/// Whether `byte` may continue a variable name.
fn name_byte(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
}

/// Whether `name` is a child-selection scheme.
fn is_scheme(name: &[u8]) -> bool {
    SCHEME_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// Parent and grouped-child roles dispatch groups without dispatching named cases.
fn assert_group_selection(test: &str, child: bool) {
    let selection = maestro_test_process::selection();
    assert_eq!(selection.is_child(), child);
    assert_eq!(selection.runs_case("any case"), !child);
    assert!(selection.runs_group(test));
    assert_eq!(selection.runs_group("different_test"), !child);
}
