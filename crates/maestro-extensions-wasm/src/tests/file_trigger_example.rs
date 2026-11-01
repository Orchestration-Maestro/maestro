//! The shipped file trigger through registered callbacks and controlled watch delivery.
#![cfg(target_os = "linux")]
use serde_json::{Value, json};

use super::documents::Answer;
use super::example_support::released_component;
use super::file_trigger_path::Trigger;
use super::host::Harness;
use maestro_test_directory::Scratch;

/// Starts the shipped component with a private guest temporary directory.
///
/// # Errors
/// Returns an error if building, compiling, mounting or starting the component fails.
fn start(root: &Scratch) -> Result<Harness, String> {
    let path = super::build::example_component("file_trigger")?;
    let (engine, component) = super::build::compile(&path).map_err(|e| e.to_string())?;
    let mut wasi = wasmtime_wasi::WasiCtxBuilder::new();
    wasi.preopened_dir(root.path(), "/tmp", wasmtime_wasi::FsPerms::ReadWrite)
        .map_err(|e| e.to_string())?;
    super::block_on(Harness::from_compiled_with_wasi(
        &engine,
        &component,
        wasi.build(),
    ))
    .map_err(|e| e.to_string())
}

/// Delivers a host event through its registered public callback.
fn deliver(host: &mut Harness, kind: &str) -> Result<Answer, String> {
    let event = if kind == "session_start" {
        json!({"type":kind,"reason":"startup"})
    } else {
        json!({"type":kind,"reason":"quit"})
    };
    let key = host
        .callback(&format!("event {kind}"))
        .map_err(|e| e.to_string())?;
    let capabilities = super::host_family::guest::Capabilities {
        ctx: host.ordinary("/controlled").map_err(|e| e.to_string())?,
        signal: None,
    };
    let exports = host.exports.clone();
    let result = super::block_on(host.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(
                accessor,
                super::host::borrow(key),
                event.to_string(),
                capabilities,
            )
            .await
    }))
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())??;
    super::block_on(host.release_watches())?;
    Answer::of(super::host_family::delivery(result))
}

/// The exact custom messages and options the host admitted, without transport wrappers.
fn messages(host: &mut Harness) -> Vec<Value> {
    host.observed()
        .transcript
        .iter()
        .filter_map(|line| {
            let value: Value =
                serde_json::from_str(line.strip_prefix("action sendMessage ")?).unwrap();
            Some(json!([value[0]["value"], value[1]["value"]]))
        })
        .collect()
}

#[test]
fn file_trigger_delivers_nonempty_contents_and_releases_watches() -> Result<(), String> {
    delivery(|| Ok(()))
}

/// Exercises delivery while another instance may be running.
///
/// # Errors
/// Returns an error if scratch creation, component startup, callback delivery,
/// the overlapping instance or resource release fails.
fn delivery(overlap: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    let root = Scratch::new().map_err(|e| e.to_string())?;
    let mut trigger = Trigger::new_in(root.path());
    let mut host = start(&root)?;
    host.observed().catalog.data.has_ui = true;
    deliver(&mut host, "session_start")?;
    assert_eq!(
        host.observed().native_watch.paths,
        ["/tmp/agent-trigger.txt"]
    );
    let notifications = super::ui_host::receipts(&host.observed().transcript);
    assert_eq!(notifications.len(), 1);
    assert_eq!(
        notifications[0].operands,
        json!(["Watching /tmp/agent-trigger.txt", "Info"])
    );
    trigger.write("outer pending");
    overlap()?;
    assert_eq!(
        std::fs::read_to_string(trigger.path()).unwrap(),
        "outer pending"
    );
    trigger.write(" \u{feff}\nRun the tests Ω\n\u{feff} ");
    host.notify_watch(0)?;
    assert_eq!(
        messages(&mut host),
        [json!([
            {"customType":"file-trigger", "content":"External trigger: Run the tests Ω", "display":true},
            {"triggerTurn":true}
        ])]
    );
    assert_eq!(std::fs::read_to_string(trigger.path()).unwrap(), "");
    restart(&mut host, &mut trigger)?;
    deliver(&mut host, "session_shutdown")?;
    deliver(&mut host, "session_shutdown")?;
    assert!(host.observed().native_watch.watches[1].upgrade().is_none());
    assert_eq!(
        released_component(&mut host)?,
        ["event session_start", "event session_shutdown"]
    );
    assert_eq!(
        host.observed().live,
        0,
        "the component released its resources"
    );
    Ok(())
}

/// Checks empty contents and replacement of the original watch.
///
/// # Errors
/// Returns an error if event or watch callback delivery fails.
fn restart(host: &mut Harness, trigger: &mut Trigger) -> Result<(), String> {
    for text in ["", " \r\n\t", "\u{2003}\u{a0}", "\u{feff}", " \u{feff}\t"] {
        trigger.write(text);
        host.notify_watch(0)?;
        assert_eq!(messages(host).len(), 1, "empty content stays silent");
    }
    host.observed().catalog.data.has_ui = false;
    deliver(host, "session_start")?;
    assert!(host.observed().native_watch.watches[0].upgrade().is_none());
    trigger.write("\u{85}second\u{85}");
    host.notify_watch(0)?;
    assert_eq!(messages(host).len(), 1, "retired watch stays silent");
    host.notify_watch(1)?;
    assert_eq!(messages(host).len(), 2);
    assert_eq!(
        messages(host)[1],
        json!([
            {"customType":"file-trigger", "content":"External trigger: \u{85}second\u{85}", "display":true},
            {"triggerTurn":true}
        ])
    );
    Ok(())
}

#[test]
fn file_trigger_ignores_transient_errors_without_clearing_rejected_text() -> Result<(), String> {
    let root = Scratch::new().map_err(|e| e.to_string())?;
    let trigger = Trigger::new_in(root.path());
    let mut host = start(&root)?;
    deliver(&mut host, "session_start")?;
    drop(trigger);
    let missing = host.notify_watch(0);
    let mut trigger = Trigger::new_in(root.path());
    trigger.write("retry me");
    host.observed().actions.error = Some("delivery refused".into());
    let rejected = host.notify_watch(0);
    let retained = std::fs::read_to_string(trigger.path()).unwrap();
    assert!(messages(&mut host).is_empty());
    host.observed().actions.error = None;
    let retry = host.notify_watch(0);
    std::fs::write(trigger.path(), b"invalid \xff UTF-8").unwrap();
    let decoded = host.notify_watch(0);
    deliver(&mut host, "session_shutdown")?;
    assert_eq!(
        released_component(&mut host)?,
        ["event session_start", "event session_shutdown"]
    );
    assert_eq!(
        host.observed().live,
        0,
        "the component released its resources"
    );
    assert_eq!(
        missing,
        Ok(()),
        "a missing file is a transient read failure"
    );
    assert_eq!(rejected, Ok(()), "a rejected delivery stays catchable");
    assert_eq!(retained, "retry me", "rejected text must not be lost");
    assert_eq!(retry, Ok(()));
    assert_eq!(decoded, Ok(()));
    assert_eq!(
        messages(&mut host),
        [
            json!([{"customType":"file-trigger","content":"External trigger: retry me","display":true},{"triggerTurn":true}]),
            json!([{"customType":"file-trigger","content":"External trigger: invalid \u{fffd} UTF-8","display":true},{"triggerTurn":true}]),
        ]
    );
    Ok(())
}

#[test]
fn file_trigger_instances_overlap_without_sharing_contents() -> Result<(), String> {
    delivery(|| {
        std::thread::spawn(file_trigger_delivers_nonempty_contents_and_releases_watches)
            .join()
            .expect("the overlapping instance must not collide")
    })
}

#[test]
fn file_trigger_paths_are_private_while_both_are_owned() -> Result<(), String> {
    let first = Scratch::new().map_err(|e| e.to_string())?;
    let second = Scratch::new().map_err(|e| e.to_string())?;
    let mut one = Trigger::new_in(first.path());
    let mut two = Trigger::new_in(second.path());
    one.write("first");
    two.write("second");
    assert_eq!(std::fs::read_to_string(one.path()).unwrap(), "first");
    assert_eq!(std::fs::read_to_string(two.path()).unwrap(), "second");
    drop(one);
    assert_eq!(std::fs::read_to_string(two.path()).unwrap(), "second");
    Ok(())
}
