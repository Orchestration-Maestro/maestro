//! Canonical command, setup and replacement capability probes.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionCommandContext, SessionManager, UserContent};
use serde_json::{Value, json};

/// Runs real command imports with their setup and replacement callback operands.
pub(super) async fn observe(
    context: &ExtensionCommandContext,
    path: String,
) -> Result<Value, String> {
    use maestro_extensions_wasm::{
        ForkData, ForkOptions, ForkPosition, NavigateTreeOptions, NewSessionCommandOptions,
        SwitchSessionOptions,
    };
    context.wait_for_idle().await?;
    let new = context
        .new_session(Some(NewSessionCommandOptions {
            data: maestro_extensions_wasm::NewSessionCommandData {
                parent_session: Some("literal parent".into()),
            },
            setup: Some(Box::new(move |session| {
                Box::pin(async move { write(&session, &path) })
            })),
            with_session: Some(continuation("new")),
        }))
        .await?;
    let fork = context
        .fork(
            "literal fork",
            Some(ForkOptions {
                data: ForkData {
                    position: Some(ForkPosition::Before),
                },
                with_session: Some(continuation("fork")),
            }),
        )
        .await?;
    let navigate = context
        .navigate_tree(
            "literal target",
            Some(NavigateTreeOptions {
                summarize: Some(true),
                custom_instructions: Some(" literal instructions ".into()),
                replace_instructions: Some(false),
                label: Some("literal navigation label".into()),
            }),
        )
        .await?;
    let switch = context
        .switch_session(
            " literal session ",
            Some(SwitchSessionOptions {
                with_session: Some(continuation("switch")),
            }),
        )
        .await?;
    Ok(
        json!({"new": new.cancelled, "fork": fork.cancelled, "navigate": navigate.cancelled,
        "switch": switch.cancelled, "reload": context.reload().await.err().ok_or("native reload must fail")?}),
    )
}

/// Reads the replacement command context and delivers both native message forms.
fn continuation(operation: &'static str) -> maestro_extensions_wasm::WithSession {
    Box::new(move |context| {
        Box::pin(async move {
            let cwd = context.cwd()?;
            context
                .send_user_message(UserContent::Text(format!("{operation}:{cwd}")), None)
                .await?;
            context.send_message(serde_json::from_value(json!({
            "customType": operation, "content": cwd, "display": false, "details": {"n": 2.5},
        })).map_err(|error| error.to_string())?, None).await
        })
    })
}

/// Exercises the writable owner while retaining its real read view.
fn write(session: &SessionManager, path: &str) -> Result<(), String> {
    session.set_session_file(path)?;
    if session.get_session_file()?.as_deref() != Some(path) || session.is_persisted()? {
        return Err("writer file or persistence mismatch".into());
    }
    let file = session.new_session(Some(maestro_extensions_wasm::NewSessionOptions {
        id: Some("literal setup id".into()),
        parent_session: Some("literal setup parent".into()),
    }))?;
    if file.as_deref() != Some(path) || session.get_session_id()? != "literal setup id" {
        return Err("writer new session mismatch".into());
    }
    let root = session.append_message(
        serde_json::from_value(json!({
            "role": "user", "content": "literal setup message", "timestamp": 12.5,
        }))
        .map_err(|error| error.to_string())?,
    )?;
    session.append_message(serde_json::from_value(json!({
        "role": "custom", "customType": "literal agent message", "content": "literal agent content",
        "display": false, "details": {"nested": [2.5, null]}, "timestamp": 13.5,
    })).map_err(|error| error.to_string())?)?;
    session.append_thinking_level_change("high")?;
    session.append_model_change("literal provider", "literal model")?;
    let entries = append_records(session, &root)?;
    let context = session.build_session_context()?;
    let children = session.get_children(&root)?;
    if children.len() != 2 || context.messages.is_empty() {
        return Err("writer context or children mismatch".into());
    }
    let reader = session.get_entries()?;
    let header = session.get_header()?;
    session.branch(&root)?;
    if session.get_leaf_id()?.as_deref() != Some(&root) {
        return Err("writer branch mismatch".into());
    }
    session.reset_leaf()?;
    if session.get_leaf_id()?.is_some() {
        return Err("writer reset mismatch".into());
    }
    session.branch(&entries)?;
    let branch = session.create_branched_session(&entries)?;
    if branch.is_some() {
        return Err("memory branch published a file".into());
    }
    session.append_custom_entry(
        "writer-report",
        Some(json!({
            "entries": reader, "context": context, "children": children, "header": header,
        })),
    )?;
    Ok(())
}

/// Appends every remaining entry variant with independently visible literal fields.
fn append_records(session: &SessionManager, root: &str) -> Result<String, String> {
    session.append_compaction(
        serde_json::from_value(json!({
            "summary": "literal compact", "firstKeptEntryId": root, "tokensBefore": 12.5,
            "details": {"z": [false, null], "a": 2.5},
        }))
        .map_err(|error| error.to_string())?,
        Some(true),
    )?;
    session.append_custom_entry("literal custom", Some(json!({"ready": true})))?;
    session.append_custom_message_entry(
        serde_json::from_value(json!({
            "customType": "literal custom message", "content": "literal content", "display": false,
            "details": [2.5, null],
        }))
        .map_err(|error| error.to_string())?,
    )?;
    session.append_label_change(root, Some("literal setup label"))?;
    session.append_session_info("literal setup name")?;
    session.branch_with_summary(
        Some(root),
        "literal branch summary",
        Some(json!({"s": 2.5})),
        Some(false),
    )
}
