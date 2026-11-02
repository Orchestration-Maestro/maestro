//! Offline locale data generation through the development command.
#![cfg(test)]

use maestro_test_directory::Scratch;
use std::{ffi::OsString, fs, path::Path};

/// The five generated assets consumed by native date-time formatting.
const ASSETS: [&str; 5] = [
    "index.tsv",
    "recipes.tsv",
    "hours.tsv",
    "calendars.tsv",
    "LICENSE",
];
/// Recorded, hash-pinned generator inputs.
const INPUTS: [&str; 6] = [
    "manifest.json",
    "oracle.json",
    "source-locales.json",
    "timeData.json",
    "calendarPreferenceData.json",
    "LICENSE",
];
/// Relative path of the date-time data owner.
const DATA: &str = "crates/maestro-text/data/date-time";

/// Stage only recorded inputs, without generated outputs.
fn fixture() -> Scratch {
    let root = Scratch::new().unwrap();
    let destination = root.path().join(DATA).join("inputs");
    fs::create_dir_all(&destination).unwrap();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-text/data/date-time/inputs");
    for name in INPUTS {
        fs::copy(source.join(name), destination.join(name)).unwrap();
    }
    root
}

/// Invoke the command without spawning a process or accessing a platform runtime.
fn run(root: &Path, check: bool) -> std::io::Result<std::process::ExitCode> {
    let mut args = vec![OsString::from("date-time-data")];
    if check {
        args.push("--check".into());
    }
    maestro_tooling::run(&args, Path::new("unused-cargo"), root)
}

/// The recorded oracle and supplemental JSON reproduce all shipped asset bytes.
#[test]
fn date_time_data_regenerates_recorded_assets() {
    let root = fixture();
    assert_eq!(
        run(root.path(), false).unwrap(),
        std::process::ExitCode::SUCCESS
    );
    let expected = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-text/data/date-time");
    for name in ASSETS {
        assert_eq!(
            fs::read(root.path().join(DATA).join(name)).unwrap(),
            fs::read(expected.join(name)).unwrap(),
            "{name}"
        );
    }
    assert_eq!(
        run(root.path(), true).unwrap(),
        std::process::ExitCode::SUCCESS
    );
}

/// Check mode reports stale data without rewriting it; regeneration repairs it.
#[test]
fn date_time_data_check_preserves_stale_assets() {
    let root = fixture();
    run(root.path(), false).unwrap();
    let path = root.path().join(DATA).join("recipes.tsv");
    fs::write(&path, b"stale").unwrap();

    assert!(run(root.path(), true).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"stale");
    run(root.path(), false).unwrap();
    run(root.path(), true).unwrap();
}

/// A changed input or replaced manifest fails before overwriting any existing asset.
#[test]
fn date_time_data_rejects_changed_inputs_before_writing() {
    for input in INPUTS {
        let root = fixture();
        let directory = root.path().join(DATA);
        for asset in ASSETS {
            fs::write(directory.join(asset), b"unchanged").unwrap();
        }
        let path = directory.join("inputs").join(input);
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b'\n');
        fs::write(path, bytes).unwrap();

        assert_eq!(
            run(root.path(), false).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData,
            "{input}"
        );
        for asset in ASSETS {
            assert_eq!(
                fs::read(directory.join(asset)).unwrap(),
                b"unchanged",
                "{input}: {asset}"
            );
        }
    }
}
