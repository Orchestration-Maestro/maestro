//! The built game exercised through its registered command and the shared scripted clock.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{custom_host::Script, host::Harness, observed::Observed};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wasmtime_wasi::WasiView;

/// Frames and saved records captured from the authored game with supplied input and ticks.
fn corpus() -> Value {
    serde_json::from_str(include_str!("space_invaders_frames.json")).unwrap()
}
/// Supplies literal key-decoder answers for plain and enhanced keyboard inputs.
fn helpers(observed: &mut Observed) {
    observed.actions.value =
        json!({"keys.is_key_release":false,"keys.matches_key":false,"visible_width":63});
    for (data, key, released) in [
        ("\x1b", "escape", false),
        ("\x1b[D", "left", false),
        ("\x1b[C", "right", false),
        ("\x1b[97;1:3u", "a", true),
        ("\x1b[97;1:2u", "a", false),
        ("\x1b[100;1:3u", "d", true),
        ("\x1b[102;1:3u", "f", true),
        (" ", "space", false),
    ] {
        observed.actions.value[format!("keys.is_key_release {}", json!([data]))] = json!(released);
        observed.actions.value[format!("keys.matches_key {}", json!([data, key]))] = json!(true);
    }
}
/// Supplies measured widths of literal oracle lines and their boxed header/footer content.
fn frame_helpers(observed: &mut Observed, frame: &[String]) {
    let ansi = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
    for row in frame {
        observed.actions.value[format!("truncate_to_width {}", json!([row, 80, "", false]))] =
            json!(row);
        let row = row.trim_end();
        let mut texts = vec![row];
        if let Some(content) = row
            .strip_prefix("\x1b[2m │\x1b[22m")
            .and_then(|s| s.strip_suffix("\x1b[2m│\x1b[22m"))
        {
            texts.push(content.trim_end());
        }
        for text in texts {
            observed.actions.value[format!("visible_width {}", json!([text]))] =
                json!(ansi.replace_all(text, "").chars().count());
        }
    }
}
/// Starts the actual component with the host's supplied random source and keyboard replies.
fn host(column: u64) -> Result<Harness, String> {
    let path = super::build::example_component("maestro-extensions-wasm", "space_invaders")?;
    let mut host = super::block_on(Harness::start(&path)).map_err(|e| e.to_string())?;
    let mut wasi = wasmtime_wasi::WasiCtxBuilder::new();
    wasi.secure_random(wasmtime_wasi::random::Deterministic::new(
        column.to_be_bytes().to_vec(),
    ));
    *host.store.data_mut().ctx().ctx = wasi.build();
    helpers(host.observed());
    host.observed().catalog.data.has_ui = true;
    assert_eq!(
        host.observed().registrations.commands,
        [(
            "invaders".into(),
            Some("Play Space Invaders!".into()),
            false,
        )]
    );
    Ok(host)
}
/// A custom save entry in the entire session history, including an explicit null discard.
fn entry(value: &Value) -> Value {
    json!({"type":"custom","id":"save","parentId":null,"timestamp":"2026-01-01T00:00:00Z","customType":"space-invaders-save","data":value})
}
/// Supplies all session entries rather than a branch projection.
fn history(host: &mut Harness, entries: &Value) {
    host.observed().readers.control(
        &json!({"bind":{"replies":[{"method":"getEntries","args":[],"value":entries}]}})
            .to_string(),
    );
}
/// Calls only the command the built component registered.
fn command(host: &mut Harness) -> Result<(), String> {
    let key = host.observed().callback("command invaders")?;
    super::example_support::invoke_command_on_component(host, key, "ignored", false)
}
/// Values appended to the session owner, never read from game internals.
fn saves(host: &mut Harness) -> Vec<Value> {
    host.observed()
        .transcript
        .iter()
        .filter_map(|line| line.strip_prefix("entry space-invaders-save "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect()
}
/// Runs a literal input/frame/tick script under the shared custom-view driver.
fn run(host: &mut Harness, case: &Value) -> Result<(), String> {
    history(host, &json!([entry(&case["start"])]));
    let frames: Vec<Vec<String>> = serde_json::from_value(case["frames"].clone()).unwrap();
    for frame in &frames {
        frame_helpers(host.observed(), frame);
    }
    host.observed().ui.custom_script = Some(Script {
        width: None,
        input_errors: BTreeMap::new(),
        inputs: serde_json::from_value(case["inputs"].clone()).unwrap(),
        frames,
        ticks: serde_json::from_value(case["ticks"].clone()).unwrap(),
        columns: 80,
    });
    command(host)?;
    assert_eq!(
        saves(host),
        serde_json::from_value::<Vec<Value>>(case["saves"].clone()).unwrap(),
        "{}",
        case["name"]
    );
    assert_eq!(host.observed().bus.removals.len(), 1);
    assert_eq!(host.observed().bus.tails.len(), 1);
    assert!(
        host.observed().bus.listeners.is_empty(),
        "disposal removes the listener"
    );
    assert!(
        host.observed().bus.unreleased.is_empty(),
        "every independent tail settled"
    );
    Ok(())
}
/// Finds one named, fully captured input/tick script.
fn script(name: &str) -> Value {
    corpus()["scripts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()
        .clone()
}

#[test]
fn space_invaders_ticks_match_reference_movement_fire_and_collisions() -> Result<(), String> {
    for case in corpus()["cases"].as_array().unwrap() {
        let mut host = host(case["column"].as_u64().unwrap())?;
        run(&mut host, case)?;
    }
    Ok(())
}

#[test]
fn space_invaders_releases_keys_restarts_and_discards_saves() -> Result<(), String> {
    for name in [
        "release_keys",
        "right_release",
        "restart_victory",
        "restart_loss",
    ] {
        let mut host = host(0)?;
        run(&mut host, &script(name))?;
    }
    Ok(())
}

#[test]
fn space_invaders_tick_tail_cancels_pending_host_sleep() -> Result<(), String> {
    let mut running = host(0)?;
    run(&mut running, &script("fresh_running"))?;
    assert!(
        running
            .observed()
            .transcript
            .iter()
            .any(|s| s == "native-wait 0 50000000")
    );
    let mut completed = host(0)?;
    run(&mut completed, &script("completed_save"))
}

#[test]
fn space_invaders_resume_redraws_current_controls_without_losing_save() -> Result<(), String> {
    let mut host = host(0)?;
    run(&mut host, &script("resume"))
}

#[test]
fn space_invaders_simultaneous_hits_keep_lives_nonnegative() -> Result<(), String> {
    let mut host = host(0)?;
    run(&mut host, &script("simultaneous_hits"))
}

/// Reports the authored headless error without reading entries or opening a custom view.
fn headless(host: &mut Harness) -> Result<(), String> {
    host.observed().catalog.data.has_ui = false;
    command(host)?;
    assert_eq!(
        super::example_support::observations(&host.observed().transcript),
        [super::example_support::Observation::Notify {
            message: "Space Invaders requires interactive mode".into(),
            kind: Some("error".into()),
        },]
    );
    assert!(saves(host).is_empty());
    host.observed().catalog.data.has_ui = true;
    Ok(())
}

#[test]
fn space_invaders_component_runs_registered_command_and_restores_latest_save() -> Result<(), String>
{
    let corpus = corpus();
    let mut host = host(0)?;
    headless(&mut host)?;
    let paused = script("resume")["start"].clone();
    for (entries, frame, key) in [
        (
            json!([entry(&Value::Null), entry(&paused)]),
            &corpus["frames"]["paused"],
            "q",
        ),
        (
            json!([entry(&paused), entry(&Value::Null)]),
            &corpus["frames"]["initial"],
            "\x1b",
        ),
        (json!([]), &corpus["frames"]["initial"], "q"),
    ] {
        history(&mut host, &entries);
        let frame: Vec<String> = serde_json::from_value(frame.clone()).unwrap();
        frame_helpers(host.observed(), &frame);
        host.observed().ui.custom_script = Some(Script {
            width: None,
            input_errors: BTreeMap::new(),
            inputs: vec![key.into()],
            frames: vec![frame; 2],
            ticks: vec![0, 0],
            columns: 80,
        });
        command(&mut host)?;
    }
    assert_eq!(
        saves(&mut host),
        [corpus["cases"][0]["start"].clone(), Value::Null]
    );
    assert!(host.observed().bus.listeners.is_empty());
    assert!(host.observed().bus.unreleased.is_empty());
    Ok(())
}

/// Every styled row fits the supplied terminal, even when no game cell is visible.
#[test]
fn space_invaders_rows_fit_narrow_terminal_widths() -> Result<(), String> {
    let corpus = corpus();
    let raw: Vec<String> = serde_json::from_value(corpus["frames"]["paused"].clone()).unwrap();
    for case in corpus["narrow_frames"].as_array().unwrap() {
        let mut host = host(0)?;
        let columns = case["columns"].as_u64().unwrap();
        let frame: Vec<String> = serde_json::from_value(case["rows"].clone()).unwrap();
        frame_helpers(host.observed(), &raw);
        for (raw, fitted) in raw.iter().zip(&frame) {
            host.observed().actions.value[format!(
                "truncate_to_width {}",
                json!([raw.trim_end(), columns, "", false])
            )] = json!(fitted);
        }
        history(&mut host, &json!([entry(&script("resume")["start"])]));
        host.observed().ui.custom_script = Some(Script {
            width: None,
            input_errors: BTreeMap::new(),
            inputs: vec!["q".into()],
            frames: vec![frame; 2],
            ticks: vec![0, 0],
            columns: u32::try_from(columns).unwrap(),
        });
        command(&mut host)?;
        assert!(saves(&mut host).is_empty());
        assert!(host.observed().bus.listeners.is_empty());
        assert!(host.observed().bus.unreleased.is_empty());
    }
    Ok(())
}

/// Failed saves leave both retrying and discarding available on the retained view.
#[test]
fn space_invaders_rejected_save_keeps_input_usable() -> Result<(), String> {
    let corpus = corpus();
    let frame: Vec<String> = serde_json::from_value(corpus["frames"]["initial"].clone()).unwrap();
    for rejected in ["\x1b", "q"] {
        for retry in ["\x1b", "q"] {
            let mut host = host(0)?;
            history(&mut host, &json!([]));
            frame_helpers(host.observed(), &frame);
            host.observed().actions.entry_error = Some("session append refused".into());
            host.observed().ui.custom_script = Some(Script {
                width: None,
                inputs: vec![rejected.into(), retry.into()],
                input_errors: [(0, "session append refused".into())].into(),
                frames: vec![frame.clone(); 3],
                ticks: vec![0; 3],
                columns: 80,
            });
            command(&mut host)?;
            let saved = if retry == "\x1b" {
                corpus["cases"][0]["start"].clone()
            } else {
                Value::Null
            };
            assert_eq!(saves(&mut host), [saved]);
            assert!(host.observed().actions.entry_error.is_none());
            assert!(host.observed().bus.listeners.is_empty());
            assert!(host.observed().bus.unreleased.is_empty());
        }
    }
    Ok(())
}
