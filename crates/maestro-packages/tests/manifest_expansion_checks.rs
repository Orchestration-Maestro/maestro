//! Manifest expansion through the existing public package manager.
#![cfg(test)]
#![cfg(target_os = "linux")]
use crate::manifest_resource_checks::write;
use crate::resolution_support;
use crate::scratch;
use maestro_packages::PackageManager;
use serde_json::json;
use std::fs;
/// Wildcards produce native matches rather than a literal missing operand.
#[test]
fn manifest_expansion_keeps_native_match_order() {
    cases("manifest_expansion_keeps_native_match_order");
}

/// Native setup recipe shared by its recorded observations.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    /// File text indexed by authored relative path.
    files: std::collections::BTreeMap<String, String>,
    /// Empty directory paths.
    dirs: Vec<String>,
    /// Authored link names and target text.
    links: std::collections::BTreeMap<String, String>,
}
/// Complete manifest inputs and public output for one invocation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    /// Failure label.
    case: String,
    /// Owning behavioral test.
    test: String,
    /// Manifest directory.
    root: String,
    /// Raw admitted or malformed manifest fields.
    manifest: serde_json::Value,
    /// Public source operands.
    sources: Vec<String>,
    /// Native filesystem inputs.
    setup: Option<Setup>,
    /// Directory whose read is denied natively.
    denied: bool,
    /// Ordered resources with complete metadata.
    expected: serde_json::Value,
}
/// Creates only files, directories and links read by the recorded corpus.
fn setup(scene: &std::path::Path, setup: &Setup) {
    for (path, text) in &setup.files {
        write(&scene.join(path), text);
    }
    for path in &setup.dirs {
        fs::create_dir_all(scene.join(path)).unwrap();
    }
    for (path, target) in &setup.links {
        let path = scene.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, path).unwrap();
    }
}
/// Drives each selected recipe through the public operation.
fn cases(test: &str) {
    let rows: Vec<Recipe> =
        serde_json::from_str(include_str!("fixtures/manifest_expansion.json")).unwrap();
    let rows: Vec<_> = rows.into_iter().filter(|r| r.test == test).collect();
    assert!(!rows.is_empty());
    resolution_support::run(async {
        for row in rows {
            run_recipe(row).await;
        }
        Ok(())
    })
    .unwrap();
}
/// Compares the full result of this invocation, not accumulated history.
async fn run_recipe(row: Recipe) {
    let scene = scratch::Scratch::new().unwrap();
    let inputs = row.setup.unwrap_or_else(|| {
        serde_json::from_str(include_str!("fixtures/glob_native_setup.json")).unwrap()
    });
    setup(scene.path(), &inputs);
    let base = scene.path().to_string_lossy().into_owned();
    let root = row.root.replace("$SCENE", &base);
    let manifest = row.manifest.to_string().replace("$SCENE", &base);
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    write(
        &std::path::Path::new(&root).join("package.json"),
        &json!({"maestro":manifest}).to_string(),
    );
    let denied = row
        .denied
        .then(|| maestro_test_directory::deny(scene.path().join("denied"), 0o0).unwrap());
    if matches!(denied, Some(None)) {
        return;
    }
    let (manager, _) = resolution_support::manager(&root);
    let sources: Vec<_> = row
        .sources
        .iter()
        .map(|s| s.replace("$SCENE", &base))
        .collect();
    let result = manager
        .resolve_extension_sources(&sources, None)
        .await
        .unwrap();
    let actual = resolution_support::output(result, &base)
        .to_string()
        .replace("$BASE", "$SCENE");
    drop(denied);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&actual).unwrap(),
        row.expected,
        "{}",
        row.case
    );
}

/// Recorded public behavior for positive manifest entries route before expansion.
#[test]
fn positive_manifest_entries_route_before_expansion() {
    cases("positive_manifest_entries_route_before_expansion");
}

/// Recorded public behavior for manifest alternatives keep preparation order.
#[test]
fn manifest_alternatives_keep_preparation_order() {
    cases("manifest_alternatives_keep_preparation_order");
}

/// Recorded public behavior for manifest patterns keep dots and authored roots.
#[test]
fn manifest_patterns_keep_dots_and_authored_roots() {
    cases("manifest_patterns_keep_dots_and_authored_roots");
}

/// Recorded public behavior for manifest patterns expand before kind collection.
#[test]
fn manifest_patterns_expand_before_kind_collection() {
    cases("manifest_patterns_expand_before_kind_collection");
}

/// Recorded public behavior for manifest ignore rules apply only during collection.
#[test]
fn manifest_ignore_rules_apply_only_during_collection() {
    cases("manifest_ignore_rules_apply_only_during_collection");
}

/// Recorded public behavior for manifest wildcards keep overrides and provenance.
#[test]
fn manifest_wildcards_keep_overrides_and_provenance() {
    cases("manifest_wildcards_keep_overrides_and_provenance");
}

/// Recorded public behavior for manifest globstars keep symlink progress.
#[test]
fn manifest_globstars_keep_symlink_progress() {
    cases("manifest_globstars_keep_symlink_progress");
}

/// Recorded public behavior for manifest wildcards keep failure boundaries.
#[test]
fn manifest_wildcards_keep_failure_boundaries() {
    cases("manifest_wildcards_keep_failure_boundaries");
}

/// Recorded public behavior for manifest unicode wildcards keep scalar results.
#[test]
fn manifest_unicode_wildcards_keep_scalar_results() {
    cases("manifest_unicode_wildcards_keep_scalar_results");
}

/// Recorded public behavior for manifest literal scan overlap keeps distinct names.
#[test]
fn manifest_literal_scan_overlap_keeps_distinct_names() {
    cases("manifest_literal_scan_overlap_keeps_distinct_names");
}

/// Recorded public behavior for manifest groups keep existing corrections.
#[test]
fn manifest_groups_keep_existing_corrections() {
    cases("manifest_groups_keep_existing_corrections");
}

/// Expansion anchors installed relative roots only when ambient context is needed.
#[test]
fn installed_wildcards_resolve_relative_constructor_roots() {
    use maestro_resources::SourceScope;
    resolution_support::run(async {
        let scene = scratch::Scratch::new()?;
        let package = scene.path().join(".maestro/npm/node_modules/pkg");
        write(&package.join("a.ts"), "fixture");
        write(
            &package.join("package.json"),
            r#"{"maestro":{"extensions":["*.ts"]}}"#,
        );
        let absolute = scene.path().to_str().unwrap();
        let resolve = |cwd: &str| {
            let (manager, _) = resolution_support::manager(cwd);
            async move {
                manager
                    .resolve_extension_sources(&["npm:pkg".into()], Some(SourceScope::Project))
                    .await
            }
        };
        let relative_from = |current: &std::path::Path| {
            maestro_path::relative(
                current.to_str().unwrap(),
                absolute,
                &maestro_path::Cwd {
                    current: current.to_str().unwrap(),
                    drive_directories: &[],
                },
            )
        };
        let check = |actual: std::io::Result<maestro_packages::ResolvedPaths>, cwd: &str| {
            let actual = actual.unwrap();
            assert_eq!(actual.extensions.len(), 1);
            assert_eq!(
                actual.extensions[0].path,
                package.join("a.ts").to_str().unwrap()
            );
            assert_eq!(
                actual.extensions[0].metadata.base_dir.as_deref(),
                Some(format!("{cwd}/.maestro/npm/node_modules/pkg").as_str())
            );
        };
        let relative = relative_from(&std::env::current_dir()?);
        check(resolve(&relative).await, &relative);
        let Some((_tree, _unlock, process)) = resolution_support::unreadable_cwd(
            "installed_wildcards_resolve_relative_constructor_roots",
        ) else {
            return Ok(());
        };
        check(resolve(absolute).await, absolute);
        let relative = relative_from(&process);
        assert_eq!(
            resolve(&relative).await.err().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        Ok(())
    })
    .unwrap();
}

/// Earlier literal children retain prepared traversal order.
#[test]
fn literal_child_promotion_keeps_order() {
    child_promotion(false);
}

/// Earlier literal children select the same authored alias during canonical dedup.
#[test]
fn literal_child_promotion_keeps_surviving_alias() {
    child_promotion(true);
}

/// Compares ordered authored paths and provenance after native promotion.
fn child_promotion(linked: bool) {
    resolution_support::run(async {
            let scene = scratch::Scratch::new()?;
            write(&scene.path().join("b/preload.ts"), "fixture");
            write(&scene.path().join("a/u.ts"), "fixture");
            if linked {
                std::os::unix::fs::symlink("../a/u.ts", scene.path().join("b/u.ts"))?;
            } else {
                write(&scene.path().join("b/u.ts"), "fixture");
            }
            write(&scene.path().join("package.json"), r#"{"maestro":{"extensions":["{b/preload.ts,*/*.ts}"]}}"#);
            let base = scene.path().to_str().unwrap();
            let (manager, _) = resolution_support::manager(base);
            let actual = manager.resolve_extension_sources(&[base.into()], None).await?;
            let resource = |name| json!({"path":format!("$BASE/{name}"),"enabled":true,"metadata":{"source":"$BASE","scope":"user","origin":"package","baseDir":"$BASE"}});
            let mut expected = vec![resource("b/preload.ts"), resource("a/u.ts")];
            if !linked { expected.push(resource("b/u.ts")); }
            assert_eq!(resolution_support::output(actual, base), json!({"extensions":expected,"skills":[],"prompts":[],"themes":[]}));
        Ok(())
    }).unwrap();
}

/// Cwd ancestry determines the first retained canonical alias.
#[test]
fn manifest_parent_scan_keeps_seeded_alias() {
    cases("manifest_parent_scan_keeps_seeded_alias");
}
/// Later patterns reuse the established listing and retain its first alias.
#[test]
fn manifest_repeated_scan_keeps_listing() {
    cases("manifest_repeated_scan_keeps_listing");
}
