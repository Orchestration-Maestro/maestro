//! Author-owned queue results never cross the component value boundary.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionCommandContext, ExtensionResult};
use std::rc::Rc;

/// Routes queue witnesses through the same author API used by extensions.
pub(super) async fn command(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let (mode, request) = request.split_once(' ').ok_or("missing queue operation")?;
    match mode {
        "value" => value(api, request.into()).await,
        "cancel" => cancellation(api, request).await,
        "mutations" => mutations(api, request).await,
        "owners" => owners(api, request).await,
        "hold" | "fail" => hold(api, context, request.into(), mode == "fail").await,
        _ => Err("unknown queue operation".into()),
    }
}

/// Returns identity and error witnesses after admission by the real native queue.
pub(super) async fn value(api: &ExtensionAPI, path: String) -> ExtensionResult<String> {
    let original = Rc::new(|| 42);
    let returned = original.clone();
    let events = api.events();
    let queued = api.with_file_mutation_queue(path.clone(), move || {
        Box::pin(async move {
            events
                .emit("builtin-queue-phase", serde_json::json!("value:start"))
                .await?;
            Ok(returned)
        })
    });
    api.events()
        .emit("builtin-queue-admitted", serde_json::json!(path))
        .await?;
    let value = queued.await?;
    let error = api
        .with_file_mutation_queue(path, || {
            Box::pin(async { Err::<(), _>("author queue error".into()) })
        })
        .await
        .err();
    Ok(
        serde_json::json!({"same":Rc::ptr_eq(&original, &value),"value":value(),"error":error})
            .to_string(),
    )
}

/// Drops an admitted operation before its native predecessor releases the file.
pub(super) async fn cancellation(api: &ExtensionAPI, request: &str) -> ExtensionResult<String> {
    let request: serde_json::Value =
        serde_json::from_str(request).map_err(|error| error.to_string())?;
    let path = request["path"]
        .as_str()
        .ok_or("missing queue path")?
        .to_owned();
    let effects = Rc::new(std::cell::Cell::new(0));
    let started = effects.clone();
    let events = api.events();
    let mut doomed = api.with_file_mutation_queue(path.clone(), move || {
        started.set(started.get() + 1);
        Box::pin(async move {
            events
                .emit("builtin-queue-late-write", serde_json::Value::Null)
                .await
        })
    });
    if request["poll"] == true {
        std::future::poll_fn(|context| {
            std::task::Poll::Ready(if doomed.as_mut().poll(context).is_pending() {
                Ok(())
            } else {
                Err("queued operation completed before predecessor release".to_owned())
            })
        })
        .await?;
    }
    drop(doomed);
    let following = api.with_file_mutation_queue(path, || Box::pin(async { Ok("live") }));
    api.events()
        .emit("builtin-queue-dropped", serde_json::Value::Null)
        .await?;
    let value = following.await?;
    Ok(serde_json::json!({"cancelledEffects":effects.get(),"following":value}).to_string())
}

/// Releases one tool while a separate native owner remains executable.
async fn owners(api: &ExtensionAPI, path: &str) -> ExtensionResult<String> {
    let unused = api.create_write_tool(
        "/".into(),
        maestro_extensions_wasm::WriteToolOptions::default(),
    )?;
    let survivor = api.create_write_tool(
        "/".into(),
        maestro_extensions_wasm::WriteToolOptions::default(),
    )?;
    drop(unused);
    let result = (survivor.execute)(
        "survivor".into(),
        serde_json::json!({"path":path,"content":"kept"}),
        None,
        None,
    )
    .await?;
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

/// Runs two native mutations concurrently, retaining no factory-local tool variable.
async fn mutations(api: &ExtensionAPI, request: &str) -> ExtensionResult<String> {
    let request: serde_json::Value =
        serde_json::from_str(request).map_err(|error| error.to_string())?;
    let path = request["path"].as_str().ok_or("missing mutation path")?;
    let edit = api.create_edit_tool(
        "/".into(),
        maestro_extensions_wasm::EditToolOptions::default(),
    )?;
    let first = (edit.execute)(
        "first".into(),
        serde_json::json!({"path":path,"edits":[{"oldText":"alpha","newText":"ALPHA"}]}),
        None,
        None,
    );
    drop(edit);
    let second = if request["write"] == true {
        let write = api.create_write_tool(
            "/".into(),
            maestro_extensions_wasm::WriteToolOptions::default(),
        )?;
        (write.execute)(
            "second".into(),
            serde_json::json!({"path":path,"content":"written last\n"}),
            None,
            None,
        )
    } else {
        let edit = api.create_edit_tool(
            "/".into(),
            maestro_extensions_wasm::EditToolOptions::default(),
        )?;
        (edit.execute)(
            "second".into(),
            serde_json::json!({"path":path,"edits":[{"oldText":"beta","newText":"BETA"}]}),
            None,
            None,
        )
    };
    let mut work = [Some(first), Some(second)];
    let mut results = [None, None];
    std::future::poll_fn(|context| {
        let _ = poll_mutations(&mut work, &mut results, context);
        std::task::Poll::Ready(())
    })
    .await;
    api.events()
        .emit("builtin-mutations-admitted", serde_json::Value::Null)
        .await?;
    std::future::poll_fn(|context| poll_mutations(&mut work, &mut results, context)).await;
    serde_json::to_string(&results).map_err(|error| error.to_string())
}

/// Pending native executions and their complete results stay separate in the witness.
type Mutation =
    maestro_extensions_wasm::ExtensionFuture<'static, maestro_extensions_wasm::AgentToolResult>;

/// Drives each native execution without assuming how many transport polls it needs.
fn poll_mutations(
    work: &mut [Option<Mutation>; 2],
    results: &mut [Option<ExtensionResult<maestro_extensions_wasm::AgentToolResult>>; 2],
    context: &mut std::task::Context<'_>,
) -> std::task::Poll<()> {
    for (work, result) in work.iter_mut().zip(results) {
        if let Some(future) = work
            && let std::task::Poll::Ready(value) = future.as_mut().poll(context)
        {
            *result = Some(value);
            *work = None;
        }
    }
    if work.iter().all(Option::is_none) {
        std::task::Poll::Ready(())
    } else {
        std::task::Poll::Pending
    }
}

/// Holds the native turn until the caller's completion signal, optionally failing it.
async fn hold(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    path: String,
    fail: bool,
) -> ExtensionResult<String> {
    let events = api.events();
    api.with_file_mutation_queue(path, move || {
        Box::pin(async move {
            events
                .emit("builtin-queue-phase", serde_json::json!("head:start"))
                .await?;
            context.wait_for_idle().await?;
            events
                .emit("builtin-queue-phase", serde_json::json!("head:end"))
                .await?;
            if fail {
                Err("queued author failure".into())
            } else {
                Ok(())
            }
        })
    })
    .await?;
    Ok("released".into())
}
