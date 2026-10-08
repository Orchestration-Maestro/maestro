use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use maestro_models::{find_env_keys, get_env_api_key};

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub struct Directory(pub PathBuf);

impl Directory {
    pub fn new() -> TestResult<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "maestro-environment-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        assert!(std::fs::remove_dir_all(&self.0).is_ok());
    }
}

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
    test: &str,
    directory: &Directory,
    variables: &[(OsString, OsString)],
    provider: &str,
    expected: (Option<Vec<&str>>, Option<&str>),
) -> TestResult {
    let mut command = Command::new(std::env::current_exe()?);
    command.env_clear().current_dir(&directory.0);
    command
        .env("HOME", &directory.0)
        .env("USERPROFILE", &directory.0);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let output = command
        .args(["--exact", test, "--nocapture"])
        .envs(variables.iter().cloned())
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
