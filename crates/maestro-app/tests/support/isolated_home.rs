//! One isolated-home child for tests that read process variables.
#![cfg(test)]

use std::collections::BTreeMap;

/// Runs `body` with a scratch base in one child whose home is `<base>/home` and whose
/// other variables are cleared, so no real home or agent directory is read.
pub fn with_isolated_home(test: &str, body: impl FnOnce(&str)) {
    with_environment(test, &[], body);
}

/// Runs `body` as [`with_isolated_home`] does, with `variables` added; `$BASE` in a value
/// is the scratch base.
pub fn with_environment(test: &str, variables: &[(&str, &str)], body: impl FnOnce(&str)) {
    let parent = (!maestro_test_process::selection().is_child())
        .then(|| maestro_test_directory::Scratch::new().unwrap());
    let base = parent.as_ref().map_or_else(
        || {
            let home = std::env::var("HOME").unwrap();
            home.strip_suffix("/home").unwrap().to_owned()
        },
        |scratch| scratch.path().display().to_string(),
    );
    let environment = |(): &()| {
        let mut environment = BTreeMap::from([("HOME".to_owned(), format!("{base}/home"))]);
        for (key, value) in variables {
            environment.insert((*key).to_owned(), value.replace("$BASE", &base));
        }
        environment
    };
    let selected = maestro_test_process::run_per_environment(test, [()], environment).unwrap();
    if selected.is_some() {
        body(&base);
    }
}
