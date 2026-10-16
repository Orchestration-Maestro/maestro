//! Resolution and home selection from caller-supplied observations.
#![cfg(test)]

use maestro_path::{homedir, resolve_with};
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

/// Recorded group resolutions of the runtime's path module.
const FIXTURE: &str = include_str!("fixtures/directory-vectors.json");

/// One recorded resolution of several operand groups.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Label shown when the case fails.
    id: String,
    /// The path flavor the case was recorded for.
    flavor: String,
    /// The operand groups.
    groups: Vec<Vec<String>>,
    /// The working directory, or `None` when reading it fails.
    cwd: Option<String>,
    /// Drive directories keyed by their `=X:` variable name.
    drives: BTreeMap<String, String>,
    /// The paths in group order, or the working-directory error.
    expected: Expected,
}

/// The recorded outcome.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum Expected {
    /// One resolved path per group.
    Ok(Vec<String>),
    /// The supplied working-directory error.
    Error(String),
}

/// Resolve `groups` with a failing-by-default working directory.
fn run(
    groups: &[&[&str]],
    cwd: Option<&str>,
    drives: Vec<(char, String)>,
) -> Result<Vec<String>, String> {
    resolve_with(
        groups,
        || drives,
        || cwd.ok_or("cwd unavailable".to_owned()),
    )
}

#[test]
fn supplied_home_preserves_present_values() {
    let fallback = || -> Option<&str> { panic!("a present home never asks the account") };
    for home in ["/home/user", "relative/home", ""] {
        assert_eq!(homedir(Some(home), fallback), Some(home));
    }
}

#[test]
fn absent_home_calls_fallback_once() {
    for account in [Some("/account"), None] {
        let calls = Cell::new(0);
        let home = homedir(None, || {
            calls.set(calls.get() + 1);
            account
        });
        assert_eq!((home, calls.get()), (account, 1));
    }
}

#[test]
fn supplied_resolution_matches_native_flavor_corpus() {
    let cases: Vec<Case> = serde_json::from_str(FIXTURE).unwrap();
    let flavor = if cfg!(windows) { "win32" } else { "posix" };
    let mut checked = 0;
    for case in cases.iter().filter(|case| case.flavor == flavor) {
        let actual = resolve_case(case, false);
        assert_eq!(actual, case.expected, "{}", case.id);
        checked += 1;
    }
    assert!(checked >= 10, "{flavor}: {checked} recorded cases");
}

#[test]
fn anchored_groups_skip_directory_callbacks() {
    let anchored: &[&[&str]] = if cfg!(windows) {
        &[&["C:\\a", "b"], &["\\\\h\\s\\x"]]
    } else {
        &[&["/a", "b"], &["rel", "/c"]]
    };
    for groups in [&[][..], anchored] {
        let result: Result<_, ()> = resolve_with(
            groups,
            || -> Vec<(char, String)> { panic!("drives are not needed") },
            || -> Result<String, ()> { panic!("cwd is not needed") },
        );
        assert_eq!(result.map(|paths| paths.len()), Ok(groups.len()));
    }
}

#[test]
fn open_groups_share_ordered_directory_observations() {
    let order = RefCell::new(Vec::new());
    let result: Result<_, ()> = resolve_with(
        &[&["/z", "b"], &["rel", "a"], &["/z", "b"]],
        || {
            order.borrow_mut().push("drives");
            Vec::<(char, String)>::new()
        },
        || {
            order.borrow_mut().push("cwd");
            Ok("/tmp".to_owned())
        },
    );
    let native = |path: &str| {
        if cfg!(windows) {
            path.replace('/', "\\")
        } else {
            path.to_owned()
        }
    };
    assert_eq!(
        result.unwrap(),
        [native("/z/b"), native("/tmp/rel/a"), native("/z/b")]
    );
    assert_eq!(*order.borrow(), ["drives", "cwd"]);
}

#[test]
fn required_cwd_error_is_returned_without_rebuilding() {
    #[derive(Debug, PartialEq)]
    struct Native(u32);
    let result = resolve_with(&[&["/ok"], &["rel"]], Vec::<(char, String)>::new, || {
        Err::<String, _>(Native(7))
    });
    assert_eq!(result, Err(Native(7)));
}

#[cfg(windows)]
#[test]
fn drive_complete_groups_skip_cwd_observation() {
    let drives = vec![('D', "D:\\drive".to_owned())];
    let result: Result<_, ()> = resolve_with(
        &[&["D:leaf"]],
        || drives.clone(),
        || -> Result<String, ()> { panic!("a supplied drive completes the path") },
    );
    assert_eq!(result.unwrap(), ["D:\\drive\\leaf"]);
    let empty = resolve_with(
        &[&["D:leaf"]],
        || vec![('D', String::new())],
        || Err::<String, _>("cwd read"),
    );
    assert_eq!(empty, Err("cwd read"));
    for open in [&["D:leaf"][..], &["\\leaf"]] {
        let calls = Cell::new(0);
        let _: Result<_, ()> = resolve_with(&[open], Vec::new, || {
            calls.set(calls.get() + 1);
            Ok("C:\\work")
        });
        assert_eq!(calls.get(), 1, "{open:?}");
    }
}

/// Adapt recorded inputs without changing native-entry assertions.
fn resolve_case(case: &Case, explicit: bool) -> Expected {
    let groups: Vec<Vec<&str>> = case
        .groups
        .iter()
        .map(|group| group.iter().map(String::as_str).collect())
        .collect();
    let groups: Vec<&[&str]> = groups.iter().map(Vec::as_slice).collect();
    let drives = case
        .drives
        .iter()
        .map(|(name, directory)| (name.chars().nth(1).unwrap(), directory.clone()))
        .collect();
    let result = if explicit {
        maestro_path::resolve_with_flavor(
            &groups,
            case.flavor == "win32",
            || drives,
            || case.cwd.as_deref().ok_or("cwd unavailable".to_owned()),
        )
    } else {
        run(&groups, case.cwd.as_deref(), drives)
    };
    match result {
        Ok(paths) => Expected::Ok(paths),
        Err(error) => Expected::Error(error),
    }
}

#[test]
fn supplied_resolution_matches_both_flavor_corpora() {
    let cases: Vec<Case> = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(cases.len(), 24);
    for case in cases {
        assert_eq!(resolve_case(&case, true), case.expected, "{}", case.id);
    }
}

/// Check an explicit flavor's output and shared directory observations.
fn assert_observations(case: &Case, expected_reads: &[&str]) {
    let groups: Vec<Vec<&str>> = case
        .groups
        .iter()
        .map(|group| group.iter().map(String::as_str).collect())
        .collect();
    let groups: Vec<&[&str]> = groups.iter().map(Vec::as_slice).collect();
    let reads = RefCell::new(Vec::new());
    let result = maestro_path::resolve_with_flavor(
        &groups,
        case.flavor == "win32",
        || {
            reads.borrow_mut().push("drives");
            case.drives
                .iter()
                .map(|(key, value)| (key.chars().nth(1).unwrap(), value.clone()))
                .collect::<Vec<_>>()
        },
        || {
            reads.borrow_mut().push("cwd");
            case.cwd.as_deref().ok_or("cwd unavailable".to_owned())
        },
    );
    let actual = match result {
        Ok(paths) => Expected::Ok(paths),
        Err(error) => Expected::Error(error),
    };
    assert_eq!(actual, case.expected, "{}", case.id);
    assert_eq!(*reads.borrow(), expected_reads, "{}", case.id);
}

#[test]
fn explicit_flavors_keep_lazy_directory_order() {
    let cases: Vec<Case> = serde_json::from_str(FIXTURE).unwrap();
    for case in cases {
        let reads: &[&str] = match case.id.as_str() {
            "p-anchored"
            | "p-empty-groups"
            | "p-spelling"
            | "w-absolute"
            | "w-unc"
            | "w-namespace"
            | "w-drive-operand"
            | "w-device-precedence" => &[],
            "w-drive-supplied" | "w-drive-mismatch" => &["drives"],
            _ => &["drives", "cwd"],
        };
        assert_observations(&case, reads);
    }
    for (groups, drives, expected, reads) in [
        (vec![], BTreeMap::new(), vec![], vec![]),
        (
            vec![vec!["D:leaf".into()], vec!["D:leaf".into()]],
            BTreeMap::from([("=D:".into(), "D:\\drive".into())]),
            vec!["D:\\drive\\leaf".into(), "D:\\drive\\leaf".into()],
            vec!["drives"],
        ),
    ] {
        assert_observations(
            &Case {
                id: "additional-windows-boundary".into(),
                flavor: "win32".into(),
                groups,
                cwd: None,
                drives,
                expected: Expected::Ok(expected),
            },
            &reads,
        );
    }
}

#[test]
fn explicit_flavors_return_original_cwd_errors() {
    for (windows, groups) in [
        (false, vec![&["relative"][..]]),
        (false, vec![&["/complete"][..], &["relative"][..]]),
        (true, vec![&["D:relative"][..]]),
        (true, vec![&["C:\\complete"][..], &["relative"][..]]),
    ] {
        let sentinel = Box::new(7_u32);
        let identity = std::ptr::from_ref(sentinel.as_ref());
        let reads = RefCell::new(Vec::new());
        let result = maestro_path::resolve_with_flavor(
            &groups,
            windows,
            || {
                reads.borrow_mut().push("drives");
                Vec::<(char, String)>::new()
            },
            || {
                reads.borrow_mut().push("cwd");
                Err::<String, _>(sentinel)
            },
        );
        let error = result.unwrap_err();
        assert_eq!(std::ptr::from_ref(error.as_ref()), identity);
        assert_eq!(*reads.borrow(), ["drives", "cwd"]);
    }
}
