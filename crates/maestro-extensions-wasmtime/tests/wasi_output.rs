//! Supplied versus inherited byte output through real component standard interfaces.
#![cfg(test)]
use super::wasi_support::{COMPONENT, Harness, run};
use maestro_extensions_wasmtime::artifact_factory_with_stdout;
use maestro_test_process::{TestProcess, selection};
use serde_json::json;
use std::{
    io::{self, Write},
    path::Path,
    sync::{Arc, Mutex},
};

/// Distinguishes guest output from harness text, including invalid UTF-8 and escapes.
const BYTES: &[u8] = b"\x1b[35mguest\0\xff\x1b[0m";

/// A caller-owned standard writer; no runtime type crosses this seam.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// A deliberate standard writer failure at the existing callback boundary.
struct Refused;
impl Write for Refused {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn component_output_uses_the_supplied_writer() {
    let selected = selection();
    let cases = [
        "inherited",
        "supplied",
        "terminal",
        "failure-p2",
        "failure-p3",
    ];
    for case in cases {
        if !selected.is_child() {
            let output = TestProcess::current("component_output_uses_the_supplied_writer")
                .unwrap()
                .case(case)
                .quiet()
                .nocapture()
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{case}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty(), "{case}: {:?}", output.stderr);
            let body = output.stdout.strip_prefix(b"\nrunning 1 test\n").unwrap();
            let expected = if case == "inherited" {
                BYTES.repeat(3)
            } else {
                Vec::new()
            };
            assert!(body.starts_with(&expected), "{case}: {body:?}");
            assert!(
                body[expected.len()..].starts_with(b".\ntest result: ok."),
                "{case}: {body:?}"
            );
        } else if selected.runs_case(case) {
            run(output_case(case));
        }
    }
}

/// Exercises both output views on one Store and then uses it again after stream closure.
async fn output_case(case: &str) {
    let captured = Capture::default();
    let host = output_host(case, &captured).await;
    if let Some(generation) = case.strip_prefix("failure-") {
        let errors = host
            .outcome(json!({"op":"output","generation":generation,"bytes":BYTES}))
            .await
            .unwrap_err();
        assert_eq!(
            errors.len(),
            1,
            "one existing callback diagnostic: {errors:?}"
        );
        assert!(captured.0.lock().unwrap().is_empty());
        return;
    }
    for generation in ["p2", "p3", "p2"] {
        let observed = host
            .request(json!({"op":"output","generation":generation,"bytes":BYTES}))
            .await;
        assert_eq!(
            observed["terminal"],
            case == "terminal",
            "{case} {generation}"
        );
    }
    if case != "inherited" {
        assert_eq!(*captured.0.lock().unwrap(), BYTES.repeat(3));
    }
    let observed = host.request(json!({"op":"environment"})).await;
    assert!(observed["p2"][1].as_str().is_some());
}

/// Selects a real inherited, supplied or failing sink without changing the host lifecycle.
async fn output_host(case: &str, captured: &Capture) -> Harness {
    let path = std::env::var(COMPONENT).unwrap();
    if case == "inherited" {
        Harness::load(Path::new(&path)).await
    } else {
        let writer: Box<dyn Write + Send> = if case.starts_with("failure") {
            Box::new(Refused)
        } else {
            Box::new(captured.clone())
        };
        Harness::factory(artifact_factory_with_stdout(
            path,
            super::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: "installed-component-host".into(),
                agent_dir: "/controlled-agent".into(),
            },
            writer,
            case == "terminal",
        ))
        .await
    }
}
