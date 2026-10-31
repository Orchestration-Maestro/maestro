//! The embedding interface at the application root: session creation from existing
//! services, the account file, and the compaction, history, resource, settings, tool and
//! theme owners programs build on.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, create_test_resource_loader, harness, on_task_set,
};
use maestro_app::{
    AgentSessionServices, CreateAgentSessionFromServicesOptions, ExtensionBindings, NoTools,
    create_agent_session_from_services,
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[test]
fn creates_a_session_over_existing_services() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            models: vec![("faux-1", true)],
            ..HarnessOptions::default()
        })
        .await;
        let settings =
            maestro_app::SettingsManager::in_memory(maestro_settings::Settings::default());
        let services = Rc::new(AgentSessionServices {
            cwd: h.cwd.clone(),
            agent_dir: h.scratch.path().display().to_string(),
            auth_storage: Arc::new(maestro_app::AuthStorage::in_memory(
                maestro_credentials::AuthStorageData::default(),
            )),
            settings_manager: Rc::new(RefCell::new(settings)),
            model_registry: Arc::clone(&h.registry),
            resource_loader: Rc::new(create_test_resource_loader(None)),
            diagnostics: Vec::new(),
            offline: None,
        });
        let manager = maestro_app::SessionManager::in_memory(Some("/elsewhere")).unwrap();
        let created = create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
            services,
            session_manager: Rc::new(RefCell::new(manager)),
            session_start_event: None,
            model: Some(h.model("faux-1")),
            thinking_level: None,
            scoped_models: Vec::new(),
            tools: None,
            no_tools: Some(NoTools::Builtin),
            custom_tools: Vec::new(),
            theme: maestro_theme::LiveTheme::default(),
        })
        .await
        .unwrap();
        let context = created.session.extension_runner().create_context();
        assert_eq!(
            context.cwd().unwrap(),
            h.cwd,
            "the services' directory wins"
        );
        assert!(
            created.session.get_active_tool_names().is_empty(),
            "no default tools"
        );
        created
            .session
            .bind_extensions(ExtensionBindings::default())
            .await
            .unwrap();
    });
}

#[test]
fn reads_and_writes_the_account_file_at_the_root() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = format!("{}/auth.json", scratch.path().display());
    let mut auth = maestro_app::model_account::cli::load_auth(&path);
    auth.insert("alpha".into(), json!({"type": "oauth"}));
    maestro_app::model_account::cli::save_auth(&path, &auth).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{\n  \"alpha\": {\n    \"type\": \"oauth\"\n  }\n}"
    );
}

#[test]
fn reaches_the_lower_owners_through_the_root() {
    assert_eq!(maestro_app::format_size(2048.0), "2.0KB");
    assert_eq!(
        maestro_app::DEFAULT_MAX_LINES,
        maestro_tools::DEFAULT_MAX_LINES
    );
    assert_eq!(
        maestro_app::DEFAULT_MAX_BYTES,
        maestro_tools::DEFAULT_MAX_BYTES
    );
    assert_eq!(maestro_app::get_language_from_path("main.rs"), Some("rust"));
    let parsed = maestro_app::strip_frontmatter("---\nname: x\n---\nbody").unwrap();
    assert_eq!(parsed, "body");
    let source = maestro_app::create_synthetic_source_info(
        "<sdk:tool>".into(),
        maestro_resources::SyntheticSourceOptions {
            source: "sdk".into(),
            scope: None,
            origin: None,
            base_dir: None,
        },
    );
    assert_eq!(source.path, "<sdk:tool>");
}

/// Asserts each name resolves to its declaring owner rather than a copy the root
/// declares.
fn assert_declared(names: &[&str]) {
    for name in names {
        assert!(
            !name.contains("maestro_app::exports"),
            "{name} is its owner's item"
        );
    }
}

#[test]
fn names_the_session_and_history_records_at_the_root() {
    assert_declared(&[
        std::any::type_name::<maestro_app::AgentSessionRuntime>(),
        std::any::type_name::<maestro_app::BashExecutorOptions>(),
        std::any::type_name::<maestro_app::BashResult>(),
        std::any::type_name::<maestro_app::BranchPreparation>(),
        std::any::type_name::<maestro_app::BranchSummaryResult>(),
        std::any::type_name::<maestro_app::CollectEntriesResult>(),
        std::any::type_name::<maestro_app::CompactionResult>(),
        std::any::type_name::<maestro_app::CompactionSettings>(),
        std::any::type_name::<maestro_app::CreateAgentSessionFromServicesOptions>(),
        std::any::type_name::<maestro_app::CreateAgentSessionOptions>(),
        std::any::type_name::<maestro_app::CreateAgentSessionResult>(),
        std::any::type_name::<maestro_app::CreateAgentSessionRuntimeFactory>(),
        std::any::type_name::<maestro_app::CreateAgentSessionRuntimeResult>(),
        std::any::type_name::<maestro_app::CutPointResult>(),
        std::any::type_name::<dyn maestro_app::EditOperations>(),
        std::any::type_name::<maestro_app::EditToolDetails>(),
        std::any::type_name::<maestro_app::EditToolInput>(),
        std::any::type_name::<maestro_app::EventBus>(),
        std::any::type_name::<maestro_app::EventBusController>(),
        std::any::type_name::<maestro_app::ExtensionFlag>(),
        std::any::type_name::<maestro_app::ExtensionShortcut>(),
        std::any::type_name::<maestro_app::FileAuthStorageBackend>(),
        std::any::type_name::<maestro_app::FileEntry>(),
        std::any::type_name::<maestro_app::FileOperations>(),
        std::any::type_name::<dyn maestro_app::FindOperations>(),
        std::any::type_name::<maestro_app::FindToolDetails>(),
        std::any::type_name::<maestro_app::FindToolInput>(),
        std::any::type_name::<maestro_app::FindToolOptions>(),
        std::any::type_name::<maestro_app::GenerateBranchSummaryOptions>(),
    ]);
}

#[test]
fn names_the_tool_resource_and_theme_records_at_the_root() {
    assert_declared(&[
        std::any::type_name::<dyn maestro_app::GrepOperations>(),
        std::any::type_name::<maestro_app::GrepToolDetails>(),
        std::any::type_name::<maestro_app::GrepToolInput>(),
        std::any::type_name::<maestro_app::GrepToolOptions>(),
        std::any::type_name::<maestro_app::ImageSettings>(),
        std::any::type_name::<maestro_app::LoadSkillsFromDirOptions>(),
        std::any::type_name::<dyn maestro_app::LsOperations>(),
        std::any::type_name::<maestro_app::LsToolDetails>(),
        std::any::type_name::<maestro_app::LsToolInput>(),
        std::any::type_name::<maestro_app::LsToolOptions>(),
        std::any::type_name::<maestro_app::NewSessionOptions>(),
        std::any::type_name::<maestro_app::PromptOptions>(),
        std::any::type_name::<maestro_app::PromptTemplate>(),
        std::any::type_name::<dyn maestro_app::ReadOperations>(),
        std::any::type_name::<maestro_app::ReadToolDetails>(),
        std::any::type_name::<maestro_app::ReadToolInput>(),
        std::any::type_name::<maestro_app::RegisteredCommand>(),
        std::any::type_name::<maestro_app::ResolvedCommand>(),
        std::any::type_name::<maestro_app::RetrySettings>(),
        std::any::type_name::<maestro_app::SessionContext>(),
        std::any::type_name::<maestro_app::SessionHeader>(),
        std::any::type_name::<maestro_app::SessionManager>(),
        std::any::type_name::<maestro_app::Skill>(),
        std::any::type_name::<maestro_app::SkillFrontmatter>(),
        std::any::type_name::<maestro_app::SourceInfo>(),
        std::any::type_name::<maestro_app::Theme>(),
        std::any::type_name::<maestro_app::ThemeColor>(),
        std::any::type_name::<maestro_app::ToolsOptions>(),
        std::any::type_name::<dyn maestro_app::WriteOperations>(),
        std::any::type_name::<maestro_app::WriteToolInput>(),
    ]);
}

#[test]
fn names_the_session_and_history_operations_at_the_root() {
    assert_declared(&[
        std::any::type_name_of_val(&maestro_app::build_session_context::<std::hash::RandomState>),
        std::any::type_name_of_val(&maestro_app::calculate_context_tokens),
        std::any::type_name_of_val(&maestro_app::collect_entries_for_branch_summary),
        std::any::type_name_of_val(&maestro_app::compact),
        std::any::type_name_of_val(
            &maestro_app::convert_to_llm::<serde_json::Value, maestro_agent::NoCustomMessages>,
        ),
        std::any::type_name_of_val(&maestro_app::create_agent_session),
        std::any::type_name_of_val(&maestro_app::create_agent_session_from_services),
        std::any::type_name_of_val(&maestro_app::create_agent_session_runtime),
        std::any::type_name_of_val(&maestro_app::create_coding_tools),
        std::any::type_name_of_val(&maestro_app::create_edit_tool),
        std::any::type_name_of_val(&maestro_app::create_event_bus),
        std::any::type_name_of_val(&maestro_app::create_find_tool),
        std::any::type_name_of_val(&maestro_app::create_find_tool_definition::<()>),
        std::any::type_name_of_val(&maestro_app::create_grep_tool),
        std::any::type_name_of_val(&maestro_app::create_grep_tool_definition::<()>),
        std::any::type_name_of_val(&maestro_app::create_ls_tool),
        std::any::type_name_of_val(&maestro_app::create_ls_tool_definition::<()>),
        std::any::type_name_of_val(&maestro_app::create_read_only_tools),
        std::any::type_name_of_val(&maestro_app::create_read_tool),
        std::any::type_name_of_val(&maestro_app::create_synthetic_source_info),
        std::any::type_name_of_val(&maestro_app::create_write_tool),
        std::any::type_name_of_val(
            &maestro_app::discover_and_load_extensions::<maestro_models::Local>,
        ),
        std::any::type_name_of_val(&maestro_app::estimate_tokens),
        std::any::type_name_of_val(&maestro_app::execute_bash_with_operations),
    ]);
}

#[test]
fn names_the_tool_resource_and_theme_operations_at_the_root() {
    assert_declared(&[
        std::any::type_name_of_val(&maestro_app::find_cut_point),
        std::any::type_name_of_val(&maestro_app::find_turn_start_index),
        std::any::type_name_of_val(&maestro_app::format_size),
        std::any::type_name_of_val(&maestro_app::format_skills_for_prompt),
        std::any::type_name_of_val(&maestro_app::generate_branch_summary),
        std::any::type_name_of_val(&maestro_app::generate_summary),
        std::any::type_name_of_val(&maestro_app::get_language_from_path),
        std::any::type_name_of_val(&maestro_app::get_last_assistant_usage::<serde_json::Value>),
        std::any::type_name_of_val(&maestro_app::get_latest_compaction_entry::<serde_json::Value>),
        std::any::type_name_of_val(&maestro_app::get_markdown_theme),
        std::any::type_name_of_val(&maestro_app::get_select_list_theme),
        std::any::type_name_of_val(&maestro_app::get_settings_list_theme),
        std::any::type_name_of_val(&maestro_app::highlight_code),
        std::any::type_name_of_val(&maestro_app::load_skills),
        std::any::type_name_of_val(&maestro_app::load_skills_from_dir),
        std::any::type_name_of_val(&maestro_app::migrate_session_entries),
        std::any::type_name_of_val(&maestro_app::parse_frontmatter),
        std::any::type_name_of_val(&maestro_app::parse_session_entries),
        std::any::type_name_of_val(&maestro_app::prepare_branch_entries),
        std::any::type_name_of_val(&maestro_app::serialize_conversation),
        std::any::type_name_of_val(&maestro_app::should_compact),
        std::any::type_name_of_val(&maestro_app::strip_frontmatter),
        std::any::type_name_of_val(&maestro_app::truncate_head),
        std::any::type_name_of_val(&maestro_app::truncate_line),
    ]);
}

#[test]
fn reaches_the_root_constants_and_file_queue() {
    let queued = maestro_app::with_file_mutation_queue("queued.txt", || async { Ok(1) });
    assert_eq!(crate::support::run_io(queued), Ok(1));
    assert_eq!(
        maestro_app::CURRENT_SESSION_VERSION,
        maestro_session::session_manager::CURRENT_SESSION_VERSION
    );
    assert!(!maestro_app::should_compact(
        1.0,
        200_000.0,
        &maestro_app::DEFAULT_COMPACTION_SETTINGS
    ));
}
