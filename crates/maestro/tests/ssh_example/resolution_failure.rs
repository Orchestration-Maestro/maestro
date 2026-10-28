//! Failed remote directory discovery cannot select local execution.
#![cfg(unix)]
use maestro_app::{ExtensionRunner, Local};
use serde_json::json;
use std::{cell::RefCell, path::Path, rc::Rc};

/// The exact failure supplied by the controlled SSH child.
const FAILURE: &str = "SSH failed (37): controlled connection failure";

/// Every coding-tool override returns the retained failure before file or shell work.
async fn refuses_tools(runner: &Rc<ExtensionRunner<Local>>) {
    let tools = runner.get_all_registered_tools();
    for (name, args) in [
        ("read", json!({"path":"local.txt"})),
        ("write", json!({"path":"local.txt","content":"changed"})),
        (
            "edit",
            json!({"path":"local.txt","edits":[{"oldText":"original","newText":"changed"}]}),
        ),
        ("bash", json!({"command":"printf changed > local.txt"})),
    ] {
        let registered = tools
            .iter()
            .find(|tool| tool.metadata().definition.name == name)
            .unwrap();
        let tool = maestro_app::wrap_registered_tool(registered, runner.clone());
        let error = (tool.execute)("unresolved".into(), args, None, None)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), FAILURE, "{name}");
    }
}

/// The user-shell hook returns rejecting operations rather than permitting a local fallback.
async fn refuses_shell(runner: &ExtensionRunner<Local>, cwd: &Path) {
    use maestro_app::{BashExecOptions, BashOperations, BashResult, UserBashEvent};
    let command = "printf changed > local.txt";
    let mut event = UserBashEvent {
        command: command.into(),
        cwd: cwd.to_str().unwrap().into(),
        exclude_from_context: false,
    };
    let result = runner
        .emit_user_bash::<Rc<dyn BashOperations<Local>>, BashResult>(&mut event)
        .await
        .unwrap()
        .expect("SSH failure must override local execution");
    let operations = result.operations.unwrap();
    let mut sink = |_: &[u8]| {};
    let error = operations
        .exec(
            command,
            cwd.to_str().unwrap(),
            BashExecOptions {
                on_data: &mut sink,
                signal: None,
                timeout: None,
                env: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), FAILURE);
}

#[test]
fn ssh_resolution_failure_refuses_local_execution() {
    if super::isolated("ssh_resolution_failure_refuses_local_execution") {
        return;
    }
    super::run(|executor| async move {
        let root = super::super::support::Root::new();
        let session = super::loaded(&root, executor).await;
        let runner = session.extension_runner();
        let errors = Rc::new(RefCell::new(Vec::new()));
        let recorded = errors.clone();
        session
            .bind_extensions(maestro_app::ExtensionBindings {
                on_error: Some(Rc::new(move |error| {
                    recorded.borrow_mut().push(format!("{error:?}"));
                    Ok(())
                })),
                ..maestro_app::ExtensionBindings::default()
            })
            .await
            .unwrap();
        std::fs::write(root.path("project/local.txt"), "original\n").unwrap();
        super::start(&runner, "user@broken").await;
        {
            let errors = errors.borrow();
            assert_eq!(errors.len(), 1);
            assert!(errors[0].contains(FAILURE), "{errors:?}");
        }
        refuses_tools(&runner).await;
        refuses_shell(&runner, &root.path("project")).await;
        assert_eq!(
            std::fs::read_to_string(root.path("project/local.txt")).unwrap(),
            "original\n"
        );
        super::start(&runner, "").await;
        assert_eq!(
            super::call(&runner, "read", json!({"path":"local.txt"})).await[0]["text"],
            "original\n"
        );
    });
}
