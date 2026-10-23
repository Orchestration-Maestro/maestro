//! FTP source login, modification checks, listing fallback, transfer and teardown.

use super::super::ftp::{INACTIVITY, instant_millis, payload, port};
use super::super::{self as get_uri, notfound::NotFound, notmodified::NotModified};
use crate::providers::http::{Route, configured_fetch};
use crate::tests::ftp_server::{
    self, Features, Held, Listing, Modification, Passive, Peer, Scenario, Silence, Transfer,
};
use std::future::Future;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

/// The silence a login reply may take and still be inside its own window.
const LOGIN_WAIT: Duration = Duration::from_secs(20);

/// Poll one step of a controlled load, without a waker to say when it may make progress.
fn step<T>(loading: &mut Pin<&mut impl Future<Output = T>>) -> Poll<T> {
    loading
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
}

/// Poll a load until it completes, letting the peer answer between steps.
async fn settle<T>(loading: &mut Pin<&mut impl Future<Output = T>>) -> T {
    loop {
        if let Poll::Ready(output) = step(loading) {
            return output;
        }
        tokio::task::yield_now().await;
    }
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
            assert!(commands.contains(&"LIST /".to_owned()), "{commands:?}");
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
        while !peer.commands().iter().any(|line| line.starts_with("RETR")) {
            tokio::task::yield_now().await;
        }
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
    crate::tests::runtime::run(async {
        // The server offers no extended passive mode and advertises a data host of its own, which
        // is the only address its data listener answers on.
        let peer = ftp_server::serve(vec![Scenario {
            passive: Passive {
                extended: None,
                standard: Some(Ipv4Addr::new(127, 0, 0, 2)),
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
    crate::tests::runtime::run_paused(async {
        let peer = silent(Silence {
            modification: false,
            ..Silence::EVERY
        });
        let url = source(&peer, "", "/script.pac");
        let mut loading = std::pin::pin!(load(&url, None));
        // Step the clock only once the loader waits for the reply the peer never sends.
        while !peer.commands().iter().any(|line| line.starts_with("MDTM")) {
            assert!(step(&mut loading).is_pending());
            tokio::task::yield_now().await;
        }
        tokio::time::advance(INACTIVITY.saturating_sub(Duration::from_secs(1))).await;
        assert!(
            step(&mut loading).is_pending(),
            "the wait is still inside its window"
        );
        tokio::time::advance(Duration::from_secs(2)).await;
        let error = settle(&mut loading)
            .await
            .expect_err("the stalled load fails");
        assert_eq!(error.to_string(), "Timeout (control socket)");
        peer.finish().unwrap();
    });
}

#[test]
fn ftp_silent_data_connection_times_out() {
    crate::tests::runtime::run_paused(async {
        let mut data = Silent;
        let mut reading = std::pin::pin!(payload(&mut data, None));
        assert!(step(&mut reading).is_pending(), "the read awaits a byte");
        tokio::time::advance(INACTIVITY.saturating_sub(Duration::from_secs(1))).await;
        assert!(
            step(&mut reading).is_pending(),
            "the read is still inside its window"
        );
        tokio::time::advance(Duration::from_secs(2)).await;
        let error = reading
            .await
            .expect_err("the stalled data connection fails the read");
        assert_eq!(error.to_string(), "Timeout (data socket)");
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
    crate::tests::runtime::run_paused(async {
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
        let url = source(&peer, "", "/script.pac");
        let mut loading = std::pin::pin!(load(&url, None));
        while !peer.commands().iter().any(|line| line.starts_with("USER")) {
            assert!(step(&mut loading).is_pending());
            tokio::task::yield_now().await;
        }
        tokio::time::advance(LOGIN_WAIT).await;
        assert!(
            step(&mut loading).is_pending(),
            "the USER reply has its own window"
        );
        releases.send(()).unwrap();
        while !peer.commands().iter().any(|line| line.starts_with("PASS")) {
            assert!(step(&mut loading).is_pending());
            tokio::task::yield_now().await;
        }
        tokio::time::advance(LOGIN_WAIT).await;
        assert!(
            step(&mut loading).is_pending(),
            "the PASS reply has its own window, past the first window's deadline"
        );
        releases.send(()).unwrap();
        let load = settle(&mut loading).await;
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
        while !peer.commands().contains(&"QUIT".to_owned()) {
            tokio::task::yield_now().await;
        }
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
