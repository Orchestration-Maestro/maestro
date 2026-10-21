//! Cases that read process variables, run in one cleared child per distinct environment.

use std::collections::BTreeMap;
use std::ffi::OsStr;

use crate::{TestProcess, TestProcessError};

/// Variable that carries the selected environment group into its child.
const GROUP: &str = "MAESTRO_TEST_GROUP";

/// Run the calling test's cases in one cleared child per distinct environment.
///
/// `environment` gives the variables a case runs with. Cases with equal variables share
/// one child, which inherits nothing else from this process except the coverage profile
/// path. Returns `Ok(None)` in the calling process once every child passed; the test then
/// returns. Returns `Ok(Some(cases))` in a child: the cases of its environment in their
/// original order, which the test body runs. Cases that need a fresh process for state
/// other than the environment keep one child each.
///
/// # Errors
///
/// Fails when `test` is not the running test, a child does not start exactly this test,
/// a child fails, or a child selects a group that does not exist. A failed child's
/// streams are kept.
pub fn run_per_environment<C, K, V>(
    test: &str,
    cases: impl IntoIterator<Item = C>,
    environment: impl Fn(&C) -> BTreeMap<K, V>,
) -> Result<Option<Vec<C>>, TestProcessError>
where
    K: AsRef<OsStr> + Ord,
    V: AsRef<OsStr> + Ord,
{
    let name = TestProcess::current(test)?.name().to_owned();
    let mut groups: BTreeMap<BTreeMap<K, V>, Vec<C>> = BTreeMap::new();
    for case in cases {
        groups.entry(environment(&case)).or_default().push(case);
    }
    if let Some(selected) = std::env::var_os(GROUP) {
        let group = selected
            .to_str()
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| groups.into_values().nth(index));
        return group.map(Some).ok_or_else(|| {
            TestProcessError::new(&name, "the selected environment group does not exist")
        });
    }
    for (index, variables) in groups.keys().enumerate() {
        let output = TestProcess::named(name.as_str())?
            .nocapture()
            .env_clear()
            .envs(variables)
            .env(GROUP, index.to_string())
            .output()?;
        if !output.status.success() {
            return Err(TestProcessError::new(
                &name,
                format!("environment group {index} failed ({})", output.status),
            )
            .with_streams(&output.stdout, &output.stderr));
        }
    }
    Ok(None)
}
