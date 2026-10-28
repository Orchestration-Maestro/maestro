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
/// path, the Windows system root and the temporary directory, which a case's own
/// variables may replace. Returns
/// `Ok(None)` in the calling process once every child passed; the test then returns.
/// Returns `Ok(Some(cases))` in a child: the cases of its environment in their original
/// order, which the test body runs. Cases that need a fresh process for state
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
            .temp_dir(std::env::temp_dir())
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
    eprintln!("premise not met: this host takes the local time zone from its settings, not TZ");
    false
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
pub(crate) const CASE: &str = "MAESTRO_CHILD_CASE";
