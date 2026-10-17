//! Ordered execution collection before native lifecycle projection.
use super::super::execution::ExecutionState;
use maestro_app::SandboxFile;
use maestro_app::browser::components::sandboxed_iframe::SandboxFileContent;

#[test]
fn execution_state_keeps_file_order_and_binary_bytes() {
    let mut state = ExecutionState::<serde_json::Value, SandboxFileContent>::default();
    for content in [
        SandboxFileContent::Text("雪\0".into()),
        SandboxFileContent::Text(String::new()),
        SandboxFileContent::Bytes(vec![0, 128, 255]),
    ] {
        state.file(SandboxFile {
            file_name: "same".into(),
            content,
            mime_type: "application/octet-stream".into(),
        });
    }
    let result = state.complete(None).unwrap();
    let files = result.files.unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.file_name.as_str())
            .collect::<Vec<_>>(),
        ["same", "same", "same"]
    );
    assert_eq!(files[0].content, SandboxFileContent::Text("雪\0".into()));
    assert_eq!(files[1].content, SandboxFileContent::Text(String::new()));
    assert_eq!(
        files[2].content,
        SandboxFileContent::Bytes(vec![0, 128, 255])
    );
}

#[test]
fn execution_state_distinguishes_absent_and_null_return() {
    for value in [
        None,
        Some(serde_json::Value::Null),
        Some(false.into()),
        Some(0.into()),
    ] {
        let mut state = ExecutionState::<_, SandboxFileContent>::default();
        assert_eq!(state.complete(value.clone()).unwrap().return_value, value);
    }
}

#[test]
fn execution_state_ignores_nonterminal_messages() {
    let mut state = ExecutionState::<serde_json::Value, SandboxFileContent>::default();
    assert!(
        state
            .receive(super::super::execution::ExecutionMessage::Other)
            .is_none()
    );
    let result = state.complete(Some(9.into())).unwrap();
    assert_eq!(result.return_value, Some(9.into()));
    assert!(result.files.unwrap().is_empty());
}

#[test]
fn execution_state_settles_once_and_keeps_first_result() {
    use super::super::execution::ExecutionMessage;
    use maestro_app::browser::components::sandboxed_iframe::SandboxError;
    for success_first in [false, true] {
        let mut state = ExecutionState::<serde_json::Value, SandboxFileContent>::default();
        let complete = ExecutionMessage::Complete(Some(9.into()));
        let error = ExecutionMessage::Error(SandboxError {
            message: "failure".into(),
            stack: "stack".into(),
        });
        let (first, second) = if success_first {
            (complete, error)
        } else {
            (error, complete)
        };
        let result = state.receive(first).unwrap();
        assert_eq!(result.success, success_first);
        if success_first {
            assert_eq!(result.return_value, Some(9.into()));
        } else {
            assert_eq!(result.error.unwrap().message, "failure");
        }
        assert!(state.receive(second).is_none());
        assert!(
            state
                .receive(ExecutionMessage::File(SandboxFile {
                    file_name: "late".into(),
                    content: SandboxFileContent::Text("late".into()),
                    mime_type: "text/plain".into()
                }))
                .is_none()
        );
        assert!(state.complete(None).is_none());
    }
}
