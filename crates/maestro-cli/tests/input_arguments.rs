//! Authored argument consumption observed through the public parser.

use crate::support;
use maestro_app::ThinkingLevel;
use maestro_cli::input::args::{
    Diagnostic, DiagnosticType, ListModels, Mode, is_valid_thinking_level, parse_args,
};

#[test]
fn argv_sets_long_version() {
    support::assert_argv_title("parses --version flag");
}

#[test]
fn argv_sets_short_version() {
    support::assert_argv_title("parses -v shorthand");
}

#[test]
fn argv_keeps_messages_beside_version_and_help() {
    support::assert_argv_title("--version takes precedence over other args");
}

#[test]
fn argv_sets_long_help() {
    support::assert_argv_title("parses --help flag");
}

#[test]
fn argv_sets_short_help() {
    support::assert_argv_title("parses -h shorthand");
}

#[test]
fn argv_sets_long_print() {
    support::assert_argv_title("parses --print flag");
}

#[test]
fn argv_sets_short_print() {
    support::assert_argv_title("parses -p shorthand");
}

#[test]
fn argv_accepts_a_frontmatter_print_prompt() {
    support::assert_argv_title("parses prompt after -p even when it starts with YAML frontmatter");
}

#[test]
fn argv_keeps_provider_options_after_print() {
    support::assert_argv_title("does not consume options after -p as prompts");
}

#[test]
fn argv_sets_long_continue() {
    support::assert_argv_title("parses --continue flag");
}

#[test]
fn argv_sets_short_continue() {
    support::assert_argv_title("parses -c shorthand");
}

#[test]
fn argv_sets_long_resume() {
    support::assert_argv_title("parses --resume flag");
}

#[test]
fn argv_sets_short_resume() {
    support::assert_argv_title("parses -r shorthand");
}

#[test]
fn argv_keeps_provider_text() {
    support::assert_argv_title("parses --provider");
}

#[test]
fn argv_keeps_model_text() {
    support::assert_argv_title("parses --model");
}

#[test]
fn argv_keeps_api_key_text() {
    support::assert_argv_title("parses --api-key");
}

#[test]
fn argv_keeps_system_prompt_text() {
    support::assert_argv_title("parses --system-prompt");
}

#[test]
fn argv_collects_one_appended_system_prompt() {
    support::assert_argv_title("parses --append-system-prompt");
}

#[test]
fn argv_collects_appended_system_prompts_in_order() {
    support::assert_argv_title("parses multiple --append-system-prompt flags");
}

#[test]
fn argv_selects_json_mode() {
    support::assert_argv_title("parses --mode");
}

#[test]
fn argv_selects_rpc_mode() {
    support::assert_argv_title("parses --mode rpc");
}

#[test]
fn argv_keeps_session_path() {
    support::assert_argv_title("parses --session");
}

#[test]
fn argv_keeps_fork_identity() {
    support::assert_argv_title("parses --fork");
}

#[test]
fn argv_keeps_export_path() {
    support::assert_argv_title("parses --export");
}

#[test]
fn argv_selects_a_thinking_level() {
    support::assert_argv_title("parses --thinking");
}

#[test]
fn argv_splits_model_patterns() {
    support::assert_argv_title("parses --models as comma-separated list");
}

#[test]
fn argv_sets_no_session() {
    support::assert_argv_title("parses --no-session flag");
}

#[test]
fn argv_collects_one_extension_path() {
    support::assert_argv_title("parses single --extension");
}

#[test]
fn argv_collects_a_short_extension_path() {
    support::assert_argv_title("parses -e shorthand");
}

#[test]
fn argv_collects_repeated_extension_paths() {
    support::assert_argv_title("parses multiple --extension flags");
}

#[test]
fn argv_disables_extension_discovery() {
    support::assert_argv_title("parses --no-extensions flag");
}

#[test]
fn argv_keeps_explicit_extensions_beside_discovery_disable() {
    support::assert_argv_title("parses --no-extensions with explicit -e flags");
}

#[test]
fn argv_collects_one_skill_path() {
    support::assert_argv_title("parses single --skill");
}

#[test]
fn argv_collects_repeated_skill_paths() {
    support::assert_argv_title("parses multiple --skill flags");
}

#[test]
fn argv_collects_one_prompt_template_path() {
    support::assert_argv_title("parses single --prompt-template");
}

#[test]
fn argv_collects_repeated_prompt_template_paths() {
    support::assert_argv_title("parses multiple --prompt-template flags");
}

#[test]
fn argv_collects_one_theme_path() {
    support::assert_argv_title("parses single --theme");
}

#[test]
fn argv_collects_repeated_theme_paths() {
    support::assert_argv_title("parses multiple --theme flags");
}

#[test]
fn argv_disables_skills() {
    support::assert_argv_title("parses --no-skills flag");
}

#[test]
fn argv_disables_prompt_templates() {
    support::assert_argv_title("parses --no-prompt-templates flag");
}

#[test]
fn argv_disables_themes() {
    support::assert_argv_title("parses --no-themes flag");
}

#[test]
fn argv_disables_context_files() {
    support::assert_argv_title("parses --no-context-files flag");
}

#[test]
fn argv_disables_context_files_with_short_alias() {
    support::assert_argv_title("parses -nc shorthand");
}

#[test]
fn argv_sets_verbose() {
    support::assert_argv_title("parses --verbose flag");
}

#[test]
fn argv_sets_offline() {
    support::assert_argv_title("parses --offline flag");
}

#[test]
fn argv_disables_all_tools() {
    support::assert_argv_title("parses --no-tools flag");
}

#[test]
fn argv_disables_all_tools_with_short_alias() {
    support::assert_argv_title("parses -nt shorthand");
}

#[test]
fn argv_disables_only_builtin_tools() {
    support::assert_argv_title("parses --no-builtin-tools flag");
}

#[test]
fn argv_disables_only_builtin_tools_with_short_alias() {
    support::assert_argv_title("parses -nbt shorthand");
}

#[test]
fn argv_splits_the_tool_allowlist() {
    support::assert_argv_title("parses --tools flag");
}

#[test]
fn argv_accepts_a_short_tool_allowlist() {
    support::assert_argv_title("parses -t shorthand");
}

#[test]
fn argv_keeps_allowlist_beside_all_tool_disable() {
    support::assert_argv_title("parses --no-tools with explicit --tools flags");
}

#[test]
fn argv_keeps_allowlist_beside_builtin_tool_disable() {
    support::assert_argv_title("parses --no-builtin-tools with explicit --tools flags");
}

#[test]
fn argv_keeps_plain_messages_in_order() {
    support::assert_argv_title("parses plain text messages");
}

#[test]
fn argv_removes_one_file_argument_prefix() {
    support::assert_argv_title("parses @file arguments");
}

#[test]
fn argv_keeps_files_and_messages_in_their_own_order() {
    support::assert_argv_title("parses mixed messages and file args");
}

#[test]
fn argv_captures_an_unknown_string_flag() {
    support::assert_argv_title("captures unknown long flags with string values");
}

#[test]
fn argv_captures_an_unknown_boolean_flag() {
    support::assert_argv_title("captures unknown boolean long flags");
}

#[test]
fn argv_captures_an_unknown_equals_flag() {
    support::assert_argv_title("captures unknown long flags with equals syntax");
}

#[test]
fn argv_keeps_combined_flags_files_and_prompt() {
    support::assert_argv_title("parses multiple flags together");
}

/// Compare the case named `title` of the recorded argument corpus.
fn check(title: &str) {
    support::assert_argv_title(title);
}

#[test]
fn missing_values_follow_the_long_and_short_option_routes() {
    for case in support::operation("args", "args_missing_each") {
        support::assert_argv(&case);
    }
    for title in [
        "missing_long_values",
        "short_option_looking_value",
        "missing_short_tools",
        "missing_short_extension",
        "empty_mode_is_consumed",
        "mode_invalid_without_previous",
        "invalid_mode_preserves",
    ] {
        check(title);
    }
}

#[test]
fn argv_preserves_option_looking_scalar_values() {
    for title in [
        "option_looking_values",
        "empty_known_values",
        "repeated_scalars",
        "resource_aliases",
    ] {
        check(title);
    }
}

#[test]
fn list_segments_use_authored_whitespace() {
    for title in ["models_tools_whitespace", "empty_lists_remain_present"] {
        check(title);
    }
}

#[test]
fn unknown_flag_replacements_keep_insertion_order() {
    for title in [
        "unknown_overwrite_order",
        "unknown_equals_keeps_the_entire_value",
        "unknown_replacement_changes_value_kind",
    ] {
        check(title);
    }
    let args = parse_args(&[
        "--z".to_owned(),
        "old".to_owned(),
        "--a=".to_owned(),
        "--z".to_owned(),
        "new".to_owned(),
        "--m".to_owned(),
        "@f".to_owned(),
    ]);
    let names: Vec<&str> = args.unknown_flags.keys().map(String::as_str).collect();
    assert_eq!(names, ["z", "a", "m"]);
    assert_eq!(
        args.unknown_flags.get("m"),
        Some(&maestro_app::FlagValue::Boolean(true))
    );
}

#[test]
fn print_consumption_distinguishes_frontmatter_files_and_options() {
    check("print_followers");
    let args = parse_args(&["-p".to_owned(), "---\ntitle: t\n---\nprompt".to_owned()]);
    assert_eq!(args.print, Some(true));
    assert_eq!(
        args.messages.back().map(String::as_str),
        Some("---\ntitle: t\n---\nprompt")
    );
}

#[test]
fn list_models_consumption_stops_at_options_and_files() {
    for title in [
        "list_models_followers",
        "list_models_without_search",
        "list_models_before_file",
        "list_models_before_option",
        "empty_model_search_is_retained",
    ] {
        check(title);
    }
    let args = parse_args(&["--list-models".to_owned(), String::new()]);
    assert_eq!(args.list_models, Some(ListModels::Search(String::new())));
}

#[test]
fn dash_and_equals_forms_do_not_create_a_new_grammar() {
    for title in ["dash_messages", "known_equals_unknown"] {
        check(title);
    }
    let args = parse_args(&["--mode".to_owned(), "json".to_owned()]).mode;
    assert_eq!(args, Some(Mode::Json));
    let args = parse_args(&["--mode=json".to_owned()]);
    assert_eq!(args.mode, None);
    assert_eq!(
        args.unknown_flags.get("mode"),
        Some(&maestro_app::FlagValue::String("json".to_owned()))
    );
}

#[test]
fn argv_traversal_reads_each_token_a_bounded_number_of_times() {
    for count in [32_usize, 64, 128] {
        let argv: Vec<String> = (0..count).map(|index| format!("message-{index}")).collect();
        let parsed = parse_args(&argv);
        let retained: Vec<&str> = parsed.messages.iter().map(String::as_str).collect();
        let authored: Vec<&str> = argv.iter().map(String::as_str).collect();
        assert_eq!(
            retained, authored,
            "every message of {count} survives once, in its authored order"
        );
        let diagnostics: Vec<_> = parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.r#type == DiagnosticType::Error)
            .collect();
        assert!(diagnostics.is_empty());
    }
}

#[test]
fn a_value_is_taken_from_the_next_token_only() {
    let args = parse_args(&[
        "--list-models".to_owned(),
        "--print".to_owned(),
        "term".to_owned(),
    ]);
    assert_eq!(
        args.list_models,
        Some(ListModels::All),
        "a following option is not skipped for a later value"
    );
    assert_eq!(args.print, Some(true));
    assert_eq!(args.messages.back().map(String::as_str), Some("term"));

    let args = parse_args(&[
        "--unknown".to_owned(),
        "--print".to_owned(),
        "value".to_owned(),
    ]);
    assert_eq!(
        args.unknown_flags.get("unknown"),
        Some(&maestro_app::FlagValue::Boolean(true)),
        "an unknown flag keeps its boolean form instead of a later value"
    );
    assert_eq!(args.messages.back().map(String::as_str), Some("value"));
}

#[test]
fn thinking_validation_keeps_a_previous_valid_choice() {
    for case in support::operation("thinking", "thinking") {
        let level = case.input.as_str().unwrap();
        assert_eq!(
            is_valid_thinking_level(level),
            case.expected.as_bool().unwrap(),
            "{}",
            case.case
        );
    }
    let retained = parse_args(&["--thinking".to_owned(), "high".to_owned()]);
    assert_eq!(retained.thinking, Some(ThinkingLevel::High));
    let invalid = parse_args(&["--thinking".to_owned(), "max".to_owned()]);
    assert_eq!(invalid.thinking, None);
    assert_eq!(
        invalid.diagnostics,
        vec![Diagnostic {
            r#type: DiagnosticType::Warning,
            message: "Invalid thinking level \"max\". Valid values: off, minimal, low, medium, high, xhigh"
                .to_owned(),
        }]
    );
    let kept = parse_args(&[
        "--thinking".to_owned(),
        "xhigh".to_owned(),
        "--thinking".to_owned(),
        "HIGH".to_owned(),
    ]);
    assert_eq!(kept.thinking, Some(ThinkingLevel::Xhigh));
    assert_eq!(kept.diagnostics.len(), 1);
}
