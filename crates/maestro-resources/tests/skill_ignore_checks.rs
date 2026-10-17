//! Directory-local ignore decisions through native skill discovery.
#![cfg(unix)]
#![cfg(test)]
use crate::support;
use maestro_resources::{
    DiagnosticType, LoadSkillsFromDirOptions, NativeResourceOperations, load_skills_from_dir,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use support::{Directory, text};

/// Native filesystem recipe and complete expected discovery.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    /// Authored files, including rule files.
    files: BTreeMap<String, String>,
    /// Empty directories or nonfile rule paths.
    #[serde(default)]
    directories: Vec<String>,
    /// Link names and their relative targets.
    #[serde(default)]
    links: BTreeMap<String, String>,
    /// Lexical suffix on the scan operand.
    #[serde(default)]
    root_suffix: String,
    /// Complete returned tree.
    expected: Expected,
}
/// Ordered returned records.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Retained instruction records.
    skills: Vec<ExpectedSkill>,
    /// Ordered validation warnings.
    diagnostics: Vec<ExpectedDiagnostic>,
}
/// Every instruction field emitted by discovery.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ExpectedSkill {
    /// Selected name.
    name: String,
    /// Authored description.
    description: String,
    /// Selected instruction path.
    file_path: String,
    /// Containing directory.
    base_dir: String,
    /// Complete local provenance.
    source_info: ExpectedSource,
    /// Prompt eligibility.
    disable_model_invocation: bool,
}
/// Complete fixture provenance.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ExpectedSource {
    /// Resource location.
    path: String,
    /// Source label.
    source: String,
    /// Resource scope.
    scope: String,
    /// Resource origin.
    origin: String,
    /// Containing directory.
    base_dir: String,
}
/// Every diagnostic field present in the recipes.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedDiagnostic {
    /// Diagnostic category.
    r#type: String,
    /// Exact warning text.
    message: String,
    /// Associated instruction path.
    path: String,
}
/// Execute one group, rejecting repeated input recipes and comparing complete ordered trees.
fn check_group(group: &str) {
    let groups: BTreeMap<String, Vec<Recipe>> =
        serde_json::from_str(include_str!("fixtures/skill_ignores.json")).unwrap();
    let mut inputs = HashSet::new();
    for (index, recipe) in groups.get(group).unwrap().iter().enumerate() {
        assert!(
            inputs.insert((
                &recipe.files,
                &recipe.directories,
                &recipe.links,
                &recipe.root_suffix
            )),
            "duplicate recipe {index}"
        );
        let dir = Directory::new();
        let root = dir.0.join("skills");
        std::fs::create_dir(&root).unwrap();
        for directory in &recipe.directories {
            std::fs::create_dir_all(root.join(directory)).unwrap();
        }
        for (name, content) in &recipe.files {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            file.write_all(content.as_bytes()).unwrap();
        }
        for (name, target) in &recipe.links {
            std::os::unix::fs::symlink(target, root.join(name)).unwrap();
        }
        let scan = format!("{}{}", text(&root), recipe.root_suffix);
        let result = load_skills_from_dir(
            LoadSkillsFromDirOptions {
                cwd: text(&dir.0),
                dir: &scan,
                source: "path",
            },
            &NativeResourceOperations,
        );
        let diagnostics: Vec<_> = result.diagnostics.iter().map(|diagnostic| {
            assert_eq!(diagnostic.r#type, DiagnosticType::Warning);
            assert!(diagnostic.collision.is_none());
            serde_json::json!({"type": "warning", "message": diagnostic.message, "path": diagnostic.path})
        }).collect();
        let actual = serde_json::json!({"skills": result.skills, "diagnostics": diagnostics});
        let expected = serde_json::to_string(&recipe.expected)
            .unwrap()
            .replace("$ROOT", text(&root));
        let expected: serde_json::Value = serde_json::from_str(&expected).unwrap();
        assert_eq!(actual, expected, "{group} recipe {index}");
    }
}

/// Case groups retain scalar identities and distinct normalization decoys.
#[test]
fn skill_ignore_literals_keep_unicode_case_groups() {
    check_group("skill_ignore_literals_keep_unicode_case_groups");
}

/// Native discoveries prove classes retain scalar membership and complete complements.
#[test]
fn skill_ignore_classes_keep_scalar_membership() {
    check_group("skill_ignore_classes_keep_scalar_membership");
}

/// Native discoveries prove question wildcards count scalars rather than UTF-8 bytes.
#[test]
fn skill_ignore_questions_count_scalars() {
    check_group("skill_ignore_questions_count_scalars");
}

/// Native discoveries prove quoted syntax and inert malformed rules remain distinct.
#[test]
fn skill_ignore_syntax_keeps_literal_and_inert_rules() {
    check_group("skill_ignore_syntax_keeps_literal_and_inert_rules");
}

/// Native discoveries prove authored whitespace and three-file precedence remain distinct.
#[test]
fn skill_ignore_ingress_keeps_whitespace_and_file_order() {
    check_group("skill_ignore_ingress_keeps_whitespace_and_file_order");
}

/// Native discoveries prove local anchors and descendant basename reach retain their scope.
#[test]
fn skill_ignore_directory_scopes_keep_anchors_and_basename_reach() {
    check_group("skill_ignore_directory_scopes_keep_anchors_and_basename_reach");
}

/// Native discoveries prove parent pruning precedes child rule loading.
#[test]
fn skill_ignore_parent_pruning_precedes_child_negations() {
    check_group("skill_ignore_parent_pruning_precedes_child_negations");
}

/// Native discoveries prove directory markers retain globstar boundaries.
#[test]
fn skill_ignore_directory_markers_keep_globstar_boundaries() {
    check_group("skill_ignore_directory_markers_keep_globstar_boundaries");
}

/// Native discoveries prove escaped initial punctuation stays literal.
#[test]
fn skill_ignore_escaped_punctuation_stays_literal() {
    check_group("skill_ignore_escaped_punctuation_stays_literal");
}

/// Native discoveries prove entry selection preserves pruning and traversal.
#[test]
fn skill_ignore_entry_selection_preserves_traversal() {
    check_group("skill_ignore_entry_selection_preserves_traversal");
}

/// Native discoveries prove directory scopes retain literal identity and component boundaries.
#[test]
fn skill_ignore_directory_scopes_keep_literal_identity() {
    check_group("skill_ignore_directory_scopes_keep_literal_identity");
}
