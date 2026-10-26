//! One isolated-home child for tests that read process variables.
#![cfg(test)]

use std::collections::BTreeMap;

/// The variable that carries the scratch base into the child.
const BASE: &str = "MAESTRO_TEST_BASE";

/// Runs `body` with a scratch base in one child whose home is `<base>/home` and whose
/// other variables are cleared, so no real home or agent directory is read.
pub fn with_isolated_home(test: &str, body: impl FnOnce(&str)) {
    with_environment(test, &[], body);
}

/// Runs `body` as [`with_isolated_home`] does, with `variables` added; `$BASE` in a value
/// is the scratch base.
pub fn with_environment(test: &str, variables: &[(&str, &str)], body: impl FnOnce(&str)) {
    with_environment_without(test, variables, &[], body);
}

/// Runs `body` as [`with_environment`] does, with `removed` cleared from the child.
pub fn with_environment_without(
    test: &str,
    variables: &[(&str, &str)],
    removed: &[&str],
    body: impl FnOnce(&str),
) {
    let parent = (!maestro_test_process::selection().is_child())
        .then(|| maestro_test_directory::Scratch::new().unwrap());
    let base = parent.as_ref().map_or_else(
        || std::env::var(BASE).expect("the child carries the scratch base"),
        |scratch| scratch.path().display().to_string(),
    );
    let environment = |(): &()| {
        let mut environment = BTreeMap::from([
            (
                if cfg!(windows) { "USERPROFILE" } else { "HOME" }.to_owned(),
                std::path::Path::new(&base)
                    .join("home")
                    .display()
                    .to_string(),
            ),
            (BASE.to_owned(), base.clone()),
        ]);
        for (key, value) in variables {
            environment.insert((*key).to_owned(), value.replace("$BASE", &base));
        }
        for key in removed {
            environment.remove(*key);
        }
        environment
    };
    let selected = maestro_test_process::run_per_environment(test, [()], environment).unwrap();
    if selected.is_some() {
        body(&base);
    }
}
