//! Bundled tool declarations and the schema nodes their authors declared.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use maestro_models::test_support::assert_declaration;
use maestro_tools::{
    BashToolOptions, EditToolOptions, FindToolOptions, GrepToolOptions, LsToolOptions,
    ReadToolOptions, ToolsManager, WriteToolOptions,
};

/// A manager that searches no directory.
fn manager() -> Arc<ToolsManager> {
    Arc::new(ToolsManager::new("/unused/search".into(), None))
}

#[test]
fn bundled_tool_schemas_keep_authored_nodes() {
    let cwd = "/work".to_owned();

    let read =
        maestro_tools::create_read_tool_definition::<()>(cwd.clone(), ReadToolOptions::default());
    assert_declaration("read", &read.definition.parameters);
    let write =
        maestro_tools::create_write_tool_definition::<()>(cwd.clone(), WriteToolOptions::default());
    assert_declaration("write", &write.definition.parameters);
    let edit =
        maestro_tools::create_edit_tool_definition::<()>(cwd.clone(), EditToolOptions::default());
    assert_declaration("edit", &edit.definition.parameters);
    let bash = maestro_tools::create_bash_tool_definition::<()>(
        cwd.clone(),
        String::new(),
        BashToolOptions::default(),
    );
    assert_declaration("bash", &bash.definition.parameters);
    let grep = maestro_tools::create_grep_tool_definition::<()>(
        cwd.clone(),
        GrepToolOptions::default(),
        manager(),
    );
    assert_declaration("grep", &grep.definition.parameters);
    let find = maestro_tools::create_find_tool_definition::<()>(
        cwd.clone(),
        FindToolOptions::default(),
        manager(),
    );
    assert_declaration("find", &find.definition.parameters);
    let ls = maestro_tools::create_ls_tool_definition::<()>(cwd.clone(), LsToolOptions::default());
    assert_declaration("ls", &ls.definition.parameters);

    let local_read = maestro_tools::reading::local::create_read_tool_definition::<()>(
        cwd.clone(),
        ReadToolOptions::default(),
    );
    assert_declaration("read", &local_read.definition.parameters);
    let local_edit = maestro_tools::editing::local::create_edit_tool_definition::<()>(
        cwd.clone(),
        EditToolOptions::default(),
    );
    assert_declaration("edit", &local_edit.definition.parameters);
    let local_write = maestro_tools::writing::local::create_write_tool_definition::<()>(
        cwd.clone(),
        WriteToolOptions::default(),
    );
    assert_declaration("write", &local_write.definition.parameters);
    let local_bash = maestro_tools::process::command::local::create_bash_tool_definition::<()>(
        cwd,
        String::new(),
        maestro_tools::process::command::local::BashToolOptions::default(),
    );
    assert_declaration("bash", &local_bash.definition.parameters);
}
