//! Failure diagnostics retain child details without adding an ignored host error flag.
#![cfg(unix)]
use super::*;

/// Failing children and invalid invocations through the registered tool.
async fn errors(session: &Rc<AgentSession>) {
    let unknown = execute(session, json!({"agent":"missing","task":"inspect"})).await;
    assert_eq!(
        unknown["content"][0]["text"],
        "Agent failed: Unknown agent: \"missing\". Available agents: \"role\"."
    );
    assert_eq!(unknown["details"]["results"][0]["agentSource"], "unknown");
    child_failures(session).await;
    let chain = execute(
        session,
        json!({"chain":[{"agent":"role","task":"fail"},{"agent":"role","task":"must not start"}]}),
    )
    .await;
    assert_eq!(
        chain["content"][0]["text"],
        "Chain stopped at step 1 (role): controlled failure"
    );
    assert_eq!(chain["details"]["results"].as_array().unwrap().len(), 1);
    let too_many = execute(
        session,
        json!({"tasks":vec![json!({"agent":"role","task":"inspect"});9]}),
    )
    .await;
    assert_eq!(
        too_many["content"][0]["text"],
        "Too many parallel tasks (9). Max is 8."
    );
    let mixed = execute(
        session,
        json!({"agent":"role","task":"inspect","chain":[{"agent":"role","task":"inspect"}]}),
    )
    .await;
    assert!(
        mixed["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("Invalid parameters.")
    );
}

/// Distinguishes native exit failures from provider-reported errors.
async fn child_failures(session: &Rc<AgentSession>) {
    for (task, expected, exit) in [
        ("fail", "Agent failed: controlled failure", 7),
        ("provider-error", "Agent error: controlled model failure", 0),
        ("provider-abort", "Agent aborted: controlled model abort", 0),
    ] {
        let result = execute(session, json!({"agent":"role","task":task})).await;
        assert_eq!(result["content"][0]["text"], expected);
        assert_eq!(result["details"]["results"][0]["exitCode"], exit);
        assert!(result.get("isError").is_none());
        if exit == 0 {
            assert_eq!(
                result["details"]["results"][0]["usage"]["contextTokens"].as_f64(),
                Some(19.0)
            );
        }
    }
}

#[test]
#[cfg(unix)]
fn subagent_failures_keep_diagnostics_and_stop_the_chain() {
    if child_fixture(
        "subagent_example::errors::subagent_failures_keep_diagnostics_and_stop_the_chain",
        None,
    )
    .is_some()
    {
        return;
    }
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            std::fs::create_dir_all(root.path("agent/agents")).unwrap();
            std::fs::write(root.path("agent/agents/role.md"), "---\nname: role\ndescription: Controlled child\nmodel: controlled-model\ntools: read, ls\n---\nPrivate role prompt\n").unwrap();
            let session = loaded(&root, local).await;
            errors(&session).await;
        })
    });
}

#[test]
fn subagent_invalid_optional_fields_do_not_enable_unrestricted_children() {
    if child_fixture("subagent_example::errors::subagent_invalid_optional_fields_do_not_enable_unrestricted_children", None).is_some() { return; }
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            controlled_role(&root, "agent/agents");
            let session = loaded(&root, local).await;
            for (field, tools, model) in [
                ("tools", "[read]", "controlled-model"),
                ("model", "read, ls", "17"),
            ] {
                std::fs::write(root.path("agent/agents/role.md"), format!("---\nname: role\ndescription: Invalid optional field\ntools: {tools}\nmodel: {model}\n---\nPrivate role prompt\n")).unwrap();
                let error = invoked(
                    &session,
                    json!({"agent":"role","task":"inspect"}),
                    None,
                    None,
                )
                .await
                .expect_err("invalid fields must not become omitted child restrictions");
                assert!(error.contains(field), "{error}");
            }
        })
    });
}

#[test]
fn subagent_empty_optional_fields_keep_child_defaults() {
    if child_fixture(
        "subagent_example::errors::subagent_empty_optional_fields_keep_child_defaults",
        None,
    )
    .is_some()
    {
        return;
    }
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            std::fs::create_dir_all(root.path("agent/agents")).unwrap();
            std::fs::write(
                root.path("agent/agents/plain.md"),
                "---\nname: plain\ndescription: Default child\ntools: ' '\nmodel: ''\n---\n",
            )
            .unwrap();
            let session = loaded(&root, local).await;
            let result = execute(&session, json!({"agent":"plain","task":"plain"})).await;
            assert_eq!(result["content"][0]["text"], "plain output");
            assert_eq!(result["details"]["results"][0]["model"], "reported-model");
            assert_eq!(result["details"]["results"][0]["usage"]["turns"], 1);
        })
    });
}

#[test]
fn subagent_usage_preserves_reported_fractional_counts() {
    if child_fixture(
        "subagent_example::errors::subagent_usage_preserves_reported_fractional_counts",
        None,
    )
    .is_some()
    {
        return;
    }
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            controlled_role(&root, "agent/agents");
            let session = loaded(&root, local).await;
            let result = execute(&session, json!({"agent":"role","task":"fractional"})).await;
            assert_eq!(
                result["details"]["results"][0]["usage"],
                json!({"input":1.75,"output":2.75,"cacheRead":0.25,"cacheWrite":1.5,"cost":0.25,"contextTokens":12.5,"turns":2})
            );
        })
    });
}
