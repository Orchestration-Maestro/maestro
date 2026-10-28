//! The shipped issue provider through registered startup, listener tails and editor requests.
use serde_json::{Value, json};
use std::task::{Context, Waker};
use std::{future::Future, pin::pin};

use super::controlled::{Controlled, Flag};
use super::documents::{Answer, Ended};
use super::example_support::{release_all, start};
use crate::component_adapter::{Capabilities, Exports, release};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/github_issue_autocomplete.rs"]
mod github_issue_autocomplete;

/// The same factory exported by the shipped component.
struct IssueAutocomplete;
impl Extension for IssueAutocomplete {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        github_issue_autocomplete::factory(api)
    }
}

/// A CLI reply with explicitly supplied text-helper decisions, not a second text algorithm.
fn reply(stdout: &str, stderr: &str, code: i32) -> Value {
    let mut value = json!({"stdout":stdout,"stderr":stderr,"code":code,"killed":false,
        "is_whitespace_char":false, "fuzzy.project":true, "fuzzy.filter":[]});
    for text in [
        " ",
        "\t",
        "\n",
        "\r",
        "\u{feff}",
        "\u{a0}",
        "\u{2003}",
        "two words",
    ] {
        value[format!("is_whitespace_char {}", json!([text]))] = json!(true);
    }
    value
}

/// Delivers a real session start with a supplied Git command result.
fn startup(host: &Controlled, exports: &Exports<Controlled>, value: Value) -> Result<(), String> {
    host.observe().actions.value = value;
    let key = host.observed().callback("event session_start")?;
    let outcome = super::block_on(exports.invoke_event(
        key,
        json!({"type":"session_start","reason":"startup"}).to_string(),
        Capabilities {
            ctx: host.context("/controlled"),
            signal: None,
        },
    ))?;
    assert_eq!(
        Answer::of(super::guest_family::delivery(outcome))?.ended,
        Ended::Returned(None)
    );
    Ok(())
}

/// Runs one already announced host-owned preload.
fn preload(host: &Controlled, value: Value) -> Result<(), String> {
    host.observe().actions.value = value;
    let tail = *host.observed().bus.tails.last().ok_or("missing preload")?;
    super::block_on(Exports::<Controlled>::invoke_tail(tail))
}

/// Literal CLI invocations observed at the public execution interface.
fn commands(host: &Controlled) -> Vec<Value> {
    host.observed()
        .transcript
        .iter()
        .filter_map(|line| serde_json::from_str(line.strip_prefix("action exec ")?).ok())
        .collect()
}

/// A suggestion request with a byte cursor, independent of the rest of its line.
fn request(text: &str, col: usize) -> Value {
    json!({"op":"suggestions","lines":[text],"cursor":[0,col],"force":null,"signal":0})
}

/// Runs the installed provider with a fresh live or aborted caller flag.
fn suggest(host: &Controlled, text: &str, aborted: bool) -> Result<Value, String> {
    let flag = Flag::default();
    if aborted {
        flag.abort();
    }
    super::block_on(host.provider_request(request(text, text.len()), &[flag]))
}

/// Drops host-owned providers, tails and registrations after each case.
fn finish(host: &Controlled) {
    let ids = host.observed().ui.providers.0.clone();
    for id in ids {
        release(id);
    }
    let mut names = vec![
        format!("event bus {}", host.observed().bus.listeners[0].1),
        "event session_start".into(),
    ];
    names.extend(
        host.observed()
            .ui
            .providers
            .0
            .iter()
            .map(|id| format!("provider factory {id}")),
    );
    names.extend(host.observed().bus.tails.iter().map(|_| "bus tail".into()));
    release_all(host, &names);
}

#[test]
fn github_issue_autocomplete_resolves_remotes_and_reports_failures() -> Result<(), String> {
    for remote in [
        "origin\tgit@github.com:owner/repo.git (fetch)\n",
        "junk\norigin https://other.invalid/o/r (fetch)\nnext https://github.com/owner/repo.git (push)",
        "\u{feff}origin\u{a0}http://github.com/owner/repo\u{2003}(fetch)\u{feff}",
        "origin https://github.com/owner/repo (fetch)\nother git@github.com:ignored/repo (fetch)",
    ] {
        let (host, exports) = super::block_on(start::<IssueAutocomplete>())?;
        startup(&host, &exports, reply(remote, "", 0))?;
        assert_eq!(
            commands(&host),
            [json!(["git",["remote","-v"],{"cwd":"/controlled","timeout":5000.0}])]
        );
        assert_eq!(
            host.observed().bus.tails.len(),
            1,
            "preload is detached from startup"
        );
        preload(&host, reply("[]", "", 0))?;
        assert_eq!(
            commands(&host)[1],
            json!(["gh",["issue","list","--repo","owner/repo","--state","open","--limit","100","--json","number,title,state"],{"cwd":"/controlled","timeout":5000.0}])
        );
        finish(&host);
    }
    for (remote, code, error) in [
        (
            "",
            128,
            "github-issue-autocomplete: cwd is not a git repository",
        ),
        (
            "",
            0,
            "github-issue-autocomplete: cwd is not a GitHub repository",
        ),
        (
            "origin ssh://git@github.com/owner/repo\norigin https://github.com/o/r/extra\norigin https://github.com//r\norigin git@github.com:o/",
            0,
            "github-issue-autocomplete: cwd is not a GitHub repository",
        ),
    ] {
        let (host, exports) = super::block_on(start::<IssueAutocomplete>())?;
        startup(&host, &exports, reply(remote, "", code))?;
        assert!(host.observed().ui.providers.0.is_empty());
        assert!(host.observed().bus.tails.is_empty());
        let receipts = super::ui_host::receipts(&host.observed().transcript);
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].operands, json!([error, "Error"]));
        finish(&host);
    }
    Ok(())
}

/// A deliberately unsorted issue inventory with more than one page of suggestions.
fn issues() -> String {
    let mut values = vec![
        json!({"number":12,"title":"Fix login 雪","state":"OPEN"}),
        json!({"number":123,"title":"Other","state":"Closed"}),
        json!({"number":9,"title":"42 things","state":"OPEN"}),
    ];
    values.extend((200..222).map(|number| json!({"number":number,"title":"Extra","state":"OPEN"})));
    Value::Array(values).to_string()
}

/// Starts a session with valid remotes; no GitHub command is driven yet.
fn resolved() -> Result<(Controlled, Exports<Controlled>), String> {
    let (host, exports) = super::block_on(start::<IssueAutocomplete>())?;
    startup(
        &host,
        &exports,
        reply("origin git@github.com:owner/repo.git (fetch)", "", 0),
    )?;
    Ok((host, exports))
}

#[test]
fn github_issue_autocomplete_filters_and_delegates_to_previous_provider() -> Result<(), String> {
    let (host, _) = resolved()?;
    let mut supplied = reply(&issues(), "", 0);
    supplied["fuzzy.filter"] = json!([2, 0]);
    preload(&host, supplied)?;
    for text in ["#1", "x #1", "x\t#1"] {
        assert_eq!(
            suggest(&host, text, false)?,
            json!({"suggestions":[[
            ["#12","#12","[open] Fix login 雪"],["#123","#123","[closed] Other"]],"#1"]})
        );
    }
    let all = suggest(&host, "#", false)?;
    assert_eq!(all["suggestions"][0].as_array().unwrap().len(), 20);
    assert_eq!(
        all["suggestions"][0][0],
        json!(["#12", "#12", "[open] Fix login 雪"])
    );
    assert_eq!(all["suggestions"][0][19][0], "#216");
    for text in ["#login", "#42"] {
        assert_eq!(
            suggest(&host, text, false)?["suggestions"][0],
            json!([
                ["#9", "#9", "[open] 42 things"],
                ["#12", "#12", "[open] Fix login 雪"]
            ])
        );
    }
    delegation(&host)?;
    assert_eq!(
        commands(&host).len(),
        2,
        "all completions reuse the preload"
    );
    finish(&host);
    Ok(())
}

/// Unrelated input, cancellation and editor operations keep the previous provider's policy.
fn delegation(host: &Controlled) -> Result<(), String> {
    for text in [
        "plain",
        "x#1",
        "##1",
        "\n#1",
        "x\u{a0}#1",
        "#two words",
        "#\u{feff}",
    ] {
        assert_eq!(
            suggest(host, text, false)?,
            json!({"suggestions":[[["base","Base","host"]],text]})
        );
    }
    assert_eq!(
        suggest(host, "#1", true)?,
        json!({"suggestions":[[["base","Base","host"]],"#1"]})
    );
    host.observe().actions.value["fuzzy.filter"] = json!([]);
    assert_eq!(
        suggest(host, "#unmatched", false)?["suggestions"][0][0][0],
        "base"
    );
    let applied = super::block_on(host.provider_request(json!({"op":"apply","lines":["take #1 now"],"cursor":[0,7],"item":["#12","#12",null],"prefix":"#1"}), &[]))?;
    assert_eq!(applied, json!({"applied":[["take #12 now"],0,8]}));
    for line in ["@file", "plain"] {
        assert_eq!(
            super::block_on(
                host.provider_request(json!({"op":"trigger","lines":[line],"cursor":[0,0]}), &[])
            )?,
            json!({"trigger":true})
        );
    }
    let mut forced = request("not an issue", 12);
    forced["force"] = json!(false);
    assert_eq!(
        super::block_on(host.provider_request(forced, &[Flag::default()]))?,
        json!({"suggestions":null})
    );
    let suffix =
        super::block_on(host.provider_request(request("#1 suffix", 2), &[Flag::default()]))?;
    assert_eq!(suffix["suggestions"][1], "#1");
    Ok(())
}

/// Counts completion wakeups without supplying a timer or executor policy.
#[derive(Default)]
struct Awakened(std::sync::atomic::AtomicUsize);
impl std::task::Wake for Awakened {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn github_issue_autocomplete_waits_for_preload_without_blocking_startup() -> Result<(), String> {
    let (host, _) = resolved()?;
    let flags = [Flag::default()];
    let mut first = pin!(host.provider_request(request("#1", 2), &flags));
    let mut second = pin!(host.provider_request(request("#1", 2), &flags));
    let wakes = [
        std::sync::Arc::new(Awakened::default()),
        std::sync::Arc::new(Awakened::default()),
    ];
    let first_waker = Waker::from(wakes[0].clone());
    let second_waker = Waker::from(wakes[1].clone());
    let mut cx = Context::from_waker(&first_waker);
    assert!(Future::poll(first.as_mut(), &mut cx).is_pending());
    assert!(Future::poll(first.as_mut(), &mut cx).is_pending());
    assert!(Future::poll(second.as_mut(), &mut Context::from_waker(&second_waker)).is_pending());
    assert_eq!(
        commands(&host).len(),
        1,
        "startup returned before gh starts"
    );
    preload(&host, reply(&issues(), "", 0))?;
    for wake in wakes {
        assert_eq!(wake.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    let expected = json!({"suggestions":[[["#12","#12","[open] Fix login 雪"],["#123","#123","[closed] Other"]],"#1"]});
    assert_eq!(super::block_on(first), Ok(expected.clone()));
    assert_eq!(super::block_on(second), Ok(expected));
    finish(&host);
    Ok(())
}

#[test]
fn github_issue_autocomplete_caches_command_and_json_failures() -> Result<(), String> {
    for (stdout, stderr, code, error) in [
        (
            "",
            " \u{feff}auth required\n\u{feff}",
            1,
            Some("github-issue-autocomplete: failed to load issues: auth required"),
        ),
        (
            "",
            "\u{feff} \n",
            7,
            Some("github-issue-autocomplete: failed to load issues: exit code 7"),
        ),
        (
            "",
            " \u{85} ",
            1,
            Some("github-issue-autocomplete: failed to load issues: \u{85}"),
        ),
        (
            "not json",
            "",
            0,
            Some("github-issue-autocomplete: failed to parse gh issue list output"),
        ),
        (
            "{}",
            "",
            0,
            Some("github-issue-autocomplete: failed to parse gh issue list output"),
        ),
        ("[]", "", 0, None),
    ] {
        let (host, _) = resolved()?;
        let mut supplied = reply(stdout, stderr, code);
        supplied["fuzzy.project"] = Value::Null;
        preload(&host, supplied)?;
        for _ in 0..2 {
            assert_eq!(suggest(&host, "#1", false)?["suggestions"][0][0][0], "base");
        }
        let notifications: Vec<_> = super::ui_host::receipts(&host.observed().transcript)
            .into_iter()
            .filter(|r| r.method == "notify")
            .map(|r| r.operands)
            .collect();
        assert_eq!(
            notifications,
            error
                .map(|error| json!([error, "Error"]))
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert_eq!(commands(&host).len(), 2);
        finish(&host);
    }
    Ok(())
}

#[test]
fn github_issue_autocomplete_component_runs_startup_preload_and_completion() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "github_issue_autocomplete")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    let mut harness = super::block_on(super::host::Harness::from_compiled(&engine, &component))
        .map_err(|error| error.to_string())?;
    component_startup(&mut harness)?;
    let tail = harness.observed().bus.tails[0];
    harness.observed().actions.value = reply(&issues(), "", 0);
    let exports = harness.exports.clone();
    super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_tail(accessor, super::host::borrow(tail))
            .await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())??;
    let provider = super::provider_host::top(harness.observed())?;
    let signal = harness.flag(false).map_err(|error| error.to_string())?;
    let op = super::provider_host::op(request("#12", 3))?;
    let result = super::block_on(super::editor_driver::provider(
        &mut harness,
        provider,
        op,
        Some(signal),
    ))
    .map_err(|error| error.to_string())??;
    assert_eq!(
        result,
        json!({"suggestions":[[["#12","#12","[open] Fix login 雪"],["#123","#123","[closed] Other"]],"#12"]})
    );
    Ok(())
}

/// Delivers session startup across the built component interface, leaving its preload pending.
fn component_startup(harness: &mut super::host::Harness) -> Result<(), String> {
    harness.observed().actions.value =
        reply("origin https://github.com/owner/repo.git (fetch)", "", 0);
    let key = harness
        .callback("event session_start")
        .map_err(|error| error.to_string())?;
    let ctx = harness
        .ordinary("/controlled")
        .map_err(|error| error.to_string())?;
    let exports = harness.exports.clone();
    let outcome = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(
                accessor,
                super::host::borrow(key),
                json!({"type":"session_start","reason":"startup"}).to_string(),
                super::host_family::guest::Capabilities { ctx, signal: None },
            )
            .await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())??;
    assert_eq!(
        Answer::of(super::host_family::delivery(outcome))?.ended,
        Ended::Returned(None)
    );
    Ok(())
}
