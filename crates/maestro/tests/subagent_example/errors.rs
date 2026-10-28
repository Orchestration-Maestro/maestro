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
                result["details"]["results"][0]["usage"]["contextTokens"],
                19
            );
        }
    }
}

#[test]
#[cfg(unix)]
fn subagent_failures_keep_diagnostics_and_stop_the_chain() {
    if child_fixture(
        "subagent_example::errors::subagent_failures_keep_diagnostics_and_stop_the_chain",
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
