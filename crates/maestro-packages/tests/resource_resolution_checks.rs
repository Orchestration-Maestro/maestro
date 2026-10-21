//! Explicit resource resolution through the manager boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[cfg(unix)]
use crate::native_support;
use crate::resolution_support;
use crate::resource_cases;
use crate::scratch;
use crate::support;
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use resolution_support::run as block_on;
use std::{fs, rc::Rc};
/// Explicit inputs select scope bases without extension suffix requirements.
#[test]
fn explicit_files_and_empty_inputs_keep_scope_bases() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&["all_kinds"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Only absent conventional paths permit local root fallback.
#[test]
fn package_root_fallback_requires_absent_conventions() {
    block_on(Box::pin(async {
        resource_cases::cases(&["root_fallback", "empty_convention", "convention_file"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Root manifests select literal entries before either index.
#[test]
fn extension_roots_choose_manifest_before_indexes() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&["child_entry_rules", "child_manifest", "literal_manifest"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Skill entries stop recursion while loose Markdown stays root-only.
#[test]
fn skill_entry_files_stop_descent_and_loose_markdown_is_root_only() {
    block_on(Box::pin(async {
        resource_cases::cases(&["skill_stop", "skill_loose_root"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Ignore files share ordered rule state across traversed directory prefixes.
#[test]
fn ignore_files_keep_declared_order_and_directory_prefixes() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&["ecma_trim"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn entry_manifests_fall_back_without_admitted_entries() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&["manifest_duplicate", "manifest_unknown_members"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn entry_manifests_keep_literal_paths_and_directory_entries() {
    block_on(Box::pin(async {
        resource_cases::cases(&["manifest_explicit_unusual"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn directory_failures_preserve_already_collected_resources() {
    block_on(Box::pin(async {
        resource_cases::cases(&["native_permission_partial"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[cfg(unix)]
#[test]
fn canonical_duplicates_keep_first_authored_path_and_metadata() {
    block_on(Box::pin(async {
        resource_cases::cases(&["canonical_first"]).await;
        resource_cases::first_authored_metadata().await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn source_and_resource_sequences_preserve_first_insertion() {
    block_on(Box::pin(async {
        resource_cases::cases(&["source_order"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn ignore_matching_keeps_case_policy_and_literal_braces() {
    block_on(Box::pin(async {
        resource_cases::cases(&["ignore_case_braces"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Recorded public resource behavior.
#[test]
fn escaped_ignore_punctuation_stays_literal() {
    block_on(Box::pin(async {
        resource_cases::cases(&["ignore_escaped_punctuation"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Discovery prunes hidden names and dependencies without widening suffix matching.
#[test]
fn discovery_skips_hidden_dependencies_and_wrong_suffixes() {
    block_on(Box::pin(async {
        resource_cases::cases(&["suffix_hidden_dependencies"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Ignored or nonfile entries permit remaining skill discovery.
#[test]
fn ignored_or_nonfile_skill_entries_allow_remaining_discovery() {
    block_on(Box::pin(async {
        resource_cases::cases(&["skill_ignored_entry", "skill_entry_directory"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Root entry errors end a local source; child errors retain earlier extensions and later kinds.
#[test]
fn entry_item_failures_keep_their_source_catch_boundaries() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&["link_files"]).await;
        Ok(())
    }))
    .unwrap();
}
/// Filesystem failures terminate cycles without pruning finite authored aliases.
#[cfg(unix)]
#[test]
fn native_cycles_use_filesystem_failures_without_global_visited_sets() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&[
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

/// Absolute operands avoid ambient reads; relative and tilde inputs propagate required failures.
#[test]
fn ambient_inputs_are_observed_only_at_their_consuming_branch() {
    block_on(Box::pin(async {
        use maestro_packages::PackageManager;
        let dir = scratch::Scratch::new().unwrap();
        let path = dir.path().join("opaque.data");
        std::fs::write(&path, "").unwrap();
        let absolute = path.to_string_lossy().into_owned();
        let (manager, effects) = resolution_support::manager("unanchored");
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
        if let Some((_tree, _unlock, process)) = resolution_support::unreadable_cwd(
            "ambient_inputs_are_observed_only_at_their_consuming_branch",
        ) {
            resource_cases::ambient_entry_resolution(&process).await;
        }
        Ok(())
    }))
    .unwrap();
}

/// Ignore-file directory identities remain literal beside authored glob syntax.
#[test]
fn ignore_directory_prefixes_are_literal_identities() {
    block_on(Box::pin(async {
        resource_cases::cases(&["ignore_literal_directories"]).await;
        Ok(())
    }))
    .unwrap();
}

/// A quoted slash is not a directory marker in ignore 7.0.5 or native Git.
/// An unquoted directory marker with trailing whitespace still excludes descendants.
#[test]
fn nested_ignore_directory_rules_keep_native_normalization() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&["ignore_unicode_case"]).await;
        Ok(())
    }))
    .unwrap();
}

/// One leading rule byte-order mark is not part of the first ignore pattern.
#[test]
fn ignore_files_strip_one_leading_byte_order_mark() {
    block_on(Box::pin(async {
        resource_cases::cases(&["ignore_file_bom"]).await;
        Ok(())
    }))
    .unwrap();
}

/// An admitted nested skill entry stops descent into a distinguishable child.
#[test]
fn nonroot_skill_entry_stops_deeper_skill_discovery() {
    block_on(Box::pin(async {
        resource_cases::cases(&["skill_nonroot_stop"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Authored ascending ranges retain punctuation and their case-equivalent members.
#[test]
fn ignore_case_matching_preserves_authored_range_members() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&[
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
        resource_cases::cases(&["unicode_greek_literals"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Question wildcards consume scalars rather than bytes or grapheme clusters.
#[test]
fn ignore_question_counts_scalars_without_grapheme_folding() {
    block_on(Box::pin(async {
        resource_cases::cases(&["unicode_scalar_question", "unicode_scalar_pair"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Ignored Greek directory equivalents are pruned before child rules load.
#[test]
fn unicode_directory_rules_prune_before_readmission() {
    block_on(Box::pin(async {
        resource_cases::cases(&["unicode_directory_pruning"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Quoted metacharacters select literal filenames rather than wildcard decoys.
#[cfg(unix)]
#[test]
fn escaped_ignore_metacharacters_select_literal_names() {
    block_on(Box::pin(async {
        resource_cases::cases(&["unicode_escaped_syntax"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Directory globstars prune their base while non-directory forms allow child readmission.
#[test]
fn terminal_directory_globstars_keep_zero_directory_matches() {
    block_on(Box::pin(async {
        resource_cases::cases(&[
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
        resource_cases::cases(&["ignore_bom_comment"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Rule preparation removes one leading byte-order mark, not two.
#[test]
fn ignore_ingress_preserves_a_second_rule_bom() {
    block_on(Box::pin(async {
        resource_cases::cases(&["ignore_double_bom"]).await;
        Ok(())
    }))
    .unwrap();
}

/// Builds installed resources while keeping the package base authored relative to the
/// working directory `current`.
async fn resolve_relative_manifest(
    entries: serde_json::Value,
    current: &std::path::Path,
) -> (std::io::Result<maestro_packages::ResolvedPaths>, String) {
    let scratch = scratch::Scratch::new().unwrap();
    let agent = maestro_path::relative(
        &current.to_string_lossy(),
        &scratch.path().to_string_lossy(),
        &maestro_path::Cwd {
            current: "",
            drive_directories: &[],
        },
    );
    let root = scratch.path().join("git/example.com/org/pkg");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("skills/demo")).unwrap();
    fs::write(root.join("src/a.ts"), "").unwrap();
    fs::write(root.join("skills/demo/SKILL.md"), "").unwrap();
    let encoded_root = serde_json::to_string(&root.to_string_lossy()).unwrap();
    fs::write(
        root.join("package.json"),
        serde_json::json!({"maestro":entries})
            .to_string()
            .replace("$ROOT", &encoded_root[1..encoded_root.len() - 1]),
    )
    .unwrap();
    let (_, settings, _) = support::manager(&serde_json::json!({}));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "/not-the-ambient-directory".into(),
            agent_dir: agent,
            settings_manager: settings,
        },
        support::Controlled(Rc::new(support::Effects {
            native_exists: true,
            ..Default::default()
        })),
    );
    let result = manager
        .resolve_extension_sources(&["git:https://example.com/org/pkg".into()], None)
        .await;
    (result, root.to_string_lossy().into_owned())
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
            let (result, root) = resolve_relative_manifest(
                serde_json::json!({"extensions":extensions,"skills":skills}),
                &std::env::current_dir()?,
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
#[cfg(unix)]
#[test]
fn relative_manifest_matching_reads_ambient_only_for_candidates() {
    let Some((tree, _unlock, process)) = resolution_support::unreadable_cwd(
        "relative_manifest_matching_reads_ambient_only_for_candidates",
    ) else {
        return;
    };
    block_on(Box::pin(async {
        let (result, _) =
            resolve_relative_manifest(serde_json::json!({"extensions":["!src/a.ts"]}), &process)
                .await;
        assert!(result.unwrap().extensions.is_empty());
        let matching = serde_json::json!({"extensions":["$ROOT/src/a.ts","!src/a.ts"]});
        let (result, _) = resolve_relative_manifest(matching.clone(), &process).await;
        assert_eq!(
            result.err().expect("required matcher cwd must fail").kind(),
            std::io::ErrorKind::PermissionDenied
        );
        let removed = tree.enter_removed("removed")?;
        let (result, _) = resolve_relative_manifest(matching, &removed).await;
        assert_eq!(
            result
                .err()
                .expect("required matcher cwd must fail")
                .raw_os_error(),
            Some(2)
        );
        Ok(())
    }))
    .unwrap();
}

/// Relative manifest literals and exclusions use the same ambient package root.
#[test]
fn relative_manifest_literals_are_excluded_from_installed_packages() {
    block_on(Box::pin(async {
    let (result, _) = resolve_relative_manifest(
        serde_json::json!({"extensions":["src/a.ts","!src/a.ts"],"skills":["skills/demo","!skills/demo"]}),
        &std::env::current_dir()?,
    ).await;
    let result = result.unwrap();
    assert!(result.extensions.is_empty());
    assert!(result.skills.is_empty());

        Ok(())
    })).unwrap();
}

/// Unfiltered absolute candidates under a relative root do not need ambient context.
#[cfg(unix)]
#[test]
fn unfiltered_relative_manifest_does_not_read_ambient_context() {
    assert_context_free_manifest(
        "unfiltered_relative_manifest_does_not_read_ambient_context",
        false,
    );
}

/// Already admitted candidates do not need exact restoration matching.
#[cfg(unix)]
#[test]
fn restoration_only_relative_manifest_does_not_read_ambient_context() {
    assert_context_free_manifest(
        "restoration_only_relative_manifest_does_not_read_ambient_context",
        true,
    );
}

/// Checks every returned field for an installed absolute candidate with an unreadable cwd.
#[cfg(unix)]
fn assert_context_free_manifest(test: &str, restore: bool) {
    let Some((_tree, _unlock, process)) = resolution_support::unreadable_cwd(test) else {
        return;
    };
    block_on(Box::pin(async {
        let mut entries = vec!["$ROOT/src/a.ts"];
        if restore {
            entries.push("+src/a.ts");
        }
        let (result, root) =
            resolve_relative_manifest(serde_json::json!({"extensions": entries}), &process).await;
        let result = result.expect("selection without matching must not require cwd");
        assert_eq!(result.extensions.len(), 1);
        let extension = &result.extensions[0];
        assert_eq!(extension.path, maestro_path::join(&[&root, "src/a.ts"]));
        assert!(extension.enabled);
        assert_eq!(extension.metadata.source, "git:https://example.com/org/pkg");
        assert_eq!(
            extension.metadata.scope,
            maestro_resources::SourceScope::User
        );
        assert_eq!(
            extension.metadata.origin,
            maestro_resources::SourceOrigin::Package
        );
        let base = maestro_path::relative(
            &process.to_string_lossy(),
            &root,
            &maestro_path::Cwd {
                current: "",
                drive_directories: &[],
            },
        );
        assert_eq!(extension.metadata.base_dir.as_deref(), Some(base.as_str()));
        assert!(result.skills.is_empty());
        assert!(result.prompts.is_empty());
        assert!(result.themes.is_empty());
        Ok(())
    }))
    .unwrap();
}

/// Resolves the package at `root`.
#[cfg(unix)]
fn resolved_relative(root: &std::path::Path) -> [Vec<String>; 4] {
    let base = root.to_string_lossy().into_owned();
    let (manager, _) = resolution_support::manager(&base);
    let mut result = None;
    resolution_support::run(async {
        result = Some(
            manager
                .resolve_extension_sources(std::slice::from_ref(&base), None)
                .await?,
        );
        Ok(())
    })
    .unwrap();
    let result = result.unwrap();
    [
        result.extensions,
        result.skills,
        result.prompts,
        result.themes,
    ]
    .map(|items| {
        items
            .into_iter()
            .map(|r| r.path.replace(&base, ""))
            .collect()
    })
}
/// Conventional collectors keep linked files and directories and skip failed link reads.
#[cfg(unix)]
#[test]
fn package_entry_links_preserve_admission_and_failure_fallback() {
    use std::os::unix::fs::symlink;
    let put = |path: &std::path::Path| native_support::write(path.to_str().unwrap(), "{}");
    let dir = scratch::Scratch::new().unwrap();
    let root = dir.path().join("pkg");
    let outside = dir.path().join("outside");
    for (kind, file, nested) in [
        ("extensions", "t.ts", "index.ts"),
        ("skills", "t.md", "SKILL.md"),
        ("prompts", "t.md", "n.md"),
        ("themes", "t.json", "n.json"),
    ] {
        put(&root.join(kind).join(format!("ok.{}", &file[2..])));
        put(&outside.join(kind).join(file));
        put(&outside.join(kind).join("dir").join(nested));
        let ext = &file[2..];
        let entries = root.join(kind);
        symlink(
            outside.join(kind).join(file),
            entries.join(format!("file.{ext}")),
        )
        .unwrap();
        symlink(outside.join(kind).join("dir"), entries.join("dir")).unwrap();
        symlink("missing", entries.join(format!("broken.{ext}"))).unwrap();
        put(&dir.path().join("locked").join(kind).join(file));
        symlink(
            dir.path().join("locked").join(kind).join(file),
            entries.join(format!("denied.{ext}")),
        )
        .unwrap();
    }
    let _locked = resource_cases::Denied::new(dir.path().join("locked"));
    let [extensions, skills, prompts, themes] = resolved_relative(&root);
    assert_eq!(
        extensions,
        [
            "/extensions/dir/index.ts",
            "/extensions/file.ts",
            "/extensions/ok.ts"
        ]
    );
    assert_eq!(
        skills,
        ["/skills/dir/SKILL.md", "/skills/file.md", "/skills/ok.md"]
    );
    assert_eq!(
        prompts,
        ["/prompts/dir/n.md", "/prompts/file.md", "/prompts/ok.md"]
    );
    assert_eq!(
        themes,
        ["/themes/dir/n.json", "/themes/file.json", "/themes/ok.json"]
    );
}
/// A linked skill entry admits only a file target; any other outcome leaves the children.
#[cfg(unix)]
#[test]
fn package_skill_root_links_follow_the_target_kind() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let cases: [(&str, &[&str]); 5] = [
        ("file", &["/skills/SKILL.md"]),
        ("directory", &["/skills/child/SKILL.md"]),
        ("socket", &["/skills/child/SKILL.md"]),
        ("dangling", &["/skills/child/SKILL.md"]),
        ("denied", &["/skills/child/SKILL.md"]),
    ];
    for (name, expected) in cases {
        let dir = scratch::Scratch::new().unwrap();
        let root = dir.path().join("pkg");
        native_support::write(root.join("skills/child/SKILL.md").to_str().unwrap(), "{}");
        let target = dir.path().join(if name == "denied" {
            "locked/target"
        } else {
            "target"
        });
        let entry = root.join("skills/SKILL.md");
        let _socket;
        match name {
            "file" | "denied" => native_support::write(target.to_str().unwrap(), "{}"),
            "directory" => fs::create_dir(&target).unwrap(),
            "socket" => _socket = UnixListener::bind(&target).unwrap(),
            _ => {}
        }
        symlink(&target, &entry).unwrap();
        let _locked =
            (name == "denied").then(|| resource_cases::Denied::new(dir.path().join("locked")));
        assert_eq!(resolved_relative(&root)[1], expected, "{name}");
    }
}
