//! Manifest expansion through the existing public package manager.
#![cfg(test)]
#![cfg(target_os = "linux")]
mod resolution_support;
mod resource_output;
mod scratch;
mod support;
use maestro_packages::PackageManager;
use serde_json::json;
use std::{fs, io::Write};
/// Wildcards produce native matches rather than a literal missing operand.
#[test]
fn manifest_expansion_keeps_native_match_order() {
    cases("manifest_expansion_keeps_native_match_order");
    resolution_support::run(async {
        let scene = scratch::Scratch::new()?;
        for name in ["a.ts", "b.ts", "z.ts", "other.md"] {
            fs::OpenOptions::new().create_new(true).write(true).open(scene.0.join(name))?;
        }
        let mut manifest = fs::OpenOptions::new().create_new(true).write(true).open(scene.0.join("package.json"))?;
        manifest.write_all(br#"{"maestro":{"extensions":["*.ts"]}}"#)?;
        drop(manifest);
        let base = scene.0.to_string_lossy().into_owned();
        let (manager, _) = resolution_support::manager(&base, resolution_support::Filesystem::default());
        let actual = manager.resolve_extension_sources(std::slice::from_ref(&base), None).await?;
        let resource = |name| json!({"path":format!("$BASE/{name}"),"enabled":true,"metadata":{"source":"$BASE","scope":"user","origin":"package","baseDir":"$BASE"}});
        assert_eq!(resolution_support::output(actual, &base), json!({"extensions":[resource("z.ts"),resource("b.ts"),resource("a.ts")],"skills":[],"prompts":[],"themes":[]}));
        Ok(())
    }).unwrap();
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
/// Exclusively writes one authored fixture file.
fn write(path: &std::path::Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
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
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, path).unwrap();
        #[cfg(windows)]
        if scene.join(target).is_dir() {
            std::os::windows::fs::symlink_dir(target, path).unwrap();
        } else {
            std::os::windows::fs::symlink_file(target, path).unwrap();
        }
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
    setup(&scene.0, &inputs);
    let base = scene.0.to_string_lossy().into_owned();
    let root = row.root.replace("$SCENE", &base);
    let manifest = row.manifest.to_string().replace("$SCENE", &base);
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    write(
        &std::path::Path::new(&root).join("package.json"),
        &json!({"maestro":manifest}).to_string(),
    );
    #[cfg(unix)]
    if row.denied {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(scene.0.join("denied"), fs::Permissions::from_mode(0o0)).unwrap();
        assert!(fs::read_dir(scene.0.join("denied")).is_err());
    }
    #[cfg(not(unix))]
    assert!(!row.denied, "native permission recipe is Unix-only");
    let (manager, _) =
        resolution_support::manager(&root, resolution_support::Filesystem::default());
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
    #[cfg(unix)]
    if row.denied {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(scene.0.join("denied"), fs::Permissions::from_mode(0o755)).unwrap();
    }
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
    nofollow_failure();
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

/// One complete controlled-phase public output.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Phase {
    /// Phase selection and failure label.
    case: String,
    /// Ordered resources and complete provenance.
    expected: serde_json::Value,
}
/// Compares one completed phase against its independent recording.
fn compare_phase(result: maestro_packages::ResolvedPaths, base: &str, case: &str) {
    let phases: Vec<Phase> =
        serde_json::from_str(include_str!("fixtures/manifest_expansion_phases.json")).unwrap();
    let expected = phases.into_iter().find(|p| p.case == case).unwrap();
    let actual = resolution_support::output(result, base)
        .to_string()
        .replace("$BASE", "$SCENE");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&actual).unwrap(),
        expected.expected,
        "{}",
        expected.case
    );
}
/// Collection changes a later search directory only after all entries expanded.
#[test]
fn manifest_expands_all_entries_before_collecting() {
    resolution_support::run(async {
        let scene = scratch::Scratch::new()?;
        write(&scene.0.join("phase/a/first.ts"), "fixture");
        write(&scene.0.join("phase/b/early.ts"), "fixture");
        write(
            &scene.0.join("phase/package.json"),
            r#"{"maestro":{"extensions":["a","b/*.ts"]}}"#,
        );
        let written = std::rc::Rc::new(std::cell::Cell::new(false));
        let observed = std::rc::Rc::clone(&written);
        let late = scene.0.join("phase/b/late.ts");
        let resources = resolution_support::Filesystem {
            on_read: Some(std::rc::Rc::new(move |path| {
                if path.ends_with("phase/a") && !observed.get() {
                    drop(
                        fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&late)?,
                    );
                    observed.set(true);
                }
                Ok(())
            })),
            ..Default::default()
        };
        let base = scene.0.to_string_lossy().into_owned();
        let root = format!("{base}/phase");
        let (manager, _) = resolution_support::manager(&root, resources);
        let first = manager
            .resolve_extension_sources(std::slice::from_ref(&root), None)
            .await?;
        assert!(written.get(), "native write completed during collection");
        compare_phase(first, &base, "before_collection");
        let second = manager
            .resolve_extension_sources(std::slice::from_ref(&root), None)
            .await?;
        compare_phase(second, &base, "next_resolution");
        Ok(())
    })
    .unwrap();
}
/// Failed no-follow metadata suppresses the affected literal, not its peer.
fn nofollow_failure() {
    resolution_support::run(async {
        let scene = scratch::Scratch::new()?;
        write(&scene.0.join("failure/bad.ts"), "fixture");
        write(&scene.0.join("failure/okay.ts"), "fixture");
        write(
            &scene.0.join("failure/package.json"),
            r#"{"maestro":{"extensions":["{bad.ts,ok*.ts}"]}}"#,
        );
        let base = scene.0.to_string_lossy().into_owned();
        let root = format!("{base}/failure");
        let resources = resolution_support::Filesystem {
            fail: std::collections::BTreeMap::from([(
                "lstatSync".into(),
                vec!["failure/bad.ts".into()],
            )]),
            ..Default::default()
        };
        let (manager, _) = resolution_support::manager(&root, resources);
        let result = manager
            .resolve_extension_sources(std::slice::from_ref(&root), None)
            .await?;
        compare_phase(result, &base, "nofollow_failure");
        Ok(())
    })
    .unwrap();
}
