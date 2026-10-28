//! Typed, complete resource observations shared by component workflow tests.
#![cfg(test)]
use maestro_packages::{ResolvedPaths, ResolvedResource};
use maestro_resources::{SourceOrigin, SourceScope};
use serde::Deserialize;
/// Recorded resource collections, including empty siblings.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedPaths {
    /// Extension resources.
    extensions: Vec<ExpectedResource>,
    /// Skill resources.
    skills: Vec<ExpectedResource>,
    /// Prompt resources.
    prompts: Vec<ExpectedResource>,
    /// Theme resources.
    themes: Vec<ExpectedResource>,
}
/// A recorded resource.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedResource {
    /// Selected spelling.
    path: String,
    /// Filter result.
    enabled: bool,
    /// Complete provenance.
    metadata: ExpectedMetadata,
}
/// Recorded provenance; an omitted directory represents the native None value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ExpectedMetadata {
    /// Source spelling.
    source: String,
    /// Selected scope.
    scope: SourceScope,
    /// Resource origin.
    origin: SourceOrigin,
    /// Optional directory.
    base_dir: Option<String>,
}
impl ExpectedPaths {
    /// Compare every native field in discovery order without JSON serialization.
    pub fn check(&self, actual: ResolvedPaths, root: &str) {
        for (expected, actual) in [
            (&self.extensions, actual.extensions),
            (&self.skills, actual.skills),
            (&self.prompts, actual.prompts),
            (&self.themes, actual.themes),
        ] {
            assert_eq!(expected.len(), actual.len());
            for (expected, actual) in expected.iter().zip(actual) {
                expected.check(&actual, root);
            }
        }
    }
}
impl ExpectedResource {
    /// Compare authored spellings after binding the recorded absolute root.
    fn check(&self, actual: &ResolvedResource, root: &str) {
        let recorded = |path: &str| recorded(path, root);
        assert_eq!(recorded(&actual.path), self.path);
        assert_eq!(actual.enabled, self.enabled);
        assert_eq!(recorded(&actual.metadata.source), self.metadata.source);
        assert_eq!(actual.metadata.scope, self.metadata.scope);
        assert_eq!(actual.metadata.origin, self.metadata.origin);
        assert_eq!(
            actual.metadata.base_dir.as_deref().map(recorded),
            self.metadata.base_dir
        );
    }
}

/// `path` as the recordings spell it: the root as `$ROOT` and, on Windows, which never
/// stores `\\` in a name, every native separator as `/`.
fn recorded(path: &str, root: &str) -> String {
    let path = path.replace(root, "$ROOT");
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path
    }
}
