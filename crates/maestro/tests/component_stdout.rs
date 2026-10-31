//! Real terminal and component composition without a runtime-to-terminal crate edge.
#![cfg(unix)]
use maestro_app::{
    Local, create_event_bus_for, create_extension_runtime, load_extension_from_factory,
};
use maestro_extensions_wasmtime::artifact_factory_with_stdout;
use maestro_test_process::{TestProcess, selection};
use maestro_tui::Terminal;
use maestro_tui_crossterm::ProcessTerminal;
use serde_json::json;
use std::{io::Read, rc::Rc};

#[path = "../../maestro-tui-crossterm/tests/support/mod.rs"]
#[allow(dead_code, reason = "Shared terminal descriptor fixture.")]
mod terminal_support;

/// Captures bytes without assuming text or an atomic descriptor write.
fn capture(output: terminal_support::Capture, count: usize) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
        let flags = fcntl_getfl(&output.0).unwrap();
        fcntl_setfl(&output.0, flags & !OFlags::NONBLOCK).unwrap();
        let mut output = std::fs::File::from(output.0);
        let mut bytes = vec![0; count];
        output.read_exact(&mut bytes).unwrap();
        bytes
    })
}

#[test]
fn component_output_stays_between_complete_host_frames() {
    let selected = selection();
    for generation in ["p2", "p3"] {
        if !selected.is_child() {
            let output = TestProcess::current("component_output_stays_between_complete_host_frames").unwrap().case(generation).nocapture()
                .env("MAESTRO_WASI_STARTUP_OUTPUT", json!({"op":"output","generation":generation,"bytes":b"\x1b[36mguest\0\xff\x1b[0m"}).to_string()).output().unwrap();
            assert!(
                output.status.success(),
                "{generation}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        } else if selected.runs_case(generation) {
            compose_frames();
        }
    }
}

/// The output descriptor is genuinely blocked before the Store thread admits escape bytes.
fn compose_frames() {
    use terminal_support::{Rig, START_BYTES, pipe_input, pipe_output, until};
    let rig = Rig::paused();
    let input = pipe_input();
    let output = pipe_output();
    rig.run(async {
        let mut terminal = ProcessTerminal::new(rig.local());
        terminal.start(Box::new(|_| {}), Box::new(|| {})).unwrap();
        output.written(START_BYTES).await;
        input.send(b"\x1b[?1u");
        output.written("\x1b[>7u").await;
        let first = format!("<frame-a>{}</frame-a>", "a".repeat(2 * 1024 * 1024));
        terminal.write(&first).unwrap();
        until(|| terminal.pending_output_bytes() < first.len()).await;
        assert!(terminal.pending_output_bytes() > 0);
        let path = std::env::var("MAESTRO_WASI_COMPONENT").unwrap();
        let factory = artifact_factory_with_stdout(
            path,
            crate::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: maestro_app::configuration::config::VERSION.into(),
                agent_dir: "/controlled-agent".into(),
            },
            Box::new(terminal.output_writer().unwrap()),
            true,
        );
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let _owner = load_extension_from_factory::<Local>(
            factory,
            "/work".into(),
            bus.for_extension(),
            create_extension_runtime::<Local>(),
            None,
        )
        .await
        .unwrap();
        terminal.write("<frame-b>whole</frame-b>").unwrap();
        let mut expected = first.into_bytes();
        expected.extend_from_slice(
            b"\x1b[36mguest\0\xff\x1b[0m<frame-b>whole</frame-b>\x1b[?2004l\x1b[<u",
        );
        let collector = capture(output, expected.len());
        terminal.stop().unwrap();
        assert_eq!(collector.join().unwrap(), expected);
    });
}

#[cfg(target_os = "linux")]
#[test]
fn shipped_component_stdout_survives_blocked_output_and_reload() {
    for generation in ["p2", "p3"] {
        let root = crate::support::Root::new();
        root.models("http://127.0.0.1:1/v1");
        let environment = root.command(&[]);
        let output = std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/component_stdout.py"
            ))
            .arg(env!("CARGO_BIN_EXE_maestro"))
            .arg(std::env::var_os("MAESTRO_WASI_COMPONENT").unwrap())
            .arg(generation)
            .arg(root.path("ack"))
            .env_clear()
            .envs(
                environment
                    .get_envs()
                    .filter_map(|(key, value)| value.map(|value| (key, value))),
            )
            .current_dir(root.path("project"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{generation}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn component_output_without_newline_is_flushed_before_terminal_start() {
    let selected = selection();
    for generation in ["p2", "p3"] {
        if !selected.is_child() {
            let output = TestProcess::current(
                "component_output_without_newline_is_flushed_before_terminal_start",
            )
            .unwrap()
            .case(generation)
            .nocapture()
            .env(
                "MAESTRO_WASI_STARTUP_OUTPUT",
                json!({"op":"output","generation":generation,"bytes":b"guest-without-newline"})
                    .to_string(),
            )
            .output()
            .unwrap();
            assert!(
                output.status.success(),
                "{generation}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else if selected.runs_case(generation) {
            let rig = terminal_support::Rig::paused();
            let output = terminal_support::pipe_output();
            let terminal = ProcessTerminal::new(rig.local());
            rig.run(async {
                let factory = artifact_factory_with_stdout(
                    std::env::var("MAESTRO_WASI_COMPONENT").unwrap(),
                    crate::support::widgets(),
                    maestro_extensions_wasmtime::Installation {
                        version: maestro_app::configuration::config::VERSION.into(),
                        agent_dir: "/controlled-agent".into(),
                    },
                    Box::new(terminal.standard_output_writer()),
                    false,
                );
                let bus = create_event_bus_for::<Local>(
                    Rc::new(|_| panic!("no tails")),
                    Rc::new(|_| panic!("no errors")),
                );
                let _owner = load_extension_from_factory::<Local>(
                    factory,
                    "/work".into(),
                    bus.for_extension(),
                    create_extension_runtime::<Local>(),
                    None,
                )
                .await
                .unwrap();
                assert_eq!(output.take(), "guest-without-newline");
            });
        }
    }
}
