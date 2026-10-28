#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, native_path, source};

/// Writes each authored source and compares the complete located diagnostics.
fn check(member: &str, files: &[(&str, &str)], expected: &[(&str, usize, &str)]) {
    let workspace = Workspace::new();
    workspace.foundation(&[member]);
    let directory = workspace.root.path().join("crates").join(member);
    for (file, contents) in files {
        source(&workspace, member, file, contents);
    }
    if member == "maestro-tui"
        && files.iter().any(|(file, _)| {
            std::path::Path::new(file).parent() == Some(std::path::Path::new("tests"))
        })
    {
        workspace.integration(member);
    }
    let result = if expected.is_empty() {
        Ok(())
    } else {
        Err(expected
            .iter()
            .map(|(file, line, api)| {
                if *api == "optional-poll" {
                    format!("{}:{line}: optional futures_util::poll! in test source; pin the future and call std::future::Future::poll once with std::task::Context::from_waker(std::task::Waker::noop())", directory.join(native_path(file)).display())
                } else if api.starts_with("tracing::") {
                    format!("{}:{line}: scoped tracing capture outside maestro-test-tracing: {api}; call maestro_test_tracing::with_default or with_subscriber instead", directory.join(native_path(file)).display())
                } else {
                    format!("{}:{line}: test-owned time limit: {api}; wait for completion instead", directory.join(native_path(file)).display())
                }
            })
            .collect::<Vec<_>>()
            .join("\n"))
    };
    assert_eq!(check_workspace(workspace.root.path()), result);
}

#[test]
fn qualified_async_timers_are_rejected() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    tokio::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 2, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    ::tokio::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 2, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    tokio::time::timeout_at(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 2, "tokio::time::timeout_at")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    ::tokio::time::timeout_at(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 2, "tokio::time::timeout_at")],
    );
}

#[test]
fn imported_async_timers_are_rejected() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time::timeout;\nasync fn sample() {\n    timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time::timeout_at;\nasync fn sample() {\n    timeout_at(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout_at")],
    );
}

#[test]
fn renamed_async_timers_are_rejected() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time::timeout as within;\nasync fn sample() {\n    within(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time::timeout_at as within;\nasync fn sample() {\n    within(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout_at")],
    );
}

#[test]
fn grouped_and_namespace_imports_keep_timer_identity() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time::{timeout, timeout_at as until};\nasync fn sample() {\n    until(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout_at")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::{time::{self as clock, timeout as within}};\nasync fn sample() {\n    clock::timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time as clock;\nasync fn sample() {\n    clock::timeout_at(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout_at")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio as runtime;\nasync fn sample() {\n    runtime::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use std::{thread::{self as threads, park_timeout as pause}};\nasync fn sample() {\n    pause(deadline);\n}\n",
        )],
        &[("src/tests.rs", 3, "std::thread::park_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time::*;\nasync fn sample() {\n    timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout")],
    );
}

#[test]
fn timed_standard_wait_methods_are_rejected() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    waiter.recv_timeout(deadline);\n}\n",
        )],
        &[("src/tests.rs", 2, "recv_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    waiter.recv_deadline(deadline);\n}\n",
        )],
        &[("src/tests.rs", 2, "recv_deadline")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    waiter.wait_timeout(guard, deadline);\n}\n",
        )],
        &[("src/tests.rs", 2, "wait_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    waiter.wait_timeout_while(guard, deadline, predicate);\n}\n",
        )],
        &[("src/tests.rs", 2, "wait_timeout_while")],
    );
}

#[test]
fn qualified_and_imported_standard_waits_are_rejected() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    std::sync::mpsc::Receiver::<()>::recv_timeout(&waiter, deadline);\n}\n",
        )],
        &[("src/tests.rs", 2, "recv_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    std::sync::Condvar::wait_timeout(&waiter, guard, deadline);\n}\n",
        )],
        &[("src/tests.rs", 2, "wait_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    std::thread::park_timeout(deadline);\n}\n",
        )],
        &[("src/tests.rs", 2, "std::thread::park_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use std::thread::park_timeout;\nasync fn sample() {\n    park_timeout(deadline);\n}\n",
        )],
        &[("src/tests.rs", 3, "std::thread::park_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use std::thread::park_timeout as park;\nasync fn sample() {\n    park(deadline);\n}\n",
        )],
        &[("src/tests.rs", 3, "std::thread::park_timeout")],
    );
}

#[test]
fn test_source_layouts_activate_the_check() {
    check(
        "maestro-tui",
        &[(
            "tests/waits.rs",
            "async fn sample() {\n    tokio::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("tests/waits.rs", 2, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "tests/support/waits.rs",
            "async fn sample() {\n    tokio::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("tests/support/waits.rs", 2, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/nested/tests.rs",
            "async fn sample() {\n    tokio::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/nested/tests.rs", 2, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests/waits.rs",
            "async fn sample() {\n    tokio::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/tests/waits.rs", 2, "tokio::time::timeout")],
    );
}

#[test]
fn test_support_members_are_checked() {
    check(
        "maestro-test-conventions",
        &[(
            "src/lib.rs",
            "async fn sample() {\n    tokio::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/lib.rs", 2, "tokio::time::timeout")],
    );
    check(
        "maestro-test-terminal",
        &[(
            "src/lib.rs",
            "async fn sample() {\n    tokio::time::timeout(deadline, operation).await;\n}\n",
        )],
        &[("src/lib.rs", 2, "tokio::time::timeout")],
    );
}

#[test]
fn inline_test_scopes_do_not_leak_into_production() {
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "async fn before() { tokio::time::timeout(d, f).await; }\n#[cfg(test)]\nmod tests {\n    mod nested {\n        async fn sample() { tokio::time::timeout_at(d, f).await; }\n    }\n}\nasync fn after() { tokio::time::timeout(d, f).await; }\n",
        )],
        &[("src/lib.rs", 5, "tokio::time::timeout_at")],
    );
}

#[test]
fn explicit_test_attributes_reach_executable_bodies() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "#![cfg(test)]\nasync fn helper() { receiver.recv_timeout(d); }\n",
        )],
        &[("src/tests.rs", 2, "recv_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "#[cfg(test)]\nasync fn helper() { let run = || receiver.recv_timeout(d); }\nasync fn production() { tokio::time::timeout(d,f).await; }\n",
        )],
        &[("src/lib.rs", 2, "recv_timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "#[cfg(unix)]\n#[cfg(test)]\nmod tests { async fn sample() { tokio::time::timeout(d, f).await; } }\n",
        )],
        &[("src/lib.rs", 3, "tokio::time::timeout")],
    );
}

#[test]
fn import_aliases_apply_file_wide() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "mod limited {\n    use tokio::time::timeout as within;\n    async fn sample() { within(d, f).await; }\n}\nmod ordinary {\n    use other::within;\n    fn sample() { within(d, f); }\n}\n",
        )],
        &[
            ("src/tests.rs", 3, "tokio::time::timeout"),
            ("src/tests.rs", 7, "tokio::time::timeout"),
        ],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time::timeout as within;\nasync fn sample() {\n    { use other::within; within(d, f); }\n    within(d, f).await;\n}\n",
        )],
        &[
            ("src/tests.rs", 3, "tokio::time::timeout"),
            ("src/tests.rs", 4, "tokio::time::timeout"),
        ],
    );
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "use tokio::time::timeout as within;\n#[cfg(test)]\nasync fn helper() { { within(d, f).await; } }\n",
        )],
        &[("src/lib.rs", 3, "tokio::time::timeout")],
    );
}

#[test]
fn import_items_are_visible_before_their_textual_declaration() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn helper() { within(d,f).await; use tokio::time::timeout as within; }\n",
        )],
        &[("src/tests.rs", 1, "tokio::time::timeout")],
    );
}

#[test]
fn unrelated_names_and_noncall_paths_are_not_timers() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "fn timeout() {}\nfn sample() { timeout(); other::timeout(); object.timeout(); }\n",
        )],
        &[],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "fn sample() { let operation = tokio::time::timeout; }\n",
        )],
        &[],
    );
}

#[test]
fn production_deadlines_are_allowed() {
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "async fn product() { tokio::time::timeout(d,f).await; tokio::time::timeout_at(d,f).await; r.recv_timeout(d); r.recv_deadline(d); c.wait_timeout(g,d); c.wait_timeout_while(g,d,p); std::thread::park_timeout(d); }\n",
        )],
        &[],
    );
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "#[cfg(not(test))]\nasync fn product() { tokio::time::timeout(d,f).await; }\n",
        )],
        &[],
    );
    for attribute in [
        "#[cfg(any(test))]",
        "#[cfg(all(test))]",
        "#[cfg_attr(test, allow(dead_code))]",
        "#[test]",
        "#[inline]",
    ] {
        check(
            "maestro-tui",
            &[(
                "src/lib.rs",
                &format!(
                    "struct W;\nimpl W {{\n    {attribute}\n    fn product() {{ std::thread::park_timeout(d); }}\n}}\n{attribute}\nfn free() {{ std::thread::park_timeout(d); }}\n"
                ),
            )],
            &[],
        );
    }
}

#[test]
fn comments_and_literals_do_not_create_timer_calls() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "// tokio::time::timeout(d, f)\n/* receiver.recv_timeout(d); /* nested comment */ */\nfn sample() {\n    let a = \"tokio::time::timeout_at(d, f)\";\n    let b = r#\"receiver.recv_deadline(d)\"#;\n    let c = b\"std::thread::park_timeout(d)\";\n    println!(\"tokio::time::timeout(d, f)\");\n}\n",
        )],
        &[],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "struct W;\nimpl W {\n    #[cfg(test)]\n    m!(std::thread::park_timeout(d););\n}\ntrait T {\n    #[cfg(test)]\n    m!(std::thread::park_timeout(d););\n}\n",
        )],
        &[],
    );
    check(
        "maestro-tui",
        &[("src/tests.rs", "std::thread::park_timeout(d)\n")],
        &[],
    );
}

#[test]
fn paused_clocks_and_completion_signals_are_allowed() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    tokio::time::advance(duration).await;\n    tokio::time::pause();\n    receiver.recv();\n    async_receiver.recv().await;\n    worker.join();\n    operation.await;\n    tokio::time::sleep(product_delay).await;\n}\n",
        )],
        &[],
    );
}

#[test]
fn tests_may_observe_a_product_deadline() {
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "async fn product() { tokio::time::timeout(real_deadline, work).await; }\n#[cfg(test)]\nmod tests {\n    async fn sample() { super::product().await; }\n}\n",
        )],
        &[],
    );
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "async fn collect(launch: Launch) -> Attempt {\n    let completion = tokio::time::timeout(std::time::Duration::from_millis(10_000), async {\n        tokio::try_join!(stdout.read_to_end(&mut bytes), child.wait())\n    }).await;\n    finish(completion)\n}\n#[cfg(test)]\nmod tests { fn sample() { let result = resolve_config_value_uncached(command, operations); } }\n",
        )],
        &[],
    );
}

#[test]
fn diagnostics_report_each_call_at_its_source_line() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    tokio::time::timeout(d,f).await; receiver.recv_timeout(d);\n    tokio::time::timeout_at(d,f).await;\n}\n",
        )],
        &[
            ("src/tests.rs", 2, "tokio::time::timeout"),
            ("src/tests.rs", 2, "recv_timeout"),
            ("src/tests.rs", 3, "tokio::time::timeout_at"),
        ],
    );
    check(
        "maestro-tui",
        &[
            (
                "src/tests.rs",
                "async fn sample() { tokio::time::timeout(d,f).await; }\n",
            ),
            (
                "src/tests/another.rs",
                "fn sample() { receiver.recv_deadline(d); }\n",
            ),
        ],
        &[
            ("src/tests.rs", 1, "tokio::time::timeout"),
            ("src/tests/another.rs", 1, "recv_deadline"),
        ],
    );
}

#[test]
fn normalized_sources_keep_real_call_lines() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "﻿async fn sample() {\r\n    let s = r#\"tokio::time::timeout(d,f)\"#;\r\n    tokio::time::r#timeout(\r\n        d, f\r\n    ).await;\r\n}\r\n",
        )],
        &[("src/tests.rs", 3, "tokio::time::timeout")],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() { (tokio::time::timeout)(d,f).await; }\n",
        )],
        &[("src/tests.rs", 1, "tokio::time::timeout")],
    );
}

#[test]
fn associated_test_bodies_restore_production_ownership() {
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "struct Worker;\nimpl Worker {\n    fn before() { std::thread::park_timeout(d); }\n    #[cfg(test)]\n    fn helper() { std::thread::park_timeout(d); }\n    fn after() { std::thread::park_timeout(d); }\n    #[cfg(test)]\n    const TEST: () = { std::thread::park_timeout(d); };\n    const PRODUCT: () = { std::thread::park_timeout(d); };\n}\ntrait Helper {\n    fn before() { std::thread::park_timeout(d); }\n    #[cfg(test)]\n    fn helper() { std::thread::park_timeout(d); }\n    fn after() { std::thread::park_timeout(d); }\n    #[cfg(test)]\n    const TEST: () = { std::thread::park_timeout(d); };\n    const PRODUCT: () = { std::thread::park_timeout(d); };\n}\n",
        )],
        &[
            ("src/lib.rs", 5, "std::thread::park_timeout"),
            ("src/lib.rs", 8, "std::thread::park_timeout"),
            ("src/lib.rs", 14, "std::thread::park_timeout"),
            ("src/lib.rs", 17, "std::thread::park_timeout"),
        ],
    );
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "struct W;\nimpl W {\n    #[cfg(test)]\n    type T = [u8; { std::thread::park_timeout(d); 1 }];\n    type P = [u8; { std::thread::park_timeout(d); 1 }];\n}\ntrait Tr {\n    #[cfg(test)]\n    type T = [u8; { std::thread::park_timeout(d); 1 }];\n    type P = [u8; { std::thread::park_timeout(d); 1 }];\n}\n",
        )],
        &[
            ("src/lib.rs", 4, "std::thread::park_timeout"),
            ("src/lib.rs", 9, "std::thread::park_timeout"),
        ],
    );
}

#[test]
fn inherited_test_context_survives_nested_associated_items() {
    let park = "std::thread::park_timeout(d);";
    let lines = [
        "#[cfg(test)]".to_owned(),
        "mod tests {".to_owned(),
        "    struct W;".to_owned(),
        "    impl W {".to_owned(),
        format!("        fn a() {{ {park} }}"),
        "        #[cfg(test)]".to_owned(),
        format!("        fn b() {{ {park} }}"),
        format!("        fn c() {{ {park} }}"),
        "    }".to_owned(),
        "    trait T {".to_owned(),
        format!("        fn a() {{ {park} }}"),
        "        #[cfg(test)]".to_owned(),
        format!("        fn b() {{ {park} }}"),
        format!("        fn c() {{ {park} }}"),
        "    }".to_owned(),
        format!("    fn nested() {{ {park} }}"),
        "    #[cfg(test)]".to_owned(),
        format!("    fn gated() {{ {park} }}"),
        format!("    fn after() {{ {park} }}"),
        "}".to_owned(),
        format!("fn production() {{ {park} }}"),
    ];
    let expected = [5, 7, 8, 11, 13, 14, 16, 18, 19]
        .map(|line| ("src/lib.rs", line, "std::thread::park_timeout"));
    check(
        "maestro-tui",
        &[("src/lib.rs", &(lines.join("\n") + "\n"))],
        &expected,
    );
}

#[test]
fn competing_namespace_aliases_preserve_each_timer() {
    for source in [
        "mod asynchronous {\n    use tokio::time as wait;\n    async fn sample() { wait::timeout(d, f).await; }\n}\nmod blocking {\n    use std::thread as wait;\n    fn sample() { wait::park_timeout(d); }\n}\n",
        "mod blocking {\n    use std::thread as wait;\n    fn sample() { wait::park_timeout(d); }\n}\nmod asynchronous {\n    use tokio::time as wait;\n    async fn sample() { wait::timeout(d, f).await; }\n}\n",
    ] {
        let (first, second) = if source.starts_with("mod asynchronous") {
            ("tokio::time::timeout", "std::thread::park_timeout")
        } else {
            ("std::thread::park_timeout", "tokio::time::timeout")
        };
        check(
            "maestro-tui",
            &[("src/tests.rs", source)],
            &[("src/tests.rs", 3, first), ("src/tests.rs", 7, second)],
        );
    }
}

#[test]
fn namespace_aliases_do_not_replace_direct_timer_paths() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tokio::time as std;\nuse std::thread as tokio;\nfn sample() {\n    ::std::thread::park_timeout(d);\n    std::thread::park_timeout(d);\n    ::tokio::time::timeout(d, f);\n    tokio::time::timeout_at(d, f);\n}\n",
        )],
        &[
            ("src/tests.rs", 4, "std::thread::park_timeout"),
            ("src/tests.rs", 5, "std::thread::park_timeout"),
            ("src/tests.rs", 6, "tokio::time::timeout"),
            ("src/tests.rs", 7, "tokio::time::timeout_at"),
        ],
    );
}

/// Complete authored source and expected public diagnostics.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PollCase {
    test: String,
    member: String,
    files: std::collections::BTreeMap<String, String>,
    expected: Vec<PollDiagnostic>,
}

/// One expected occurrence in an authored source.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PollDiagnostic {
    file: String,
    line: usize,
    kind: String,
    api: Option<String>,
}

/// Replay every complete input assigned to this public behavior witness.
fn check_optional_poll(test: &str) {
    let cases: Vec<PollCase> =
        serde_json::from_str(include_str!("fixtures/optional_poll.json")).unwrap();
    let selected: Vec<_> = cases.iter().filter(|case| case.test == test).collect();
    assert!(!selected.is_empty());
    for case in selected {
        let files: Vec<_> = case
            .files
            .iter()
            .map(|(file, source)| (file.as_str(), source.as_str()))
            .collect();
        let expected: Vec<_> = case
            .expected
            .iter()
            .map(|diagnostic| {
                let api = match diagnostic.kind.as_str() {
                    "optional-poll" => {
                        assert!(diagnostic.api.is_none());
                        "optional-poll"
                    }
                    "time-limit" => diagnostic.api.as_deref().unwrap(),
                    _ => panic!("unknown diagnostic kind"),
                };
                (diagnostic.file.as_str(), diagnostic.line, api)
            })
            .collect();
        check(&case.member, &files, &expected);
    }
}

#[test]
fn qualified_optional_poll_macros_are_rejected() {
    check_optional_poll("qualified_optional_poll_macros_are_rejected");
}

#[test]
fn imported_optional_poll_macros_keep_their_identity() {
    check_optional_poll("imported_optional_poll_macros_keep_their_identity");
}

#[test]
fn assertion_tokens_do_not_hide_optional_poll_macros() {
    check_optional_poll("assertion_tokens_do_not_hide_optional_poll_macros");
}

#[test]
fn optional_poll_checks_follow_existing_test_layouts() {
    check_optional_poll("optional_poll_checks_follow_existing_test_layouts");
}

#[test]
fn optional_poll_test_scopes_restore_production_ownership() {
    check_optional_poll("optional_poll_test_scopes_restore_production_ownership");
}

#[test]
fn unrelated_source_does_not_trigger_optional_poll_rule() {
    check_optional_poll("unrelated_source_does_not_trigger_optional_poll_rule");
}

#[test]
fn optional_poll_diagnostics_keep_order_and_occurrence_count() {
    check_optional_poll("optional_poll_diagnostics_keep_order_and_occurrence_count");
}

#[test]
fn optional_poll_keeps_timer_diagnostics_unchanged() {
    check_optional_poll("optional_poll_keeps_timer_diagnostics_unchanged");
}

#[test]
fn optional_poll_alias_candidates_do_not_hide_direct_paths() {
    check_optional_poll("optional_poll_alias_candidates_do_not_hide_direct_paths");
}

#[test]
fn optional_poll_aliases_keep_the_file_wide_scope() {
    check_optional_poll("optional_poll_aliases_keep_the_file_wide_scope");
}

#[test]
fn optional_poll_locations_survive_source_normalization() {
    check_optional_poll("optional_poll_locations_survive_source_normalization");
}

#[test]
fn standard_library_single_poll_is_allowed() {
    check_optional_poll("standard_library_single_poll_is_allowed");
}

#[test]
fn qualified_scoped_tracing_captures_are_rejected() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "fn sample() {\n    tracing::subscriber::with_default(subscriber, action);\n    tracing::subscriber::set_default(subscriber);\n    ::tracing::dispatcher::with_default(&dispatch, action);\n    tracing::dispatcher::set_default(&dispatch);\n}\n",
        )],
        &[
            ("src/tests.rs", 2, "tracing::subscriber::with_default"),
            ("src/tests.rs", 3, "tracing::subscriber::set_default"),
            ("src/tests.rs", 4, "tracing::dispatcher::with_default"),
            ("src/tests.rs", 5, "tracing::dispatcher::set_default"),
        ],
    );
}

#[test]
fn imported_scoped_tracing_captures_keep_their_identity() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tracing::subscriber::with_default as scoped;\nuse tracing::dispatcher;\nfn sample() {\n    scoped(subscriber, action);\n    dispatcher::set_default(&dispatch);\n}\n",
        )],
        &[
            ("src/tests.rs", 4, "tracing::subscriber::with_default"),
            ("src/tests.rs", 5, "tracing::dispatcher::set_default"),
        ],
    );
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use tracing::subscriber::*;\nfn sample() {\n    set_default(subscriber);\n}\n",
        )],
        &[("src/tests.rs", 3, "tracing::subscriber::set_default")],
    );
}

#[test]
fn future_subscriber_method_is_rejected_at_the_method() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "async fn sample() {\n    operation\n        .with_subscriber(subscriber)\n        .await;\n}\n",
        )],
        &[(
            "src/tests.rs",
            3,
            "tracing::instrument::WithSubscriber::with_subscriber",
        )],
    );
}

#[test]
fn owner_captures_and_production_dispatch_are_allowed() {
    check(
        "maestro-tui",
        &[(
            "src/tests.rs",
            "use maestro_test_tracing::with_subscriber;\nasync fn sample() {\n    maestro_test_tracing::with_default(subscriber, action);\n    with_subscriber(operation, subscriber).await;\n}\n",
        )],
        &[],
    );
    check(
        "maestro-test-tracing",
        &[(
            "src/lib.rs",
            "fn sample() {\n    tracing::subscriber::with_default(subscriber, action);\n    operation.with_subscriber(subscriber);\n}\n",
        )],
        &[],
    );
    check(
        "maestro-tui",
        &[(
            "src/lib.rs",
            "fn sample() {\n    tracing::dispatcher::with_default(&dispatch, action);\n}\n",
        )],
        &[],
    );
}
