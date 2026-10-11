#![cfg(test)]

mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

/// Writes each authored source and compares the complete located diagnostics.
fn check(member: &str, files: &[(&str, &str)], expected: &[(&str, usize, &str)]) {
    let workspace = Workspace::new();
    workspace.foundation(&[member]);
    let directory = workspace.root.join("crates").join(member);
    for (file, contents) in files {
        let path = directory.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    let result = if expected.is_empty() {
        Ok(())
    } else {
        Err(expected
            .iter()
            .map(|(file, line, api)| {
                format!(
                    "{}:{line}: test-owned time limit: {api}; wait for completion instead",
                    directory.join(file).display()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    };
    assert_eq!(check_workspace(&workspace.root), result);
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
