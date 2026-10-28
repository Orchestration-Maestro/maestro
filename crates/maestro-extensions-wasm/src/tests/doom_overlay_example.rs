//! The composed overlay exercised through its command and retained custom view.
use super::{custom_host::Script, host::Harness};
use crate::{OverlayAnchor, OverlayMargin, OverlayMarginValue, OverlayOptions, SizeValue};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write, path::Path};

mod build;

/// The verifier used before the shipped example creates a downloaded cache.
#[path = "../../../../examples/extensions/doom-overlay/wad_finder/checksum.rs"]
mod checksum;

/// Starts the composed component with read-only controlled WAD bytes.
fn host() -> Result<Harness, String> {
    let path = build::component()?;
    let (engine, component) = super::build::compile(&path).map_err(|e| e.to_string())?;
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tests/doom_overlay_example");
    let mut wasi = wasmtime_wasi::WasiCtxBuilder::new();
    wasi.preopened_dir(fixtures, "/fixture", wasmtime_wasi::FsPerms::ReadOnly)
        .map_err(|e| e.to_string())?;
    let mut host = super::block_on(Harness::from_compiled_with_wasi(
        &engine,
        &component,
        wasi.build(),
    ))
    .map_err(|e| e.to_string())?;
    host.observed().catalog.data.has_ui = true;
    helpers(&mut host);
    host.observed().ui.custom_key_release = Some(true);
    host.observed().ui.custom_overlay = Some(OverlayOptions {
        width: Some(SizeValue::Percentage("75%".into())),
        max_height: Some(SizeValue::Percentage("95%".into())),
        anchor: Some(OverlayAnchor::Center),
        margin: Some(OverlayMarginValue::Sides(OverlayMargin {
            top: Some(1),
            ..OverlayMargin::default()
        })),
        ..OverlayOptions::default()
    });
    assert_eq!(
        host.observed().registrations.commands,
        [(
            "doom-overlay".into(),
            Some("Play DOOM in an overlay. Q pauses and closes it.".into()),
            false
        )]
    );
    Ok(host)
}

/// Supplies literal decoder and authored-path answers.
fn helpers(host: &mut Harness) {
    host.observed().actions.value = json!({"keys.is_key_release":false,"keys.matches_key":false,"keys.parse_key":null,
        "paths.resolve_to_cwd":"/fixture/doom1.wad", "paths.guest_path":"/engine-root",
        "paths.make_temp_dir":"/engine-root"});
    for (operation, args, result) in [
        (
            "paths.resolve_to_cwd",
            json!(["/fixture/doom1.wad", "/work"]),
            json!("/fixture/doom1.wad"),
        ),
        (
            "paths.guest_path",
            json!(["/fixture/doom1.wad"]),
            json!("/fixture/doom1.wad"),
        ),
        (
            "keys.is_key_release",
            json!(["\u{1b}[119;1:3u"]),
            json!(true),
        ),
        (
            "keys.matches_key",
            json!(["\u{1b}[119;1:3u", "w"]),
            json!(true),
        ),
        ("keys.parse_key", json!(["\u{1b}[119;1:3u"]), json!("w")),
        ("keys.parse_key", json!(["W"]), json!("shift+w")),
    ] {
        host.observed().actions.value[format!("{operation} {args}")] = result;
    }
}

/// Known sample positions in the stub's horizontal and vertical colour gradients.
fn samples(columns: u32) -> (&'static [u8], &'static [(u8, u8)], &'static str) {
    match columns {
        8 => (
            &[0, 80, 160, 240, 64, 144, 224, 48],
            &[
                (0, 1),
                (2, 3),
                (4, 5),
                (6, 7),
                (8, 9),
                (10, 11),
                (12, 13),
                (14, 15),
                (16, 17),
                (18, 19),
            ],
            " DOOM | ",
        ),
        40 => (
            &[
                0, 16, 32, 48, 64, 80, 96, 112, 128, 144, 160, 176, 192, 208, 224, 240, 0, 16, 32,
                48, 64, 80, 96, 112, 128, 144, 160, 176, 192, 208, 224, 240, 0, 16, 32, 48, 64, 80,
                96, 112,
            ],
            &[
                (0, 0),
                (1, 2),
                (3, 4),
                (5, 5),
                (6, 7),
                (8, 9),
                (10, 10),
                (11, 12),
                (13, 14),
                (15, 15),
                (16, 17),
                (18, 19),
            ],
            " DOOM | Q=Pause | WASD=Move | Shift+WASD",
        ),
        _ => panic!("unsupported recorded width"),
    }
}

/// Worked colour samples, not the renderer's scaling calculation.
fn frame(columns: u32, red: u8, green: u8, blue: u8) -> Vec<String> {
    let (xs, ys, footer) = samples(columns);
    let mut rows = Vec::new();
    for &(top, bottom) in ys {
        let mut row = String::new();
        for x in xs {
            let r = red.wrapping_add(*x);
            let g1 = green.wrapping_add(top);
            let g2 = green.wrapping_add(bottom);
            write!(
                row,
                "\u{1b}[38;2;{r};{g1};{blue}m\u{1b}[48;2;{r};{g2};{blue}m▀"
            )
            .unwrap();
        }
        rows.push(format!("{row}\u{1b}[0m"));
    }
    rows.push(format!("\u{1b}[2m{footer}\u{1b}[0m"));
    rows
}

/// Runs only the registered command with supplied input and timer completions.
fn play(
    host: &mut Harness,
    display: (Vec<Vec<String>>, u32),
    inputs: &[&str],
    ticks: Vec<usize>,
) -> Result<(), String> {
    host.observed().ui.custom_script = Some(Script {
        width: None,
        frames: display.0,
        inputs: inputs.iter().map(|s| (*s).into()).collect(),
        input_errors: BTreeMap::new(),
        ticks,
        columns: display.1,
    });
    let key = host.observed().callback("command doom-overlay")?;
    let before = host
        .observed()
        .transcript
        .iter()
        .filter(|s| s.ends_with(" custom []"))
        .count();
    super::example_support::invoke_command_on_component(host, key, "/fixture/doom1.wad", false)?;
    let after = host
        .observed()
        .transcript
        .iter()
        .filter(|s| s.ends_with(" custom []"))
        .count();
    assert_eq!(after, before + 1, "the command must show its overlay");
    Ok(())
}

/// The engine, held-key releases and game clock survive closing and resizing the view.
#[test]
fn doom_overlay_retains_frames_keys_and_clock_across_pause_resume() -> Result<(), String> {
    let mut host = host()?;
    play(
        &mut host,
        (
            vec![
                frame(8, 1, 0, 0),
                frame(8, 1, 173, 255),
                frame(8, 1, 173, 0),
                frame(8, 2, 182, 255),
                frame(8, 2, 163, 255),
                frame(8, 2, 162, 255),
                frame(8, 2, 255, 0),
            ],
            8,
        ),
        &["w", "\u{1b}[119;1:3u", "W", "f", " ", "q"],
        vec![1, 0, 0, 1, 0, 0, 0],
    )?;
    play(
        &mut host,
        (vec![frame(8, 3, 255, 0), frame(8, 3, 255, 0)], 8),
        &["q"],
        vec![1, 0],
    )?;
    play(
        &mut host,
        (vec![frame(40, 4, 255, 0), frame(40, 4, 255, 0)], 40),
        &["q"],
        vec![1, 0],
    )?;
    assert!(
        host.observed()
            .transcript
            .iter()
            .any(|s| s == "native-wait 0 28571428")
    );
    assert!(
        host.observed().bus.unreleased.is_empty(),
        "disposal settles all timer tails"
    );
    Ok(())
}

/// Enhanced confirmation keys reach game prompts with or without Shift.
#[test]
fn doom_overlay_forwards_enhanced_confirmation_keys() -> Result<(), String> {
    let mut host = host()?;
    for (data, key) in [
        ("\u{1b}[121u", "y"),
        ("\u{1b}[121;2u", "shift+y"),
        ("\u{1b}[110u", "n"),
        ("\u{1b}[110;2u", "shift+n"),
    ] {
        host.observed().actions.value[format!("keys.matches_key {}", json!([data, key]))] =
            json!(true);
    }
    play(
        &mut host,
        (
            vec![
                frame(8, 1, 0, 0),
                frame(8, 1, 121, 255),
                frame(8, 1, 121, 255),
                frame(8, 1, 110, 255),
                frame(8, 1, 110, 255),
                frame(8, 1, 255, 0),
            ],
            8,
        ),
        &[
            "\u{1b}[121u",
            "\u{1b}[121;2u",
            "\u{1b}[110u",
            "\u{1b}[110;2u",
            "q",
        ],
        vec![1, 0, 0, 0, 0, 0],
    )
}

/// Standard synthetic bytes accept only their own complete SHA-256 digest.
#[test]
fn checksum_accepts_exact_bytes_and_rejects_modified_body() {
    let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert!(checksum::matches_checksum(b"abc", digest));
    assert!(!checksum::matches_checksum(b"abd", digest));
    assert!(!checksum::matches_checksum(b"abc", ""));
}
