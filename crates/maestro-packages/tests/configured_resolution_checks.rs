//! Configured package resolution through the public operation.
#![cfg(unix)]
use crate::configured_resolution_support;
use crate::resolution_support;
use crate::scratch;
use crate::support;
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use maestro_settings::SettingsManager;
use serde_json::json;
use std::{cell::RefCell, fs, path::Path, rc::Rc};

#[test]
fn empty_settings_have_no_package_resources() {
    use maestro_packages::PackageManager;
    let scratch = scratch::Scratch::new().unwrap();
    let base = scratch.path().to_str().unwrap();
    let (manager, effects) = resolution_support::manager(
        base,
        configured_resolution_support::owned_filesystem(scratch.path()),
    );
    *effects.home_override.borrow_mut() = Some(format!("{base}/home"));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = std::rc::Rc::clone(&events);
    manager.set_progress_callback(Some(std::rc::Rc::new(move |event| {
        sink.borrow_mut().push(event.r#type);
        Ok(())
    })));
    resolution_support::run(async {
        let result = manager.resolve(None).await?;
        assert_eq!(
            resolution_support::output(result, base),
            serde_json::json!({"extensions":[],"skills":[],"prompts":[],"themes":[]})
        );
        assert!(effects.calls.borrow().is_empty());
        assert!(events.borrow().is_empty());
        Ok(())
    })
    .unwrap();
}

#[test]
fn configured_entries_use_each_scope_base() {
    configured_resolution_support::check(&[
        "locals-extensions",
        "locals-skills",
        "locals-prompts",
        "locals-themes",
    ]);
}

#[test]
fn local_entries_apply_patterns_to_discovered_candidates() {
    configured_resolution_support::check(&[
        "local-pattern-0",
        "local-pattern-1",
        "local-pattern-2",
        "local-pattern-3",
        "local-pattern-4",
        "local-pattern-5",
        "local-pattern-6",
        "local-pattern-7",
        "local-pattern-8",
        "local-pattern-9",
        "local-pattern-10",
        "local-pattern-11",
        "local-pattern-12",
        "local-pattern-13",
        "local-pattern-14",
        "local-pattern-15",
        "local-pattern-16",
        "local-pattern-17",
        "local-pattern-18",
        "local-pattern-19",
        "local-pattern-20",
        "local-pattern-21",
    ]);
}

#[test]
fn package_filters_select_then_override_without_reordering() {
    configured_resolution_support::check(&[
        "filter-0",
        "filter-1",
        "filter-2",
        "filter-3",
        "filter-4",
        "filter-5",
        "filter-6",
        "filter-7",
        "filter-8",
        "filter-9",
        "filter-10",
        "filter-11",
        "filter-12",
        "filter-13",
        "filter-14",
        "filter-15",
        "filter-16",
        "filter-17",
        "filter-18",
        "filter-19",
        "filter-20",
        "filter-21",
        "filter-22",
    ]);
}

#[test]
fn automatic_prompts_and_themes_are_shallow() {
    configured_resolution_support::check(&["auto-shallow"]);
}

#[test]
fn authored_duplicates_select_better_precedence_and_keep_equal_first() {
    configured_resolution_support::check(&[
        "disabled-identical-path",
        "disabled-alias-correction",
        "equal-ranked-authored-path",
        "replacement-keeps-authored-slot",
        "same-authored-path",
    ]);
}

#[test]
fn missing_source_actions_are_sequential_and_exact() {
    configured_resolution_support::check(&[
        "npm-default",
        "npm-skip",
        "npm-error",
        "npm-install",
        "npm-throw",
        "git-default",
        "git-skip",
        "git-error",
        "git-install",
        "git-throw",
    ]);
}

#[test]
fn configured_literal_roots_do_not_expand_again() {
    configured_resolution_support::check(&["literal-star-base"]);
}

#[test]
fn configured_directories_honor_extension_entry_manifests() {
    configured_resolution_support::check(&["configured-entry-manifest"]);
}

#[test]
fn configured_paths_keep_authored_trim_and_tilde_rules() {
    configured_resolution_support::check(&[
        "path-efbbbf2e2e2f6c6f63616c2e7473efbbbf",
        "path-c2852e2e2f6c6f63616c2e7473c285",
        "path-7e66696c652e7473",
        "path-7e2f66696c652e7473",
        "path-7e",
    ]);
}

#[test]
fn local_entries_split_only_star_question_and_modifiers() {
    configured_resolution_support::check(&["literal-brackets-braces"]);
}

#[test]
fn package_filters_cannot_restore_manifest_exclusions() {
    configured_resolution_support::check(&["manifest-layer"]);
}

#[test]
fn manifest_restorations_define_filtered_candidate_order() {
    configured_resolution_support::check(&["manifest-order-true", "manifest-order-false"]);
}

#[test]
fn unspecified_filters_use_manifest_defaults_and_empty_filters_disable() {
    configured_resolution_support::check(&[
        "default-absent",
        "default-empty",
        "default-empty-filter",
        "default-filtered",
        "default-null",
        "default-false",
        "default-zero",
        "default-empty-string",
    ]);
}

#[test]
fn filters_cover_all_resource_kinds_and_skill_parent_identity() {
    configured_resolution_support::check(&["filter-kinds"]);
}

#[test]
fn filtered_empty_directories_do_not_become_extensions() {
    configured_resolution_support::check(&["empty-filter-dir"]);
}

#[test]
fn local_files_ignore_package_resource_filters() {
    configured_resolution_support::check(&["filter-file"]);
}

#[test]
fn local_missing_sources_skip_without_callbacks() {
    configured_resolution_support::check(&["local-missing"]);
}

#[test]
fn local_filter_failures_are_caught_after_prior_kinds() {
    configured_resolution_support::check(&["local-bad-filter"]);
}

#[test]
fn remote_filter_failures_propagate_after_admission() {
    configured_resolution_support::check(&["remote-bad-filter"]);
}

#[test]
fn same_scope_package_duplicates_keep_first_filter() {
    configured_resolution_support::check(&["duplicate-filter"]);
}

#[test]
fn project_package_identity_overrides_user_pins_and_filters() {
    configured_resolution_support::check(&["scope-identity"]);
}

#[test]
fn local_package_identity_uses_scope_bases() {
    configured_resolution_support::check(&["local-identity"]);
}

#[test]
fn git_transport_and_ref_aliases_dedupe_before_missing_callback() {
    configured_resolution_support::check(&["git-identity"]);
}

#[test]
fn canonical_collisions_follow_stable_resource_precedence() {
    configured_resolution_support::check(&["canonical-ranks"]);
}

#[test]
fn stable_precedence_preserves_each_rank_and_kind_order() {
    configured_resolution_support::check(&[
        "all-ranks-extensions",
        "all-ranks-skills",
        "all-ranks-prompts",
        "all-ranks-themes",
    ]);
}

#[test]
fn disabled_canonical_winners_do_not_revive_through_aliases() {
    configured_resolution_support::check(&["disabled-alias"]);
}

#[test]
fn automatic_extensions_reuse_smart_entry_discovery() {
    configured_resolution_support::check(&["auto-extensions"]);
}

#[test]
fn automatic_skills_distinguish_conventional_and_agents_roots() {
    configured_resolution_support::check(&["auto-skill-modes"]);
}

#[test]
fn automatic_skill_root_entry_stops_descent() {
    configured_resolution_support::check(&["auto-skill-stop"]);
}

#[test]
fn agents_ancestry_is_nearest_first_and_stops_at_git_marker() {
    configured_resolution_support::check(&["agents-git-directory", "agents-ancestry"]);
}

#[test]
fn non_git_ancestry_reaches_root_without_stealing_home_scope() {
    configured_resolution_support::check(&["agents-home"]);
}

#[test]
fn automatic_symlink_aliases_keep_first_provenance() {
    configured_resolution_support::check(&["auto-link"]);
}

#[test]
fn automatic_ignore_roots_do_not_inherit_parent_rules() {
    configured_resolution_support::check(&[
        "agents-ignored-root",
        "agents-unanchored-root",
        "ignore-root",
    ]);
}

#[test]
fn automatic_overrides_ignore_positive_patterns() {
    configured_resolution_support::check(&["auto-overrides", "auto-positive-pattern-only"]);
}

#[test]
fn automatic_read_failures_do_not_hide_other_kinds() {
    configured_resolution_support::check(&["auto-denied"]);
}

#[test]
fn user_missing_acquisition_uses_global_install_path() {
    configured_resolution_support::check(&["user-acquire"]);
}

#[test]
fn offline_skips_before_callback_but_installed_sources_remain() {
    configured_resolution_support::check(&[
        "offline-_1_",
        "offline-_true_",
        "offline-_TRUE_",
        "offline-_yes_",
        "offline-_YeS_",
        "offline-_0_",
        "offline-_false_",
        "offline-__true__",
        "offline-__",
    ]);
}

#[test]
fn installed_unpinned_packages_do_not_query_registry() {
    configured_resolution_support::check(&["unpinned"]);
}

#[test]
fn pinned_versions_compare_literal_installed_values() {
    configured_resolution_support::check(&["pin-1.0.0", "pin-2.0.0", "pin-^1.0.0"]);
}

#[test]
fn acquisition_failure_stops_later_sources() {
    configured_resolution_support::check(&["install-failure"]);
}

#[test]
fn settings_snapshots_survive_callback_edits_until_next_run() {
    configured_resolution_support::check(&["snapshot"]);
}

#[test]
fn late_selected_fields_fail_after_completed_acquisition() {
    configured_resolution_support::check(&["late-field"]);
}

#[test]
fn null_settings_lists_default_without_validating_unread_members() {
    configured_resolution_support::check(&["null-lists"]);
}

#[test]
fn ambient_reads_follow_public_resolution_phases() {
    configured_resolution_support::check(&[
        "home-required",
        "home-avoided",
        "cwd-avoided",
        "cwd-required",
    ]);
}

#[test]
fn top_level_skill_filters_match_parent_paths() {
    configured_resolution_support::check(&["local-skill-pattern"]);
}

#[test]
fn top_level_patterns_apply_to_themes_and_prompts() {
    configured_resolution_support::check(&["local-other-patterns", "local-other-restores"]);
}

#[test]
fn missing_callback_admission_waits_before_next_source() {
    configured_resolution_support::check(&["callback-gate"]);
}
#[test]
fn missing_callback_borrow_ends_with_operation() {
    configured_resolution_support::borrowed_callback();
}

#[test]
fn project_home_failure_precedes_malformed_user_list() {
    configured_resolution_support::check(&["project-home-before-user-decode"]);
}

#[test]
fn relative_automatic_and_installed_roots_share_ambient_matching_context() {
    let scratch = scratch::Scratch::new_in(Path::new(".")).unwrap();
    let base = scratch.path().to_str().unwrap().strip_prefix("./").unwrap();
    let user = format!("{base}/user");
    let project = format!("{base}/project");
    let package = format!("{project}/.maestro/npm/node_modules/pkg");
    for root in [&user, &package] {
        fs::create_dir_all(format!("{root}/prompts")).unwrap();
        for name in ["a", "b", "c"] {
            fs::write(format!("{root}/prompts/{name}.md"), name).unwrap();
        }
    }
    fs::write(format!("{project}/.git"), "marker").unwrap();
    fs::write(format!("{package}/package.json"), "{}").unwrap();
    let effects = Rc::new(support::Effects {
        native_exists: true,
        resources: Some(Rc::new(configured_resolution_support::owned_filesystem(
            scratch.path(),
        ))),
        ..Default::default()
    });
    *effects.home_override.borrow_mut() = Some(format!("{base}/home"));
    for (patterns, expected) in [
        (json!(["prompts/a.md"]), [true, false, false]),
        (json!(["!prompts/a.md"]), [false, true, true]),
        (json!(["!prompts/*", "+prompts/a.md"]), [true, false, false]),
        (
            json!(["!prompts/*", "+prompts/a.md", "-prompts/a.md"]),
            [false, false, false],
        ),
    ] {
        fs::write(
            format!("{user}/settings.json"),
            json!({"prompts":patterns}).to_string(),
        )
        .unwrap();
        fs::write(
            format!("{project}/.maestro/settings.json"),
            json!({"packages":[{"source":"npm:pkg", "prompts":patterns}]}).to_string(),
        )
        .unwrap();
        let settings = Rc::new(RefCell::new(SettingsManager::create(
            &project, &user, ".maestro",
        )));
        let manager = DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: project.clone(),
                agent_dir: user.clone(),
                settings_manager: settings,
            },
            support::Controlled(Rc::clone(&effects)),
        );
        resolution_support::run(async {
            let result = manager.resolve(None).await?;
            assert_relative_selection(&result, [&user, &package], &patterns, expected);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn automatic_directory_alias_collisions_keep_project_owner_for_each_kind() {
    configured_resolution_support::check(&["auto-four-kind-links"]);
}

/// Checks authored paths and independent enabled decisions after completed resolution.
#[cfg(test)]
fn assert_relative_selection(
    result: &maestro_packages::ResolvedPaths,
    roots: [&str; 2],
    patterns: &serde_json::Value,
    expected: [bool; 3],
) {
    for (root_index, root) in roots.into_iter().enumerate() {
        let records: Vec<_> = result
            .prompts
            .iter()
            .filter(|r| r.path.starts_with(root))
            .collect();
        assert_eq!(records.len(), 3);
        for (index, name) in ["a", "b", "c"].iter().enumerate() {
            let record = records
                .iter()
                .find(|r| r.path == format!("{root}/prompts/{name}.md"))
                .unwrap();
            let enabled = if root_index == 0 && *patterns == json!(["prompts/a.md"]) {
                true
            } else {
                expected[index]
            };
            assert_eq!(record.enabled, enabled, "{root}/{name}: {patterns}");
        }
    }
}

#[test]
fn automatic_relative_home_keeps_authored_path_and_exact_override() {
    let scratch = scratch::Scratch::new_in(Path::new(".")).unwrap();
    let base = scratch.path().to_str().unwrap().strip_prefix("./").unwrap();
    let home = format!("{base}/home");
    let user = format!("{base}/user");
    let project = format!("{base}/project");
    let skill = format!("{home}/.agents/skills/k/SKILL.md");
    fs::create_dir_all(Path::new(&skill).parent().unwrap()).unwrap();
    fs::write(&skill, "skill").unwrap();
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(&project).unwrap();
    fs::write(format!("{project}/.git"), "marker").unwrap();
    let effects = Rc::new(support::Effects {
        native_exists: true,
        resources: Some(Rc::new(configured_resolution_support::owned_filesystem(
            scratch.path(),
        ))),
        ..Default::default()
    });
    *effects.home_override.borrow_mut() = Some(home);
    for (patterns, enabled) in [(json!([]), true), (json!([format!("-{skill}")]), false)] {
        fs::write(
            format!("{user}/settings.json"),
            json!({"skills":patterns}).to_string(),
        )
        .unwrap();
        let settings = Rc::new(RefCell::new(SettingsManager::create(
            &project, &user, ".maestro",
        )));
        let manager = DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: project.clone(),
                agent_dir: user.clone(),
                settings_manager: settings,
            },
            support::Controlled(Rc::clone(&effects)),
        );
        resolution_support::run(async {
            let result = manager.resolve(None).await?;
            assert_eq!(result.skills.len(), 1);
            assert_eq!(result.skills[0].path, skill);
            assert_eq!(result.skills[0].enabled, enabled);
            assert_eq!(
                result.skills[0].metadata.scope,
                maestro_resources::SourceScope::User
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn symlinked_fixture_parent_keeps_lexical_resource_boundaries() {
    use maestro_packages::PackageManager;
    let scratch = scratch::Scratch::new().unwrap();
    let parent = scratch.path().join("native");
    std::fs::create_dir(&parent).unwrap();
    let alias = scratch.path().join("alias");
    std::os::unix::fs::symlink(&parent, &alias).unwrap();
    let root = alias.join("owned");
    std::fs::create_dir(&root).unwrap();
    let inside = root.join("inside.md");
    let outside = alias.join("outside.md");
    std::fs::write(&inside, "inside prompt").unwrap();
    std::fs::write(&outside, "outside prompt").unwrap();
    let base = root.to_str().unwrap();
    let (manager, effects) =
        resolution_support::manager(base, configured_resolution_support::owned_filesystem(&root));
    *effects.home_override.borrow_mut() = Some(format!("{base}/home"));
    resolution_support::run(async {
        let result = manager
            .resolve_extension_sources(&[inside.to_string_lossy().into_owned()], None)
            .await?;
        assert_eq!(result.extensions.len(), 1);
        assert_eq!(result.extensions[0].path, inside.to_string_lossy());
        let result = manager
            .resolve_extension_sources(&[outside.to_string_lossy().into_owned()], None)
            .await?;
        assert!(result.extensions.is_empty());
        Ok(())
    })
    .unwrap();
}
