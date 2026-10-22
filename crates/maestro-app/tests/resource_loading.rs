//! Resource discovery, overrides, provenance and missing-path diagnostics against the
//! recorded corpus.
#![cfg(unix)]
use crate::resource_fixtures::{
    ExtensionSpec, LOADER, LoaderCorpus, Root, check_loader_cases, loader, loader_case, observe,
    on_task_set, resource_options, run_loader_case, with_isolated_home,
};
use maestro_app::{LoadPromptsResult, LoadThemesResult, ResourceDiagnostic, ResourceLoader};
use maestro_resources::{
    DiagnosticType, LoadSkillsResult, Skill, SourceInfo, SourceOrigin, SourceScope,
};
use maestro_settings::{Settings, SettingsManager};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The skill every override witness injects.
fn injected_skill() -> Skill {
    Skill {
        name: "injected".into(),
        description: "Injected skill".into(),
        file_path: "/fake/path".into(),
        base_dir: "/fake".into(),
        source_info: SourceInfo {
            path: "/fake/path".into(),
            source: "custom".into(),
            scope: SourceScope::Temporary,
            origin: SourceOrigin::TopLevel,
            base_dir: None,
        },
        disable_model_invocation: false,
    }
}

#[test]
fn constructor_results_are_empty_before_reload() {
    check_loader_cases(
        "constructor_results_are_empty_before_reload",
        &["constructor_results_are_empty"],
    );
}

#[test]
fn user_skill_files_are_discovered() {
    check_loader_cases(
        "user_skill_files_are_discovered",
        &["user_skill_file_is_discovered"],
    );
}

#[test]
fn skill_directories_ignore_extra_markdown() {
    check_loader_cases(
        "skill_directories_ignore_extra_markdown",
        &["auto_skill_directory_ignores_extra_markdown"],
    );
}

#[test]
fn user_prompts_are_discovered() {
    check_loader_cases(
        "user_prompts_are_discovered",
        &["user_prompt_is_discovered"],
    );
}

#[test]
fn project_resources_win_name_collisions() {
    check_loader_cases(
        "project_resources_win_name_collisions",
        &["project_resources_win_name_collisions"],
    );
}

#[test]
fn settings_exclusions_hide_automatic_resources() {
    check_loader_cases(
        "settings_exclusions_hide_automatic_resources",
        &["settings_exclusions_hide_automatic_resources"],
    );
}

#[test]
fn cwd_context_file_is_discovered() {
    check_loader_cases(
        "cwd_context_file_is_discovered",
        &["cwd_agents_file_is_discovered"],
    );
}

#[test]
fn disabled_context_files_are_not_discovered() {
    check_loader_cases(
        "disabled_context_files_are_not_discovered",
        &["no_context_files_skips_discovery"],
    );
}

#[test]
fn project_system_prompt_file_is_discovered() {
    check_loader_cases(
        "project_system_prompt_file_is_discovered",
        &["project_system_file_is_discovered"],
    );
}

#[test]
fn project_append_prompt_file_is_discovered() {
    check_loader_cases(
        "project_append_prompt_file_is_discovered",
        &["project_append_file_is_discovered"],
    );
}

#[test]
fn extended_resources_carry_extension_provenance() {
    check_loader_cases(
        "extended_resources_carry_extension_provenance",
        &["extended_resources_keep_extension_metadata"],
    );
}

#[test]
fn disabled_skills_skip_discovery() {
    check_loader_cases(
        "disabled_skills_skip_discovery",
        &["no_skills_skips_discovery"],
    );
}

#[test]
fn disabled_skills_keep_additional_paths() {
    check_loader_cases(
        "disabled_skills_keep_additional_paths",
        &["no_skills_keeps_additional_paths"],
    );
}

#[test]
fn missing_extra_paths_are_reported_per_kind() {
    check_loader_cases(
        "missing_extra_paths_are_reported_per_kind",
        &[
            "missing_extra_extension_path_is_reported",
            "missing_extra_skill_path_is_reported",
            "missing_extra_prompt_path_is_reported",
            "missing_extra_theme_path_is_reported",
        ],
    );
}

#[test]
fn theme_paths_admit_json_files_directories_and_links() {
    check_loader_cases(
        "theme_paths_admit_json_files_directories_and_links",
        &[
            "theme_path_must_be_json",
            "theme_directory_admits_json_files_and_links",
            "explicit_theme_name_collision",
        ],
    );
}

/// The invalid-theme warning carries the theme owner's own error for that file.
#[test]
fn invalid_theme_files_warn_with_the_theme_error() {
    with_isolated_home("invalid_theme_files_warn_with_the_theme_error", |base| {
        let mut case = loader_case("invalid_theme_file_warns");
        let root = Root::new(base, &case.name);
        root.write_tree(&case.tree);
        let file = root.fill("$ROOT/bad.json");
        let error = maestro_theme::load_theme_from_path(&file, None).unwrap_err();
        case.expected["themeDiagnostics"][0]["message"] = root.normalize(&json!(error.to_string()));
        let expected = case.expected.clone();
        let observed =
            on_task_set(|local| async move { run_loader_case(&root, &case, local).await });
        assert_eq!(observed, expected);
    });
}

#[test]
fn prompt_name_collisions_keep_the_first() {
    check_loader_cases(
        "prompt_name_collisions_keep_the_first",
        &["prompt_name_collision_keeps_first"],
    );
}

#[test]
fn system_prompt_sources_follow_precedence() {
    check_loader_cases(
        "system_prompt_sources_follow_precedence",
        &[
            "global_system_file_is_fallback",
            "project_system_files_win_over_global",
            "explicit_prompt_texts_and_files",
            "explicit_prompt_text_is_kept",
            "explicit_empty_prompt_skips_discovery",
        ],
    );
}

#[test]
fn disabled_prompts_and_themes_keep_extra_paths() {
    check_loader_cases(
        "disabled_prompts_and_themes_keep_extra_paths",
        &["no_prompts_and_no_themes_keep_extra_paths"],
    );
}

#[test]
fn aliased_extra_paths_are_deduplicated_canonically() {
    check_loader_cases(
        "aliased_extra_paths_are_deduplicated_canonically",
        &["aliased_extra_skill_path_is_deduplicated"],
    );
}

#[test]
fn package_resources_keep_package_provenance() {
    check_loader_cases(
        "package_resources_keep_package_provenance",
        &["package_resources_keep_package_metadata"],
    );
}

#[test]
fn ancestor_agent_skill_directories_are_discovered() {
    check_loader_cases(
        "ancestor_agent_skill_directories_are_discovered",
        &["ancestor_agents_skills_are_discovered"],
    );
}

/// Resources an extension contributes after a reload keep the reload's package provenance
/// for the package resources rebuilt beside them.
#[test]
fn extension_resources_keep_package_provenance_after_reload() {
    check_loader_cases(
        "extension_resources_keep_package_provenance_after_reload",
        &["extension_resources_after_reload_keep_package_metadata"],
    );
}

/// Relative extra paths load against the loader's directory and are not reported missing.
#[test]
fn extra_path_existence_uses_the_resolved_path() {
    check_loader_cases(
        "extra_path_existence_uses_the_resolved_path",
        &["relative_extra_paths_use_loader_cwd"],
    );
}

/// Padded, home-relative and bare-home extra paths load without a missing-path error.
#[test]
fn extra_paths_trim_and_expand_home() {
    check_loader_cases(
        "extra_paths_trim_and_expand_home",
        &["tilde_and_whitespace_extra_paths_resolve"],
    );
}

/// A package moved from user to project settings is attributed to the project on the next
/// reload, because each reload indexes provenance afresh.
#[test]
fn reload_rebuilds_package_provenance() {
    with_isolated_home("reload_rebuilds_package_provenance", |base| {
        let case = loader_case("package_resources_keep_package_metadata");
        let root = Root::new(base, "reload_rebuilds_package_provenance");
        root.write_tree(&case.tree);
        let scopes = on_task_set(|local| async move {
            let loader = loader(&root, &case, resource_options(&root, &case, local));
            loader.reload().await.unwrap();
            let packages = std::fs::read_to_string(root.fill("$ROOT/agent/settings.json")).unwrap();
            std::fs::write(root.fill("$ROOT/agent/settings.json"), "{}").unwrap();
            std::fs::create_dir_all(root.fill("$ROOT/project/$CONFIG")).unwrap();
            std::fs::write(root.fill("$ROOT/project/$CONFIG/settings.json"), packages).unwrap();
            let first = loader.get_skills();
            loader.reload().await.unwrap();
            let second = loader.get_skills();
            [first, second].map(|skills| skills.skills[0].source_info.scope)
        });
        assert_eq!(scopes, [SourceScope::User, SourceScope::Project]);
    });
}

/// A diagnostic with only a type and a message.
fn note(message: &str) -> ResourceDiagnostic {
    ResourceDiagnostic {
        r#type: DiagnosticType::Warning,
        message: message.into(),
        path: None,
        collision: None,
    }
}

/// Seven overrides each receive the freshly loaded base, in reference order, and their
/// replacements are stored.
#[test]
fn overrides_see_base_results_in_reference_order() {
    with_isolated_home("overrides_see_base_results_in_reference_order", |base| {
        let corpus: LoaderCorpus = serde_json::from_str(LOADER).unwrap();
        let row = &corpus.overrides[0];
        let mut case = loader_case("constructor_results_are_empty");
        case.tree.clear();
        let root = Root::new(base, "overrides");
        std::fs::create_dir_all(root.fill("$ROOT/agent/skills")).unwrap();
        std::fs::write(
            root.fill("$ROOT/agent/skills/s.md"),
            "---\nname: auto-skill\ndescription: auto\n---\nSkill content here.",
        )
        .unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let observed = on_task_set(|local| {
            let seen = Rc::clone(&seen);
            async move {
                let mut options = resource_options(&root, &case, local);
                install_overrides(&mut options, &seen);
                let loader = loader(&root, &case, options);
                loader.reload().await.unwrap();
                root.normalize(&observe(&loader))
            }
        });
        assert_eq!(Value::Array(seen.take()), row["seen"]);
        assert_eq!(observed, row["expected"]);
    });
}

/// Installs the seven recorded overrides, each noting what it received.
fn install_overrides(
    options: &mut maestro_app::ResourceLoaderOptions<maestro_models::Local>,
    seen: &Rc<RefCell<Vec<Value>>>,
) {
    let record = |seen: &Rc<RefCell<Vec<Value>>>| {
        let seen = Rc::clone(seen);
        move |value: Value| seen.borrow_mut().push(value)
    };
    let note_extensions = record(seen);
    options.extensions_override = Some(Box::new(move |mut base| {
        note_extensions(json!(base.extensions.len()));
        base.errors.push(maestro_app::ExtensionLoadError {
            path: "<override>".into(),
            error: "override note".into(),
        });
        base
    }));
    let note_skills = record(seen);
    options.skills_override = Some(Box::new(move |base| {
        note_skills(json!(
            base.skills.iter().map(|s| &s.name).collect::<Vec<_>>()
        ));
        LoadSkillsResult {
            skills: vec![injected_skill()],
            diagnostics: Vec::new(),
        }
    }));
    let note_prompts = record(seen);
    options.prompts_override = Some(Box::new(move |base| {
        note_prompts(json!(base.prompts.len()));
        LoadPromptsResult {
            prompts: Vec::new(),
            diagnostics: vec![note("prompts replaced")],
        }
    }));
    let note_themes = record(seen);
    options.themes_override = Some(Box::new(move |base| {
        note_themes(json!(base.themes.len()));
        LoadThemesResult::default()
    }));
    let note_files = record(seen);
    options.agents_files_override = Some(Box::new(move |base| {
        note_files(json!(base.len()));
        vec![maestro_app::ContextFile {
            path: "/virtual/AGENTS.md".into(),
            content: "virtual".into(),
        }]
    }));
    let note_prompt = record(seen);
    options.system_prompt_override = Some(Box::new(move |base| {
        note_prompt(json!(base));
        Some("Custom system prompt".into())
    }));
    let note_append = record(seen);
    options.append_system_prompt_override = Some(Box::new(move |mut base| {
        note_append(json!(base));
        base.push("added".into());
        base
    }));
}

/// An extension whose path the override moved to a missing file is based at its parent
/// directory instead of failing the reload.
#[test]
fn missing_override_extension_path_uses_its_parent_directory() {
    with_isolated_home(
        "missing_override_extension_path_uses_its_parent_directory",
        |base| {
            let corpus: LoaderCorpus = serde_json::from_str(LOADER).unwrap();
            let row = &corpus.overrides[1];
            let case = loader_case("constructor_results_are_empty");
            let root = Root::new(base, "missing_override");
            let missing = root.fill("$ROOT/missing/ext.ts");
            let observed = on_task_set(|local| async move {
                let mut options = resource_options(&root, &case, local);
                options.extension_factories =
                    vec![crate::resource_fixtures::factory(ExtensionSpec::default())];
                options.extensions_override = Some(Box::new(move |base| {
                    base.extensions[0].write().unwrap().path = missing.clone();
                    base
                }));
                let loader = loader(&root, &case, options);
                loader.reload().await.unwrap();
                let extensions = loader.get_extensions();
                let extension = extensions.extensions[0].read().unwrap();
                root.normalize(&json!({
                    "extensionPath": extension.path,
                    "sourceInfo": *extension.source_info.read().unwrap(),
                }))
            });
            assert_eq!(observed, row["expected"]);
        },
    );
}

/// Settings applied in memory survive the reload that refreshes them from storage.
#[test]
fn in_memory_settings_survive_resource_reload() {
    with_isolated_home("in_memory_settings_survive_resource_reload", |base| {
        let corpus: LoaderCorpus = serde_json::from_str(LOADER).unwrap();
        let expected = &corpus.settings["expected"];
        let mut case = loader_case("constructor_results_are_empty");
        case.tree.clear();
        let root = Root::new(base, "settings_reload");
        let settings: Settings = Settings(
            json!({"defaultThinkingLevel": "high", "images": {"autoResize": false}, "compaction": {"enabled": false}})
                .as_object()
                .unwrap()
                .clone(),
        );
        let manager = Rc::new(RefCell::new(SettingsManager::in_memory(settings)));
        let shared = Rc::clone(&manager);
        on_task_set(|local| async move {
            let mut options = resource_options(&root, &case, local);
            options.disabled.extend([
                maestro_app::ResourceKind::Extensions,
                maestro_app::ResourceKind::Skills,
                maestro_app::ResourceKind::PromptTemplates,
                maestro_app::ResourceKind::Themes,
                maestro_app::ResourceKind::ContextFiles,
            ]);
            let loader = maestro_app::DefaultResourceLoader::new(
                maestro_app::DefaultResourceLoaderOptions {
                    cwd: root.path.clone(),
                    agent_dir: root.agent(),
                    settings_manager: Some(shared),
                    resources: options,
                },
            );
            loader.reload().await.unwrap();
        });
        let manager = manager.borrow();
        let observed = json!({
            "defaultThinkingLevel": manager.get_default_thinking_level().map(Value::from),
            "imageAutoResize": manager.get_image_auto_resize(),
            "compactionEnabled": manager.get_compaction_enabled(),
        });
        assert_eq!(&observed, expected);
    });
}
