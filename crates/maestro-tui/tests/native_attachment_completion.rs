//! Native recursive search and subprocess lifecycle observations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
/// Shared native filesystem fixtures.
pub mod fixtures {
    pub mod attachments;
    pub mod completion_native;
    pub mod futures;
    pub mod native_attachments;
}
use fixtures::completion_native::Tree;
#[cfg(unix)]
use fixtures::native_attachments::{child_script, native_cases};
use fixtures::native_attachments::{entries, executable, paths, query, runtime};
use maestro_cancellation::Cancellation;
use maestro_tui::autocomplete::{CompletionOptions, CursorPosition, NativeAutocompleteOperations};
use maestro_tui::{AutocompleteProvider, CombinedAutocompleteProvider};

#[test]
fn native_attachment_search_finds_nested_and_scoped_matches() {
    runtime().block_on(async {
        let tree = Tree::new().unwrap();
        entries(
            &tree,
            &[
                "alpha.txt",
                "ALPHA.md",
                "alpha/nested/deep.txt",
                "top/middle/file.txt",
            ],
        );
        for (text, expected) in [
            ("@alpha", vec!["alpha/", "ALPHA.md", "alpha.txt"]),
            ("@alpha.txt", vec!["alpha.txt"]),
            ("@deep", vec!["alpha/nested/deep.txt"]),
            ("@middle/file", vec!["top/middle/file.txt"]),
            ("@alpha/nested/d", vec!["alpha/nested/deep.txt"]),
        ] {
            assert_eq!(
                paths(query(&tree.authored(), text).await, text),
                expected,
                "{text}"
            );
        }
        let all = paths(query(&tree.authored(), "@").await, "@");
        assert_eq!(all.len(), 8);
        assert!(all.contains(&"alpha/nested/deep.txt".to_owned()));
        let sibling = Tree::new().unwrap();
        entries(&sibling, &["nested/outside.txt"]);
        let relative = format!(
            "../{}/nested/",
            sibling.0.file_name().unwrap().to_str().unwrap()
        );
        let text = format!("@{relative}out");
        assert_eq!(
            paths(query(&tree.authored(), &text).await, &text),
            [format!("{relative}outside.txt")]
        );
    });
}

#[test]
fn native_attachment_search_selects_full_path_fallbacks() {
    runtime().block_on(async {
        let selective = Tree::new().unwrap();
        entries(
            &selective,
            &[
                "packages/tui/src/autocomplete.rs",
                "packages/ai/src/autocomplete.rs",
                "src/components/Button.rs",
                "src/utils/helpers.rs",
            ],
        );
        for (text, expected) in [
            ("@tui/src/auto", "packages/tui/src/autocomplete.rs"),
            ("@components/", "src/components/Button.rs"),
        ] {
            assert_eq!(
                paths(query(&selective.authored(), text).await, text),
                [expected],
                "{text}"
            );
        }
    });
}

#[test]
fn native_attachment_search_recurses_below_external_scope() {
    runtime().block_on(async {
        let tree = Tree::new().unwrap();
        let sibling = Tree::new().unwrap();
        entries(
            &sibling,
            &[
                "nested/deeper/also-alpha.txt",
                "nested/alpha.txt",
                "nested/deeper/zzz.txt",
            ],
        );
        let relative = format!("../{}/", sibling.0.file_name().unwrap().to_str().unwrap());
        let text = format!("@{relative}alpha");
        assert_eq!(
            paths(query(&tree.authored(), &text).await, &text),
            [
                format!("{relative}nested/alpha.txt"),
                format!("{relative}nested/deeper/also-alpha.txt")
            ]
        );
    });
}

#[cfg(unix)]
#[test]
fn native_attachment_search_respects_ignores_and_follows_links() {
    runtime().block_on(async {
        native_cases("ignore").await;
        let tree = Tree::new().unwrap();
        entries(
            &tree,
            &[
                ".git/HEAD",
                ".hidden/config",
                ".github/job",
                ".gitignore",
                "ignored.txt",
                "kept.txt",
            ],
        );
        std::fs::write(tree.0.join(".gitignore"), "ignored.txt\n").unwrap();
        let outside = Tree::new().unwrap();
        entries(&outside, &["nested/external.txt"]);
        std::os::unix::fs::symlink(&outside.0, tree.0.join("linked-dir")).unwrap();
        std::os::unix::fs::symlink(
            outside.0.join("nested/external.txt"),
            tree.0.join("linked-file.txt"),
        )
        .unwrap();
        let found = paths(query(&tree.authored(), "@").await, "@");
        let mut sorted = found;
        sorted.sort();
        assert_eq!(
            sorted,
            [
                ".github/",
                ".github/job",
                ".gitignore",
                ".hidden/",
                ".hidden/config",
                "kept.txt",
                "linked-dir/",
                "linked-dir/nested/",
                "linked-dir/nested/external.txt",
                "linked-file.txt"
            ]
        );
        assert_eq!(
            paths(query(&tree.authored(), "@linked-dir").await, "@linked-dir"),
            ["linked-dir/"]
        );
        assert_eq!(
            paths(query(&tree.authored(), "@external").await, "@external"),
            ["linked-dir/nested/external.txt"]
        );
        assert_eq!(
            paths(
                query(&tree.authored(), "@linked-file").await,
                "@linked-file"
            ),
            ["linked-file.txt"]
        );
    });
}

#[test]
fn native_attachment_search_preserves_quoted_insertion() {
    runtime().block_on(async {
        let tree = Tree::new().unwrap();
        entries(&tree, &["my folder/alpha.txt", "my folder/other.txt"]);
        assert_eq!(
            paths(query(&tree.authored(), "@my").await, "@my"),
            ["my folder/"]
        );
        let provider = CombinedAutocompleteProvider::new(
            vec![],
            tree.authored(),
            Some(executable()),
            NativeAutocompleteOperations::default(),
        );
        let directory_lines = ["@\"my folder/\"".into()];
        let directory_result = provider
            .get_suggestions(
                &directory_lines,
                CursorPosition { line: 0, col: 12 },
                CompletionOptions {
                    signal: &Cancellation::new(),
                    force: None,
                },
            )
            .await
            .unwrap();
        let mut directory_paths = paths(directory_result, "@\"my folder/");
        directory_paths.sort();
        assert_eq!(
            directory_paths,
            ["my folder/alpha.txt", "my folder/other.txt"]
        );
        let lines = ["say @\"my folder/a\" tail".into()];
        let cursor = CursorPosition { line: 0, col: 17 };
        let result = provider
            .get_suggestions(
                &lines,
                cursor,
                CompletionOptions {
                    signal: &Cancellation::new(),
                    force: None,
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.prefix, "@\"my folder/a");
        assert_eq!(
            result.items,
            vec![maestro_tui::AutocompleteItem {
                value: "@\"my folder/alpha.txt\"".into(),
                label: "alpha.txt".into(),
                description: Some("my folder/alpha.txt".into())
            }]
        );
        let applied = provider.apply_completion(&lines, cursor, &result.items[0], &result.prefix);
        assert_eq!(
            (applied.lines, applied.cursor_line, applied.cursor_col),
            (vec!["say @\"my folder/alpha.txt\"  tail".to_owned()], 0, 27)
        );
    });
}

#[test]
fn native_attachment_search_is_independent_of_base_name() {
    runtime().block_on(async {
        let tree = Tree::new().unwrap();
        for name in ["alpha-base", "other-base"] {
            entries(
                &tree,
                &[
                    &format!("{name}/alpha.txt"),
                    &format!("{name}/nested/alpha.md"),
                ],
            );
        }
        let first = query(tree.0.join("alpha-base").to_str().unwrap(), "@alpha").await;
        let second = query(tree.0.join("other-base").to_str().unwrap(), "@alpha").await;
        assert_eq!(first, second);
        assert_eq!(paths(first, "@alpha"), ["alpha.txt", "nested/alpha.md"]);
    });
}

#[cfg(unix)]
#[test]
fn native_literal_queries_keep_the_selected_filename() {
    runtime().block_on(async {
        native_cases("literal").await;
    });
}

#[cfg(unix)]
#[test]
fn native_filename_records_preserve_whitespace_and_separators() {
    runtime().block_on(async {
        native_cases("identity").await;
    });
}

#[cfg(unix)]
#[test]
fn native_child_abort_waits_for_cleanup() {
    runtime().block_on(async {
    use maestro_tui::autocomplete::AutocompleteOperations;
    use tokio::io::AsyncReadExt;
    let tree = Tree::new().unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let script = child_script(
        &tree,
        &format!(
            "import socket,sys\nsys.stdout.buffer.write(b'partial\\0'); sys.stdout.flush()\ns=socket.create_connection(('127.0.0.1',{}))\ns.sendall(b'READY')\ns.recv(1)",
            listener.local_addr().unwrap().port()
        ),
    );
    let operations = NativeAutocompleteOperations::default();
    let signal = Cancellation::new();
    let run = operations.run_fd(&script, &[], &signal);
    let observe = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut ready = [0; 5];
        socket.read_exact(&mut ready).await.unwrap();
        assert_eq!(&ready, b"READY");
        signal.abort();
        let mut after = vec![];
        socket.read_to_end(&mut after).await.unwrap();
        assert!(after.is_empty());
    };
    let (result, ()) = tokio::join!(run, observe);
    let cancelled = result.unwrap_err();
    assert_eq!(cancelled.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(cancelled.to_string(), "search cancelled");
    let preaborted = Cancellation::new();
    preaborted.abort();
    assert_eq!(
        operations
            .run_fd("missing-executable", &[], &preaborted)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert!(
        operations
            .run_fd("missing-executable", &[], &Cancellation::new())
            .await
            .is_err()
    );
    let failure = child_script(
        &tree,
        "import sys\nsys.stdout.buffer.write(b'partial\\0')\nsys.exit(7)",
    );
    assert_eq!(
        operations
            .run_fd(&failure, &[], &Cancellation::new())
            .await
            .unwrap_err()
            .to_string(),
        "search process failed"
    );
    });
}

/// Polls a pinned future a bounded number of times, once per scheduler turn.
#[cfg(unix)]
async fn settle<F: std::future::Future>(mut run: std::pin::Pin<&mut F>) -> Option<F::Output> {
    for _ in 0..1000 {
        if let std::task::Poll::Ready(output) = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(std::future::Future::poll(run.as_mut(), cx))
        })
        .await
        {
            return Some(output);
        }
        tokio::task::yield_now().await;
    }
    None
}

#[cfg(unix)]
#[test]
fn native_cancelled_search_does_not_wait_for_descendant_pipes() {
    runtime().block_on(async {
    use maestro_tui::autocomplete::AutocompleteOperations;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let tree = Tree::new().unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let script = child_script(
        &tree,
        &format!(
            "import os,socket\ndef hold(role):\n    s=socket.create_connection(('127.0.0.1',{}))\n    s.sendall(b'READY'+role); s.recv(1)\nif os.fork()==0:\n    hold(b'D'); os._exit(0)\nhold(b'P')",
            listener.local_addr().unwrap().port()
        ),
    );
    let operations = NativeAutocompleteOperations::default();
    let signal = Cancellation::new();
    let run = operations.run_fd(&script, &[], &signal);
    tokio::pin!(run);
    let mut sockets = vec![];
    let ready = async {
        while sockets.len() < 2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut ready = [0; 6];
            socket.read_exact(&mut ready).await.unwrap();
            assert_eq!(&ready[..5], b"READY");
            sockets.push((ready[5], socket));
        }
    };
    tokio::select! {
        _ = &mut run => panic!("search ended before cancellation"),
        () = ready => {}
    }
    signal.abort();
    let (_, mut parent) = sockets.remove(sockets.iter().position(|(role, _)| *role == b'P').unwrap());
    let mut sink = vec![];
    let result = tokio::select! {
        result = &mut run => Some(result),
        _ = parent.read_to_end(&mut sink) => None,
    };
    let result = match result {
        Some(result) => Some(result),
        None => settle(run.as_mut()).await,
    };
    let settled_before_release = result.is_some();
    let (_, mut descendant) = sockets.remove(0);
    let _ = descendant.write_all(b"go").await;
    let mut rest = vec![];
    let _ = descendant.read_to_end(&mut rest).await;
    let result = match result {
        Some(result) => result,
        None => run.await,
    };
    assert!(settled_before_release, "search waited for the descendant's pipes");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
    });
}

#[cfg(unix)]
#[test]
fn native_child_drains_stderr_before_returning_stdout() {
    runtime().block_on(async {
    use maestro_tui::autocomplete::AutocompleteOperations;
    let tree = Tree::new().unwrap();
    let script = child_script(
        &tree,
        "import sys\nassert sys.stdin.buffer.read()==b''\nsys.stderr.buffer.write(b'x'*1048576); sys.stderr.flush()\nsys.stdout.buffer.write(b'sentinel\\0')",
    );
    let operations = NativeAutocompleteOperations::default();
    assert_eq!(
        operations
            .run_fd(&script, &[], &Cancellation::new())
            .await
            .unwrap(),
        b"sentinel\0"
    );
    });
}
