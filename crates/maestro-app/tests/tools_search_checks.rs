//! Native search and directory listing through the application's executable factories.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::{
    run_io,
    session_harness::{case, recorded_number, run_native_tool_child},
};
use serde_json::json;

#[test]
fn ls_keeps_dotfiles_and_directory_suffix() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    std::fs::write(root.path().join(".hidden-file"), "secret").unwrap();
    std::fs::create_dir(root.path().join(".hidden-dir")).unwrap();
    let tool = maestro_app::create_ls_tool(
        root.path().display().to_string(),
        maestro_app::LsToolOptions::default(),
    );
    let result = run_io((tool.execute)("ls".into(), json!({}), None, None)).unwrap();
    assert_eq!(
        json!(result),
        case("tools_root", "ls_keeps_dotfiles_and_directory_suffix")["expected"]["ok"]
    );
}

/// Finds the installed native program and refuses release lookup or acquisition.
async fn search_manager(
    root: &maestro_test_directory::Scratch,
    program: maestro_app::SearchTool,
) -> std::sync::Arc<maestro_tools::ToolsManager> {
    let manager = std::sync::Arc::new(maestro_tools::ToolsManager::new(
        root.path().display().to_string(),
        Some(std::sync::Arc::new(|_| {
            Box::pin(async { panic!("native qualification must not download search programs") })
        })),
    ));
    assert!(
        manager.get_tool_path(program).await.is_some(),
        "{program:?} must be installed in the build image"
    );
    manager
}

#[test]
fn grep_single_file_keeps_filename_and_line() {
    if run_native_tool_child("grep_single_file_keeps_filename_and_line") {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    std::fs::write(
        root.path().join("example.txt"),
        "first line\nmatch line\nlast line",
    )
    .unwrap();
    run_io(async {
        let manager = search_manager(&root, maestro_app::SearchTool::Rg).await;
        let tool = maestro_app::create_grep_tool(
            root.path().display().to_string(),
            maestro_app::GrepToolOptions::default(),
            manager,
        );
        let result = (tool.execute)(
            "grep".into(),
            json!({"pattern":"match","path":"example.txt"}),
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            json!(result),
            json!({"content":[{"type":"text","text":"example.txt:2: match line"}]})
        );
    });
}

#[test]
fn grep_limit_retains_context_and_notice() {
    if run_native_tool_child("grep_limit_retains_context_and_notice") {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    std::fs::write(
        root.path().join("context.txt"),
        "before\nmatch one\nafter\nmiddle\nmatch two\nafter two",
    )
    .unwrap();
    run_io(async {
        let manager = search_manager(&root, maestro_app::SearchTool::Rg).await;
        let tool = maestro_app::create_grep_tool(
            root.path().display().to_string(),
            maestro_app::GrepToolOptions::default(),
            manager,
        );
        let result = (tool.execute)(
            "grep".into(),
            json!({"pattern":"match","path":"context.txt","limit":1,"context":1}),
            None,
            None,
        )
        .await
        .unwrap();
        let mut observed = json!(result);
        let limit = observed["details"]["matchLimitReached"].as_f64().unwrap();
        observed["details"]["matchLimitReached"] = recorded_number(limit);
        assert_eq!(
            observed,
            json!({"content":[{"type":"text","text":"context.txt-1- before\ncontext.txt:2: match one\ncontext.txt-3- after\n\n[1 matches limit reached. Use limit=2 for more, or refine pattern]"}],"details":{"matchLimitReached":1}})
        );
    });
}

#[test]
fn find_keeps_unignored_hidden_paths() {
    if run_native_tool_child("find_keeps_unignored_hidden_paths") {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    std::fs::create_dir(root.path().join(".secret")).unwrap();
    std::fs::write(root.path().join(".secret/hidden.txt"), "hidden").unwrap();
    std::fs::write(root.path().join("visible.txt"), "visible").unwrap();
    run_io(async {
        let manager = search_manager(&root, maestro_app::SearchTool::Fd).await;
        let tool = maestro_app::create_find_tool(
            root.path().display().to_string(),
            maestro_app::FindToolOptions::default(),
            manager,
        );
        let result = (tool.execute)("find".into(), json!({"pattern":"**/*.txt"}), None, None)
            .await
            .unwrap();
        let [maestro_models::UserBlock::Text(output)] = result.content.as_slice() else {
            panic!("find must return one text block");
        };
        assert_eq!(
            output
                .text
                .lines()
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from([".secret/hidden.txt", "visible.txt"])
        );
        assert!(result.details.is_none());
    });
}

#[test]
fn find_applies_ignore_rules_without_repository_metadata() {
    if run_native_tool_child("find_applies_ignore_rules_without_repository_metadata") {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    std::fs::write(root.path().join(".gitignore"), "ignored.txt\n").unwrap();
    std::fs::write(root.path().join("ignored.txt"), "ignored").unwrap();
    std::fs::write(root.path().join("kept.txt"), "kept").unwrap();
    run_io(async {
        let cwd = root.path().display().to_string();
        let manager = search_manager(&root, maestro_app::SearchTool::Fd).await;
        let tool = maestro_app::create_find_tool(
            cwd.clone(),
            maestro_app::FindToolOptions::default(),
            manager,
        );
        let expected = json!({"content":[{"type":"text","text":"kept.txt"}]});
        let outside = (tool.execute)("find".into(), json!({"pattern":"**/*.txt"}), None, None)
            .await
            .unwrap();
        assert_eq!(json!(outside), expected);
        let initialized = maestro_app::exec_command(
            "git",
            &["init".into(), "--quiet".into(), "--template=".into()],
            &cwd,
            maestro_app::ExecOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(initialized.code, 0, "{}", initialized.stderr);
        std::fs::write(root.path().join(".git/meta.txt"), "metadata").unwrap();
        let inside = (tool.execute)("find".into(), json!({"pattern":"**/*.txt"}), None, None)
            .await
            .unwrap();
        assert_eq!(json!(inside), expected);
    });
}

#[test]
fn find_preserves_invalid_glob_failure() {
    if run_native_tool_child("find_preserves_invalid_glob_failure") {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    run_io(async {
        let manager = search_manager(&root, maestro_app::SearchTool::Fd).await;
        let tool = maestro_app::create_find_tool(
            root.path().display().to_string(),
            maestro_app::FindToolOptions::default(),
            manager,
        );
        let error = (tool.execute)("find".into(), json!({"pattern":"["}), None, None)
            .await
            .unwrap_err();
        assert!(error.message.to_lowercase().contains("glob"), "{error:?}");
    });
}

#[test]
fn find_accepts_flag_like_literal_pattern() {
    if run_native_tool_child("find_accepts_flag_like_literal_pattern") {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    run_io(async {
        let manager = search_manager(&root, maestro_app::SearchTool::Fd).await;
        let tool = maestro_app::create_find_tool(
            root.path().display().to_string(),
            maestro_app::FindToolOptions::default(),
            manager,
        );
        let result = (tool.execute)("find".into(), json!({"pattern":"--help"}), None, None)
            .await
            .unwrap();
        assert_eq!(
            json!(result),
            json!({"content":[{"type":"text","text":"No files found matching pattern"}]})
        );
    });
}

#[test]
fn find_zero_limit_has_no_false_count_notice() {
    if run_native_tool_child("find_zero_limit_has_no_false_count_notice") {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let expected = (0..1005)
        .map(|index| format!("file-{index:04}.txt"))
        .collect::<std::collections::BTreeSet<_>>();
    for name in &expected {
        std::fs::write(root.path().join(name), "entry").unwrap();
    }
    run_io(async {
        let manager = search_manager(&root, maestro_app::SearchTool::Fd).await;
        let tool = maestro_app::create_find_tool(
            root.path().display().to_string(),
            maestro_app::FindToolOptions::default(),
            manager,
        );
        let result = (tool.execute)(
            "find".into(),
            json!({"pattern":"**/*.txt","limit":0}),
            None,
            None,
        )
        .await
        .unwrap();
        let [maestro_models::UserBlock::Text(output)] = result.content.as_slice() else {
            panic!("find must return one text block");
        };
        assert_eq!(
            output
                .text
                .lines()
                .map(str::to_owned)
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        assert!(result.details.is_none());
    });
}
