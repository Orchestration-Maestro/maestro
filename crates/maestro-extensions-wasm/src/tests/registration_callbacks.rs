//! Author registration observations through both component adapters.
use super::scenario::Driver;
use serde_json::json;

/// Exercises distinct callbacks and independent releases with optional metadata.
async fn forwarding(driver: &mut impl Driver) -> Result<(), String> {
    let corpus: super::registration_corpus::FlagCorpus =
        serde_json::from_str(include_str!("registration_corpus.json")).unwrap();
    command_corpus(driver, corpus.commands).await?;
    shortcut_corpus(driver, corpus.shortcuts).await?;
    replacement_capture(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_registration_forwards_each_owned_callback() -> Result<(), String> {
    on_both_adapters!(forwarding)
}

/// Reads a flag through authored code and checks the completed operation's observation.
async fn read_flag(
    driver: &mut impl Driver,
    name: &str,
    expected: Option<&super::registration_fixture::Value>,
) -> Result<(), String> {
    driver
        .run_plain_command("capture", &format!("flag-read {name}"))
        .await?;
    assert_eq!(
        driver.transcript().last(),
        Some(&format!("entry flag-read {}", json!(expected)))
    );
    Ok(())
}

/// Checks kind/default independence and exact host replies against recorded cases.
async fn flags(driver: &mut impl Driver) -> Result<(), String> {
    use super::registration_corpus::FlagCorpus;
    assert_eq!(
        driver.flag_registrations().first(),
        Some(&(
            "factory-flag".into(),
            json!({"description":"","type":"string","default":false})
        ))
    );
    let corpus: FlagCorpus =
        serde_json::from_str(include_str!("registration_corpus.json")).unwrap();
    flag_combinations(driver, corpus.flags).await?;
    literal_flags(driver, corpus.literal_flags).await?;
    flag_replacements(driver, corpus.replacements).await?;
    for case in corpus.live_reads {
        register_flag(
            driver,
            "live",
            Some(case.input),
            super::registration_fixture::Kind::String,
        )
        .await?;
        for value in case.expected {
            driver.flag_reply("live", Ok(value.as_ref().map(|v| json!(v))));
            read_flag(driver, "live", value.as_ref()).await?;
        }
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_flag_values_preserve_absence_false_and_empty() -> Result<(), String> {
    on_both_adapters!(flags)
}

/// Sends a registration without implementing default or override resolution.
async fn register_flag(
    driver: &mut impl Driver,
    name: &str,
    default: Option<super::registration_fixture::Value>,
    kind: super::registration_fixture::Kind,
) -> Result<(), String> {
    let flag = super::registration_fixture::Flag {
        name: name.into(),
        description: None,
        kind,
        default,
    };
    driver
        .run_plain_command("capture", &format!("flag-register {}", json!(flag)))
        .await
}

/// Converts transport observations back to strict candidates, preserving ordered records.
fn candidates(
    items: Option<Vec<serde_json::Value>>,
) -> Option<Vec<super::registration_fixture::Candidate>> {
    items.map(|items| {
        items
            .into_iter()
            .map(|item| serde_json::from_value(item).unwrap())
            .collect()
    })
}

/// Exercises literal inputs and None/empty/list results, including barrier-held futures.
async fn completions(driver: &mut impl Driver) -> Result<(), String> {
    let corpus: super::registration_corpus::FlagCorpus =
        serde_json::from_str(include_str!("registration_corpus.json")).unwrap();
    completion_corpus(driver, corpus.completions).await?;
    driver
        .run_plain_command("capture", "completion-example")
        .await?;
    let id = driver.identity("completion environment")?;
    for case in corpus.example {
        assert_eq!(
            candidates(driver.completion(id, &case.input).await?),
            case.expected
        );
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_completion_results_keep_prefixes_fields_and_order() -> Result<(), String> {
    on_both_adapters!(completions)
}

/// Synchronous rejection and release boundaries close the ownership observation set.
async fn ownership(driver: &mut impl Driver) -> Result<(), String> {
    rejected_ownership(driver).await?;
    driver.registration_error(None);
    release_orders(driver).await?;
    retained_alias(driver).await?;
    driver
        .run_plain_command(
            "capture",
            &format!(
                "ownership {}",
                json!({"name":"owned","completion":true,"alias":false,"reenter":true})
            ),
        )
        .await?;
    assert!(driver.release("completion owned").await);
    assert_eq!(
        driver.transcript().last().unwrap(),
        r#"entry reentered {"Ok":null}"#
    );
    driver.run_plain_command("fresh", "call").await?;
    assert!(driver.release("command fresh").await);
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_registration_rejection_and_release_keep_callback_ownership() -> Result<(), String> {
    on_both_adapters!(ownership)
}

/// Invokes the selected callback, optionally holding its first async barrier.
async fn failure_call(
    driver: &mut impl Driver,
    request: &super::registration_fixture::Failure,
) -> Result<(), String> {
    use super::registration_fixture::{CallbackKind, Mode};
    let held = matches!(request.mode, Mode::Pending);
    match request.kind {
        CallbackKind::Command => {
            if held {
                driver.command_held("failure", "args").await
            } else {
                driver.run_plain_command("failure", "args").await
            }
        }
        CallbackKind::Completion => {
            let id = driver.identity("completion failure")?;
            if held {
                driver.completion_held(id, "prefix").await.map(|_| ())
            } else {
                driver.completion(id, "prefix").await.map(|_| ())
            }
        }
        CallbackKind::Shortcut => {
            let id = driver.identity("shortcut failure")?;
            if held {
                driver.shortcut_held(id, "/error-origin").await
            } else {
                driver.shortcut(id, "/error-origin").await
            }
        }
    }
}

/// Host and author failures retain supplied text and earlier effects without poisoning dispatch.
async fn errors(driver: &mut impl Driver) -> Result<(), String> {
    host_errors(driver).await?;
    let corpus: super::registration_corpus::FlagCorpus =
        serde_json::from_str(include_str!("registration_corpus.json")).unwrap();
    callback_errors(driver, corpus.callback_errors).await?;
    invalid_dispatch(driver).await?;
    driver
        .run_plain_command("capture", "completion-example")
        .await?;
    let released = driver.identity("completion environment")?;
    let diagnostic = driver.callback_error(released);
    assert!(driver.release("completion environment").await);
    let released = driver.reborrow(driver.dropped().last().copied().unwrap())?;
    assert_eq!(
        driver.completion(released, "released").await.unwrap_err(),
        diagnostic
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_registration_errors_are_catchable_without_poisoning_callbacks() -> Result<(), String> {
    on_both_adapters!(errors)
}

/// A running call survives table removal; independent context aliases retain their own origin.
async fn pending(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "pending-registrations")
        .await?;
    let completion = driver.identity("completion pending")?;
    driver.pending_pair(completion, false).await?;
    assert!(
        driver
            .transcript()
            .contains(&r#"entry released "pending completion""#.into())
    );
    let shortcut = driver.identity("shortcut pending")?;
    driver.pending_pair(shortcut, true).await?;
    assert!(
        driver
            .transcript()
            .contains(&r#"entry released "pending shortcut""#.into())
    );
    let live = driver.live();
    driver
        .run_plain_command("context-control", "report")
        .await?;
    assert_eq!(
        driver.transcript().last().unwrap(),
        r#"entry context-alias "/first""#
    );
    driver
        .run_plain_command("context-control", "drop-one")
        .await?;
    assert_eq!(
        driver.transcript().last().unwrap(),
        r#"entry context-alias "/first""#
    );
    assert_eq!(driver.live(), live);
    driver
        .run_plain_command("context-control", "drop-all")
        .await?;
    assert_eq!(driver.live(), live - 1);
    driver.release_all().await;
    assert_eq!(
        driver.live(),
        0,
        "ownership transcript: {:?}",
        driver.transcript()
    );
    Ok(())
}

#[test]
fn maestro_pending_callbacks_keep_contexts_and_captured_values() -> Result<(), String> {
    on_both_adapters!(pending)
}

/// Recorded shortcut spellings and optional metadata cross both generated routes.
async fn shortcut_corpus(
    driver: &mut impl Driver,
    cases: Vec<
        super::registration_corpus::Case<
            super::registration_corpus::ShortcutInput,
            super::registration_corpus::ShortcutExpected,
        >,
    >,
) -> Result<(), String> {
    use super::registration_fixture::{Mode, Shortcut};
    for case in cases {
        let request = Shortcut {
            key: case.input.key.clone(),
            description: case.input.description,
            mode: case.input.mode,
        };
        driver
            .run_plain_command("capture", &format!("shortcut {}", json!(request)))
            .await?;
        assert_eq!(
            driver.registrations().1.last(),
            Some(&(case.expected.key, case.expected.description))
        );
        let id = driver.identity(&format!("shortcut {}", case.input.key))?;
        match case.input.mode {
            Mode::Immediate => driver.shortcut(id, &case.input.cwd).await?,
            Mode::Pending => driver.shortcut_held(id, &case.input.cwd).await?,
        }
        assert_eq!(
            driver.transcript().last().unwrap(),
            &format!("entry shortcut-origin {}", json!(case.expected.cwd))
        );
        assert!(
            driver
                .release(&format!("shortcut {}", case.input.key))
                .await
        );
    }
    Ok(())
}

/// Host replacement does not mutate an earlier retained callable.
async fn replacement_capture(driver: &mut impl Driver) -> Result<(), String> {
    driver.run_plain_command("capture", &format!("registration {}", json!({"name":"replace-capture","description":"replacement","completion":true,"key":"super+r"}))).await?;
    let old = driver.identity("completion replace-capture")?;
    driver.run_plain_command("capture", &format!("registration {}", json!({"name":"replace-capture","description":"","completion":false,"key":"super+s"}))).await?;
    assert_eq!(
        driver.registrations().0.last(),
        Some(&("replace-capture".into(), Some(String::new()), false))
    );
    assert_eq!(
        candidates(driver.completion(old, "old captured callback").await?).unwrap()[0].value,
        "old captured callback"
    );
    Ok(())
}

/// Command metadata and invocation use independently supplied contexts.
async fn command_corpus(
    driver: &mut impl Driver,
    cases: Vec<
        super::registration_corpus::Case<
            super::registration_corpus::Command,
            super::registration_corpus::Command,
        >,
    >,
) -> Result<(), String> {
    for case in cases {
        let description = case.input.description.as_deref();
        let completion = case.input.completion;
        let key = "ctrl+t";
        let request = json!({"name":case.input.name, "description":description, "completion":completion, "key":key});
        driver
            .run_plain_command("capture", &format!("registration {request}"))
            .await?;
        let (commands, shortcuts) = driver.registrations();
        assert_eq!(
            commands.last(),
            Some(&(
                case.expected.name,
                case.expected.description,
                case.expected.completion
            ))
        );
        assert_eq!(
            shortcuts.last(),
            Some(&(key.into(), description.map(str::to_owned)))
        );
        let command = driver.identity("command run Ω")?;
        let shortcut = driver.identity(&format!("shortcut {key}"))?;
        assert_ne!(command, shortcut);
        driver.run_plain_command("run Ω", "literal args Ω").await?;
        driver.shortcut(shortcut, "/ordinary-origin").await?;
        assert!(
            driver.transcript().iter().any(|entry| entry
                == r#"entry registered-command {"args":"literal args Ω","cwd":"/work"}"#)
        );
        assert!(
            driver
                .transcript()
                .iter()
                .any(|entry| entry == r#"entry registered-shortcut "/ordinary-origin""#)
        );
        release_command(driver, completion, command, description).await?;
        assert!(driver.release(&format!("shortcut {key}")).await);
    }
    Ok(())
}

/// Command and completion entries release independently after invocation.
async fn release_command(
    driver: &mut impl Driver,
    completion: bool,
    command: u32,
    description: Option<&str>,
) -> Result<(), String> {
    if completion {
        let id = driver.identity("completion run Ω")?;
        assert_ne!(id, command);
        initial_completion(driver, command, id).await?;
        if description == Some("") {
            return completion_first(driver).await;
        }
        assert!(driver.release("command run Ω").await);
        assert_eq!(
            driver.completion(id, "after release").await?,
            Some(vec![
                json!({"value":"after release", "label":"completion Ω", "description":""})
            ])
        );
        assert!(driver.release("completion run Ω").await);
    } else {
        assert!(driver.release("command run Ω").await);
    }
    Ok(())
}

/// Declared kinds, defaults and supplied overrides remain independent.
async fn flag_combinations(
    driver: &mut impl Driver,
    cases: Vec<
        super::registration_corpus::Case<
            super::registration_corpus::FlagInput,
            super::registration_corpus::FlagExpected,
        >,
    >,
) -> Result<(), String> {
    for case in cases {
        let input = case.input;
        let expected = case.expected;
        driver.flag_reply(&input.name, Ok(expected.unknown.as_ref().map(|v| json!(v))));
        read_flag(driver, &input.name, expected.unknown.as_ref()).await?;
        let flag = super::registration_fixture::Flag {
            name: input.name.clone(),
            description: input.description,
            kind: input.kind,
            default: input.default,
        };
        driver
            .run_plain_command("capture", &format!("flag-register {}", json!(flag)))
            .await?;
        assert_eq!(
            driver.flag_registrations().last(),
            Some(&(
                input.name.clone(),
                json!({"description":expected.description,"type":expected.kind,"default":expected.default})
            ))
        );
        // The source oracle supplies the policy result; the host does not repeat that policy.
        driver.flag_reply(&input.name, Ok(expected.value.as_ref().map(|v| json!(v))));
        read_flag(driver, &input.name, expected.value.as_ref()).await?;
        if let Some(value) = input.override_value.as_ref() {
            driver.flag_reply(&input.name, Ok(Some(json!(value))));
            read_flag(driver, &input.name, Some(value)).await?;
        }
    }
    Ok(())
}

/// Literal flag lookup keeps selected names distinct from decoys.
async fn literal_flags(
    driver: &mut impl Driver,
    cases: Vec<super::registration_corpus::LiteralFlag>,
) -> Result<(), String> {
    for case in cases {
        driver.flag_reply(&case.input, Ok(Some(json!(case.expected))));
        let decoy = format!("{}-decoy", case.input);
        driver.flag_reply(&decoy, Ok(case.decoy.as_ref().map(|v| json!(v))));
        register_flag(
            driver,
            &case.input,
            None,
            super::registration_fixture::Kind::String,
        )
        .await?;
        assert_eq!(driver.flag_registrations().last().unwrap().0, case.input);
        read_flag(driver, &case.input, Some(&case.expected)).await?;
        read_flag(driver, &decoy, case.decoy.as_ref()).await?;
    }
    Ok(())
}

/// Replacement metadata does not impose a guest default policy.
async fn flag_replacements(
    driver: &mut impl Driver,
    cases: Vec<
        super::registration_corpus::Case<
            super::registration_corpus::Replacement,
            super::registration_corpus::FlagExpected,
        >,
    >,
) -> Result<(), String> {
    for case in cases {
        register_flag(
            driver,
            "replace",
            case.input.first.clone(),
            super::registration_fixture::Kind::Boolean,
        )
        .await?;
        assert_eq!(
            driver.flag_registrations().last().unwrap().1,
            json!({"description":null,"type":"boolean","default":case.input.first})
        );
        let flag = super::registration_fixture::Flag {
            name: "replace".into(),
            description: Some("second".into()),
            kind: super::registration_fixture::Kind::String,
            default: case.input.second,
        };
        driver
            .run_plain_command("capture", &format!("flag-register {}", json!(flag)))
            .await?;
        assert_eq!(
            driver.flag_registrations().last(),
            Some(&(
                "replace".into(),
                json!({"description":case.expected.description,"type":case.expected.kind,"default":case.expected.default})
            ))
        );
        driver.flag_reply(
            "replace",
            Ok(case.expected.value.as_ref().map(|v| json!(v))),
        );
        read_flag(driver, "replace", case.expected.value.as_ref()).await?;
    }
    Ok(())
}

/// Each completion outcome preserves its full ordered fields and readiness.
async fn completion_corpus(
    driver: &mut impl Driver,
    cases: Vec<
        super::registration_corpus::Case<
            super::registration_fixture::Completion,
            super::registration_corpus::CompletionExpected,
        >,
    >,
) -> Result<(), String> {
    use super::registration_fixture::Mode;
    for case in cases {
        driver
            .run_plain_command("capture", &format!("completion {}", json!(case.input)))
            .await?;
        let id = driver.identity("completion complete")?;
        let before = driver.transcript().len();
        let answer = match case.input.mode {
            Mode::Immediate => driver.completion(id, &case.input.prefix).await?,
            Mode::Pending => driver.completion_held(id, &case.input.prefix).await?,
        };
        assert_eq!(candidates(answer), case.expected.answer);
        assert_eq!(
            driver.transcript()[before],
            format!("entry completion-prefix {}", json!(case.expected.prefix))
        );
        if matches!(case.input.mode, Mode::Pending) {
            assert!(
                driver.transcript()[before..]
                    .iter()
                    .any(|line| line.starts_with("command suspended after wait-for-idle"))
            );
        }
        assert!(driver.release("command complete").await);
        assert!(driver.release("completion complete").await);
    }
    Ok(())
}

/// Rejected callbacks and identities are destroyed after the rejecting operation ends.
async fn rejected_ownership(driver: &mut impl Driver) -> Result<(), String> {
    for completion in [false, true] {
        driver.registration_error(Some("registration refused: Ω\nline".into()));
        let before = driver.dropped().len();
        let start = driver.transcript().len();
        assert_eq!(driver.run_plain_command("capture", &format!("ownership {}", json!({"name":"owned","completion":completion,"alias":false,"reenter":false}))).await.unwrap_err(), "registration refused: Ω\nline");
        assert_eq!(
            driver.dropped().len() - before,
            if completion { 2 } else { 1 }
        );
        assert!(driver.transcript()[start..].contains(&r#"entry released "owned command""#.into()));
        assert_eq!(
            driver.transcript()[start..].contains(&r#"entry released "owned completion""#.into()),
            completion
        );
    }
    let before = driver.dropped().len();
    assert_eq!(
        driver
            .run_plain_command("capture", "owned-shortcut")
            .await
            .unwrap_err(),
        "registration refused: Ω\nline"
    );
    assert_eq!(driver.dropped().len() - before, 1);
    assert_eq!(
        driver.transcript().last().unwrap(),
        r#"entry released "owned shortcut""#
    );
    Ok(())
}

/// Either release order leaves the other callback callable.
async fn release_orders(driver: &mut impl Driver) -> Result<(), String> {
    for completion_first in [false, true] {
        driver
            .run_plain_command(
                "capture",
                &format!(
                    "ownership {}",
                    json!({"name":"owned","completion":true,"alias":false,"reenter":false})
                ),
            )
            .await?;
        let id = driver.identity("completion owned")?;
        release_order(driver, id, completion_first).await?;
    }
    Ok(())
}

/// A surviving authored alias owns its capture until explicitly removed.
async fn retained_alias(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command(
            "capture",
            &format!(
                "ownership {}",
                json!({"name":"owned","completion":true,"alias":true,"reenter":false})
            ),
        )
        .await?;
    let start = driver.transcript().len();
    assert!(driver.release("completion owned").await);
    assert!(!driver.transcript()[start..].contains(&r#"entry released "owned completion""#.into()));
    driver
        .run_plain_command("alias-control", "retained")
        .await?;
    assert_eq!(
        driver.transcript().last().unwrap(),
        r#"entry alias-answer ["retained"]"#
    );
    driver.run_plain_command("alias-control", "drop").await?;
    assert_eq!(
        driver.transcript().last().unwrap(),
        r#"entry released "owned completion""#
    );
    assert!(driver.release("command alias-control").await);
    assert!(driver.release("command owned").await);
    Ok(())
}

/// Host import errors cross the synchronous facade unchanged.
async fn host_errors(driver: &mut impl Driver) -> Result<(), String> {
    let message = "host refusal: Ω\nsecond line";
    driver.registration_error(Some(message.into()));
    for request in [
        format!(
            "registration {}",
            json!({"name":"failure-import","description":null,"completion":true,"key":"ctrl+t"})
        ),
        "owned-shortcut".into(),
        format!(
            "flag-register {}",
            json!({"name":"unknown","description":"","type":"boolean","default":false})
        ),
    ] {
        assert_eq!(
            driver
                .run_plain_command("capture", &request)
                .await
                .unwrap_err(),
            message
        );
    }
    driver.registration_error(None);
    driver.flag_reply("unknown", Err(message.into()));
    assert_eq!(
        driver
            .run_plain_command("capture", "flag-read unknown")
            .await
            .unwrap_err(),
        message
    );
    driver.flag_reply("unknown", Ok(None));
    read_flag(driver, "unknown", None).await?;
    Ok(())
}

/// Authored errors retain earlier effects and subsequent healthy dispatch.
async fn callback_errors(
    driver: &mut impl Driver,
    cases: Vec<
        super::registration_corpus::Case<
            super::registration_fixture::Failure,
            super::registration_corpus::FailureExpected,
        >,
    >,
) -> Result<(), String> {
    use super::registration_fixture::Mode;
    for case in cases {
        driver
            .run_plain_command(
                "capture",
                &format!("callback-failure {}", json!(case.input)),
            )
            .await?;
        let start = driver.transcript().len();
        assert_eq!(
            failure_call(driver, &case.input).await.unwrap_err(),
            case.expected.error
        );
        let effects = driver.transcript()[start..]
            .iter()
            .filter_map(|line| {
                line.strip_prefix("entry callback-effect ")
                    .map(|value| serde_json::from_str::<String>(value).unwrap())
            })
            .collect::<Vec<_>>();
        assert_eq!(effects, case.expected.effects);
        let mut healthy = case.input;
        healthy.mode = Mode::Immediate;
        failure_call(driver, &healthy).await?;
        assert_eq!(
            driver.transcript().last().unwrap(),
            &format!("entry healthy {}", json!(healthy.kind.name()))
        );
        for name in ["command failure", "completion failure", "shortcut failure"] {
            driver.release(name).await;
        }
    }
    Ok(())
}

/// Missing and wrong-kind calls preserve the identity diagnostic and drop lent contexts.
async fn invalid_dispatch(driver: &mut impl Driver) -> Result<(), String> {
    let unknown = driver.unregistered()?;
    assert_eq!(
        driver.completion(unknown, "missing").await.unwrap_err(),
        "no callback registered for identity 4242"
    );
    let live = driver.live();
    assert_eq!(
        driver.shortcut(unknown, "/missing").await.unwrap_err(),
        "no callback registered for identity 4242"
    );
    assert_eq!(driver.live(), live);
    let command = driver.identity("command capture")?;
    let diagnostic = driver.callback_error(command);
    assert_eq!(
        driver.completion(command, "wrong-kind").await.unwrap_err(),
        diagnostic
    );
    assert_eq!(
        driver.shortcut(command, "/wrong-kind").await.unwrap_err(),
        diagnostic
    );
    Ok(())
}

/// Completion is not interchangeable with the required command callback.
async fn initial_completion(driver: &mut impl Driver, command: u32, id: u32) -> Result<(), String> {
    let before = driver.transcript();
    assert!(driver.completion(command, "wrong kind").await.is_err());
    assert_eq!(driver.transcript(), before);
    assert_eq!(
        driver.completion(id, "prefix Ω").await?,
        Some(vec![
            json!({"value":"prefix Ω", "label":"completion Ω", "description":""})
        ])
    );
    Ok(())
}

/// Releases one accepted callback and observes the independently surviving operation.
async fn release_order(
    driver: &mut impl Driver,
    id: u32,
    completion_first: bool,
) -> Result<(), String> {
    if completion_first {
        assert!(driver.release("completion owned").await);
        driver
            .run_plain_command("owned", "surviving command")
            .await?;
        assert_eq!(
            driver.transcript().last().unwrap(),
            r#"entry owned-command "surviving command""#
        );
        assert!(driver.release("command owned").await);
    } else {
        assert!(driver.release("command owned").await);
        assert_eq!(
            candidates(driver.completion(id, "surviving completion").await?).unwrap()[0].value,
            "surviving completion"
        );
        assert!(driver.release("completion owned").await);
    }
    Ok(())
}

/// Removing completion leaves the separately registered command callable.
async fn completion_first(driver: &mut impl Driver) -> Result<(), String> {
    assert!(driver.release("completion run Ω").await);
    driver
        .run_plain_command("run Ω", "after completion release")
        .await?;
    assert_eq!(
        driver.transcript().last().unwrap(),
        r#"entry registered-command {"args":"after completion release","cwd":"/work"}"#
    );
    assert!(driver.release("command run Ω").await);
    Ok(())
}
