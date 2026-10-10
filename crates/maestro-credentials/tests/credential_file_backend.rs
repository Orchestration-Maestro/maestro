//! Native credential file backend through the public interface.
#[cfg(test)]
mod support;
#[cfg(test)]
mod tests {

    use super::support::{TempDir, is_child, run_child};
    use maestro_credentials::{
        ApiKeyCredential, AuthCredential, AuthStorage, AuthStorageBackend, FileAuthStorageBackend,
    };
    use std::fs::{File, TryLockError};
    use std::time::{Duration, Instant};

    /// Stored API key credential.
    fn api_key(key: &str) -> AuthCredential {
        AuthCredential::ApiKey(ApiKeyCredential {
            key: key.to_owned(),
        })
    }

    /// Pretty text of one API key record per entry.
    fn pretty(records: &[(&str, &str)]) -> String {
        let body: Vec<String> = records
            .iter()
            .map(|(name, key)| {
                format!(
                    "  \"{name}\": {{\n    \"type\": \"api_key\",\n    \"key\": \"{key}\"\n  }}"
                )
            })
            .collect();
        format!("{{\n{}\n}}", body.join(",\n"))
    }

    /// Mode bits of a path.
    #[cfg(unix)]
    fn mode(path: &str) -> u32 {
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(path).unwrap().permissions())
            & 0o777
    }

    /// Set the mode bits of a path.
    #[cfg(unix)]
    fn set_mode(path: &str, bits: u32) {
        std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(bits)).unwrap();
    }

    /// Hold the exclusive lock of the sidecar for `path`.
    fn hold_lock(path: &str) -> File {
        let file = File::options()
            .create(true)
            .write(true)
            .truncate(false)
            .open(format!("{path}.lock"))
            .unwrap();
        file.lock().unwrap();
        file
    }

    #[cfg(unix)]
    #[test]
    fn missing_credentials_file_is_initialized_privately() {
        let dir = TempDir::new("initialize");
        let path = dir.path("nested/agent/auth.json");
        let storage = AuthStorage::create(&path);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&dir.path("nested")), 0o700);
        assert_eq!(mode(&dir.path("nested/agent")), 0o700);
        assert!(storage.list().is_empty());
        assert!(storage.drain_errors().is_empty());
        assert!(std::path::Path::new(&format!("{path}.lock")).exists());
    }

    #[cfg(unix)]
    #[test]
    fn zero_byte_file_left_by_interrupted_creation_stays_usable() {
        let dir = TempDir::new("zero-byte");
        let path = dir.path("agent/auth.json");
        std::fs::create_dir(dir.path("agent")).unwrap();
        set_mode(&dir.path("agent"), 0o755);
        std::fs::write(&path, "").unwrap();
        set_mode(&path, 0o644);
        let storage = AuthStorage::create(&path);
        assert!(storage.drain_errors().is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
        assert_eq!((mode(&path), mode(&dir.path("agent"))), (0o644, 0o755));
        storage.set("openai", api_key("o"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("openai", "o")])
        );
        assert_eq!(mode(&path), 0o600);
    }

    #[test]
    fn relative_credentials_path_follows_working_directory() {
        if is_child() {
            let storage = AuthStorage::create("auth.json");
            storage.set("openai", api_key("o"));
            assert!(storage.drain_errors().is_empty());
            return;
        }
        let dir = TempDir::new("relative");
        run_child(
            "tests::relative_credentials_path_follows_working_directory",
            dir.root(),
            &[],
        );
        assert_eq!(
            std::fs::read_to_string(dir.path("auth.json")).unwrap(),
            pretty(&[("openai", "o")])
        );
    }

    #[test]
    fn competing_creator_cannot_truncate_locked_winner() {
        let dir = TempDir::new("competing");
        let path = dir.path("auth.json");
        let holder = hold_lock(&path);
        let storage = AuthStorage::create(&path);
        let errors = storage.drain_errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0].downcast_ref::<TryLockError>(),
            Some(TryLockError::WouldBlock)
        ));
        assert!(!std::path::Path::new(&path).exists());
        std::fs::write(&path, pretty(&[("winner", "w")])).unwrap();
        drop(holder);
        storage.set("late", api_key("l"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("winner", "w")])
        );
        storage.reload();
        assert_eq!(storage.list(), ["winner"]);
        storage.set("late", api_key("l"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("winner", "w"), ("late", "l")])
        );
    }

    #[test]
    fn contended_lock_retries_then_records_failure() {
        let dir = TempDir::new("contended");
        let path = dir.path("auth.json");
        let storage = AuthStorage::create(&path);
        let initial = std::fs::read_to_string(&path).unwrap();
        let holder = hold_lock(&path);
        let started = Instant::now();
        storage.set("openai", api_key("o"));
        assert!(started.elapsed() >= Duration::from_millis(180));
        let errors = storage.drain_errors();
        assert!(matches!(
            errors[0].downcast_ref::<TryLockError>(),
            Some(TryLockError::WouldBlock)
        ));
        assert_eq!(storage.list(), ["openai"]);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), initial);
        storage.reload();
        assert_eq!(storage.drain_errors().len(), 1);
        drop(holder);
        storage.set("google", api_key("g"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), initial);
        storage.reload();
        storage.set("google", api_key("g"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("google", "g")])
        );
    }

    #[test]
    fn lock_acquisition_failure_skips_the_callback() {
        let dir = TempDir::new("acquire-failure");
        let path = dir.path("auth.json");
        std::fs::create_dir(format!("{path}.lock")).unwrap();
        let backend = FileAuthStorageBackend::new(&path);
        let result = backend.with_lock(&mut |_| panic!("callback must not run"));
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn missing_symlink_target_is_initialized_and_live_link_is_kept() {
        let dir = TempDir::new("symlink");
        let target = dir.path("real.json");
        let link = dir.path("auth.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let storage = AuthStorage::create(&link);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "{}");
        assert_eq!(mode(&target), 0o600);
        assert!(storage.drain_errors().is_empty());

        let live = dir.path("live.json");
        let kept = dir.path("kept.json");
        std::fs::write(&kept, pretty(&[("a", "1")])).unwrap();
        std::os::unix::fs::symlink(&kept, &live).unwrap();
        let storage = AuthStorage::create(&live);
        assert_eq!(storage.list(), ["a"]);
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            pretty(&[("a", "1")])
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_link_chain_creates_final_target_and_existing_file_is_kept() {
        let dir = TempDir::new("chain");
        let target = dir.path("final.json");
        let middle = dir.path("middle.json");
        let link = dir.path("auth.json");
        std::os::unix::fs::symlink("final.json", &middle).unwrap();
        std::os::unix::fs::symlink("middle.json", &link).unwrap();
        let storage = AuthStorage::create(&link);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "{}");
        assert_eq!(mode(&target), 0o600);
        assert!(storage.drain_errors().is_empty());

        let winner = dir.path("winner.json");
        std::fs::write(&winner, pretty(&[("a", "1")])).unwrap();
        let storage = AuthStorage::create(&winner);
        assert_eq!(storage.list(), ["a"]);
        assert_eq!(
            std::fs::read_to_string(&winner).unwrap(),
            pretty(&[("a", "1")])
        );
    }

    #[test]
    fn callback_read_and_write_failures_release_the_lock() {
        let dir = TempDir::new("release");
        let path = dir.path("auth.json");
        let backend = FileAuthStorageBackend::new(&path);
        let free = || {
            let sidecar = File::options()
                .write(true)
                .open(format!("{path}.lock"))
                .unwrap();
            assert!(sidecar.try_lock().is_ok());
        };
        let error = backend
            .with_lock(&mut |_| Err("callback failed".into()))
            .unwrap_err();
        assert_eq!(error.to_string(), "callback failed");
        free();
        let result = backend.with_lock(&mut |_| {
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            Ok(Some("text".into()))
        });
        assert!(result.is_err());
        free();
        let result = backend.with_lock(&mut |_| panic!("read failure precedes the callback"));
        assert!(result.is_err());
        free();
    }

    #[cfg(unix)]
    #[test]
    fn writes_restrict_existing_file_to_owner() {
        let dir = TempDir::new("restrict");
        let path = dir.path("auth.json");
        std::fs::write(&path, pretty(&[("a", "1")])).unwrap();
        set_mode(&path, 0o644);
        let storage = AuthStorage::create(&path);
        assert_eq!(mode(&path), 0o644);
        storage.set("b", api_key("2"));
        assert_eq!(mode(&path), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn removed_or_absent_records_rewrite_the_document() {
        let dir = TempDir::new("rewrite");
        let path = dir.path("auth.json");
        std::fs::write(&path, "{\"a\":{\"type\":\"api_key\",\"key\":\"1\"}}").unwrap();
        let storage = AuthStorage::create(&path);
        storage.remove("absent");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("a", "1")])
        );
        std::fs::remove_file(&path).unwrap();
        storage.set("b", api_key("2"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            pretty(&[("b", "2")])
        );
        assert_eq!(mode(&path), 0o600);
        assert_eq!(storage.list(), ["a", "b"]);
    }

    #[test]
    fn lossy_utf8_and_integer_times_survive_rewrite() {
        let dir = TempDir::new("lossy");
        let path = dir.path("auth.json");
        let mut bytes = b"{\"p\":{\"type\":\"api_key\",\"key\":\"k".to_vec();
        bytes.extend([0xFF, 0xC3]);
        bytes.extend(b"z\"},\"o\":{\"type\":\"oauth\",\"refresh\":\"r\",\"access\":\"a\",\"expires\":1730000000000,\"zeta\":1,\"alpha\":{\"y\":1,\"x\":2}}}");
        std::fs::write(&path, bytes).unwrap();
        let storage = AuthStorage::create(&path);
        assert_eq!(storage.get("p"), Some(api_key("k\u{fffd}\u{fffd}z")));
        storage.set("other", api_key("x"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"key\": \"k\u{fffd}\u{fffd}z\""));
        assert!(text.contains("\"expires\": 1730000000000,\n    \"zeta\": 1,\n    \"alpha\": {\n      \"y\": 1,\n      \"x\": 2\n    }"));
    }
}
