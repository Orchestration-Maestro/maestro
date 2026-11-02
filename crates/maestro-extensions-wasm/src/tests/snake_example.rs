//! Snake exercised through its registered command and retained custom component.
use super::{custom_host::Script, host::Harness};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wasmtime_wasi::WasiView;

/// Starts the built example with controlled randomness and literal keyboard replies.
fn host() -> Result<Harness, String> {
    let path = super::build::example_component("snake")?;
    let mut host = super::block_on(Harness::start(&path)).map_err(|e| e.to_string())?;
    random(&mut host, vec![0]);
    host.observed().catalog.data.has_ui = true;
    let mut keys = json!({"keys.matches_key":false});
    for (data, key) in [
        ("\x1b", "escape"),
        ("\x1b[A", "up"),
        ("\x1b[B", "down"),
        ("\x1b[C", "right"),
        ("\x1b[D", "left"),
    ] {
        keys[format!("keys.matches_key {}", json!([data, key]))] = json!(true);
    }
    host.observed().actions.value = keys;
    assert_eq!(
        host.observed().registrations.commands,
        [("snake".into(), Some("Play Snake!".into()), false)]
    );
    Ok(host)
}
/// Replaces the WASI source before the game requests any random food coordinates.
fn random(host: &mut Harness, bytes: Vec<u8>) {
    let mut wasi = wasmtime_wasi::WasiCtxBuilder::new();
    wasi.secure_random(wasmtime_wasi::random::Deterministic::new(bytes));
    *host.store.data_mut().ctx().ctx = wasi.build();
}
/// A worked saved game in the authored JSON format.
fn state(snake: &[(i32, i32)], food: (i32, i32), direction: &str, score: u64) -> Value {
    let point = |(x, y)| json!({"x":x,"y":y});
    json!({"snake":snake.iter().copied().map(point).collect::<Vec<_>>(), "food":point(food),
        "direction":direction,"nextDirection":direction,"score":score,"highScore":20,"gameOver":false})
}
/// A matching custom session entry, including an explicit null clearing entry.
fn entry(value: &Value) -> Value {
    json!({"type":"custom","id":"save","parentId":null,"timestamp":"2026-01-01T00:00:00Z","customType":"snake-save","data":value})
}
/// Supplies the whole session's entries rather than a branch projection.
fn history(host: &mut Harness, entries: &Value) {
    host.observed().readers.control(
        &json!({"bind":{"replies":[{"method":"getEntries","args":[],"value":entries}]}})
            .to_string(),
    );
}
/// Executes the registered command on the built component.
fn command(host: &mut Harness) -> Result<(), String> {
    let key = host.observed().callback("command snake")?;
    super::example_support::invoke_command_on_component(host, key, "ignored", false)
}
/// Drives input and tick completions and verifies listener/tail cleanup at close.
fn run(
    host: &mut Harness,
    entries: &Value,
    inputs: &[&str],
    ticks: Vec<usize>,
    display: (Vec<Vec<String>>, u32),
) -> Result<(), String> {
    history(host, entries);
    let frames = display
        .0
        .into_iter()
        .map(|frame| {
            frame
                .into_iter()
                .map(|line| {
                    let width = usize::try_from(display.1).unwrap();
                    let fitted = super::snake_frames::fitted(&line, width);
                    assert!(super::snake_frames::visible(&fitted) <= width);
                    host.observed().actions.value
                        [format!("truncate_to_width {}", json!([line, width, "", false]))] =
                        json!(fitted);
                    fitted
                })
                .collect()
        })
        .collect();
    host.observed().ui.custom_script = Some(Script {
        width: None,
        input_errors: BTreeMap::new(),
        inputs: inputs.iter().map(|s| (*s).into()).collect(),
        frames,
        ticks,
        columns: display.1,
    });
    command(host)?;
    assert_eq!(
        host.observed().bus.removals.len(),
        1,
        "the custom view was built and disposed"
    );
    assert!(
        host.observed().bus.listeners.is_empty(),
        "disposal unsubscribes the game"
    );
    assert!(
        host.observed().bus.unreleased.is_empty(),
        "every announced tail settled"
    );
    Ok(())
}
/// Saved values received by the session owner, not by inspecting game internals.
fn saves(host: &mut Harness) -> Vec<Value> {
    host.observed()
        .transcript
        .iter()
        .filter_map(|s| s.strip_prefix("entry snake-save "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
use super::snake_frames::frame;

/// Headless use reports the authored error without reading history or opening a view.
#[test]
fn snake_headless_command_requires_interactive_mode() -> Result<(), String> {
    let mut host = host()?;
    host.observed().catalog.data.has_ui = false;
    command(&mut host)?;
    assert_eq!(
        super::example_support::observations(&host.observed().transcript),
        [super::example_support::Observation::Notify {
            message: "Snake requires interactive mode".into(),
            kind: Some("error".into())
        }]
    );
    assert!(saves(&mut host).is_empty());
    Ok(())
}
/// Restoring starts paused; quitting while paused leaves the latest save untouched.
#[test]
fn snake_restores_latest_save_paused_and_disposes_without_clearing() -> Result<(), String> {
    for (key, width) in [("\x1b", 84), ("q", 20), ("Q", 0)] {
        let mut host = host()?;
        let game = state(&[(20, 7), (19, 7), (18, 7)], (21, 7), "right", 10);
        run(
            &mut host,
            &json!([entry(&Value::Null), entry(&game)]),
            &[key],
            vec![0, 0],
            (
                vec![frame(&game, "paused", width), frame(&game, "paused", width)],
                u32::try_from(width).unwrap(),
            ),
        )?;
        assert!(saves(&mut host).is_empty());
        assert!(host.observed().bus.tails.is_empty());
    }
    Ok(())
}
/// Narrow frames fit while movement still collides at the full board's wall.
#[test]
fn snake_narrow_frames_fit_without_changing_collision_boundaries() -> Result<(), String> {
    for width in [0, 1, 2, 3, 20, 40, 42, 43, 45, 46, 84] {
        let mut host = host()?;
        let game = state(&[(38, 7), (37, 7), (36, 7)], (0, 0), "right", 10);
        let moved = state(&[(39, 7), (38, 7), (37, 7)], (0, 0), "right", 10);
        let mut over = moved.clone();
        over["gameOver"] = json!(true);
        run(
            &mut host,
            &json!([entry(&game)]),
            &["x", "x", "q"],
            vec![0, 1, 1, 0],
            (
                vec![
                    frame(&game, "paused", width),
                    frame(&moved, "playing", width),
                    frame(&over, "over", width),
                    frame(&over, "over", width),
                ],
                u32::try_from(width).unwrap(),
            ),
        )?;
        assert_eq!(saves(&mut host), [Value::Null]);
    }
    Ok(())
}
/// Tick completions grow the body, apply turns, reject reversal and persist the moved state.
#[test]
fn snake_ticks_move_grow_turn_and_save_through_session_entries() -> Result<(), String> {
    let mut host = host()?;
    let game = state(&[(20, 7), (19, 7), (18, 7)], (21, 7), "right", 0);
    let grown = state(&[(21, 7), (20, 7), (19, 7), (18, 7)], (0, 0), "right", 10);
    let moved = state(&[(21, 6), (21, 7), (20, 7), (19, 7)], (0, 0), "up", 10);
    let mut queued = moved.clone();
    queued["nextDirection"] = json!("left");
    let frames = vec![
        frame(&game, "paused", 84),
        frame(&grown, "playing", 84),
        frame(&moved, "playing", 84),
        frame(&moved, "playing", 84),
        frame(&queued, "playing", 84),
        frame(&queued, "playing", 84),
    ];
    run(
        &mut host,
        &json!([entry(&game)]),
        &["x", "W", "\x1b[B", "a", "\x1b"],
        vec![0, 1, 1, 0, 0, 0],
        (frames, 84),
    )?;
    assert_eq!(saves(&mut host), [queued]);
    assert!(
        host.observed()
            .transcript
            .iter()
            .any(|s| s == "native-wait 0 100000000")
    );
    Ok(())
}
/// Wall and self collisions stop movement; restarting retains the high score and clears the save.
#[test]
fn snake_collisions_restart_and_clear_saved_state() -> Result<(), String> {
    for (snake, direction) in [
        (vec![(39, 7), (38, 7), (37, 7)], "right"),
        (vec![(20, 7), (20, 8), (21, 8), (21, 7), (22, 7)], "right"),
    ] {
        let mut host = host()?;
        let game = state(&snake, (5, 5), direction, 10);
        let mut over = game.clone();
        over["gameOver"] = json!(true);
        let fresh = state(&[(20, 7), (19, 7), (18, 7)], (0, 0), "right", 0);
        run(
            &mut host,
            &json!([entry(&game)]),
            &["x", "R", "q"],
            vec![0, 1, 0, 0],
            (
                vec![
                    frame(&game, "paused", 84),
                    frame(&over, "over", 84),
                    frame(&fresh, "playing", 84),
                    frame(&fresh, "playing", 84),
                ],
                84,
            ),
        )?;
        assert_eq!(saves(&mut host), [Value::Null, Value::Null]);
    }
    Ok(())
}
/// Null saves start fresh; finished saves start fresh with their high score.
#[test]
fn snake_new_games_and_completed_saves_start_running() -> Result<(), String> {
    let mut over = state(&[(1, 1)], (2, 1), "right", 10);
    over["gameOver"] = json!(true);
    for history in [
        json!([]),
        json!([entry(&over), entry(&Value::Null)]),
        json!([entry(&over)]),
    ] {
        let high = if history.as_array().unwrap().len() == 1 {
            20
        } else {
            0
        };
        let mut host = host()?;
        let mut moved = state(&[(21, 7), (20, 7), (19, 7)], (0, 0), "right", 0);
        moved["highScore"] = json!(high);
        run(
            &mut host,
            &history,
            &["\x1b"],
            vec![1, 0],
            (
                vec![frame(&moved, "playing", 84), frame(&moved, "playing", 84)],
                84,
            ),
        )?;
        assert_eq!(saves(&mut host), [moved]);
    }
    Ok(())
}
/// Invalid persisted coordinates are rejected before custom-view admission.
#[test]
fn snake_rejects_invalid_saved_positions() -> Result<(), String> {
    for snake in [vec![], vec![(-1, 0)], vec![(40, 14)], vec![(0, 15)]] {
        let mut host = host()?;
        history(
            &mut host,
            &json!([entry(&state(&snake, (5, 5), "right", 0))]),
        );
        assert_eq!(command(&mut host), Err("Invalid Snake save".into()));
        assert!(saves(&mut host).is_empty());
        assert!(host.observed().bus.listeners.is_empty());
    }
    Ok(())
}
/// The last food completes a full board instead of retrying an impossible random spawn.
#[test]
fn snake_filled_board_ends_without_spawning_food() -> Result<(), String> {
    let mut host = host()?;
    let mut snake = vec![(38, 14)];
    snake.extend(
        (0..15)
            .flat_map(|y| (0..40).map(move |x| (x, y)))
            .filter(|p| ![(38, 14), (39, 14)].contains(p)),
    );
    let game = state(&snake, (39, 14), "right", 20);
    snake.insert(0, (39, 14));
    let mut over = state(&snake, (39, 14), "right", 30);
    over["gameOver"] = json!(true);
    over["highScore"] = json!(30);
    run(
        &mut host,
        &json!([entry(&game)]),
        &["x", "\x1b"],
        vec![0, 1, 0],
        (
            vec![
                frame(&game, "paused", 84),
                frame(&over, "over", 84),
                frame(&over, "over", 84),
            ],
            84,
        ),
    )?;
    assert_eq!(saves(&mut host), [over]);
    Ok(())
}

/// Both arrow keys and either letter case turn only at the next tick boundary.
#[test]
fn snake_controls_apply_arrow_and_letter_turns() -> Result<(), String> {
    for (keys, direction, head) in [
        (["w", "W", "\x1b[A"], "up", (20, 6)),
        (["s", "S", "\x1b[B"], "down", (20, 8)),
        (["a", "A", "\x1b[D"], "left", (19, 7)),
        (["d", "D", "\x1b[C"], "right", (21, 7)),
    ] {
        for key in keys {
            let mut host = host()?;
            let game = state(&[(20, 7), (20, 8), (20, 9)], (0, 0), "up", 0);
            let game = if direction == "down" {
                state(&[(20, 7), (19, 7), (18, 7)], (0, 0), "right", 0)
            } else {
                game
            };
            let body = game["snake"][1].clone();
            let mut moved = state(&[head, (20, 7), (0, 0)], (0, 0), direction, 0);
            moved["snake"][2] = body;
            run(
                &mut host,
                &json!([entry(&game)]),
                &["x", key, "\x1b"],
                vec![0, 0, 1, 0],
                (
                    vec![
                        frame(&game, "paused", 84),
                        frame(&game, "playing", 84),
                        frame(&moved, "playing", 84),
                        frame(&moved, "playing", 84),
                    ],
                    84,
                ),
            )?;
            assert_eq!(saves(&mut host), [moved]);
        }
    }
    Ok(())
}

/// The initial food sampler retries a point that would be hidden inside the starting body.
#[test]
fn snake_initial_food_avoids_the_whole_body() -> Result<(), String> {
    let mut host = host()?;
    let bytes: Vec<_> = [19_u64, 7, 0, 0]
        .into_iter()
        .flat_map(u64::to_be_bytes)
        .collect();
    random(&mut host, bytes);
    let mut fresh = state(&[(20, 7), (19, 7), (18, 7)], (0, 0), "right", 0);
    fresh["highScore"] = json!(0);
    run(
        &mut host,
        &json!([]),
        &["q"],
        vec![0, 0],
        (
            vec![frame(&fresh, "playing", 84), frame(&fresh, "playing", 84)],
            84,
        ),
    )?;
    assert_eq!(saves(&mut host), [Value::Null]);
    Ok(())
}
