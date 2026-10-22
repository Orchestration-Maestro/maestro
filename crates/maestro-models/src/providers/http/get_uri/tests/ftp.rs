//! FTP source login, modification checks, listing fallback, transfer and teardown.

use super::super::ftp::{basename, dirname, instant_millis, port};
use super::super::{self as get_uri, notfound::NotFound, notmodified::NotModified};
use crate::providers::http::{Route, configured_fetch};
use crate::tests::ftp_server::{self, Features, Listing, Modification, Peer, Scenario, Transfer};

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
            transfer: Transfer::Body(body.clone()),
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
            modification: Modification::Missing,
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
            features: Features::Machine,
            modification: Modification::Unsupported,
            listing: Listing::Lines(lines),
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
fn ftp_text_listing_supplies_no_modification_instant() {
    crate::tests::runtime::run(async {
        for (announced, lines) in [
            (Features::Text, ftp_server::unix("script.pac")),
            (Features::Absent, ftp_server::unix("script.pac")),
            (Features::Refused, ftp_server::unix("script.pac")),
            (Features::Text, ftp_server::dos("script.pac")),
            (Features::Absent, ftp_server::dos("script.pac")),
        ] {
            let peer = ftp_server::serve(vec![Scenario {
                features: announced,
                modification: Modification::Unsupported,
                listing: Listing::Lines(lines),
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
            modification: Modification::Unsupported,
            listing: Listing::Lines(ftp_server::machine("other.pac", Some("20240101120000"))),
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
            modification: Modification::Unsupported,
            listing: Listing::Lines(vec!["readable nonsense".to_owned()]),
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
            modification: Modification::Unsupported,
            listing: Listing::Refused,
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
                transfer: Transfer::Body(b"second script".to_vec()),
                ..Scenario::default()
            },
            Scenario {
                modification: Modification::Instant("20240101120001"),
                transfer: Transfer::Body(b"second script".to_vec()),
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
                transfer: Transfer::Body(b"first".to_vec()),
                ..Scenario::default()
            },
            Scenario {
                modification: Modification::Instant("20240101120000.000"),
                transfer: Transfer::Body(b"second".to_vec()),
                ..Scenario::default()
            },
            Scenario {
                modification: Modification::Instant("20240101120000.500"),
                transfer: Transfer::Body(b"third".to_vec()),
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
            transfer: Transfer::Refused,
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
fn ftp_interrupted_transfer_reports_an_error() {
    crate::tests::runtime::run(async {
        let peer = ftp_server::serve(vec![Scenario {
            transfer: Transfer::Interrupted(b"half a script, cut off".to_vec()),
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
                    modification: Modification::Missing,
                    ..Scenario::default()
                },
                false,
            ),
            (
                "refused listing",
                Scenario {
                    modification: Modification::Unsupported,
                    listing: Listing::Refused,
                    ..Scenario::default()
                },
                false,
            ),
            (
                "refused transfer",
                Scenario {
                    transfer: Transfer::Refused,
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
        let mut peer = ftp_server::serve(vec![Scenario::default()]).unwrap();
        let mut events = peer.events().unwrap();
        let uri = url::Url::parse(&source(&peer, "", "/script.pac")).unwrap();
        let task = tokio::spawn(async move {
            let fetch = configured_fetch(Route::default());
            get_uri::load(&uri, None, &fetch).await
        });
        while let Some(command) = events.recv().await {
            if command.starts_with("RETR") {
                break;
            }
        }
        task.abort();
        let _ = task.await;
        let closed = peer.finish().unwrap();
        assert!(closed.control, "the control connection closed");
        assert!(closed.data, "the pending data connection closed");
    });
}

#[test]
fn ftp_path_segments_follow_the_reference_rules() {
    for (path, directory, name) in [
        ("/script.pac", "/", "script.pac"),
        ("/a/b/c.pac", "/a/b", "c.pac"),
        ("/a/b/", "/a", "b"),
        ("/", "/", ""),
        ("/a", "/", "a"),
        ("script.pac", ".", "script.pac"),
    ] {
        assert_eq!(dirname(path), directory, "{path}");
        assert_eq!(basename(path), name, "{path}");
    }
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
