//! One isolated-home child for tests that read process variables.

use std::collections::BTreeMap;

use crate::TestProcessError;

/// The variable that carries the scratch base into the child.
const BASE: &str = "MAESTRO_TEST_BASE";

/// Runs `body` with a scratch base in one child whose home is `<base>/home` and whose
/// other variables are cleared, so no real home or agent directory is read.
///
/// # Errors
/// Returns the failure to create the scratch base in the parent, to read the base in the
/// child, or to run the case's child.
pub fn with_isolated_home(test: &str, body: impl FnOnce(&str)) -> Result<(), TestProcessError> {
    with_environment(test, &[], body)
}

/// Runs `body` as [`with_isolated_home`] does, with `variables` added; `$BASE` in a value
/// is the scratch base.
///
/// # Errors
/// Returns every failure [`with_isolated_home`] returns.
pub fn with_environment(
    test: &str,
    variables: &[(&str, &str)],
    body: impl FnOnce(&str),
) -> Result<(), TestProcessError> {
    with_environment_without(test, variables, &[], body)
}

/// Runs `body` as [`with_environment`] does, with `removed` cleared from the child.
///
/// # Errors
/// Returns the failure to create the scratch base in the parent, to read the base in the
/// child, or to run the case's child.
pub fn with_environment_without(
    test: &str,
    variables: &[(&str, &str)],
    removed: &[&str],
    body: impl FnOnce(&str),
) -> Result<(), TestProcessError> {
    let failure = |step: &str| TestProcessError::new(test, step);
    let scratch = if crate::selection().is_child() {
        None
    } else {
        Some(
            maestro_test_directory::Scratch::new()
                .map_err(|error| failure("the scratch base is not creatable").caused_by(error))?,
        )
    };
    let base = scratch.as_ref().map_or_else(
        || {
            std::env::var(BASE)
                .map_err(|error| failure("the child carries the scratch base").caused_by(error))
        },
        |scratch| Ok(scratch.path().display().to_string()),
    )?;
    let environment = |(): &()| {
        let home = std::path::Path::new(&base)
            .join("home")
            .display()
            .to_string();
        // The cost report reads `HOME` on every platform; Windows lookups read `USERPROFILE`.
        let mut environment = BTreeMap::from([
            ("HOME".to_owned(), Some(home.clone())),
            (BASE.to_owned(), Some(base.clone())),
        ]);
        if cfg!(windows) {
            environment.insert("USERPROFILE".to_owned(), Some(home));
        }
        // The prepared native fixture stays reachable from a cleared child, so a case can
        // install controlled programs without repeating the artifact path.
        if let Some(artifact) = std::env::var_os("MAESTRO_NATIVE_FIXTURE") {
            environment.insert(
                "MAESTRO_NATIVE_FIXTURE".to_owned(),
                Some(artifact.to_string_lossy().into_owned()),
            );
        }
        for (key, value) in variables {
            environment.insert((*key).to_owned(), Some(value.replace("$BASE", &base)));
        }
        for key in removed {
            environment.insert((*key).to_owned(), None);
        }
        environment
    };
    if crate::run_per_environment(test, [()], environment)?.is_some() {
        body(&base);
    }
    Ok(())
}
