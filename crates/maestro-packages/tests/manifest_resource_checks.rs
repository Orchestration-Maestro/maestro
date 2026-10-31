//! Recorded manifest selection through the public package manager.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::scratch;
use crate::support;

use maestro_packages::{DefaultPackageManager, PackageManagerOptions};
use maestro_packages::{PackageManager, ResolvedPaths, ResolvedResource};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};
use std::{fs, io::Write, path::Path};
/// Recorded manifest cases.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Complete case allocation.
    rows: Vec<Row>,
}
/// One independent oracle observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Row {
    /// Owning test.
    test: String,
    /// Diagnostic case name.
    id: String,
    /// Package directory.
    package_root: String,
    /// Package-relative files.
    files: Vec<String>,
    /// Raw manifest contents.
    #[serde(default)]
    text: String,
    /// Authored sources.
    sources: Vec<String>,
    /// Expected progress events.
    events: Vec<serde_json::Value>,
    /// Additional root-relative files.
    #[serde(default)]
    outside: Vec<String>,
    /// Creates a native alias.
    #[serde(default)]
    alias: bool,
    /// Leaves the manifest absent.
    #[serde(default)]
    no_manifest: bool,
    /// Makes the manifest path a directory.
    #[serde(default)]
    manifest_directory: bool,
    /// Successful output.
    expected: Expected,
}
/// Expected public resources by kind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Extensions.
    extensions: Vec<Resource>,
    /// Skills.
    skills: Vec<Resource>,
    /// Prompts.
    prompts: Vec<Resource>,
    /// Themes.
    themes: Vec<Resource>,
}
/// One complete returned resource.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    /// Authored path.
    path: String,
    /// Admission flag.
    enabled: bool,
    /// Provenance.
    metadata: Metadata,
}
/// Source provenance fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Metadata {
    /// Source operand.
    source: String,
    /// Scope.
    scope: maestro_packages::SourceScope,
    /// Origin.
    origin: maestro_resources::SourceOrigin,
    /// Package base.
    base_dir: Option<String>,
}
/// Exclusively creates one setup file.
pub(crate) fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}
/// Creates the native recipe before invoking discovery.
fn setup(row: &Row, root: &str) {
    let package = row.package_root.replace("$ROOT", root);
    for file in &row.files {
        write(&Path::new(&package).join(file), "");
    }
    for file in &row.outside {
        write(&Path::new(root).join(file), "");
    }
    let manifest = Path::new(&package).join("package.json");
    if row.manifest_directory {
        fs::create_dir(&manifest).unwrap();
    } else if !row.no_manifest {
        let encoded_root = serde_json::to_string(root).unwrap();
        write(
            &manifest,
            &row.text
                .replace("$ROOT", &encoded_root[1..encoded_root.len() - 1]),
        );
    }
    if row.alias {
        #[cfg(unix)]
        std::os::unix::fs::symlink("a.ts", Path::new(&package).join("src/link.ts")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file("a.ts", Path::new(&package).join("src/link.ts"))
            .unwrap();
    }
}
/// Compares ordered resources and every provenance field.
fn compare(actual: &[ResolvedResource], expected: &[Resource], root: &str, id: &str) {
    assert_eq!(actual.len(), expected.len(), "{id}: resource count");
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(
            actual.path,
            maestro_path::normalize(&expected.path.replace("$ROOT", root)),
            "{id}: path"
        );
        assert_eq!(actual.enabled, expected.enabled, "{id}: enabled");
        assert_eq!(
            actual.metadata.source,
            expected.metadata.source.replace("$ROOT", root),
            "{id}: source"
        );
        assert_eq!(
            actual.metadata.scope, expected.metadata.scope,
            "{id}: scope"
        );
        assert_eq!(
            actual.metadata.origin, expected.metadata.origin,
            "{id}: origin"
        );
        assert_eq!(
            actual.metadata.base_dir,
            expected
                .metadata
                .base_dir
                .as_ref()
                .map(|s| maestro_path::normalize(&s.replace("$ROOT", root))),
            "{id}: base"
        );
    }
}
/// Checks the public result of the same completed invocation.
fn compare_result(actual: &ResolvedPaths, expected: &Expected, root: &str, id: &str) {
    compare(&actual.extensions, &expected.extensions, root, id);
    compare(&actual.skills, &expected.skills, root, id);
    compare(&actual.prompts, &expected.prompts, root, id);
    compare(&actual.themes, &expected.themes, root, id);
}
/// Executes only the cases owned by this test.
fn cases(test: &str) {
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/manifest_resources.json")).unwrap();
    let rows: Vec<_> = corpus
        .rows
        .into_iter()
        .filter(|row| row.test == test)
        .filter(|row| {
            !cfg!(windows)
                || !matches!(
                    row.id.as_str(),
                    "literal_metachar_files" | "literal_star_candidate" | "literal_star_removal"
                )
        })
        .collect();
    assert!(!rows.is_empty(), "test must own recorded cases");
    crate::runtime::run(async move {
        for row in rows {
            run_case(row).await;
        }
    });
}
/// Drives one recipe and checks only its completed invocation.
async fn run_case(row: Row) {
    let scratch = scratch::Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let (_, settings, _) = support::manager(&serde_json::json!({}));
    let effects = Rc::new(support::Effects::default());
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: maestro_path::join(&[&root, "project"]),
            agent_dir: maestro_path::join(&[&root, "agent"]),
            settings_manager: settings,
        },
        support::Controlled(effects.clone()),
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = events.clone();
    manager.set_progress_callback(Some(Rc::new(move |event| {
        observed.borrow_mut().push(event.r#type);
        Ok(())
    })));
    setup(&row, &root);
    let sources: Vec<_> = row
        .sources
        .iter()
        .map(|s| s.replace("$ROOT", &root))
        .collect();
    let result = manager.resolve_extension_sources(&sources, None).await;
    compare_result(&result.unwrap(), &row.expected, &root, &row.id);
    assert_eq!(
        events.borrow().len(),
        row.events.len(),
        "{}: progress",
        row.id
    );
    assert!(row.events.is_empty());
    assert!(effects.calls.borrow().is_empty());
}
/// Unreadable and false manifest values retain conventions.
#[test]
fn missing_or_unreadable_manifests_use_conventions() {
    cases("missing_or_unreadable_manifests_use_conventions");
}
/// Present manifests suppress omitted conventions.
#[test]
fn present_manifests_suppress_unlisted_conventions() {
    cases("present_manifests_suppress_unlisted_conventions");
}

/// Recorded literal manifest entries preserve order and file kinds behavior.
#[test]
fn literal_manifest_entries_preserve_order_and_file_kinds() {
    cases("literal_manifest_entries_preserve_order_and_file_kinds");
}

/// Recorded brace and bracket manifest entries are literal paths behavior.
#[test]
fn brace_and_bracket_manifest_entries_are_literal_paths() {
    cases("brace_and_bracket_manifest_entries_are_literal_paths");
}

/// Recorded manifest exclusions match all path views behavior.
#[test]
fn manifest_exclusions_match_all_path_views() {
    cases("manifest_exclusions_match_all_path_views");
}

/// Skill files also offer parent directory views.
#[test]
fn skill_exclusions_also_match_parent_directories() {
    cases("skill_exclusions_also_match_parent_directories");
}
/// Restoration uses exact candidate identities, not matching patterns.
#[test]
fn force_includes_use_exact_paths_not_wildcards() {
    cases("force_includes_use_exact_paths_not_wildcards");
}

/// Recorded skill force overrides use exact parent paths behavior.
#[test]
fn skill_force_overrides_use_exact_parent_paths() {
    cases("skill_force_overrides_use_exact_parent_paths");
}

/// Recorded force exclusion wins independently of entry order behavior.
#[test]
fn force_exclusion_wins_independently_of_entry_order() {
    cases("force_exclusion_wins_independently_of_entry_order");
}

/// Recorded force reinclusion keeps original candidate order behavior.
#[test]
fn force_reinclusion_keeps_original_candidate_order() {
    cases("force_reinclusion_keeps_original_candidate_order");
}

/// Recorded override only manifests never discover new candidates behavior.
#[test]
fn override_only_manifests_never_discover_new_candidates() {
    cases("override_only_manifests_never_discover_new_candidates");
}

/// Recorded empty selected fields do not read conventions behavior.
#[test]
fn empty_selected_fields_do_not_read_conventions() {
    cases("empty_selected_fields_do_not_read_conventions");
}

/// Recorded late manifest failures keep earlier local resources behavior.
#[test]
fn late_manifest_failures_keep_earlier_local_resources() {
    cases("late_manifest_failures_keep_earlier_local_resources");
}

/// Recorded failed manifest kind contributes no partial entries behavior.
#[test]
fn failed_manifest_kind_contributes_no_partial_entries() {
    cases("failed_manifest_kind_contributes_no_partial_entries");
}

/// Recorded a failed local source does not stop later sources behavior.
#[test]
fn a_failed_local_source_does_not_stop_later_sources() {
    cases("a_failed_local_source_does_not_stop_later_sources");
}

/// Recorded installed remote malformed lists are ignored behavior.
#[test]
fn installed_remote_malformed_lists_are_ignored() {
    cases("installed_remote_malformed_lists_are_ignored");
}

/// A malformed resource list is ignored while valid lists still load, with and without
/// a resource filter on the package.
#[test]
fn malformed_resource_lists_are_ignored_while_valid_lists_load() {
    for filtered in [false, true] {
        crate::runtime::run(async move {
            let scratch = scratch::Scratch::new().unwrap();
            let root = scratch.path().to_string_lossy();
            let package = format!("{root}/pkg");
            write(&Path::new(&package).join("prompts/a.md"), "");
            write(
                &Path::new(&package).join("package.json"),
                r#"{"maestro":{"skills":5,"prompts":["prompts/a.md"]}}"#,
            );
            let entry = if filtered {
                serde_json::json!({"source": package, "skills": ["*"]})
            } else {
                serde_json::json!(package)
            };
            let (_, settings, _) = support::manager(&serde_json::json!({"packages": [entry]}));
            let manager = DefaultPackageManager::new(
                PackageManagerOptions {
                    cwd: maestro_path::join(&[&root, "project"]),
                    agent_dir: maestro_path::join(&[&root, "agent"]),
                    settings_manager: settings,
                },
                support::Controlled(Rc::new(support::Effects::default())),
            );

            let result = manager.resolve(None).await.unwrap();

            assert!(result.skills.is_empty(), "filtered: {filtered}");
            let prompts: Vec<_> = result.prompts.iter().map(|p| p.path.as_str()).collect();
            assert_eq!(
                prompts,
                [format!("{package}/prompts/a.md")],
                "filtered: {filtered}"
            );
        });
    }
}

/// Recorded manifest duplicates use the last selected member behavior.
#[test]
fn manifest_duplicates_use_the_last_selected_member() {
    cases("manifest_duplicates_use_the_last_selected_member");
}

/// Recorded escaped manifest keys select the same field behavior.
#[test]
fn escaped_manifest_keys_select_the_same_field() {
    cases("escaped_manifest_keys_select_the_same_field");
}

/// Recorded unknown manifest fields are not decoded as resources behavior.
#[test]
fn unknown_manifest_fields_are_not_decoded_as_resources() {
    cases("unknown_manifest_fields_are_not_decoded_as_resources");
}

/// Recorded manifest excluded aliases are not deduplicated early behavior.
#[test]
fn manifest_excluded_aliases_are_not_deduplicated_early() {
    cases("manifest_excluded_aliases_are_not_deduplicated_early");
}

/// Recorded manifest results keep enabled metadata for each kind behavior.
#[test]
fn manifest_results_keep_enabled_metadata_for_each_kind() {
    cases("manifest_results_keep_enabled_metadata_for_each_kind");
}

/// Recorded manifest pattern text is not trimmed behavior.
#[test]
fn manifest_pattern_text_is_not_trimmed() {
    cases("manifest_pattern_text_is_not_trimmed");
}

/// Recorded manifest exclusions keep explicit dependency resources behavior.
#[test]
fn manifest_exclusions_keep_explicit_dependency_resources() {
    cases("manifest_exclusions_keep_explicit_dependency_resources");
}

/// Recorded manifest overrides apply to prompts and themes behavior.
#[test]
fn manifest_overrides_apply_to_prompts_and_themes() {
    cases("manifest_overrides_apply_to_prompts_and_themes");
}

/// Unfiltered kinds in object registrations use conventions for malformed manifest lists.
#[test]
fn filtered_packages_fall_back_only_for_absent_or_malformed_lists() {
    for value in [
        serde_json::json!(5),
        serde_json::json!("x"),
        serde_json::json!(["a", 7]),
        serde_json::json!([]),
    ] {
        crate::runtime::run(async move {
            let scratch = scratch::Scratch::new().unwrap();
            let root = scratch.path().to_str().unwrap();
            let package = maestro_path::join(&[root, "pkg"]);
            let files = [
                "extensions/a.wasm",
                "skills/a.md",
                "prompts/a.md",
                "themes/a.json",
            ];
            for file in files {
                write(&Path::new(&package).join(file), "");
            }
            write(
                &Path::new(&package).join("package.json"),
                &serde_json::json!({"maestro": {
                    "extensions":value, "skills":value, "prompts":value, "themes":value
                }})
                .to_string(),
            );
            let (_, settings, _) =
                support::manager(&serde_json::json!({"packages":[{"source":package}]}));
            let manager = DefaultPackageManager::new(
                PackageManagerOptions {
                    cwd: maestro_path::join(&[root, "project"]),
                    agent_dir: maestro_path::join(&[root, "agent"]),
                    settings_manager: settings,
                },
                support::Controlled(Rc::new(support::Effects::default())),
            );

            let result = manager.resolve(None).await.unwrap();

            let actual: Vec<_> = [
                &result.extensions,
                &result.skills,
                &result.prompts,
                &result.themes,
            ]
            .into_iter()
            .flatten()
            .map(|r| r.path.clone())
            .collect();
            let expected: Vec<_> = if value == serde_json::json!([]) {
                Vec::new()
            } else {
                files
                    .map(|file| maestro_path::join(&[&package, file]))
                    .into()
            };
            assert_eq!(actual, expected, "manifest list: {value}");
        });
    }
}
