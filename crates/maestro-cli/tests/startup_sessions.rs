//! Session lookups, stored-history choices and their opened results.

use crate::support::startup;
use maestro_app::{Presence, ReadonlySessionManager, SessionManager};
use maestro_cli::startup::main::sessions::{
    ResolvedSession, SessionOperation, SessionSelection, open_selected_session,
    resolve_session_path, select_session_operation,
};
use maestro_test_directory::Scratch;
use serde_json::Value;

/// The first header record of a stored history.
fn header(path: &str) -> Value {
    let text = std::fs::read_to_string(path).expect("stored history");
    serde_json::from_str(text.lines().next().expect("header line")).expect("header record")
}

/// The authored paths one lookup must answer with itself.
const AUTHORED: [&str; 3] = ["/direct/session.jsonl", "sub/session", "back\\slash"];

#[test]
fn session_locators_choose_first_local_then_global() {
    let scratch = Scratch::new().unwrap();
    let agent_dir = scratch.path().join("agent");
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let local_dir = scratch.path().join("local");
    let local = startup::stored_history(&local_dir, "local-one", "project-session", path(&cwd));
    let other = scratch.path().join("agent/sessions/other-project");
    let global = startup::stored_history(&other, "global-one", "global-456", "/other/project");
    let ambiguous = ["amb-1", "amb-2"]
        .iter()
        .map(|id| startup::stored_history(&local_dir, id, id, path(&cwd)))
        .collect::<Vec<_>>();

    startup::runtime().block_on(async {
        let here = (path(&cwd), path(&local_dir), path(&agent_dir));
        assert_eq!(
            lookup("project", here).await.unwrap(),
            ResolvedSession::Local(local.clone())
        );
        assert_eq!(
            lookup("global", here).await.unwrap(),
            ResolvedSession::Global {
                path: global.clone(),
                cwd: "/other/project".to_owned(),
            }
        );
        assert_eq!(
            lookup("nothing", here).await.unwrap(),
            ResolvedSession::NotFound("nothing".to_owned())
        );
        assert_eq!(
            lookup("UPPER.JSONL", here).await.unwrap(),
            ResolvedSession::NotFound("UPPER.JSONL".to_owned()),
            "an uppercase suffix stays a session identifier"
        );
        for authored in AUTHORED {
            assert_eq!(
                lookup(authored, here).await.unwrap(),
                ResolvedSession::Path(authored.to_owned()),
                "{authored}"
            );
        }
        let listed = listed_prefix(&local_dir, &agent_dir, &cwd, "amb-").await;
        assert!(ambiguous.contains(&listed));
        assert_eq!(
            lookup("amb-", here).await.unwrap(),
            ResolvedSession::Local(listed),
            "an ambiguous prefix keeps the listing order"
        );
    });
}

#[test]
fn a_local_match_alone_answers_without_the_global_root() {
    let scratch = Scratch::new().unwrap();
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let local_dir = scratch.path().join("local");
    let local = startup::stored_history(&local_dir, "local-one", "project-session", path(&cwd));
    let broken = scratch.path().join("broken/agent");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("sessions"), "not a directory").unwrap();

    startup::runtime().block_on(async {
        let here = (path(&cwd), path(&local_dir), path(&broken));
        assert_eq!(
            lookup("project", here).await.unwrap(),
            ResolvedSession::Local(local)
        );
        assert_eq!(
            lookup("global", here).await.unwrap(),
            ResolvedSession::NotFound("global".to_owned()),
            "only a missing local match reads the global root"
        );
    });
}

/// Resolve one argument against the supplied directories.
async fn lookup(argument: &str, here: (&str, &str, &str)) -> std::io::Result<ResolvedSession> {
    resolve_session_path(argument, here.0, Some(here.1), here.2).await
}

/// The first listed session of the given prefix, in the owner's order.
async fn listed_prefix(
    local_dir: &std::path::Path,
    agent_dir: &std::path::Path,
    cwd: &std::path::Path,
    prefix: &str,
) -> String {
    let listed = SessionManager::list(path(cwd), Some(path(local_dir)), path(agent_dir), None)
        .await
        .unwrap();
    listed
        .into_iter()
        .find(|session| {
            matches!(&session.id, Presence::Present(Value::String(id)) if id.starts_with(prefix))
        })
        .expect("an ambiguous local match")
        .path
}

/// The authored text of a path.
fn path(value: &std::path::Path) -> &str {
    value.to_str().expect("authored path")
}

/// Set a stored history's modification time to a whole epoch second.
fn set_modified(path: &str, seconds: u64) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("an open history")
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
        .expect("a set modification time");
}

#[test]
fn explicit_foreign_fork_copies_in_without_confirming() {
    let scratch = Scratch::new().unwrap();
    let agent_dir = scratch.path().join("agent");
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let local_dir = scratch.path().join("local");
    let other = scratch.path().join("agent/sessions/other-project");
    let global = startup::stored_history(&other, "global-one", "global-456", "/other/project");
    let here = (path(&cwd), path(&agent_dir), path(&local_dir));

    assert_eq!(
        select(&["--fork", "global"], here).unwrap(),
        SessionSelection::Selected(SessionOperation::Fork {
            path: global.clone(),
        }),
        "an explicit foreign fork copies the session in without asking"
    );
    let forked = open_selected_session(
        &SessionOperation::Fork {
            path: global.clone(),
        },
        here.0,
        Some(here.2),
        here.1,
    )
    .unwrap();
    let forked_path = forked
        .get_session_file()
        .expect("a forked filename")
        .to_owned();
    assert_ne!(
        forked_path, global,
        "a foreign fork copies into this project"
    );
    assert_eq!(
        header(&forked_path)["parentSession"].as_str(),
        Some(global.as_str())
    );
    assert_eq!(
        select(&["--session", "global"], here).unwrap(),
        SessionSelection::ConfirmFork {
            path: global,
            cwd: "/other/project".to_owned(),
        },
        "only an explicit session confirms a foreign match"
    );
}

#[test]
fn session_operation_precedence_is_explicit() {
    let scratch = Scratch::new().unwrap();
    let agent_dir = scratch.path().join("agent");
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let session_dir = scratch.path().join("sessions");
    let first = startup::stored_history(&session_dir, "first", "first-1", path(&cwd));
    let second = startup::stored_history(&session_dir, "second", "second-2", path(&cwd));
    set_modified(&first, 1_000_000);
    set_modified(&second, 2_000_000);
    let here = (path(&cwd), path(&agent_dir), path(&session_dir));

    let cases: Vec<(Vec<&str>, SessionSelection)> = vec![
        (
            vec![
                "--no-session",
                "--session",
                "first",
                "--resume",
                "--continue",
            ],
            SessionSelection::Selected(SessionOperation::InMemory),
        ),
        (
            vec![
                "--fork",
                "first",
                "--session",
                "second",
                "--resume",
                "--continue",
            ],
            SessionSelection::Selected(SessionOperation::Fork {
                path: first.clone(),
            }),
        ),
        (
            vec!["--session", "first", "--resume", "--continue"],
            SessionSelection::Selected(SessionOperation::Open {
                path: first.clone(),
            }),
        ),
        (vec!["--resume", "--continue"], SessionSelection::Resume),
        (
            vec!["--continue"],
            SessionSelection::Selected(SessionOperation::ContinueRecent),
        ),
        (vec![], SessionSelection::Selected(SessionOperation::Create)),
    ];
    for (tokens, expected) in &cases {
        assert_eq!(&select(tokens, here).unwrap(), expected, "{tokens:?}");
    }
    for tokens in [vec!["--fork", "nothing"], vec!["--session", "nothing"]] {
        assert_eq!(
            select(&tokens, here).unwrap(),
            SessionSelection::NotFound("nothing".to_owned()),
            "{tokens:?}"
        );
    }

    opened_histories(here, &first, &second);
}

/// Assert the history each selected operation opens.
fn opened_histories(here: (&str, &str, &str), first: &str, second: &str) {
    let opened = |tokens: &[&str]| {
        open_selected_session(&operation(tokens, here), here.0, Some(here.2), here.1).unwrap()
    };
    let memory = opened(&["--no-session"]);
    assert!(
        !memory.is_persisted(),
        "an unsaved session publishes nothing"
    );

    let continued = opened(&["--continue"]);
    assert_eq!(
        continued.get_session_file(),
        Some(second),
        "continue opens the most recently active history"
    );

    let created = opened(&[]);
    let created_path = created.get_session_file().expect("a selected filename");
    assert_ne!(created_path, first);
    assert_ne!(created_path, second);

    let in_place = opened(&["--session", "first"]);
    let before = std::fs::read_to_string(first).unwrap();
    assert_eq!(in_place.get_session_file(), Some(first));
    assert_eq!(
        std::fs::read_to_string(first).unwrap(),
        before,
        "opening a history rewrites nothing"
    );

    let forked = opened(&["--fork", "first"]);
    let forked_path = forked
        .get_session_file()
        .expect("a forked filename")
        .to_owned();
    assert_ne!(forked_path, first);
    assert_eq!(
        header(&forked_path)["parentSession"].as_str(),
        Some(first),
        "a fork records its source"
    );
    assert_eq!(header(first)["id"].as_str(), Some("first-1"));
}

/// The operation the supplied arguments select.
fn select(tokens: &[&str], here: (&str, &str, &str)) -> std::io::Result<SessionSelection> {
    startup::runtime().block_on(select_session_operation(
        &startup::args(tokens),
        here.0,
        Some(here.2),
        here.1,
    ))
}

/// The selected operation of the supplied arguments.
fn operation(tokens: &[&str], here: (&str, &str, &str)) -> SessionOperation {
    match select(tokens, here).unwrap() {
        SessionSelection::Selected(operation) => operation,
        other => panic!("{tokens:?} selected {other:?}"),
    }
}
