//! Cases that read process variables, run in one cleared child per distinct environment.

use std::collections::BTreeMap;
use std::ffi::OsStr;

use crate::child::{repeat_stderr, write_stderr};
use crate::{TestProcess, TestProcessError};

/// Variable that carries the selected environment group into its child.
const GROUP: &str = "MAESTRO_TEST_GROUP";

/// The premise a host that takes its local zone from its settings fails.
const SETTINGS_ZONE_PREMISE: &str =
    "premise not met: this host takes the local time zone from its settings, not TZ\n";

/// Run the calling test's cases in one cleared child per distinct environment.
///
/// `environment` gives the edits a case's child applies: `Some` sets a variable and `None`
/// removes it, in key order after the child's cleared environment. Cases with equal edits
/// share one child, which inherits nothing else from this process except the coverage profile
/// path, the compiled-code cache home the run's setup exports, the Windows system root and
/// the temporary directory, which a case's own edits may replace. A case that names a kept
/// variable as `None` therefore reaches its child without it.
/// Returns `Ok(None)` in the calling process once every child passed; the test then returns.
/// Returns `Ok(Some(cases))` in a child: the cases of its environment in their original
/// order, which the test body runs. Cases that need a fresh process for state
/// other than the environment keep one child each. A passing child's standard error is
/// repeated here through the standard error handle rather than the print macros the harness
/// keeps, such as the premise that left one of its cases out, so `cargo test` writes that line
/// to the terminal and a runner that captures the test process shows it on a failure or when
/// asked. Its standard output stays hidden.
///
/// # Errors
///
/// Fails when `test` is not the running test, a child does not start exactly this test,
/// a child fails, a child selects a group that does not exist, or a passing child's standard
/// error cannot be repeated. A failed child's streams are kept.
pub fn run_per_environment<C, K, V>(
    test: &str,
    cases: impl IntoIterator<Item = C>,
    environment: impl Fn(&C) -> BTreeMap<K, Option<V>>,
) -> Result<Option<Vec<C>>, TestProcessError>
where
    K: AsRef<OsStr> + Ord,
    V: AsRef<OsStr> + Ord,
{
    let name = TestProcess::current(test)?.name().to_owned();
    let mut groups: BTreeMap<BTreeMap<K, Option<V>>, Vec<C>> = BTreeMap::new();
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
    for (index, edits) in groups.keys().enumerate() {
        let mut child = TestProcess::named(name.as_str())?
            .nocapture()
            .env_clear()
            .temp_dir(maestro_test_directory::temporary_root());
        for (key, value) in edits {
            child = match value {
                Some(value) => child.env(key, value),
                None => child.env_remove(key),
            };
        }
        let output = child.env(GROUP, index.to_string()).output()?;
        if !output.status.success() {
            return Err(TestProcessError::new(
                &name,
                format!("environment group {index} failed ({})", output.status),
            )
            .with_streams(&output.stdout, &output.stderr));
        }
        repeat_stderr(&name, &output.stderr)?;
    }
    Ok(None)
}

/// The cases selected for this process; its wire format stays private to this crate.
#[derive(Debug)]
pub struct CaseSelection {
    /// The process role selected by its launcher.
    kind: SelectionKind,
}

/// The parent, an unrestricted child, a named case, or an environment group.
#[derive(Debug)]
enum SelectionKind {
    /// The caller that launches children.
    Parent,
    /// A child that runs every case and group.
    All,
    /// One caller-supplied case identifier.
    Case(String),
    /// A group belonging to this full test identity.
    Group(String),
}

impl CaseSelection {
    /// Whether this process was launched to run a case or environment group.
    #[must_use]
    pub fn is_child(&self) -> bool {
        !matches!(self.kind, SelectionKind::Parent)
    }

    /// The case identifier this process was launched with, which a test reads to pick its
    /// stage. Fixture data a stage needs travels in its own variable.
    ///
    /// Reports `None` in the parent, in a wildcard child and in an environment group.
    #[must_use]
    pub fn case(&self) -> Option<&str> {
        match &self.kind {
            SelectionKind::Case(selected) => Some(selected),
            SelectionKind::Parent | SelectionKind::All | SelectionKind::Group(_) => None,
        }
    }

    /// Whether this process runs the supplied case identifier.
    ///
    /// Parents and wildcard children accept every case; grouped children accept none.
    #[must_use]
    pub fn runs_case(&self, case: &str) -> bool {
        match &self.kind {
            SelectionKind::Parent | SelectionKind::All => true,
            SelectionKind::Case(selected) => selected == case,
            SelectionKind::Group(_) => false,
        }
    }

    /// Whether this process runs environment groups belonging to the supplied test.
    ///
    /// Parents and wildcard children accept every group; named-case children accept none.
    #[must_use]
    pub fn runs_group(&self, test: &str) -> bool {
        match &self.kind {
            SelectionKind::Parent | SelectionKind::All => true,
            SelectionKind::Case(_) => false,
            SelectionKind::Group(name) => crate::selection::matches_name(name, test),
        }
    }
}

/// Whether a child's local time zone follows its `TZ` variable, the premise of a test
/// that selects a zone that way. Unix hosts read `TZ`; Windows reads its system zone, so
/// there this reports the unmet premise on standard error and returns `false`.
#[must_use]
pub fn local_zone_follows_tz() -> bool {
    if cfg!(unix) {
        return true;
    }
    report_settings_zone_premise();
    false
}

/// Reports the premise that this host takes its local zone from its settings, not `TZ`.
///
/// The harness keeps what the print macros write, so this line reaches the standard error of
/// a test that passes only through the stream handle.
fn report_settings_zone_premise() {
    // The run has no other channel for a case it skipped, and a host that cannot take the
    // line still skips it.
    let _ = write_stderr(SETTINGS_ZONE_PREMISE.as_bytes());
}

/// Return this process's typed case or environment-group selection.
#[must_use]
pub fn selection() -> CaseSelection {
    let kind = if std::env::var_os(GROUP).is_some() {
        SelectionKind::Group(std::thread::current().name().unwrap_or_default().to_owned())
    } else {
        match std::env::var(CASE).ok().as_deref() {
            Some("*") => SelectionKind::All,
            Some(case) => SelectionKind::Case(case.to_owned()),
            None => SelectionKind::Parent,
        }
    };
    CaseSelection { kind }
}

/// Variable that carries a single case identifier into its child.
///
/// A caller that hands a child its environment instead of launching it through
/// [`TestProcess`] names this variable and passes the case identifier as its value.
pub const CASE: &str = "MAESTRO_CHILD_CASE";

#[cfg(test)]
mod tests {
    use super::{SETTINGS_ZONE_PREMISE, report_settings_zone_premise};
    use crate::TestProcess;

    /// Variable that marks the child reporting the premise.
    const PREMISE_CHILD: &str = "MTP_ZONE_PREMISE";

    /// A passing test keeps the premise it skipped a case for, past the harness capture.
    #[test]
    fn the_settings_zone_premise_passes_the_harness_capture() {
        let test = "the_settings_zone_premise_passes_the_harness_capture";
        if std::env::var_os(PREMISE_CHILD).is_some() {
            report_settings_zone_premise();
            return;
        }
        let output = TestProcess::current(test)
            .unwrap()
            .env(PREMISE_CHILD, "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(SETTINGS_ZONE_PREMISE.trim_end()),
            "a passing test's premise never passed the harness capture: {output:?}"
        );
    }
}
