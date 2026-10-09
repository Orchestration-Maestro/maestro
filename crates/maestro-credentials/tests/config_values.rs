//! Configuration resolution through controlled and native operations.
#[cfg(test)]
mod tests {
    use maestro_credentials::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Serializes observations of the process-wide cache.
    static SERIAL: Mutex<()> = Mutex::new(());
    /// Controlled environment, outputs and executed commands.
    #[derive(Default)]
    struct Operations {
        /// Exact environment keys.
        env: RefCell<HashMap<String, String>>,
        /// Next command output.
        output: RefCell<Option<Vec<u8>>>,
        /// Commands executed in order.
        calls: RefCell<Vec<String>>,
    }
    impl ConfigValueOperations for Operations {
        fn environment(&self, name: &str) -> Option<String> {
            self.env.borrow().get(name).cloned()
        }
        fn execute(&self, command: &str) -> Option<Vec<u8>> {
            self.calls.borrow_mut().push(command.to_owned());
            self.output.borrow().clone()
        }
    }
    #[test]
    fn config_values_choose_exact_environment_or_literal() {
        let operations = Operations::default();
        for resolve in [resolve_config_value, resolve_config_value_uncached] {
            for literal in ["", "literal", " key", "KEY"] {
                assert_eq!(resolve(literal, &operations).as_deref(), Some(literal));
            }
            for value in ["first", "second", "", " \n "] {
                operations
                    .env
                    .borrow_mut()
                    .insert("KEY".into(), value.into());
                assert_eq!(
                    resolve("KEY", &operations).as_deref(),
                    Some(if value.is_empty() { "KEY" } else { value })
                );
                assert_eq!(resolve("key", &operations).as_deref(), Some("key"));
            }
            operations.env.borrow_mut().clear();
        }
        assert!(operations.calls.borrow().is_empty());
    }
    #[test]
    fn config_prefix_removes_only_one_bang() {
        let operations = Operations::default();
        operations
            .env
            .borrow_mut()
            .insert("!!command".into(), "wrong".into());
        *operations.output.borrow_mut() = Some(b"key".to_vec());
        assert_eq!(
            resolve_config_value_uncached("!!command", &operations).as_deref(),
            Some("key")
        );
        assert_eq!(&*operations.calls.borrow(), &["!command"]);
        assert_eq!(
            resolve_config_value_uncached(" !command", &operations).as_deref(),
            Some(" !command")
        );
    }
    #[test]
    fn command_output_trims_only_outer_whitespace() {
        let operations = Operations::default();
        for character in [
            '\u{9}', '\u{a}', '\u{b}', '\u{c}', '\u{d}', ' ', '\u{a0}', '\u{1680}', '\u{2000}',
            '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}',
            '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}',
            '\u{3000}', '\u{feff}',
        ] {
            *operations.output.borrow_mut() =
                Some(format!("{character}value{character}").into_bytes());
            assert_eq!(
                resolve_config_value_uncached("!trim", &operations).as_deref(),
                Some("value"),
                "U+{:04X}",
                u32::from(character)
            );
        }
        for value in [
            "\u{85}value\u{85}",
            "\u{1c}value\u{1c}",
            "\u{200b}value\u{200b}",
            "🔑",
            "first\nsecond",
        ] {
            *operations.output.borrow_mut() = Some(value.as_bytes().to_vec());
            assert_eq!(
                resolve_config_value_uncached("!trim", &operations).as_deref(),
                Some(value)
            );
        }
        for value in ["", " \n\t"] {
            *operations.output.borrow_mut() = Some(value.as_bytes().to_vec());
            assert_eq!(resolve_config_value_uncached("!trim", &operations), None);
        }
    }
    #[test]
    fn maestro_command_cache_uses_full_command() {
        let _serial = SERIAL.lock().unwrap();
        clear_config_value_cache();
        let operations = Operations::default();
        *operations.output.borrow_mut() = Some(b"first".to_vec());
        assert_eq!(
            resolve_config_value("!same", &operations).as_deref(),
            Some("first")
        );
        *operations.output.borrow_mut() = Some(b"second".to_vec());
        assert_eq!(
            resolve_config_value("!same", &operations).as_deref(),
            Some("first")
        );
        assert_eq!(
            resolve_config_value("!same ", &operations).as_deref(),
            Some("second")
        );
        clear_config_value_cache();
        assert_eq!(
            resolve_config_value("!same", &operations).as_deref(),
            Some("second")
        );
        assert_eq!(&*operations.calls.borrow(), &["same", "same ", "same"]);
    }
    #[test]
    fn command_failures_are_cached() {
        let _serial = SERIAL.lock().unwrap();
        clear_config_value_cache();
        let operations = Operations::default();
        assert_eq!(resolve_config_value("!failed", &operations), None);
        *operations.output.borrow_mut() = Some(b"second".to_vec());
        assert_eq!(resolve_config_value("!failed", &operations), None);
        assert_eq!(
            resolve_config_value("!failed ", &operations).as_deref(),
            Some("second")
        );
        clear_config_value_cache();
        assert_eq!(
            resolve_config_value("!failed", &operations).as_deref(),
            Some("second")
        );
        assert_eq!(operations.calls.borrow().len(), 3);
    }
    #[test]
    fn config_throwing_values_keep_empty_literals_and_error_text() {
        let operations = Operations::default();
        assert_eq!(
            resolve_config_value_or_throw("", "key", &operations).unwrap(),
            ""
        );
        assert_eq!(
            resolve_config_value_or_throw("!!bad", "provider \"x\"\nkey", &operations)
                .unwrap_err()
                .to_string(),
            "Failed to resolve provider \"x\"\nkey from shell command: !bad"
        );
        *operations.output.borrow_mut() = Some(b" key ".to_vec());
        assert_eq!(
            resolve_config_value_or_throw("!good", "key", &operations).unwrap(),
            "key"
        );
    }
    #[test]
    fn uncached_resolution_leaves_command_cache_unchanged() {
        let _serial = SERIAL.lock().unwrap();
        clear_config_value_cache();
        let operations = Operations::default();
        *operations.output.borrow_mut() = Some(b"cached".to_vec());
        assert_eq!(
            resolve_config_value("!fresh", &operations).as_deref(),
            Some("cached")
        );
        *operations.output.borrow_mut() = Some(b"fresh".to_vec());
        assert_eq!(
            resolve_config_value_uncached("!fresh", &operations).as_deref(),
            Some("fresh")
        );
        *operations.output.borrow_mut() = Some(b"fresh-again".to_vec());
        assert_eq!(
            resolve_config_value_or_throw("!fresh", "key", &operations).unwrap(),
            "fresh-again"
        );
        assert_eq!(
            resolve_config_value("!fresh", &operations).as_deref(),
            Some("cached")
        );
        assert_eq!(operations.calls.borrow().len(), 3);
    }
    /// Build ordered headers without imposing HTTP policy.
    fn headers(entries: &[(&str, &str)]) -> indexmap::IndexMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect()
    }
    #[test]
    fn headers_cached_filter_only_empty_values() {
        let _serial = SERIAL.lock().unwrap();
        clear_config_value_cache();
        let operations = Operations::default();
        assert_eq!(resolve_headers(None, &operations), None);
        assert_eq!(resolve_headers(Some(&headers(&[])), &operations), None);
        assert_eq!(
            resolve_headers(Some(&headers(&[("A", ""), ("B", "!bad")])), &operations),
            None
        );
        *operations.output.borrow_mut() = Some(b"command".to_vec());
        let result = resolve_headers(
            Some(&headers(&[
                ("Z", ""),
                ("A", "one"),
                ("M", "!ok"),
                ("C", "tail"),
            ])),
            &operations,
        )
        .unwrap();
        assert_eq!(
            result.keys().map(String::as_str).collect::<Vec<_>>(),
            ["A", "M", "C"]
        );
        assert_eq!(
            result.values().map(String::as_str).collect::<Vec<_>>(),
            ["one", "command", "tail"]
        );
    }
    #[test]
    fn headers_fresh_keep_empty_values_and_bypass_cache() {
        let _serial = SERIAL.lock().unwrap();
        clear_config_value_cache();
        let operations = Operations::default();
        assert_eq!(
            resolve_headers_or_throw(None, "key", &operations).unwrap(),
            None
        );
        assert_eq!(
            resolve_headers_or_throw(Some(&headers(&[])), "key", &operations).unwrap(),
            None
        );
        assert_eq!(resolve_config_value("!header", &operations), None);
        assert!(
            resolve_headers_or_throw(Some(&headers(&[("A", "!header")])), "key", &operations)
                .is_err()
        );
        *operations.output.borrow_mut() = Some(b"fresh".to_vec());
        let result = resolve_headers_or_throw(
            Some(&headers(&[
                ("Z", ""),
                ("A", "one"),
                ("M", "!header"),
                ("C", "tail"),
            ])),
            "key",
            &operations,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            result.keys().map(String::as_str).collect::<Vec<_>>(),
            ["Z", "A", "M", "C"]
        );
        assert_eq!(
            result.values().map(String::as_str).collect::<Vec<_>>(),
            ["", "one", "fresh", "tail"]
        );
        assert_eq!(resolve_config_value("!header", &operations), None);
        assert_eq!(operations.calls.borrow().len(), 3);
    }
    #[test]
    fn headers_stop_at_first_failed_command() {
        let operations = Operations::default();
        let error = resolve_headers_or_throw(
            Some(&headers(&[("First", "!bad"), ("Later", "!side-effect")])),
            "provider \"fixture\"",
            &operations,
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Failed to resolve provider \"fixture\" header \"First\" from shell command: bad"
        );
        assert_eq!(&*operations.calls.borrow(), &["bad"]);
    }
    #[test]
    fn header_names_remain_literal_and_ordered() {
        let operations = Operations::default();
        let input = headers(&[
            ("z", "one"),
            ("A", "two"),
            ("a", "three"),
            ("__proto__", "four"),
            ("constructor", "five"),
            ("toString", "six"),
        ]);
        for result in [
            resolve_headers(Some(&input), &operations).unwrap(),
            resolve_headers_or_throw(Some(&input), "key", &operations)
                .unwrap()
                .unwrap(),
        ] {
            assert_eq!(
                result.keys().map(String::as_str).collect::<Vec<_>>(),
                ["z", "A", "a", "__proto__", "constructor", "toString"]
            );
            assert_eq!(
                result.values().map(String::as_str).collect::<Vec<_>>(),
                ["one", "two", "three", "four", "five", "six"]
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn native_helpers_support_pipes_and_multiline_output() {
        let operations =
            ProcessConfigValueOperations::new(|| unreachable!("Unix must avoid selection"));
        for (command, expected) in [
            ("printf hello | sed 's/$/-world/'", "hello-world"),
            ("printf 'first\nsecond\n'", "first\nsecond"),
            ("printf noise >&2; printf key", "key"),
        ] {
            assert_eq!(
                resolve_config_value_uncached(&format!("!{command}"), &operations).as_deref(),
                Some(expected)
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn command_empty_or_failed_output_returns_absence() {
        let operations = ProcessConfigValueOperations::new(|| unreachable!());
        for command in [
            "!printf ''",
            "!printf wrong; exit 7",
            "!kill -TERM $$",
            "!maestro_missing_helper_146",
            "!",
        ] {
            assert_eq!(
                resolve_config_value_uncached(command, &operations),
                None,
                "{command}"
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn native_helpers_decode_invalid_utf8_lossily() {
        let operations = ProcessConfigValueOperations::new(|| unreachable!());
        assert_eq!(
            resolve_config_value_uncached("!printf '\\377key'", &operations).as_deref(),
            Some("�key")
        );
        assert_eq!(
            resolve_config_value_uncached(
                "!printf '\\357\\273\\277key\\357\\273\\277'",
                &operations
            )
            .as_deref(),
            Some("key")
        );
    }
    #[cfg(unix)]
    #[test]
    fn native_helpers_work_inside_running_runtime() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let operations = ProcessConfigValueOperations::new(|| unreachable!());
            assert_eq!(
                resolve_config_value_uncached("!printf nested", &operations).as_deref(),
                Some("nested")
            );
        });
    }
    /// Reinvoke one test with isolated environment and working directory.
    #[cfg(unix)]
    fn child_test(name: &str, root: &std::path::Path) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("tests::{name}"), "--nocapture"])
            .env("MAESTRO_CONFIG_CHILD", "yes")
            .env("MAESTRO_CONFIG_VALUE", "inherited-value")
            .current_dir(root)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    /// Disposable fixture directory whose contents belong to this test.
    #[cfg(unix)]
    struct Directory(std::path::PathBuf);
    #[cfg(unix)]
    impl Directory {
        /// Create a distinct test directory.
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("maestro-config-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    #[cfg(unix)]
    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[cfg(unix)]
    #[test]
    fn native_helpers_inherit_environment_and_cwd() {
        if std::env::var_os("MAESTRO_CONFIG_CHILD").is_some() {
            let operations = ProcessConfigValueOperations::new(|| unreachable!());
            assert_eq!(
                resolve_config_value_uncached(
                    "!printf '%s' \"$MAESTRO_CONFIG_VALUE\"; cat relative-key",
                    &operations
                )
                .as_deref(),
                Some("inherited-value-relative")
            );
            return;
        }
        let root = Directory::new("inherit");
        std::fs::write(root.0.join("relative-key"), "-relative").unwrap();
        child_test("native_helpers_inherit_environment_and_cwd", &root.0);
    }
    #[cfg(unix)]
    #[test]
    fn config_cache_survives_adapter_and_cwd_changes() {
        if std::env::var_os("MAESTRO_CONFIG_CHILD").is_some() {
            let first = ProcessConfigValueOperations::new(|| unreachable!());
            let second = ProcessConfigValueOperations::new(|| unreachable!());
            let root = std::env::current_dir().unwrap();
            std::env::set_current_dir(root.join("a")).unwrap();
            assert_eq!(
                resolve_config_value("!cat key", &first).as_deref(),
                Some("from-a")
            );
            std::env::set_current_dir(root.join("b")).unwrap();
            assert_eq!(
                resolve_config_value("!cat key", &second).as_deref(),
                Some("from-a")
            );
            assert_eq!(
                resolve_config_value_uncached("!cat key", &second).as_deref(),
                Some("from-b")
            );
            clear_config_value_cache();
            assert_eq!(
                resolve_config_value("!cat key", &second).as_deref(),
                Some("from-b")
            );
            return;
        }
        let root = Directory::new("cwd");
        for (directory, value) in [("a", "from-a"), ("b", "from-b")] {
            std::fs::create_dir(root.0.join(directory)).unwrap();
            std::fs::write(root.0.join(directory).join("key"), value).unwrap();
        }
        child_test("config_cache_survives_adapter_and_cwd_changes", &root.0);
    }
    #[cfg(unix)]
    #[test]
    fn native_helpers_timeout_without_fallback() {
        let root = Directory::new("timeout");
        let ready = root.0.join("ready");
        let operations = ProcessConfigValueOperations::new(|| unreachable!());
        let command = format!("!printf ready > '{}'; exec sleep 11", ready.display());
        assert_eq!(resolve_config_value_uncached(&command, &operations), None);
        assert_eq!(std::fs::read_to_string(ready).unwrap(), "ready");
    }
}
