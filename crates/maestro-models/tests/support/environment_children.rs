#![cfg(not(target_arch = "wasm32"))]
use std::ffi::OsString;

use maestro_models::{find_env_keys, get_env_api_key};

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub use maestro_test_directory::Scratch as Directory;

pub fn child() -> TestResult<bool> {
    let Ok(expectation) = std::env::var("MAESTRO_ENV_EXPECTATION") else {
        return Ok(false);
    };
    let (provider, names, value): (String, Option<Vec<String>>, Option<String>) =
        serde_json::from_str(&expectation)?;
    for _ in 0..2 {
        assert_eq!(find_env_keys(&provider), names);
        assert_eq!(get_env_api_key(&provider), value);
        assert_eq!(
            maestro_models::records::stream::get_env_api_key(&provider),
            value
        );
    }
    if let Some(path) = std::env::var_os("MAESTRO_ENV_REMOVE") {
        std::fs::remove_file(path)?;
        assert_eq!(get_env_api_key(&provider), value);
    }
    if let Some(path) = std::env::var_os("MAESTRO_ENV_CREATE") {
        std::fs::write(path, b"not credential contents")?;
        assert_eq!(get_env_api_key(&provider), value);
    }
    Ok(true)
}

pub fn run(
    process: maestro_test_process::TestProcess,
    directory: &Directory,
    variables: &[(OsString, OsString)],
    provider: &str,
    expected: (Option<Vec<&str>>, Option<&str>),
) -> TestResult {
    let test = process.name().to_owned();
    let output = process
        .current_dir(directory.path())
        .env("HOME", directory.path())
        .env("USERPROFILE", directory.path())
        .nocapture()
        .envs(variables.iter().map(|(key, value)| (key, value)))
        .env(
            "MAESTRO_ENV_EXPECTATION",
            serde_json::to_string(&(provider, expected.0, expected.1))?,
        )
        .output()?;
    assert!(
        output.status.success(),
        "{test} child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

pub fn vars(values: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    values
        .iter()
        .map(|(key, value)| ((*key).into(), (*value).into()))
        .collect()
}
