//! The prepared-bundle inputs a setup child must not inherit.

use maestro_test_process::{TestProcess, without_prepared_bundles};

/// The inputs a setup child reads from the run that starts it.
const INPUTS: [&str; 3] = ["TEST_PREPARE_BASE", "TEST_PREPARE_OUTPUT", "TEST_ARCHIVE"];

/// The ignored probe the helper command re-runs; it reports a bundle input it inherited.
#[test]
#[ignore = "runs only inside a controlled child"]
fn prepared_bundle_probe() {
    for name in INPUTS {
        assert!(
            std::env::var_os(name).is_none(),
            "the setup child inherited {name}"
        );
    }
}

#[test]
fn prepared_bundle_inputs_never_reach_the_setup_child() {
    let mut command = TestProcess::child(prepared_bundle_probe)
        .unwrap()
        .command()
        .unwrap();
    for name in INPUTS {
        command.env(name, "a bundle the run exported");
    }
    without_prepared_bundles(&mut command);

    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
