//! FTP source login, modification checks, listing fallback, transfer and teardown.

use super::super::ftp::parse_control_response::Frames;
use super::super::ftp::{CONSUMED, INACTIVITY, instant_millis, payload, port};
use super::super::{self as get_uri, notfound::NotFound, notmodified::NotModified};
use crate::providers::http::{Route, configured_fetch};
use crate::tests::ftp_server::{
    self, Dribble, Features, Held, Listing, Modification, Passive, Peer, Scenario, Silence,
    Transfer,
};
use std::future::Future;
use std::net::Ipv4Addr;
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::Instant;

/// The silence a login reply may take and still be inside its own window.
const LOGIN_WAIT: Duration = Duration::from_secs(20);

/// Keep the paused clock still until the returned sender is dropped.
///
/// The paused runtime advances its clock by itself whenever it has nothing to run, which would
/// expire a window while the scripted peer, on its own thread, is still writing. A running
/// blocking task stops that, so only the test moves the clock.
fn hold_clock() -> std::sync::mpsc::Sender<()> {
    let (release, held) = std::sync::mpsc::channel::<()>();
    drop(tokio::task::spawn_blocking(move || held.recv().ok()));
    release
}

/// Run a timing test on a paused runtime whose clock only the test moves.
fn run_held(body: impl Future<Output = ()> + Send + 'static) {
    crate::tests::runtime::run_paused(async {
        let _clock = hold_clock();
        if let Err(error) = tokio::spawn(body).await {
            std::panic::resume_unwind(error.into_panic());
        }
    });
}

/// Work running as its own task, woken by its own sockets and timers, which reports the instant
/// it ended with its output.
struct Running<T> {
    /// The ending instant and output, once the work ended.
    ended: oneshot::Receiver<(Instant, T)>,
    /// The bytes the connections of this thread consumed.
    consumed: tokio::sync::watch::Receiver<usize>,
    /// The bytes consumed before the work started.
    before: usize,
}

impl<T: std::fmt::Debug + Send + 'static> Running<T> {
    /// Start `work` as a task and return once its first poll ran, so the windows it opens at
    /// once start before the clock moves.
    async fn spawn(work: impl Future<Output = T> + Send + 'static) -> Self {
        let consumed = CONSUMED.with(tokio::sync::watch::Sender::subscribe);
        let before = *consumed.borrow();
        let (started, start) = oneshot::channel();
        let (report, ended) = oneshot::channel();
        tokio::spawn(async move {
            // The rest of this poll runs `work` up to its first wait before the body resumes.
            started.send(()).ok();
            let output = work.await;
            report.send((Instant::now(), output)).ok();
        });
        start.await.expect("the task starts");
        Self {
            ended,
            consumed,
            before,
        }
    }

    /// Require that the work has not ended yet.
    fn pending(&mut self, context: &str) {
        if let Ok((_, output)) = self.ended.try_recv() {
            panic!("{context}, but the work ended with {output:?}");
        }
    }

    /// Wait until the peer logged `count` lines starting with `line` and the work consumed every
    /// byte the peer wrote by then, so a window the bytes moved is moved before the clock is.
    async fn reached(&mut self, peer: &Peer, line: &str, count: usize) {
        let before = self.before;
        let consumed = &mut self.consumed;
        let settled = async {
            peer.reached(line, count).await;
            let written = peer.written();
            consumed
                .wait_for(|total| total - before >= written)
                .await
                .ok();
        };
        tokio::select! {
            () = settled => {}
            ended = &mut self.ended => {
                panic!("the work ended with {ended:?} before the peer reached {line:?}")
            }
        }
    }

    /// The instant and output the work ended with.
    async fn ended(self) -> (Instant, T) {
        self.ended.await.expect("the task reports its end")
    }
}

/// Load `uri` as a task.
async fn loading(uri: String) -> Running<Result<get_uri::Source, get_uri::Failure>> {
    Running::spawn(async move { load(&uri, None).await }).await
}

/// The log line a scripted peer writes when it reaches `stage`.
fn marker(stage: &str) -> String {
    format!("{}{stage}", ftp_server::STAGE)
}

/// Move the held clock to `at` after `origin`.
async fn to(origin: Instant, at: Duration) {
    tokio::time::advance((origin + at).saturating_duration_since(Instant::now())).await;
}

/// Require that the work ends at the held instant, exactly `at` after `origin`, with the timeout of
/// `socket`.
async fn expires<T>(
    running: Running<Result<T, get_uri::Failure>>,
    origin: Instant,
    at: Duration,
    socket: &str,
) where
    T: std::fmt::Debug + Send + 'static,
{
    let (ended, output) = running.ended().await;
    let error = output.err().map(|error| error.to_string());
    assert_eq!(error, Some(format!("Timeout ({socket})")));
    assert_eq!(
        ended - origin,
        at,
        "the {socket} expired at the wrong instant"
    );
}

/// A whole number of seconds.
fn seconds(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

/// The instant one millisecond before `seconds`.
fn before(seconds: u64) -> Duration {
    Duration::from_millis(seconds * 1000 - 1)
}

/// A release channel and the receiver a scripted peer waits on.
fn gated() -> (std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>) {
    std::sync::mpsc::channel()
}

/// A reply sent in whole pieces, which needs no release from the test.
fn whole(parts: &'static [&'static [u8]]) -> Dribble {
    let (sender, releases) = gated();
    drop(sender);
    Dribble { parts, releases }
}

/// A source URL naming `peer`, with optional user info and a path.
fn source(peer: &Peer, userinfo: &str, path: &str) -> String {
    format!("ftp://{userinfo}{}{path}", peer.address())
}

/// Load one script through the shared source loader.
async fn load(
    uri: &str,
    cache: Option<&get_uri::Cache>,
) -> Result<get_uri::Source, get_uri::Failure> {
    let fetch = configured_fetch(Route::default());
    get_uri::load(&url::Url::parse(uri).unwrap(), cache, &fetch).await
}

/// A `FEAT` reply that advertises no machine listing.
const TEXT_FEATURES: Features = Features("211-Features:\r\n MDTM\r\n211 End");

/// The single-line `FEAT` reply of a server without optional features.
const ABSENT_FEATURES: Features = Features("211 no features");

/// A refused `FEAT`.
const REFUSED_FEATURES: Features = Features("500 unknown command");

/// A refused `MDTM`, which sends the loader to the directory listing.
const UNSUPPORTED: Modification = Modification::Reply("500 unknown command");

/// The scripted peer for a server that leaves one kind of command unanswered.
fn silent(silence: Silence) -> Peer {
    ftp_server::serve(vec![Scenario {
        silence,
        ..Scenario::default()
    }])
    .unwrap()
}

/// A `LIST`-only answer whose data connection carries these lines.
fn listing(lines: Vec<String>) -> Listing {
    Listing {
        lines,
        refusal: None,
    }
}

/// The number of file transfer commands the peer read.
fn transfers(commands: &[String]) -> usize {
    commands
        .iter()
        .filter(|line| line.starts_with("RETR"))
        .count()
}

/// A Unix text-listing entry, which carries no comparable modification stamp.
fn unix(name: &str) -> Vec<String> {
    vec![format!("-rw-r--r-- 1 user group 4 Nov 5 13:46 {name}")]
}

/// A DOS text-listing entry, which carries no comparable modification stamp.
fn dos(name: &str) -> Vec<String> {
    vec![format!("11-05-20  01:46PM                  4 {name}")]
}

#[test]
fn ftp_login_defaults_to_anonymous_and_guest() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        let commands = peer.commands();
        assert!(
            commands.contains(&"USER anonymous".to_owned()),
            "{commands:?}"
        );
        assert!(commands.contains(&"PASS guest".to_owned()), "{commands:?}");
        peer.finish().unwrap();

        let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        load(&source(&peer, "user@", "/script.pac"), None)
            .await
            .unwrap();
        let commands = peer.commands();
        assert!(commands.contains(&"USER user".to_owned()), "{commands:?}");
        assert!(commands.contains(&"PASS guest".to_owned()), "{commands:?}");
        peer.finish().unwrap();

        let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        let named = format!("ftp://localhost:{}/script.pac", peer.address().port());
        load(&named, None).await.unwrap();
        assert!(peer.commands().contains(&"USER anonymous".to_owned()));
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_port_follows_the_reference_rule() {
    for (uri, expected) in [
        ("ftp://host/script.pac", 21),
        ("ftp://host:0/script.pac", 21),
        ("ftp://host:21/script.pac", 21),
        ("ftp://host:2121/script.pac", 2121),
    ] {
        assert_eq!(port(&url::Url::parse(uri).unwrap()), expected, "{uri}");
    }
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert!(peer.commands().contains(&"USER anonymous".to_owned()));
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_login_decodes_url_credentials() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        load(
            &source(
                &peer,
                "us%20er:p%3Ass%40x@",
                "/dir%20with%20space/script.pac",
            ),
            None,
        )
        .await
        .unwrap();
        let commands = peer.commands();
        assert!(commands.contains(&"USER us er".to_owned()), "{commands:?}");
        assert!(commands.contains(&"PASS p:ss@x".to_owned()), "{commands:?}");
        assert!(
            commands.contains(&"MDTM /dir with space/script.pac".to_owned()),
            "{commands:?}"
        );
        peer.finish().unwrap();

        for path in ["/%zz/script.pac", "/%2/script.pac"] {
            let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
            let error = load(&source(&peer, "", path), None).await.unwrap_err();
            assert_eq!(error.to_string(), "Invalid percent escape", "{path}");
            assert!(peer.commands().is_empty(), "{path}");
            peer.finish().unwrap();
        }
    });
}

#[test]
fn ftp_transfers_bytes_in_binary_mode() {
    crate::tests::runtime::run(async {
        let body = b"a\r\nb\x00c\xc3\xa9".to_vec();
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer::body(body.clone()),
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert_eq!(loaded.bytes, body);
        let commands = peer.commands();
        let typed = commands.iter().position(|line| line == "TYPE I").unwrap();
        let transferred = commands
            .iter()
            .position(|line| line == "RETR /script.pac")
            .unwrap();
        assert!(typed < transferred, "{commands:?}");

        let closed = peer.finish().unwrap();
        assert!(closed.control && closed.data);
    });
}

#[test]
fn ftp_reports_not_found_when_the_server_refuses_the_file() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            modification: Modification::MISSING,
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.is::<NotFound>(), "{error}");
        assert_eq!(error.to_string(), NotFound.to_string());
        let commands = peer.commands();
        assert!(
            !commands.iter().any(|line| line.starts_with("RETR")),
            "{commands:?}"
        );
        assert!(
            !commands.iter().any(|line| line.starts_with("LIST")),
            "{commands:?}"
        );
        assert!(
            !commands.iter().any(|line| line.starts_with("MLSD")),
            "{commands:?}"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_machine_listing_supplies_the_modification_instant() {
    crate::tests::runtime::run(async {
        let mut lines = ftp_server::machine("script.pac", Some("20240101120000"));
        lines.push("total 2".to_owned());
        let peer = ftp_server::serve(vec![Scenario {
            features: Features::MACHINE,
            modification: UNSUPPORTED,
            listing: listing(lines),
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert!(matches!(
            loaded.cache,
            get_uri::Cache::Ftp(Some(1_704_110_400_000))
        ));
        let commands = peer.commands();
        assert!(
            commands
                .iter()
                .any(|line| line.starts_with("OPTS MLST type;size;modify;")),
            "{commands:?}"
        );
        assert!(commands.contains(&"MLSD /".to_owned()), "{commands:?}");
        let unchanged = load(&source(&peer, "", "/script.pac"), Some(&loaded.cache)).await;
        assert!(unchanged.unwrap_err().is::<NotModified>());
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_machine_listing_keeps_every_accepted_entry() {
    // A machine listing the reference accepts, whose entry the native parser would refuse or
    // rewrite: a fractional stamp, a symbolic-link type, a semicolon in the name, and a name that
    // starts with a space.
    crate::tests::runtime::run(async {
        for (facts, path, instant) in [
            (
                "type=file;modify=20240101120000.500; script.pac",
                "/script.pac",
                1_704_110_400_500,
            ),
            (
                "type=OS.unix=symlink;modify=20240101120000; script.pac",
                "/script.pac",
                1_704_110_400_000,
            ),
            (
                "type=file;modify=20240101120000; sc;ript.pac",
                "/sc;ript.pac",
                1_704_110_400_000,
            ),
            (
                "type=file;modify=20240101120000;  leading.pac",
                "/%20leading.pac",
                1_704_110_400_000,
            ),
        ] {
            let peer = ftp_server::serve(vec![Scenario {
                modification: UNSUPPORTED,
                listing: listing(vec![facts.to_owned()]),
                ..Scenario::default()
            }])
            .unwrap();
            let loaded = load(&source(&peer, "", path), None).await.unwrap();
            assert!(
                matches!(loaded.cache, get_uri::Cache::Ftp(Some(found)) if found == instant),
                "{facts}"
            );
            peer.finish().unwrap();
        }
    });
}

#[test]
fn ftp_text_listing_supplies_no_modification_instant() {
    crate::tests::runtime::run(async {
        for (announced, lines) in [
            (TEXT_FEATURES, unix("script.pac")),
            (ABSENT_FEATURES, unix("script.pac")),
            (REFUSED_FEATURES, unix("script.pac")),
            (TEXT_FEATURES, dos("script.pac")),
            (ABSENT_FEATURES, dos("script.pac")),
        ] {
            let peer = ftp_server::serve(vec![Scenario {
                features: announced,
                modification: UNSUPPORTED,
                listing: listing(lines),
                ..Scenario::default()
            }])
            .unwrap();
            let error = load(&source(&peer, "", "/script.pac"), None)
                .await
                .unwrap_err();
            assert!(error.is::<NotFound>(), "{error}");
            let commands = peer.commands();
            assert!(commands.contains(&"LIST -a /".to_owned()), "{commands:?}");
            assert!(
                !commands.iter().any(|line| line.starts_with("MLSD")),
                "{commands:?}"
            );
            assert!(
                !commands.iter().any(|line| line.starts_with("OPTS MLST")),
                "{commands:?}"
            );
            peer.finish().unwrap();
        }
    });
}

#[test]
fn ftp_listing_without_the_named_entry_reports_not_found() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            modification: UNSUPPORTED,
            listing: listing(ftp_server::machine("other.pac", Some("20240101120000"))),
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.is::<NotFound>(), "{error}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_unparsable_listing_reports_its_own_error() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            modification: UNSUPPORTED,
            listing: listing(vec!["readable nonsense".to_owned()]),
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "The FTP server uses a directory-listing format other than MLSD, Unix or DOS"
        );
        assert!(!error.is::<NotFound>());
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_listing_refusal_keeps_the_server_status() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            modification: UNSUPPORTED,
            listing: Listing {
                lines: Vec::new(),
                refusal: Some("550 listing refused"),
            },
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("550 listing refused"), "{error}");
        assert!(!error.is::<NotFound>());
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_unchanged_instant_reuses_the_retained_script() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        let unchanged = load(&source(&peer, "", "/script.pac"), Some(&loaded.cache)).await;
        assert!(unchanged.unwrap_err().is::<NotModified>());
        let modified = peer
            .commands()
            .into_iter()
            .filter(|line| line.starts_with("MDTM"))
            .count();
        assert_eq!(modified, 2, "the second load revalidated the file");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_changed_instant_reloads_the_script() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![
            Scenario::default(),
            Scenario {
                transfer: Transfer::body(b"second script".to_vec()),
                ..Scenario::default()
            },
            Scenario {
                modification: Modification::Instant("20240101120001"),
                transfer: Transfer::body(b"second script".to_vec()),
                ..Scenario::default()
            },
        ])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        let cached = load(&source(&peer, "", "/script.pac"), Some(&loaded.cache)).await;
        assert!(
            cached.unwrap_err().is::<NotModified>(),
            "equal instants keep the script"
        );
        let reloaded = load(&source(&peer, "", "/script.pac"), Some(&loaded.cache))
            .await
            .unwrap();
        assert_eq!(reloaded.bytes, b"second script");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_subsecond_instants_decide_reuse() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![
            Scenario {
                modification: Modification::Instant("20240101120000.500"),
                transfer: Transfer::body(b"first".to_vec()),
                ..Scenario::default()
            },
            Scenario {
                modification: Modification::Instant("20240101120000.000"),
                transfer: Transfer::body(b"second".to_vec()),
                ..Scenario::default()
            },
            Scenario {
                modification: Modification::Instant("20240101120000.500"),
                transfer: Transfer::body(b"third".to_vec()),
                ..Scenario::default()
            },
        ])
        .unwrap();
        let first = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert!(matches!(
            first.cache,
            get_uri::Cache::Ftp(Some(1_704_110_400_500))
        ));
        let reloaded = load(&source(&peer, "", "/script.pac"), Some(&first.cache))
            .await
            .unwrap();
        assert_eq!(reloaded.bytes, b"second");
        let reloaded = load(&source(&peer, "", "/script.pac"), Some(&reloaded.cache))
            .await
            .unwrap();
        assert_eq!(reloaded.bytes, b"third");
        let unchanged = load(&source(&peer, "", "/script.pac"), Some(&reloaded.cache)).await;
        assert!(unchanged.unwrap_err().is::<NotModified>());
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_failed_login_keeps_the_server_status() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            login: false,
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("530 login incorrect"), "{error}");
        let commands = peer.commands();
        assert!(
            !commands.iter().any(|line| line.starts_with("MDTM")),
            "{commands:?}"
        );
        assert!(
            !commands.iter().any(|line| line.starts_with("RETR")),
            "{commands:?}"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_transfer_refusal_reports_not_found() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                refusal: Some("550 no such file"),
                ..Transfer::body(Vec::new())
            },
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.is::<NotFound>(), "{error}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_transfer_refusal_does_not_retry_the_data_connection() {
    crate::tests::runtime::run(async {
        // The server offers extended passive mode only, so a second preparation would replace the
        // file's own refusal with an unrelated one.
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                refusal: Some("550 no such file"),
                ..Transfer::body(Vec::new())
            },
            passive: Passive {
                extended: Some('|'),
                standard: None,
                extended_port: None,
            },
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.is::<NotFound>(), "{error}");
        let commands = peer.commands();
        assert_eq!(transfers(&commands), 1, "{commands:?}");
        assert!(
            !commands.iter().any(|line| line.starts_with("PASV")),
            "{commands:?}"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_transient_transfer_refusal_is_not_retried() {
    crate::tests::runtime::run(async {
        // A transient refusal of the transfer command is the caller's answer, not a reason to
        // prepare the data connection again and ask once more.
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                refusal: Some("450 file busy"),
                ..Transfer::body(Vec::new())
            },
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("450 file busy"), "{error}");
        assert!(!error.is::<NotFound>(), "{error}");
        let commands = peer.commands();
        assert_eq!(transfers(&commands), 1, "{commands:?}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_ibm_i_extended_passive_reply_transfers() {
    crate::tests::runtime::run(async {
        // A server on IBM i delimits its extended passive port with `!` rather than `|`.
        let peer = ftp_server::serve(vec![Scenario {
            passive: Passive {
                extended: Some('!'),
                standard: None,
                extended_port: None,
            },
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        let commands = peer.commands();
        assert!(commands.contains(&"EPSV".to_owned()), "{commands:?}");
        assert!(
            !commands.iter().any(|line| line.starts_with("PASV")),
            "{commands:?}"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_interrupted_transfer_reports_an_error() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                body: b"half a script, cut off".to_vec(),
                completion: Some("426 transfer aborted"),
                ..Transfer::body(Vec::new())
            },
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(!error.is::<NotFound>(), "{error}");
        assert!(error.to_string().contains("426"), "{error}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_closes_control_and_data_connections() {
    crate::tests::runtime::run(async {
        for (case, scenario, transferred) in [
            ("success", Scenario::default(), true),
            (
                "missing",
                Scenario {
                    modification: Modification::MISSING,
                    ..Scenario::default()
                },
                false,
            ),
            (
                "refused listing",
                Scenario {
                    modification: UNSUPPORTED,
                    listing: Listing {
                        lines: Vec::new(),
                        refusal: Some("550 listing refused"),
                    },
                    ..Scenario::default()
                },
                false,
            ),
            (
                "refused transfer",
                Scenario {
                    transfer: Transfer {
                        refusal: Some("550 no such file"),
                        ..Transfer::body(Vec::new())
                    },
                    ..Scenario::default()
                },
                false,
            ),
            (
                "rejected login",
                Scenario {
                    login: false,
                    ..Scenario::default()
                },
                false,
            ),
        ] {
            let peer = ftp_server::serve(vec![scenario]).unwrap();
            let _ = load(&source(&peer, "", "/script.pac"), None).await;
            let closed = peer.finish().unwrap();
            assert!(closed.control, "{case}");
            assert_eq!(closed.data, transferred, "{case}");
        }
    });
}

#[test]
fn ftp_cancelled_load_releases_the_connection() {
    crate::tests::runtime::run(async {
        // The server sends these bytes and keeps the data connection open, so the load can only
        // end by being cancelled.
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                completion: None,
                ..Transfer::body(b"half a script".to_vec())
            },
            ..Scenario::default()
        }])
        .unwrap();
        let uri = url::Url::parse(&source(&peer, "", "/script.pac")).unwrap();
        let task = tokio::spawn(async move {
            let fetch = configured_fetch(Route::default());
            get_uri::load(&uri, None, &fetch).await
        });
        peer.reached("RETR", 1).await;
        assert!(
            !task.is_finished(),
            "the load must still be reading when it is cancelled"
        );
        task.abort();
        let outcome = task.await.expect_err("cancelling the load fails it");
        assert!(outcome.is_cancelled(), "{outcome}");
        let closed = peer.finish().unwrap();
        assert!(closed.control, "the control connection closed");
        assert!(closed.data, "the pending data connection closed");
    });
}

#[test]
fn ftp_path_segments_follow_the_reference_rules() {
    // The loader splits a source path with the path owner, so this pins the rules it relies on,
    // including the empty segment that separates it from trimming each side.
    for (path, directory, name) in [
        ("/script.pac", "/", "script.pac"),
        ("/a/b/c.pac", "/a/b", "c.pac"),
        ("/a/b/", "/a", "b"),
        ("/", "/", ""),
        ("/a", "/", "a"),
        ("script.pac", ".", "script.pac"),
        ("/a//b.pac", "/a/", "b.pac"),
    ] {
        assert_eq!(maestro_path::posix::dirname(path), directory, "{path}");
        assert_eq!(maestro_path::posix::basename(path, None), name, "{path}");
    }
}

#[test]
fn ftp_listing_fallback_asks_for_the_empty_path_segment() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            modification: UNSUPPORTED,
            listing: listing(ftp_server::machine("b.pac", Some("20240101120000"))),
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/a//b.pac"), None).await.unwrap();
        assert!(matches!(
            loaded.cache,
            get_uri::Cache::Ftp(Some(1_704_110_400_000))
        ));
        let commands = peer.commands();
        assert!(
            commands.contains(&"MDTM /a//b.pac".to_owned()),
            "{commands:?}"
        );
        assert!(commands.contains(&"MLSD /a/".to_owned()), "{commands:?}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_epsv_only_server_transfers() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            passive: Passive {
                extended: Some('|'),
                standard: None,
                extended_port: None,
            },
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        let commands = peer.commands();
        assert!(commands.contains(&"EPSV".to_owned()), "{commands:?}");
        assert!(
            !commands.iter().any(|line| line.starts_with("PASV")),
            "{commands:?}"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_advertised_data_host_is_kept() {
    // The data listener answers on a second loopback address, which not every host has.
    if std::net::TcpListener::bind("127.0.0.2:0").is_err() {
        eprintln!("the advertised data host needs a host that binds 127.0.0.2");
        return;
    }
    crate::tests::runtime::run(async {
        // The server offers no extended passive mode and advertises a data host of its own, which
        // is the only address its data listener answers on.
        let peer = ftp_server::serve(vec![Scenario {
            passive: Passive {
                extended: None,
                standard: Some(Ipv4Addr::new(127, 0, 0, 2)),
                extended_port: None,
            },
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        let commands = peer.commands();
        assert!(
            commands.iter().any(|line| line.starts_with("PASV")),
            "{commands:?}"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_silent_server_times_out_the_control_connection() {
    run_held(async {
        let peer = silent(Silence {
            modification: false,
            ..Silence::EVERY
        });
        let mut running = loading(source(&peer, "", "/script.pac")).await;
        // The window starts once the loader waits for the reply the peer never sends.
        running.reached(&peer, "MDTM", 1).await;
        let origin = Instant::now();
        to(origin, before(30)).await;
        running.pending("the wait is still inside its window");
        to(origin, INACTIVITY).await;
        expires(running, origin, INACTIVITY, "control socket").await;
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_silent_data_connection_times_out() {
    run_held(async {
        let origin = Instant::now();
        let mut running = Running::spawn(async { payload(&mut Silent, None).await }).await;
        running.pending("the read awaits a byte");
        to(origin, before(30)).await;
        running.pending("the read is still inside its window");
        to(origin, INACTIVITY).await;
        expires(running, origin, INACTIVITY, "data socket").await;
    });
}

/// A data connection that never sends another byte.
struct Silent;

impl tokio::io::AsyncRead for Silent {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Pending
    }
}

#[test]
fn ftp_login_window_resets_for_each_reply() {
    run_held(async {
        let (releasing, held) = std::sync::mpsc::channel();
        let peer = ftp_server::serve(vec![Scenario {
            held: Some(Held {
                commands: &["USER", "PASS"],
                releases: held,
            }),
            ..Scenario::default()
        }])
        .unwrap();
        // Declared after the peer, so a failing step drops every sender before the peer waits for
        // its held reply.
        let releases = releasing;
        let mut running = loading(source(&peer, "", "/script.pac")).await;
        running.reached(&peer, "USER", 1).await;
        tokio::time::advance(LOGIN_WAIT).await;
        running.pending("the USER reply has its own window");
        releases.send(()).unwrap();
        running.reached(&peer, "PASS", 1).await;
        tokio::time::advance(LOGIN_WAIT).await;
        running.pending("the PASS reply has its own window, past the first window's deadline");
        releases.send(()).unwrap();
        let (_, load) = running.ended().await;
        let loaded = load.expect("both replies are inside their own windows");
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_teardown_does_not_wait_for_the_quit_reply() {
    crate::tests::runtime::run(async {
        // The server answers every command but `QUIT`, so a completed load only returns when its
        // teardown stops waiting for that reply.
        let peer = silent(Silence {
            quit: false,
            ..Silence::EVERY
        });
        let started = std::time::Instant::now();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        assert!(
            started.elapsed() < INACTIVITY,
            "the load waited {:?} for the QUIT reply",
            started.elapsed()
        );
        peer.reached("QUIT", 1).await;
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_listing_read_stops_at_the_reference_limit() {
    crate::tests::runtime::run(async {
        let mut data: &[u8] = b"0123456789";
        let error = payload(&mut data, Some(4)).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("The FTP server sent a directory listing larger than"),
            "{error}"
        );
        let mut data: &[u8] = b"0123456789";
        assert_eq!(payload(&mut data, Some(10)).await.unwrap(), b"0123456789");
        let mut data: &[u8] = b"0123456789";
        assert_eq!(payload(&mut data, None).await.unwrap(), b"0123456789");
    });
}

#[test]
fn ftp_listing_beyond_the_reference_limit_is_refused() {
    crate::tests::runtime::run(async {
        // The reference client refuses a listing beyond 40 MiB instead of accumulating it, so a
        // server that keeps sending is stopped rather than answered.
        let line = "type=file;size=4;modify=20240101120000; filler.pac";
        let mut lines = Vec::new();
        let mut bytes = 0;
        while bytes <= 40 * 1024 * 1024 {
            lines.push(line.to_owned());
            bytes += line.len() + 2;
        }
        let peer = ftp_server::serve(vec![Scenario {
            modification: Modification::Reply("500 unknown command"),
            listing: listing(lines),
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("The FTP server sent a directory listing larger than 41943040 bytes"),
            "{error}"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_instants_parse_whole_and_fractional_seconds() {
    assert_eq!(instant_millis("20240101120000"), Some(1_704_110_400_000));
    assert_eq!(
        instant_millis("20240101120000.500"),
        Some(1_704_110_400_500)
    );
    assert_eq!(
        instant_millis("20240101120000.000"),
        Some(1_704_110_400_000)
    );
    for malformed in ["", "garbage", "2024010112000", "2024010112000x"] {
        assert_eq!(instant_millis(malformed), None, "{malformed}");
    }
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            modification: Modification::Instant("20240101120000.500"),
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_control_activity_keeps_greeting_alive() {
    run_held(async {
        let (releasing, held) = gated();
        let peer = ftp_server::serve(vec![Scenario {
            replies: Some((
                &[""],
                Dribble {
                    parts: &[b"2", b"20 scripted", b" ftp ready\r\n"],
                    releases: held,
                },
            )),
            ..Scenario::default()
        }])
        .unwrap();
        let releases = releasing;
        let mut running = loading(source(&peer, "", "/script.pac")).await;
        running.reached(&peer, &marker("piece-0"), 1).await;
        tokio::time::advance(LOGIN_WAIT).await;
        running.pending("a greeting piece keeps the window open");
        releases.send(()).unwrap();
        running.reached(&peer, &marker("piece-1"), 1).await;
        tokio::time::advance(LOGIN_WAIT).await;
        running.pending("the second piece resets the window");
        releases.send(()).unwrap();
        let (_, loaded) = running.ended().await;
        let loaded = loaded.expect("the dribbled greeting is admitted");
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        peer.finish().unwrap();
    });
    crate::tests::runtime::run(async {
        // A greeting that is not a positive completion is its own refusal.
        for greeting in [
            "120 service ready in five minutes",
            "331 need an account",
            "550 no service",
        ] {
            let peer = ftp_server::serve(vec![Scenario {
                greeting,
                ..Scenario::default()
            }])
            .unwrap();
            let error = load(&source(&peer, "", "/script.pac"), None)
                .await
                .unwrap_err();
            assert_eq!(error.to_string(), greeting);
            assert!(peer.commands().is_empty(), "{greeting}");
            peer.finish().unwrap();
        }
    });
}

#[test]
fn ftp_control_activity_keeps_feature_reply_alive() {
    run_held(async {
        let (releasing, held) = gated();
        let peer = ftp_server::serve(vec![Scenario {
            replies: Some((
                &["FEAT"],
                Dribble {
                    parts: &[
                        b"211-Features:\r\n ML",
                        b"ST\r\n 220 inner\r\n MD",
                        b"TM\r\n211 End\r\n",
                    ],
                    releases: held,
                },
            )),
            ..Scenario::default()
        }])
        .unwrap();
        let releases = releasing;
        let mut running = loading(source(&peer, "", "/script.pac")).await;
        running.reached(&peer, "FEAT", 1).await;
        running.reached(&peer, &marker("piece-0"), 1).await;
        tokio::time::advance(LOGIN_WAIT).await;
        running.pending("a multiline feature reply keeps its window open");
        releases.send(()).unwrap();
        running.reached(&peer, &marker("piece-1"), 1).await;
        tokio::time::advance(LOGIN_WAIT).await;
        running.pending("an inner line with another code does not end the group");
        releases.send(()).unwrap();
        let (_, loaded) = running.ended().await;
        let loaded = loaded.expect("the dribbled feature reply is read");
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        let commands = peer.commands();
        assert!(
            commands
                .iter()
                .any(|line| line.starts_with("OPTS MLST type;size;modify;")),
            "{commands:?}"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_control_activity_keeps_completion_alive() {
    run_held(async {
        let (releasing, held) = gated();
        let peer = ftp_server::serve(vec![Scenario {
            completion: Some(Dribble {
                parts: &[b"22", b"6 transfer complete\r\n"],
                releases: held,
            }),
            ..Scenario::default()
        }])
        .unwrap();
        let releases = releasing;
        let mut running = loading(source(&peer, "", "/script.pac")).await;
        running.reached(&peer, &marker("piece-0"), 1).await;
        tokio::time::advance(LOGIN_WAIT).await;
        running.pending("completion bytes after the data end still reset the control window");
        releases.send(()).unwrap();
        let (_, loaded) = running.ended().await;
        let loaded = loaded.expect("the dribbled completion settles the transfer");
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_split_control_prefix_keeps_the_complete_reply() {
    let mut frames = Frames::default();
    let mut replies = Vec::new();
    for piece in ["2", "2", "0 Ready\r\n", "2", "20 S\u{e9}", "cond\r\n"] {
        replies.extend(frames.push(piece).unwrap());
    }
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0].message, "220 Ready");
    assert_eq!(replies[1].message, "220 S\u{e9}cond");

    // Incomplete input at the end of the connection is a failure, not an empty reply.
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            greeting: "220 incomplete",
            hang_up: true,
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Server sent FIN packet unexpectedly, closing connection."
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_data_activity_outlives_a_reply_window() {
    run_held(async {
        let (releasing, held) = gated();
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                dribble: Some(Dribble {
                    parts: &[b"abc", b"def", b"ghi"],
                    releases: held,
                }),
                ..Transfer::body(Vec::new())
            },
            ..Scenario::default()
        }])
        .unwrap();
        let releases = releasing;
        let mut running = loading(source(&peer, "", "/script.pac")).await;
        running.reached(&peer, "RETR", 1).await;
        for piece in ["piece-0", "piece-1"] {
            running.reached(&peer, &marker(piece), 1).await;
            tokio::time::advance(LOGIN_WAIT).await;
            running.pending("data bytes keep the transfer alive past a control window");
            releases.send(()).unwrap();
        }
        let (_, loaded) = running.ended().await;
        let loaded = loaded.expect("the dribbled transfer completes");
        assert_eq!(loaded.bytes, b"abcdefghi");
        peer.finish().unwrap();
    });
}

/// A scenario whose `RETR` answer waits until the test admits the transfer.
fn admitted_by(admission: std::sync::mpsc::Receiver<()>, transfer: Transfer) -> Scenario {
    Scenario {
        held: Some(Held {
            commands: &["RETR"],
            releases: admission,
        }),
        transfer,
        ..Scenario::default()
    }
}

/// A transfer whose data connection sends these pieces, one per release, and then stays open
/// until its close is released.
fn dribbled(
    parts: &'static [&'static [u8]],
    pieces: std::sync::mpsc::Receiver<()>,
    close: std::sync::mpsc::Receiver<()>,
) -> Transfer {
    Transfer {
        dribble: Some(Dribble {
            parts,
            releases: pieces,
        }),
        close_after: Some(close),
        ..Transfer::body(Vec::new())
    }
}

#[test]
fn ftp_data_window_starts_at_admission() {
    // The data connection connects before the command and stays silent; the preliminary reply
    // that admits the transfer twenty seconds later starts its window.
    for opening in ["150", "125"] {
        run_held(async move {
            let (admit, admission) = gated();
            let (piece, pieces) = gated();
            let (close, closing) = gated();
            let replies = (opening == "125").then(|| {
                (
                    &["RETR"] as &'static [&'static str],
                    whole(&[b"125 data connection already open\r\n"]),
                )
            });
            let peer = ftp_server::serve(vec![Scenario {
                replies,
                ..admitted_by(admission, dribbled(&[b"", b"never"], pieces, closing))
            }])
            .unwrap();
            let mut running = loading(source(&peer, "", "/script.pac")).await;
            running.reached(&peer, "RETR", 1).await;
            let origin = Instant::now();
            to(origin, seconds(20)).await;
            admit.send(()).unwrap();
            running.reached(&peer, &marker("piece-0"), 1).await;
            to(origin, seconds(30)).await;
            running.pending(&format!("{opening}: the data window starts at admission"));
            to(origin, before(50)).await;
            running.pending(&format!("{opening}: the admitted window is still open"));
            to(origin, seconds(50)).await;
            expires(running, origin, seconds(50), "data socket").await;
            drop((piece, close));
            peer.finish().unwrap();
        });
    }
}

#[test]
fn ftp_late_data_byte_moves_the_admitted_window() {
    // (admitted at, first byte at): the byte restarts the window wherever the admission put it.
    for (admitted, byte) in [(20, 35), (0, 25)] {
        run_held(async move {
            let (admit, admission) = gated();
            let (piece, pieces) = gated();
            let (close, closing) = gated();
            let peer = ftp_server::serve(vec![admitted_by(
                admission,
                dribbled(&[b"", b"x"], pieces, closing),
            )])
            .unwrap();
            let mut running = loading(source(&peer, "", "/script.pac")).await;
            running.reached(&peer, "RETR", 1).await;
            let origin = Instant::now();
            to(origin, seconds(admitted)).await;
            admit.send(()).unwrap();
            running.reached(&peer, &marker("piece-0"), 1).await;
            to(origin, seconds(byte)).await;
            running.pending("the admitted window is open when the byte arrives");
            piece.send(()).unwrap();
            running.reached(&peer, &marker("piece-1"), 1).await;
            to(origin, seconds(admitted + 30)).await;
            running.pending("the byte replaced the admitted window");
            to(origin, before(byte + 30)).await;
            running.pending("the byte's window is still open");
            to(origin, seconds(byte + 30)).await;
            expires(running, origin, seconds(byte + 30), "data socket").await;
            drop(close);
            peer.finish().unwrap();
        });
    }
}

#[test]
fn ftp_early_completion_keeps_the_data_window() {
    // (admitted at, completion at): a completion before the data ends neither ends nor restarts
    // the data window.
    for (admitted, completion) in [(20, 40), (0, 20)] {
        run_held(async move {
            let (admit, admission) = gated();
            let (piece, pieces) = gated();
            let (close, closing) = gated();
            let peer = ftp_server::serve(vec![admitted_by(
                admission,
                Transfer {
                    completion_first: true,
                    ..dribbled(&[b"", b""], pieces, closing)
                },
            )])
            .unwrap();
            let mut running = loading(source(&peer, "", "/script.pac")).await;
            running.reached(&peer, "RETR", 1).await;
            let origin = Instant::now();
            to(origin, seconds(admitted)).await;
            admit.send(()).unwrap();
            running.reached(&peer, &marker("piece-0"), 1).await;
            to(origin, seconds(completion)).await;
            piece.send(()).unwrap();
            running.reached(&peer, &marker("completion-sent"), 1).await;
            to(origin, before(admitted + 30)).await;
            running.pending("the completion leaves the data window open");
            to(origin, seconds(admitted + 30)).await;
            expires(running, origin, seconds(admitted + 30), "data socket").await;
            drop(close);
            peer.finish().unwrap();
        });
    }
}

#[test]
fn ftp_data_end_hands_the_window_back_to_control() {
    // The data ends at 25 seconds; the completion never comes, or its first fragment comes at 45.
    for fragment in [None, Some(45)] {
        run_held(async move {
            let (piece, pieces) = gated();
            let (close, closing) = gated();
            let (complete, completing) = gated();
            let peer = ftp_server::serve(vec![Scenario {
                transfer: dribbled(&[b"", b"x"], pieces, closing),
                completion: Some(Dribble {
                    parts: &[b"", b"22", b"6 transfer complete\r\n"],
                    releases: completing,
                }),
                ..Scenario::default()
            }])
            .unwrap();
            let mut running = loading(source(&peer, "", "/script.pac")).await;
            running.reached(&peer, &marker("piece-0"), 1).await;
            let origin = Instant::now();
            to(origin, seconds(20)).await;
            piece.send(()).unwrap();
            running.reached(&peer, &marker("piece-1"), 1).await;
            to(origin, seconds(25)).await;
            close.send(()).unwrap();
            // The peer starts its completion only once the client closed its end of the data.
            running.reached(&peer, &marker("piece-0"), 2).await;
            let expiry = match fragment {
                None => 55,
                Some(at) => {
                    to(origin, seconds(at)).await;
                    complete.send(()).unwrap();
                    running.reached(&peer, &marker("piece-1"), 2).await;
                    at + 30
                }
            };
            to(origin, before(expiry)).await;
            running.pending("the control window runs from its last activity");
            to(origin, seconds(expiry)).await;
            expires(running, origin, seconds(expiry), "control socket").await;
            drop(complete);
            peer.finish().unwrap();
        });
    }
}

#[test]
fn ftp_optional_commands_keep_terminal_failures() {
    run_held(async {
        for (command, silenced) in [
            ("OPTS", &["OPTS"] as &'static [&'static str]),
            ("FEAT", &["FEAT"]),
            ("STRU", &["STRU"]),
        ] {
            let peer = silent(Silence {
                commands: silenced,
                ..Silence::EVERY
            });
            let mut running = loading(source(&peer, "", "/script.pac")).await;
            running.reached(&peer, command, 1).await;
            let origin = Instant::now();
            to(origin, before(30)).await;
            running.pending("a silent optional command waits for its window");
            to(origin, INACTIVITY).await;
            expires(running, origin, INACTIVITY, "control socket").await;
            assert!(
                peer.commands()
                    .last()
                    .is_some_and(|last| last.starts_with(command)),
                "{command}: {:?}",
                peer.commands()
            );
            peer.finish().unwrap();
        }
    });
}

#[test]
fn ftp_password_never_enters_diagnostics() {
    // A sentinel assembled at run time, so it is not a credential-shaped literal.
    let sentinel = concat!("hunter2", "-sentinel");
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        let loaded = load(
            &source(&peer, &format!("user:{sentinel}@"), "/script.pac"),
            None,
        )
        .await
        .unwrap();
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        assert!(
            peer.commands().contains(&format!("PASS {sentinel}")),
            "the decoded password reaches the private wire"
        );
        peer.finish().unwrap();
    });
    rejected_credentials_stay_out_of_diagnostics(sentinel);
    refused_login_stays_out_of_diagnostics(sentinel);
}

/// A rejected command reports its control characters without the command.
fn rejected_credentials_stay_out_of_diagnostics(sentinel: &str) {
    crate::tests::runtime::run(async {
        for injected in [
            format!("{sentinel}\rextra"),
            format!("{sentinel}\n"),
            format!("{sentinel}\0"),
        ] {
            let encoded: String = percent_encoding::utf8_percent_encode(
                &injected,
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string();
            let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
            let error = load(
                &source(&peer, &format!("user:{encoded}@"), "/script.pac"),
                None,
            )
            .await
            .unwrap_err();
            assert_eq!(
                error.to_string(),
                "Invalid command: Contains control characters."
            );
            for shown in [error.to_string(), format!("{error:?}")] {
                assert!(!shown.contains(sentinel), "{shown}");
                assert!(!shown.contains("PASS"), "{shown}");
            }
            assert!(
                !peer.commands().iter().any(|line| line.starts_with("PASS")),
                "the rejected command never reaches the wire"
            );
            peer.finish().unwrap();
        }
    });
}

/// A refused login keeps the server's answer and no credential.
fn refused_login_stays_out_of_diagnostics(sentinel: &str) {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            login: false,
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(
            &source(&peer, &format!("user:{sentinel}@"), "/script.pac"),
            None,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("530 login incorrect"), "{error}");
        for shown in [error.to_string(), format!("{error:?}")] {
            assert!(!shown.contains(sentinel), "{shown}");
        }
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_credential_never_reaches_a_captured_trace() {
    // A sentinel assembled at run time, so it is not a credential-shaped literal.
    let sentinel = concat!("hunter2", "-sentinel");
    // The control: the same capture sees a DEBUG event that carries this value, so the silence
    // asserted below means nothing logged it rather than nothing was watched.
    let control = Captured::default();
    maestro_test_tracing::with_default(control.clone(), || super::probe_credential(sentinel));
    assert!(
        control.text().contains(sentinel),
        "the capture sees a DEBUG event carrying the sentinel"
    );

    let traces = Captured::default();
    let loaded = maestro_test_tracing::with_default(traces.clone(), || {
        crate::tests::runtime::run(async {
            let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
            let loaded = load(
                &source(&peer, &format!("user:{sentinel}@"), "/script.pac"),
                None,
            )
            .await
            .expect("the login completes");
            assert!(
                peer.commands().contains(&format!("PASS {sentinel}")),
                "the decoded password reaches the private wire"
            );
            peer.finish().unwrap();
            loaded
        })
    });
    assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
    let captured = traces.text();
    assert!(
        !captured.contains(sentinel),
        "the load logs no credential: {captured}"
    );
    assert!(
        !captured.contains("PASS"),
        "the load logs no command: {captured}"
    );
}

/// Every field of every event one scoped subscriber received, rendered as `name=value`.
#[derive(Clone, Default)]
struct Captured(std::sync::Arc<std::sync::Mutex<String>>);

impl Captured {
    /// The text every event wrote.
    fn text(&self) -> String {
        self.0.lock().unwrap().clone()
    }
}

impl tracing::Subscriber for Captured {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut fields = Fields(String::new());
        event.record(&mut fields);
        let mut captured = self.0.lock().unwrap();
        captured.push_str(&fields.0);
        captured.push('\n');
    }
}

/// The fields one event carried.
struct Fields(String);

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write as _;
        write!(self.0, "{}={value:?} ", field.name()).ok();
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        use std::fmt::Write as _;
        write!(self.0, "{}={value} ", field.name()).ok();
    }
}

#[test]
fn ftp_control_and_data_finish_in_either_order() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        peer.finish().unwrap();
    });
    run_held(async {
        let (releasing, held) = gated();
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                completion_first: true,
                close_after: Some(held),
                ..Transfer::body(b"early completion".to_vec())
            },
            ..Scenario::default()
        }])
        .unwrap();
        let releases = releasing;
        let mut running = loading(source(&peer, "", "/script.pac")).await;
        running.reached(&peer, &marker("completion-sent"), 1).await;
        running.pending("an early completion waits for the end of the data");
        releases.send(()).unwrap();
        let (_, loaded) = running.ended().await;
        let loaded = loaded.expect("both endings settle the transfer");
        assert_eq!(loaded.bytes, b"early completion");
        peer.finish().unwrap();
    });
    crate::tests::runtime::run(async {
        // A reply that asks for an answer the client cannot give fails instead of hanging.
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                completion: Some("350 restart at 3"),
                ..Transfer::body(b"rest".to_vec())
            },
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Unexpected FTP response is requesting an answer: 350 restart at 3"
        );
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_control_failure_interrupts_data_read() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer {
                body: b"half a script".to_vec(),
                keep_open: true,
                completion: Some("550 no such file"),
                ..Transfer::body(Vec::new())
            },
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.is::<NotFound>(), "{error}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_passive_connect_failure_tries_the_remaining_strategy() {
    crate::tests::runtime::run(async {
        // The extended passive reply announces a port nothing listens on, so the session falls
        // back to the classic mode and submits the transfer once.
        let peer = ftp_server::serve(vec![Scenario {
            passive: Passive {
                extended: Some('|'),
                standard: Some(Ipv4Addr::LOCALHOST),
                extended_port: Some(1),
            },
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert_eq!(loaded.bytes, b"function FindProxyForURL(){return 'DIRECT'}");
        let commands = peer.commands();
        assert!(commands.contains(&"EPSV".to_owned()), "{commands:?}");
        assert!(
            commands.iter().any(|line| line.starts_with("PASV")),
            "{commands:?}"
        );
        assert_eq!(transfers(&commands), 1, "{commands:?}");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_list_candidates_retain_the_first_successful_choice() {
    crate::tests::runtime::run(async {
        // With a machine listing advertised, a refused MLSD lets the first LIST candidate answer,
        // and the machine entry it carries still supplies the instant.
        let peer = ftp_server::serve(vec![Scenario {
            replies: Some((&["MLSD"], whole(&[b"550 no machine listing\r\n"]))),
            modification: UNSUPPORTED,
            listing: listing(ftp_server::machine("script.pac", Some("20240101120000"))),
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert!(matches!(
            loaded.cache,
            get_uri::Cache::Ftp(Some(1_704_110_400_000))
        ));
        let commands = peer.commands();
        let order: Vec<&str> = candidates(&commands);
        assert_eq!(order, ["MLSD /", "LIST -a /"], "{commands:?}");
        peer.finish().unwrap();

        // Without a machine listing, only the LIST candidates are tried, in order.
        let peer = ftp_server::serve(vec![Scenario {
            features: TEXT_FEATURES,
            modification: UNSUPPORTED,
            listing: listing(ftp_server::machine("script.pac", Some("20240101120000"))),
            ..Scenario::default()
        }])
        .unwrap();
        let loaded = load(&source(&peer, "", "/script.pac"), None).await.unwrap();
        assert!(matches!(
            loaded.cache,
            get_uri::Cache::Ftp(Some(1_704_110_400_000))
        ));
        let commands = peer.commands();
        assert_eq!(candidates(&commands), ["LIST -a /"], "{commands:?}");
        peer.finish().unwrap();

        // Every candidate refusing keeps the server's last answer.
        let peer = ftp_server::serve(vec![Scenario {
            modification: UNSUPPORTED,
            listing: Listing {
                lines: Vec::new(),
                refusal: Some("550 listing refused"),
            },
            ..Scenario::default()
        }])
        .unwrap();
        let error = load(&source(&peer, "", "/script.pac"), None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("550 listing refused"), "{error}");
        let commands = peer.commands();
        assert_eq!(
            candidates(&commands),
            ["MLSD /", "LIST -a /", "LIST /"],
            "{commands:?}"
        );
        peer.finish().unwrap();
    });
}

/// The listing commands the peer read, in order.
fn candidates(commands: &[String]) -> Vec<&str> {
    commands
        .iter()
        .map(String::as_str)
        .filter(|line| ["MLSD", "LIST"].iter().any(|verb| line.starts_with(verb)))
        .collect()
}

#[test]
fn ftp_control_response_limit_is_retained() {
    let mut frames = Frames::default();
    let boundary = format!("220 {}", "x".repeat(65_532));
    assert_eq!(boundary.encode_utf16().count(), 65_536);
    assert!(frames.push(&boundary).is_ok(), "the boundary is admitted");
    assert!(frames.push("x").is_err(), "one unit more is refused");

    let mut multibyte = Frames::default();
    let boundary = format!("220 {}", "\u{1d11e}".repeat(32_766));
    assert_eq!(boundary.encode_utf16().count(), 65_536);
    assert!(multibyte.push(&boundary).is_ok(), "units, not bytes, count");
    assert!(
        multibyte.push("\u{1d11e}").is_err(),
        "one scalar more is refused"
    );

    let mut consumed = Frames::default();
    for line in ["220 ", "220 "] {
        let reply = format!("{line}{}\r\n", "y".repeat(40_000));
        assert_eq!(consumed.push(&reply).unwrap().len(), 1);
    }
}

#[test]
fn ftp_control_parser_work_stays_linear() {
    // One multiline reply, pushed one byte at a time, as a fragmented peer sends it.
    let reply = |lines: usize| {
        let mut text = String::from("211-Features:\r\n");
        for _ in 0..lines {
            text.push_str(" MLST type;size;modify;\r\n");
        }
        text.push_str("211 End\r\n");
        text
    };
    let mut counted = Vec::new();
    for lines in [64, 128, 256] {
        let text = reply(lines);
        let mut frames = Frames::default();
        let mut replies = 0;
        for byte in text.as_bytes() {
            let pushed = std::str::from_utf8(std::slice::from_ref(byte)).unwrap();
            replies += frames.push(pushed).unwrap().len();
        }
        assert_eq!(replies, 1, "{lines} lines form one reply");
        let visits = frames.visits();
        assert!(
            visits <= 6 * text.len(),
            "{lines} lines: {visits} characters examined for {} bytes",
            text.len()
        );
        counted.push((text.len(), rescan_work(&text)));
    }
    // A framer that rescans everything not yet formed into a reply on every read grows with the
    // square of the reply's length, so this control fails the bound the incremental framing
    // passes.
    let (bytes, rescan) = counted[2];
    assert!(
        rescan > 6 * bytes,
        "the rescan negative control must exceed the linear bound: {rescan} for {bytes}"
    );
    let (small, rescan_small) = counted[0];
    assert!(
        rescan / rescan_small > 8,
        "the rescan control grows faster than its input: {rescan_small} for {small} then {rescan}"
    );
}

/// The work of a framer that rescans its retained text on every pushed byte.
fn rescan_work(text: &str) -> usize {
    let mut retained = 0;
    let mut work = 0;
    for _ in text.bytes() {
        retained += 1;
        work += retained;
    }
    work
}
