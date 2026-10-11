//! Explicit resource resolution through the manager boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
mod resolution_support;
mod scratch;
use resolution_support::run as block_on;
mod support;
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use std::{fs, rc::Rc};
/// Explicit inputs select scope bases without extension suffix requirements.
#[test]
fn explicit_files_and_empty_inputs_keep_scope_bases() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "empty_sources",
            "missing_local",
            "explicit_opaque_file",
            "empty_source_user",
            "scope_user",
            "scope_project",
            "scope_temporary",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}
/// Four conventional kinds retain complete provenance and traversal order.
#[test]
fn conventional_resources_keep_kind_order_and_provenance() {
    block_on(Box::pin(async {
        resolution_support::cases(&["all_kinds"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Only absent conventional paths permit local root fallback.
#[test]
fn package_root_fallback_requires_absent_conventions() {
    block_on(Box::pin(async {
        resolution_support::cases(&["root_fallback", "empty_convention", "convention_file"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Root manifests select literal entries before either index.
#[test]
fn extension_roots_choose_manifest_before_indexes() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "root_index_ts",
            "root_index_js",
            "root_index_directory",
            "root_manifest",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}
/// Child extension entries do not implicitly load helper modules.
#[test]
fn extension_children_select_entries_without_loading_helpers() {
    block_on(Box::pin(async {
        resolution_support::cases(&["child_entry_rules", "child_manifest", "literal_manifest"])
            .await;
        Ok(())
    }))
    .unwrap();
}
/// Skill entries stop recursion while loose Markdown stays root-only.
#[test]
fn skill_entry_files_stop_descent_and_loose_markdown_is_root_only() {
    block_on(Box::pin(async {
        resolution_support::cases(&["skill_stop", "skill_loose_root"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Ignore files share ordered rule state across traversed directory prefixes.
#[test]
fn ignore_files_keep_declared_order_and_directory_prefixes() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "ignore_order",
            "ignore_parent_pruning",
            "ignore_nested",
            "ignore_nested_basename",
            "ignore_read_failure",
            "unicode_file_order",
            "unicode_nested_scope",
            "unicode_basename_scope",
            "unicode_readmission",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn source_paths_keep_authored_whitespace_semantics() {
    block_on(Box::pin(async {
        resolution_support::cases(&["ecma_trim"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn entry_manifests_fall_back_without_admitted_entries() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "manifest_native_read_failure",
            "manifest_empty_list",
            "manifest_missing_field",
            "manifest_null_field",
            "manifest_wrong_field",
            "manifest_wrong_bool",
            "manifest_wrong_object",
            "manifest_wrong_root",
            "manifest_invalid_json",
            "manifest_null_root",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn entry_manifests_select_last_members_without_consuming_unrelated_fields() {
    block_on(Box::pin(async {
        resolution_support::cases(&["manifest_duplicate", "manifest_unknown_members"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn entry_manifests_keep_literal_paths_and_directory_entries() {
    block_on(Box::pin(async {
        resolution_support::cases(&["manifest_explicit_unusual"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn directory_failures_preserve_already_collected_resources() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "read_dir_partial",
            "native_permission_partial",
            "local_stat_failure",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[cfg(unix)]
#[test]
fn canonical_duplicates_keep_first_authored_path_and_metadata() {
    block_on(Box::pin(async {
        resolution_support::cases(&["canonical_first"]).await;
        resolution_support::first_authored_metadata().await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn source_and_resource_sequences_preserve_first_insertion() {
    block_on(Box::pin(async {
        resolution_support::cases(&["source_order"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn ignore_matching_keeps_case_policy_and_literal_braces() {
    block_on(Box::pin(async {
        resolution_support::cases(&["ignore_case_braces"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn escaped_ignore_punctuation_stays_literal() {
    block_on(Box::pin(async {
        resolution_support::cases(&["ignore_escaped_punctuation"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Discovery prunes hidden names and dependencies without widening suffix matching.
#[test]
fn discovery_skips_hidden_dependencies_and_wrong_suffixes() {
    block_on(Box::pin(async {
        resolution_support::cases(&["suffix_hidden_dependencies"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Ignored or nonfile entries permit remaining skill discovery.
#[test]
fn ignored_or_nonfile_skill_entries_allow_remaining_discovery() {
    block_on(Box::pin(async {
        resolution_support::cases(&["skill_ignored_entry", "skill_entry_directory"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Root entry errors end a local source; child errors retain earlier extensions and later kinds.
#[test]
fn entry_item_failures_keep_their_source_catch_boundaries() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "manifest_bad_item_root",
            "manifest_bad_item_child",
            "earlier_source_survives_bad_root",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}
/// The link fixture follows file targets and skips broken entries.
#[cfg(unix)]
#[test]
fn native_links_follow_targets_and_skip_broken_entries() {
    block_on(Box::pin(async {
        resolution_support::cases(&["link_files"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Filesystem failures terminate cycles without pruning finite authored aliases.
#[cfg(unix)]
#[test]
fn native_cycles_use_filesystem_failures_without_global_visited_sets() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "self_link",
            "growing_link",
            "finite_repeated_link",
            "ignore_bounds_cycle",
            "finite_alias_first",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}
/// Installed unpinned npm and Git sources reuse scoped lookup without acquisition.
#[test]
fn installed_sources_keep_scope_paths_without_acquisition() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "installed_npm_user",
            "installed_npm_project",
            "installed_git_user",
            "installed_git_project",
            "installed_remote_no_layout",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}

/// Canonical effects happen after collection, and failures retain authored identity.
#[test]
fn canonical_failures_preserve_authored_identity() {
    block_on(Box::pin(async {
        use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
        use std::{cell::Cell, rc::Rc};
        let dir = scratch::Scratch::new().unwrap();
        let a = dir.0.join("a.ts");
        let b = dir.0.join("b.ts");
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        let base = dir.0.to_string_lossy().into_owned();
        let calls = Rc::new(Cell::new(0));
        let observed = Rc::clone(&calls);
        let remove = b.clone();
        let resources = resolution_support::Filesystem {
            canonical: Some(Rc::new(move |path| {
                observed.set(observed.get() + 1);
                if path.ends_with("a.ts") {
                    std::fs::remove_file(&remove)?;
                }
                Err(std::io::ErrorKind::PermissionDenied.into())
            })),
            ..Default::default()
        };
        let (_, settings, _) = support::manager(&serde_json::json!({}));
        let manager = DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: base.clone(),
                agent_dir: base.clone(),
                settings_manager: settings,
            },
            support::Controlled(Rc::new(support::Effects {
                resources: Some(Rc::new(resources)),
                ..Default::default()
            })),
        );
        let sources = vec![
            a.to_string_lossy().into_owned(),
            a.to_string_lossy().into_owned(),
            b.to_string_lossy().into_owned(),
        ];
        let result = manager
            .resolve_extension_sources(&sources, None)
            .await
            .unwrap();
        assert_eq!(
            result
                .extensions
                .into_iter()
                .map(|r| r.path)
                .collect::<Vec<_>>(),
            [sources[0].clone(), sources[2].clone()]
        );
        assert_eq!(
            calls.get(),
            2,
            "one canonical observation per authored path"
        );
        Ok(())
    }))
    .unwrap();
}

/// Replacement filesystem adapters retain their supplied extension-entry sequence.
#[test]
fn swapped_filesystem_adapters_keep_supplied_entry_order() {
    block_on(Box::pin(async {
        use maestro_packages::PackageManager;
        let dir = scratch::Scratch::new().unwrap();
        std::fs::create_dir(dir.0.join("extensions")).unwrap();
        for name in ["a.ts", "z.ts"] {
            std::fs::write(dir.0.join("extensions").join(name), "").unwrap();
        }
        let base = dir.0.to_string_lossy().into_owned();
        let (manager, _) = resolution_support::manager(
            &base,
            resolution_support::Filesystem {
                order: vec!["z.ts".into(), "a.ts".into()],
                ..Default::default()
            },
        );
        let result = manager
            .resolve_extension_sources(std::slice::from_ref(&base), None)
            .await
            .unwrap();
        assert_eq!(
            result
                .extensions
                .into_iter()
                .map(|r| r.path)
                .collect::<Vec<_>>(),
            [
                format!("{base}/extensions/z.ts"),
                format!("{base}/extensions/a.ts")
            ]
        );
        Ok(())
    }))
    .unwrap();
}

/// Absolute operands avoid ambient reads; relative and tilde inputs propagate required failures.
#[test]
fn ambient_inputs_are_observed_only_at_their_consuming_branch() {
    block_on(Box::pin(async {
        use maestro_packages::PackageManager;
        let dir = scratch::Scratch::new().unwrap();
        let path = dir.0.join("opaque.data");
        std::fs::write(&path, "").unwrap();
        let absolute = path.to_string_lossy().into_owned();
        let (manager, effects) =
            resolution_support::manager("unanchored", resolution_support::Filesystem::default());
        effects.fail_ambient.set(true);
        let result = manager
            .resolve_extension_sources(std::slice::from_ref(&absolute), None)
            .await
            .unwrap();
        assert_eq!(result.extensions[0].path, absolute);
        assert!(effects.reads.borrow().is_empty());
        let error = manager
            .resolve_extension_sources(&["relative".into()], None)
            .await
            .err()
            .unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(&*effects.reads.borrow(), &["cwd"]);
        let start = effects.reads.borrow().len();
        assert!(
            manager
                .resolve_extension_sources(&["~/path".into()], None)
                .await
                .is_err()
        );
        assert_eq!(&effects.reads.borrow()[start..], &["home"]);
        #[cfg(unix)]
        resolution_support::ambient_entry_resolution().await;
        Ok(())
    }))
    .unwrap();
}

/// Pinned npm, missing and temporary remote modes do not admit unchecked contents.
#[test]
fn deferred_remote_modes_do_not_admit_unchecked_contents() {
    block_on(Box::pin(async {
        use maestro_packages::{PackageManager, SourceScope};
        let dir = scratch::Scratch::new().unwrap();
        for folder in [
            ".maestro/npm/node_modules/pkg/extensions",
            "git/github.com/team/repo/extensions",
        ] {
            let path = dir.0.join(folder);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("unchecked.ts"), "").unwrap();
        }
        let base = dir.0.to_string_lossy().into_owned();
        let (manager, effects) =
            resolution_support::manager(&base, resolution_support::Filesystem::default());
        for (source, scope) in [
            ("npm:pkg@1", SourceScope::Project),
            ("npm:missing", SourceScope::Project),
            ("git:https://github.com/team/missing", SourceScope::User),
            ("git:https://github.com/team/repo", SourceScope::Temporary),
        ] {
            let start = effects.calls.borrow().len();
            let result = manager
                .resolve_extension_sources(&[source.into()], Some(scope))
                .await
                .unwrap();
            assert_eq!(
                resolution_support::output(result, &base),
                serde_json::json!({"extensions":[],"skills":[],"prompts":[],"themes":[]})
            );
            assert!(effects.calls.borrow()[start..].is_empty());
            assert_eq!(effects.spawns.get(), 0);
        }
        Ok(())
    }))
    .unwrap();
}

/// Ignore-file directory identities remain literal beside authored glob syntax.
#[test]
fn ignore_directory_prefixes_are_literal_identities() {
    block_on(Box::pin(async {
        resolution_support::cases(&["ignore_literal_directories"]).await;
        Ok(())
    }))
    .unwrap();
}

/// A quoted slash is not a directory marker in ignore 7.0.5 or native Git.
/// An unquoted directory marker with trailing whitespace still excludes descendants.
#[test]
fn nested_ignore_directory_rules_keep_native_normalization() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "ignore_scoped_directory_normalization",
            "ignore_scoped_escaped_slash",
            "ignore_deeper_negation",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}

/// Unicode case folding excludes the accented name without excluding its decoy.
#[test]
fn ignore_matching_folds_unicode_literal_names() {
    block_on(Box::pin(async {
        resolution_support::cases(&["ignore_unicode_case"]).await;
        Ok(())
    }))
    .unwrap();
}

/// One leading rule byte-order mark is not part of the first ignore pattern.
#[test]
fn ignore_files_strip_one_leading_byte_order_mark() {
    block_on(Box::pin(async {
        resolution_support::cases(&["ignore_file_bom"]).await;
        Ok(())
    }))
    .unwrap();
}

/// An admitted nested skill entry stops descent into a distinguishable child.
#[test]
fn nonroot_skill_entry_stops_deeper_skill_discovery() {
    block_on(Box::pin(async {
        resolution_support::cases(&["skill_nonroot_stop"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Authored ascending ranges retain punctuation and their case-equivalent members.
#[test]
fn ignore_case_matching_preserves_authored_range_members() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "ignore_cross_case_range",
            "ignore_ascii_range",
            "unicode_accent_member",
            "unicode_accent_upper_range",
            "unicode_accent_lower_range",
            "unicode_cross_case_range",
            "unicode_negated_bang",
            "unicode_negated_caret",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}

/// An invalid range matches nothing without preventing later rules from applying.
#[test]
fn invalid_ignore_range_keeps_following_rules_active() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "ignore_invalid_range_continuation",
            "unicode_malformed_range",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}

/// Equivalent Greek scalars are excluded without consuming an ASCII decoy.
#[test]
fn unicode_ignore_literals_select_equivalent_names() {
    block_on(Box::pin(async {
        resolution_support::cases(&["unicode_greek_literals"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Question wildcards consume scalars rather than bytes or grapheme clusters.
#[test]
fn ignore_question_counts_scalars_without_grapheme_folding() {
    block_on(Box::pin(async {
        resolution_support::cases(&["unicode_scalar_question", "unicode_scalar_pair"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Ignored Greek directory equivalents are pruned before child rules load.
#[test]
fn unicode_directory_rules_prune_before_readmission() {
    block_on(Box::pin(async {
        resolution_support::cases(&["unicode_directory_pruning"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Quoted metacharacters select literal filenames rather than wildcard decoys.
#[cfg(unix)]
#[test]
fn escaped_ignore_metacharacters_select_literal_names() {
    block_on(Box::pin(async {
        resolution_support::cases(&["unicode_escaped_syntax"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Directory globstars prune their base while non-directory forms allow child readmission.
#[test]
fn terminal_directory_globstars_keep_zero_directory_matches() {
    block_on(Box::pin(async {
        resolution_support::cases(&[
            "directory_globstar",
            "anchored_directory_globstar",
            "repeated_directory_globstar",
            "non_directory_globstar",
            "directory_globstar_readmission",
            "anchored_directory_globstar_readmission",
            "repeated_directory_globstar_readmission",
        ])
        .await;
        Ok(())
    }))
    .unwrap();
}

/// Comment selection precedes single rule-level byte-order-mark removal.
#[test]
fn ignore_ingress_selects_comments_before_rule_preparation() {
    block_on(Box::pin(async {
        resolution_support::cases(&["ignore_bom_comment"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Rule preparation removes one leading byte-order mark, not two.
#[test]
fn ignore_ingress_preserves_a_second_rule_bom() {
    block_on(Box::pin(async {
        resolution_support::cases(&["ignore_double_bom"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Builds installed resources while keeping the package base authored relative.
async fn resolve_relative_manifest(
    entries: serde_json::Value,
    fail_current: bool,
) -> (
    std::io::Result<maestro_packages::ResolvedPaths>,
    usize,
    String,
) {
    let scratch = scratch::Scratch::new().unwrap();
    let current = std::env::current_dir().unwrap();
    let agent = maestro_path::relative(
        &current.to_string_lossy(),
        &scratch.0.to_string_lossy(),
        &maestro_path::Cwd {
            current: "",
            drive_directories: &[],
        },
    );
    let root = scratch.0.join("git/example.com/org/pkg");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("skills/demo")).unwrap();
    fs::write(root.join("src/a.ts"), "").unwrap();
    fs::write(root.join("skills/demo/SKILL.md"), "").unwrap();
    fs::write(
        root.join("package.json"),
        serde_json::json!({"maestro":entries})
            .to_string()
            .replace("$ROOT", &root.to_string_lossy()),
    )
    .unwrap();
    let reads = Rc::new(std::cell::Cell::new(0));
    let (_, settings, _) = support::manager(&serde_json::json!({}));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "/not-the-ambient-directory".into(),
            agent_dir: agent,
            settings_manager: settings,
        },
        support::Controlled(Rc::new(support::Effects {
            native_exists: true,
            resources: Some(Rc::new(resolution_support::Filesystem {
                fail_current,
                current_reads: reads.clone(),
                ..Default::default()
            })),
            ..Default::default()
        })),
    );
    let result = manager
        .resolve_extension_sources(&["git:https://example.com/org/pkg".into()], None)
        .await;
    (result, reads.get(), root.to_string_lossy().into_owned())
}

/// Relative modifiers select candidate and skill parent identities, not configured cwd.
#[test]
fn relative_installed_roots_apply_each_override() {
    block_on(Box::pin(async {
        for (modifiers, admitted) in [
            (vec!["!src/a.ts"], false),
            (vec!["!src/a.ts", "+src/a.ts"], true),
            (vec!["-src/a.ts"], false),
        ] {
            let skill_modifiers: Vec<_> = modifiers
                .iter()
                .map(|s| s.replace("src/a.ts", "skills/demo"))
                .collect();
            let mut extensions = vec!["$ROOT/src/a.ts".to_owned()];
            extensions.extend(modifiers.into_iter().map(str::to_owned));
            let mut skills = vec!["$ROOT/skills/demo/SKILL.md".to_owned()];
            skills.extend(skill_modifiers);
            let (result, reads, root) = resolve_relative_manifest(
                serde_json::json!({"extensions":extensions,"skills":skills}),
                false,
            )
            .await;
            let result = result.unwrap();
            assert_eq!(
                result
                    .extensions
                    .iter()
                    .map(|r| r.path.as_str())
                    .collect::<Vec<_>>(),
                if admitted {
                    vec![maestro_path::join(&[&root, "src/a.ts"])]
                } else {
                    vec![]
                }
            );
            assert_eq!(
                result
                    .skills
                    .iter()
                    .map(|r| r.path.as_str())
                    .collect::<Vec<_>>(),
                if admitted {
                    vec![maestro_path::join(&[&root, "skills/demo/SKILL.md"])]
                } else {
                    vec![]
                }
            );
            assert!(reads > 0, "relative matcher views require ambient cwd");
            for resource in result.extensions.iter().chain(&result.skills) {
                assert!(!maestro_path::is_absolute(
                    resource.metadata.base_dir.as_deref().unwrap()
                ));
            }
        }

        Ok(())
    }))
    .unwrap();
}

/// Empty candidate sets avoid ambient reads; matching propagates required cwd failure.
#[test]
fn relative_manifest_matching_reads_ambient_only_for_candidates() {
    block_on(Box::pin(async {
        let (result, reads, _) =
            resolve_relative_manifest(serde_json::json!({"extensions":["!src/a.ts"]}), true).await;
        assert!(result.unwrap().extensions.is_empty());
        assert_eq!(reads, 0);
        let (result, reads, _) = resolve_relative_manifest(
            serde_json::json!({"extensions":["$ROOT/src/a.ts","!src/a.ts"]}),
            true,
        )
        .await;
        assert_eq!(
            result.err().expect("required matcher cwd must fail").kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(reads, 1);

        Ok(())
    }))
    .unwrap();
}

/// Relative manifest literals and exclusions use the same ambient package root.
#[test]
fn relative_manifest_literals_are_excluded_from_installed_packages() {
    block_on(Box::pin(async {
    let (result, reads, _) = resolve_relative_manifest(
        serde_json::json!({"extensions":["src/a.ts","!src/a.ts"],"skills":["skills/demo","!skills/demo"]}),
        false,
    ).await;
    let result = result.unwrap();
    assert!(result.extensions.is_empty());
    assert!(result.skills.is_empty());
    assert!(reads > 0);

        Ok(())
    })).unwrap();
}
