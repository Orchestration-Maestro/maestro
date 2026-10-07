mod support;
use maestro_credentials::*;
use maestro_models::*;
use std::sync::Arc;
use support::*;

#[test]
fn private_files_and_new_parents_use_restricted_modes() {
    let scratch = Scratch::new();
    let path = scratch.0.join("new/nested/credentials.json");
    let storage = Arc::new(FileCredentialStorage::new(path.clone()).unwrap());
    let credentials = owner(storage);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    credentials
        .set(
            "chosen",
            Credential::ApiKey {
                value: secret("private"),
            },
            &Cancellation::new(),
        )
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for parent in [scratch.0.join("new"), scratch.0.join("new/nested")] {
            assert_eq!(
                std::fs::metadata(parent).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let existing = scratch.0.join("existing");
        std::fs::create_dir(&existing).unwrap();
        std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o755)).unwrap();
        let other = owner(Arc::new(
            FileCredentialStorage::new(existing.join("credentials.json")).unwrap(),
        ));
        other
            .set(
                "chosen",
                Credential::ApiKey {
                    value: secret("private"),
                },
                &Cancellation::new(),
            )
            .unwrap();
        assert_eq!(
            std::fs::metadata(existing).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let reopened = owner(Arc::new(FileCredentialStorage::new(path.clone()).unwrap()));
        reopened
            .set(
                "second",
                Credential::ApiKey {
                    value: secret("private"),
                },
                &Cancellation::new(),
            )
            .unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn independent_file_writers_preserve_unrelated_providers() {
    use std::io::{BufRead, Write};
    if let Some(path) = std::env::var_os("MAESTRO_TEST_WRITER_PATH") {
        let credentials = owner(Arc::new(FileCredentialStorage::new(path.into()).unwrap()));
        println!("READY");
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "GO");
        println!("WAITING");
        std::io::stdout().flush().unwrap();
        credentials
            .set(
                "child",
                Credential::ApiKey {
                    value: secret("child-key"),
                },
                &Cancellation::new(),
            )
            .unwrap();
        return;
    }
    let scratch = Scratch::new();
    let path = scratch.0.join("credentials.json");
    let storage = FileCredentialStorage::new(path.clone()).unwrap();
    replace(&storage, r#"{"alien":{"unknown":[true,3]}}"#);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "independent_file_writers_preserve_unrelated_providers",
            "--nocapture",
        ])
        .env("MAESTRO_TEST_WRITER_PATH", &path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert!(stdout.read_line(&mut line).unwrap() > 0);
        if line.trim() == "READY" {
            break;
        }
    }
    storage
        .transact(&Cancellation::new(), &mut |bytes| {
            child.stdin.as_mut().unwrap().write_all(b"GO\n").unwrap();
            loop {
                line.clear();
                assert!(stdout.read_line(&mut line).unwrap() > 0);
                if line.trim() == "WAITING" {
                    break;
                }
            }
            let independent = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            assert!(matches!(
                independent.try_lock(),
                Err(std::fs::TryLockError::WouldBlock)
            ));
            let mut data: serde_json::Value =
                serde_json::from_str(bytes.unwrap().expose()).unwrap();
            data["parent"] = serde_json::json!({"type":"api_key", "key":"parent-key"});
            Ok(Some(secret(&data.to_string())))
        })
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_str(&bytes(&storage)).unwrap();
    assert_eq!(result["alien"], serde_json::json!({"unknown":[true,3]}));
    assert_eq!(result["parent"]["key"], "parent-key");
    assert_eq!(result["child"]["key"], "child-key");
}

#[test]
fn explicit_locations_do_not_discover_ambient_installations() {
    let scratch = Scratch::new();
    let unrelated_home = scratch.0.join("unrelated-home");
    let unrelated_config = scratch.0.join("unrelated-config");
    std::fs::create_dir(&unrelated_home).unwrap();
    std::fs::create_dir(&unrelated_config).unwrap();
    std::fs::write(unrelated_home.join("credentials.json"), "untouched-home").unwrap();
    std::fs::write(
        unrelated_config.join("credentials.json"),
        "untouched-config",
    )
    .unwrap();
    let explicit = scratch.0.join("explicit/credentials.json");
    let file = FileCredentialStorage::new(explicit.clone()).unwrap();
    let _native = NativeSecretResolver::new(scratch.0.clone()).unwrap();
    assert!(!explicit.exists());
    let credentials = owner(Arc::new(file));
    assert!(credentials.list().is_empty());
    let memory = owner(memory());
    memory
        .set(
            "chosen",
            Credential::ApiKey {
                value: secret("literal"),
            },
            &Cancellation::new(),
        )
        .unwrap();
    assert_eq!(request(memory, "chosen").0.err(), None);
    assert_eq!(
        std::fs::read_to_string(unrelated_home.join("credentials.json")).unwrap(),
        "untouched-home"
    );
    assert_eq!(
        std::fs::read_to_string(unrelated_config.join("credentials.json")).unwrap(),
        "untouched-config"
    );
    assert_eq!(std::fs::read_to_string(explicit).unwrap(), "{}");
    assert_eq!(
        FileCredentialStorage::new("relative".into()).err(),
        Some(CredentialError::InvalidPath)
    );
    assert_eq!(
        NativeSecretResolver::new("relative".into()).err(),
        Some(CredentialError::InvalidPath)
    );
}

#[test]
fn native_helpers_are_lazy_private_and_directory_bound() {
    if let Some(root) = std::env::var_os("MAESTRO_TEST_HELPER_ROOT") {
        let root = std::path::PathBuf::from(root);
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let native = Arc::new(NativeSecretResolver::new(first.clone()).unwrap());
        let command =
            "!printf 'STDOUT_SENTINEL'; printf 'STDERR_SENTINEL' >&2; printf x >> invocations";
        let mut inputs = options();
        inputs.secrets = native.clone();
        let storage = Arc::new(MemoryCredentialStorage::new(Some(secret(
            &serde_json::json!({"chosen":{"type":"api_key","key":command}}).to_string(),
        ))));
        let credentials = Arc::new(
            Credentials::new(Arc::new(ReadOnlyCredentialStorage::new(storage)), inputs).unwrap(),
        );
        assert_eq!(credentials.list(), vec!["chosen"]);
        assert!(credentials.status("chosen").configured);
        assert!(!first.join("invocations").exists());
        credentials.set_runtime_auth("chosen".into(), Some(auth("override")));
        assert_eq!(request(credentials.clone(), "chosen").0.err(), None);
        assert!(!first.join("invocations").exists());
        credentials.set_runtime_auth("chosen".into(), None);
        let (result, adapter) = request(credentials, "chosen");
        assert_eq!(result.err(), None);
        assert_secret(&adapter.resolutions()[0], "STDOUT_SENTINEL", "stored");
        assert_eq!(
            adapter.calls()[0].options.api_key.as_deref(),
            Some("STDOUT_SENTINEL")
        );
        let same = NativeSecretResolver::new(first.clone()).unwrap();
        assert_eq!(
            block_on(same.resolve(secret(command), Cancellation::new()))
                .unwrap()
                .unwrap()
                .expose(),
            "STDOUT_SENTINEL"
        );
        assert_eq!(
            std::fs::read_to_string(first.join("invocations")).unwrap(),
            "x"
        );
        let other = NativeSecretResolver::new(second.clone()).unwrap();
        assert_eq!(
            block_on(other.resolve(secret(command), Cancellation::new()))
                .unwrap()
                .unwrap()
                .expose(),
            "STDOUT_SENTINEL"
        );
        assert_eq!(
            std::fs::read_to_string(second.join("invocations")).unwrap(),
            "x"
        );
        for (resolver, expected) in [(&same, &first), (&other, &second)] {
            assert_eq!(
                block_on(resolver.resolve(secret("!pwd"), Cancellation::new()))
                    .unwrap()
                    .unwrap()
                    .expose(),
                expected.to_str().unwrap()
            );
        }
        assert_eq!(
            block_on(same.resolve(
                secret("!printf 'literal\\n' | tr '[:lower:]' '[:upper:]'"),
                Cancellation::new()
            ))
            .unwrap()
            .unwrap()
            .expose(),
            "LITERAL"
        );
        for command in [
            "!printf 'OUTPUT_FAILURE_SENTINEL'; printf 'ERROR_FAILURE_SENTINEL' >&2; exit 2",
            "!maestro-synthetic-missing-command",
            "!printf '\\377'",
            "!printf ''",
        ] {
            assert!(
                block_on(same.resolve(secret(command), Cancellation::new()))
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(
            block_on(same.resolve(secret("MAESTRO_TEST_NATIVE_VALUE"), Cancellation::new()))
                .unwrap()
                .unwrap()
                .expose(),
            "native-environment"
        );
        return;
    }
    let scratch = Scratch::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_helpers_are_lazy_private_and_directory_bound",
            "--nocapture",
        ])
        .env("MAESTRO_TEST_HELPER_ROOT", &scratch.0)
        .env("MAESTRO_TEST_NATIVE_VALUE", "native-environment")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "child test failed: {stdout} {stderr}"
    );
    for sentinel in [
        "STDOUT_SENTINEL",
        "STDERR_SENTINEL",
        "OUTPUT_FAILURE_SENTINEL",
        "ERROR_FAILURE_SENTINEL",
    ] {
        assert!(!stdout.contains(sentinel));
        assert!(!stderr.contains(sentinel));
    }
}

#[cfg(unix)]
#[test]
fn native_helper_stdout_limit_accepts_boundary_and_reaps_overflow() {
    let scratch = Scratch::new();
    let native = NativeSecretResolver::new(scratch.0.clone()).unwrap();
    std::fs::write(scratch.0.join("boundary"), vec![b'x'; 1_048_576]).unwrap();
    let boundary = block_on(native.resolve(secret("!/bin/cat boundary"), Cancellation::new()))
        .unwrap()
        .unwrap();
    assert_eq!(boundary.expose(), "x".repeat(1_048_576));

    std::fs::write(scratch.0.join("overflow"), vec![b'x'; 1_048_577]).unwrap();
    let overflow = block_on(native.resolve(
        secret("!echo $$ > helper.pid; /bin/cat overflow; exec /bin/sleep 1"),
        Cancellation::new(),
    ))
    .unwrap();
    assert!(overflow.is_none(), "oversized stdout must yield no value");
    let pid = std::fs::read_to_string(scratch.0.join("helper.pid")).unwrap();
    assert!(
        !std::process::Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success(),
        "overflow helper must be reaped before resolution completes"
    );
}

#[cfg(unix)]
#[test]
fn native_helper_uses_absolute_shell_despite_shadowed_path() {
    use std::os::unix::fs::PermissionsExt;
    if let Some(root) = std::env::var_os("MAESTRO_TEST_SHELL_ROOT") {
        let native = NativeSecretResolver::new(root.into()).unwrap();
        let result = block_on(native.resolve(secret("!printf real-shell"), Cancellation::new()))
            .unwrap()
            .unwrap();
        assert_eq!(result.expose(), "real-shell");
        assert!(!result.expose().contains("SHADOW_SHELL_SENTINEL"));
        return;
    }
    let scratch = Scratch::new();
    let shadow = scratch.0.join("sh");
    std::fs::write(&shadow, "#!/bin/sh\nprintf SHADOW_SHELL_SENTINEL\n").unwrap();
    std::fs::set_permissions(&shadow, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_helper_uses_absolute_shell_despite_shadowed_path",
            "--nocapture",
        ])
        .env("MAESTRO_TEST_SHELL_ROOT", &scratch.0)
        .env("PATH", &scratch.0)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "child test failed: {stdout} {stderr}"
    );
    assert!(!stdout.contains("SHADOW_SHELL_SENTINEL"));
    assert!(!stderr.contains("SHADOW_SHELL_SENTINEL"));
}

#[cfg(unix)]
#[test]
fn updating_existing_file_restricts_permissions_and_replaces_contents() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let path = scratch.0.join("credentials.json");
    std::fs::write(&path, r#"{"chosen":{"type":"api_key","key":"old"}}"#).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let credentials = owner(Arc::new(FileCredentialStorage::new(path.clone()).unwrap()));
    credentials
        .set(
            "chosen",
            Credential::ApiKey {
                value: secret("new"),
            },
            &Cancellation::new(),
        )
        .unwrap();
    let current: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(current["chosen"]["key"], "new");
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
