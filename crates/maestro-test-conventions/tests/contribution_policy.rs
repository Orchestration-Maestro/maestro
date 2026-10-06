#[path = "../src/contribution_policy/mod.rs"]
mod contribution_policy;
#[path = "support/policy.rs"]
mod policy_support;
mod support;
use contribution_policy::run;
use policy_support::{RecordingProcess, event, fixture};
use serde_json::json;

#[test]
fn comment_commands_observe_word_boundaries_and_issue_precedence() {
    let w = fixture();
    for body in ["", "no command", "_lgtm", "lgtm2", "xlgtm"] {
        let mut e = event("created");
        e["comment"]["body"] = json!(body);
        let mut p = RecordingProcess::default();
        assert_eq!(
            run(
                &w.root,
                "approve-contributor",
                "issue_comment",
                &e.to_string(),
                None,
                &mut p
            )
            .unwrap(),
            vec![("status".into(), "skipped".into())]
        );
        assert!(p.requests.is_empty());
    }
    let mut e = event("created");
    e["comment"].as_object_mut().unwrap().remove("body");
    let mut p = RecordingProcess::default();
    assert_eq!(
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &e.to_string(),
            None,
            &mut p
        )
        .unwrap(),
        vec![("status".into(), "skipped".into())]
    );
    assert!(p.requests.is_empty());
    for (body, capability) in [
        ("LGTM then LGtMi", "issue"),
        ("lgtmi\nlgtm", "issue"),
        ("(LGTM)!", "pr"),
        ("élgtmé", "pr"),
    ] {
        let mut e = event("created");
        e["comment"]["body"] = json!(body);
        let mut p = RecordingProcess::default();
        policy_support::update_replies(&mut p);
        assert_eq!(
            run(
                &w.root,
                "approve-contributor",
                "issue_comment",
                &e.to_string(),
                Some("policy-app"),
                &mut p
            )
            .unwrap(),
            vec![
                ("status".into(), "added".into()),
                ("capability".into(), capability.into())
            ]
        );
    }
}

#[test]
fn approval_permissions_skip_before_reading_policy() {
    let w = fixture();
    std::fs::remove_file(w.root.join(".github/APPROVED_CONTRIBUTORS")).unwrap();
    for permission in ["read", "triage", "none", "error"] {
        let mut p = RecordingProcess::default();
        if permission == "error" {
            p.fail();
        } else {
            p.reply(json!({"permission":permission}));
        }
        let result = run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &event("created").to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(result, vec![("status".into(), "skipped".into())]);
        assert_eq!(p.requests.len(), 1);
        assert!(
            p.requests[0]
                .args
                .contains(&"repos/fixture/project/collaborators/Reviewer/permission".into())
        );
    }
    for permission in ["admin", "maintain", "write"] {
        let mut e = event("created");
        e["comment"]["user"]["login"] = json!("review[bot]");
        let mut p = RecordingProcess::default();
        p.reply(json!({"permission":permission}));
        assert!(
            run(
                &w.root,
                "approve-contributor",
                "issue_comment",
                &e.to_string(),
                None,
                &mut p
            )
            .is_err()
        );
        assert_eq!(p.requests.len(), 1);
        assert!(p.requests[0].args[3].contains("review[bot]"));
    }
}

#[test]
fn approval_parser_handles_case_whitespace_and_duplicates() {
    let w = fixture();
    for (author, contents, capability) in [
        (
            "ALICE",
            "# note\nAlice PR\nalice ISSUE\nalice bad\n",
            "issue",
        ),
        ("ALICE", "\u{feff}Alice\u{feff}PR\u{feff}\r\n", "pr"),
        ("İ", "i\u{307} pr\n", "pr"),
        ("ΟΣ", "ος pr\n", "pr"),
        ("Alice\u{85}", "Alice\u{85} pr\n", "pr"),
        ("\u{85}Alice", "\u{85}Alice pr\n", "pr"),
        ("Alice\u{180e}", "Alice\u{180e} pr\n", "pr"),
        ("Alice\u{200b}", "Alice\u{200b} pr\n", "pr"),
    ] {
        std::fs::write(w.root.join(".github/APPROVED_CONTRIBUTORS"), contents).unwrap();
        let mut e = event("created");
        e["issue"]["user"]["login"] = json!(author);
        e["comment"]["body"] = json!("lgtmi");
        let mut p = RecordingProcess::default();
        p.reply(json!({"permission":"write"}));
        p.reply(json!({}));
        assert_eq!(
            run(
                &w.root,
                "approve-contributor",
                "issue_comment",
                &e.to_string(),
                None,
                &mut p
            )
            .unwrap(),
            vec![
                ("status".into(), "already".into()),
                ("capability".into(), capability.into())
            ]
        );
        assert_eq!(
            p.requests[1].input.as_ref().unwrap()["body"],
            format!("@{author} is already approved.")
        );
    }
}

#[test]
fn approval_replaces_invalid_utf8_before_matching_existing_user() {
    let w = fixture();
    std::fs::write(
        w.root.join(".github/APPROVED_CONTRIBUTORS"),
        b"\xff\nAlice issue\n",
    )
    .unwrap();
    let mut e = event("created");
    e["comment"]["body"] = json!("lgtmi");
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"write"}));
    p.reply(json!({}));
    assert_eq!(
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &e.to_string(),
            None,
            &mut p
        )
        .unwrap(),
        vec![
            ("status".into(), "already".into()),
            ("capability".into(), "issue".into())
        ]
    );
    assert_eq!(p.requests.len(), 2);
    assert_eq!(
        p.requests[1].input.as_ref().unwrap()["body"],
        "@Alice is already approved."
    );
}

#[test]
fn approval_addition_preserves_lines_and_newline() {
    let w = fixture();
    std::fs::write(
        w.root.join(".github/APPROVED_CONTRIBUTORS"),
        "# comment\r\nmalformed\nBob\tISSUE\n\n",
    )
    .unwrap();
    let mut p = RecordingProcess::default();
    policy_support::update_replies(&mut p);
    assert_eq!(
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &event("created").to_string(),
            Some("policy-app"),
            &mut p
        )
        .unwrap(),
        vec![
            ("status".into(), "added".into()),
            ("capability".into(), "pr".into())
        ]
    );
    assert_eq!(
        policy_support::changed_content(&p),
        "# comment\r\nmalformed\nBob issue\n\n\nAlice pr\n"
    );
    assert!(
        !p.requests
            .iter()
            .any(|r| r.args.iter().any(|s| s.ends_with("/comments")))
    );
    for (before, expected) in [
        ("", "\nMiXeD pr\n"),
        (
            "# preserve\r\nAlice\u{85}pr\nwrong one three\n",
            "# preserve\r\nAlice\u{85}pr\nwrong one three\n\nMiXeD pr\n",
        ),
    ] {
        std::fs::write(w.root.join(".github/APPROVED_CONTRIBUTORS"), before).unwrap();
        let mut e = event("created");
        e["issue"]["user"]["login"] = json!("MiXeD");
        let mut p = RecordingProcess::default();
        policy_support::update_replies(&mut p);
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &e.to_string(),
            Some("policy-app"),
            &mut p,
        )
        .unwrap();
        assert_eq!(policy_support::changed_content(&p), expected);
    }
}

#[test]
fn approval_upgrade_changes_last_duplicate_only() {
    let w = fixture();
    std::fs::write(
        w.root.join(".github/APPROVED_CONTRIBUTORS"),
        "Alice PR\nALICE\tISSUE\nAlice nope\n\n",
    )
    .unwrap();
    let mut p = RecordingProcess::default();
    policy_support::update_replies(&mut p);
    assert_eq!(
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &event("created").to_string(),
            Some("policy-app"),
            &mut p
        )
        .unwrap(),
        vec![
            ("status".into(), "updated".into()),
            ("capability".into(), "pr".into())
        ]
    );
    assert_eq!(
        policy_support::changed_content(&p),
        "Alice pr\nALICE pr\nAlice nope\n"
    );
}

#[test]
fn approval_already_never_downgrades_or_writes() {
    let w = fixture();
    for (contents, command) in [
        ("  Alice\t PR\r\n", "lgtm"),
        ("  Alice\t PR\r\n", "lgtmi"),
        ("Alice issue\n", "lgtmi"),
    ] {
        std::fs::write(w.root.join(".github/APPROVED_CONTRIBUTORS"), contents).unwrap();
        let mut e = event("created");
        e["comment"]["body"] = json!(command);
        let mut p = RecordingProcess::default();
        p.reply(json!({"permission":"maintain"}));
        p.reply(json!({}));
        let outputs = run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &e.to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(outputs[0], ("status".into(), "already".into()));
        assert_eq!(p.requests.len(), 2);
        assert!(
            p.requests[1]
                .args
                .contains(&"repos/fixture/project/issues/7/comments".into())
        );
        assert_eq!(
            std::fs::read_to_string(w.root.join(".github/APPROVED_CONTRIBUTORS")).unwrap(),
            contents
        );
    }
}

#[test]
fn approval_updates_use_signed_pull_requests() {
    let w = fixture();
    let mut p = RecordingProcess::default();
    policy_support::update_replies(&mut p);
    run(
        &w.root,
        "approve-contributor",
        "issue_comment",
        &event("created").to_string(),
        Some("policy-app"),
        &mut p,
    )
    .unwrap();
    assert_eq!(p.requests.len(), 7);
    assert_eq!(
        p.requests[1].args[3],
        "repos/fixture/project/git/ref/heads/trunk"
    );
    assert_eq!(
        p.requests[3].input,
        Some(json!({"ref":"refs/heads/chore/approve-contributor-7-42","sha":"trusted-head"}))
    );
    assert_eq!(p.requests[2].args, ["rev-parse", "HEAD"]);
    let commit = p.requests[4].input.as_ref().unwrap();
    assert_eq!(
        commit["variables"]["input"]["expectedHeadOid"],
        "trusted-head"
    );
    assert_eq!(
        commit["variables"]["input"]["fileChanges"]["additions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        commit["variables"]["input"]["fileChanges"]["additions"][0]["path"],
        ".github/APPROVED_CONTRIBUTORS"
    );
    assert_eq!(
        commit["variables"]["input"]["message"]["headline"],
        "chore: approve contributor Alice"
    );
    assert_eq!(p.requests[5].input.as_ref().unwrap()["base"], "trunk");
    assert_eq!(
        p.requests[6].args,
        [
            "pr",
            "merge",
            "81",
            "--repo",
            "fixture/project",
            "--squash",
            "--auto",
            "--match-head-commit",
            "signed-head"
        ]
    );
    for (index, request) in p.requests.into_iter().enumerate() {
        assert_eq!(request.program, if index == 2 { "git" } else { "gh" });
        assert_eq!(request.cwd, w.root);
    }
}

#[test]
fn gates_exempt_bots_and_write_collaborators() {
    let w = fixture();
    for (workflow, name, field) in [
        ("issue-gate", "issues", "issue"),
        ("pr-gate", "pull_request_target", "pull_request"),
    ] {
        let mut e = event("opened");
        if field == "pull_request" {
            e["pull_request"] = e["issue"].clone();
        }
        for author in ["auto[bot]", "dependabot[bot]"] {
            e[field]["user"]["login"] = json!(author);
            let mut p = RecordingProcess::default();
            assert!(
                run(&w.root, workflow, name, &e.to_string(), None, &mut p)
                    .unwrap()
                    .is_empty()
            );
            assert!(p.requests.is_empty());
        }
        e[field]["user"]["login"] = json!("auto[BOT]");
        for permission in ["admin", "maintain", "write"] {
            let mut p = RecordingProcess::default();
            p.reply(json!({"permission":permission}));
            run(&w.root, workflow, name, &e.to_string(), None, &mut p).unwrap();
            assert_eq!(p.requests.len(), 1);
        }
    }
}

#[test]
fn gates_distinguish_issue_and_pr_capabilities() {
    let w = fixture();
    for pr in [false, true] {
        for capability in ["issue", "pr", "unknown", ""] {
            let mut p = RecordingProcess::default();
            p.reply(json!({"permission":"read"}));
            policy_support::content(&mut p, &format!("Alice {capability}\n"));
            let exempt = capability == "pr" || (!pr && capability == "issue");
            if !exempt {
                p.reply(json!({}));
                p.reply(json!({}));
            }
            run(
                &w.root,
                if pr { "pr-gate" } else { "issue-gate" },
                if pr { "pull_request_target" } else { "issues" },
                &policy_support::gate_event(pr).to_string(),
                None,
                &mut p,
            )
            .unwrap();
            assert_eq!(p.requests.len(), if exempt { 2 } else { 4 });
            if !exempt {
                assert_eq!(
                    p.requests.last().unwrap().input,
                    Some(json!({"state":"closed"}))
                );
            }
        }
    }
}

#[test]
fn gates_parse_malformed_lines_and_last_duplicate() {
    let w = fixture();
    for text in [
        "# comment\n\n Alice\tPR\nAlice unknown\ninvalid\nthree fields here\n",
        "Alice issue\nalice PR\n",
        "\u{feff}ALICE\u{feff}Pr\u{feff}\r\n",
        "\u{85}Alice pr\nAlice pr\n",
    ] {
        let mut p = RecordingProcess::default();
        p.reply(json!({"permission":"none"}));
        policy_support::content(&mut p, text);
        run(
            &w.root,
            "pr-gate",
            "pull_request_target",
            &policy_support::gate_event(true).to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(p.requests.len(), 2);
    }
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"none"}));
    policy_support::content(&mut p, "Alice PR\nalice issue\nALICE nope\n");
    p.reply(json!({}));
    p.reply(json!({}));
    run(
        &w.root,
        "pr-gate",
        "pull_request_target",
        &policy_support::gate_event(true).to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert_eq!(p.requests.len(), 4);
}

#[test]
fn gates_read_only_default_branch_content() {
    let w = fixture();
    for pr in [false, true] {
        let mut p = RecordingProcess::default();
        p.reply(json!({"permission":"read"}));
        policy_support::content(&mut p, "Alice pr");
        run(
            &w.root,
            if pr { "pr-gate" } else { "issue-gate" },
            if pr { "pull_request_target" } else { "issues" },
            &policy_support::gate_event(pr).to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(
            p.requests[1].args[3],
            "repos/fixture/project/contents/.github/APPROVED_CONTRIBUTORS"
        );
        assert_eq!(p.requests[1].input, Some(json!({"ref":"trunk"})));
        assert!(
            p.requests[1]
                .args
                .windows(2)
                .any(|pair| pair == ["--raw-field", "ref=trunk"])
        );
        for invalid in [
            json!([]),
            json!({"content":null}),
            json!({"content":42}),
            json!({}),
        ] {
            let mut p = RecordingProcess::default();
            p.reply(json!({"permission":"read"}));
            p.reply(invalid);
            assert_eq!(
                run(
                    &w.root,
                    if pr { "pr-gate" } else { "issue-gate" },
                    if pr { "pull_request_target" } else { "issues" },
                    &policy_support::gate_event(pr).to_string(),
                    None,
                    &mut p
                )
                .unwrap_err(),
                "Expected file content for .github/APPROVED_CONTRIBUTORS"
            );
            assert_eq!(p.requests.len(), 2);
        }
    }
}

#[test]
fn pull_request_guidance_precedes_closure() {
    let w = fixture();
    let mut p = RecordingProcess::default();
    p.fail();
    policy_support::content(&mut p, "");
    p.reply(json!({}));
    p.reply(json!({}));
    run(
        &w.root,
        "pr-gate",
        "pull_request_target",
        &policy_support::gate_event(true).to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert_eq!(
        p.requests[2].args[3],
        "repos/fixture/project/issues/7/comments"
    );
    assert_eq!(p.requests[3].args[3], "repos/fixture/project/pulls/7");
    assert_eq!(
        p.requests[2].input.as_ref().unwrap()["body"],
        "This PR was auto-closed. Only contributors approved with `lgtm` can open PRs. Open an issue first.\n\nMaintainers review auto-closed issues daily. Issues that do not meet the quality bar in [CONTRIBUTING.md](https://github.com/fixture/project/blob/trunk/CONTRIBUTING.md) will not be reopened or receive a reply.\n\nIf a maintainer replies `lgtmi`, your future issues will stay open. If a maintainer replies `lgtm`, your future issues and PRs will stay open.\n\nSee [CONTRIBUTING.md](https://github.com/fixture/project/blob/trunk/CONTRIBUTING.md)."
    );
}

#[test]
fn github_content_decoding_preserves_utf8() {
    use base64::Engine;
    let w = fixture();
    for text in ["Alice pr", "\u{feff}Älice pr", "用户 pr"] {
        let mut e = policy_support::gate_event(true);
        e["pull_request"]["user"]["login"] = json!(if text.contains('Ä') {
            "ÄLICE"
        } else if text.contains('用') {
            "用户"
        } else {
            "Alice"
        });
        let b64 = base64::engine::general_purpose::STANDARD.encode(text);
        for encoded in [
            b64.clone(),
            format!(" \n{}\r\n", b64),
            b64.trim_end_matches('=')
                .replace('+', "-")
                .replace('/', "_"),
        ] {
            let mut p = RecordingProcess::default();
            p.reply(json!({"permission":"read"}));
            p.reply(json!({"content":encoded}));
            run(
                &w.root,
                "pr-gate",
                "pull_request_target",
                &e.to_string(),
                None,
                &mut p,
            )
            .unwrap();
            assert_eq!(p.requests.len(), 2);
        }
    }
    let mut e = policy_support::gate_event(true);
    e["pull_request"]["user"]["login"] = json!("�");
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"read"}));
    p.reply(json!({"content":"/yBwcg"}));
    run(
        &w.root,
        "pr-gate",
        "pull_request_target",
        &e.to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert_eq!(p.requests.len(), 2);
}

#[test]
fn policy_process_inputs_are_not_shell_code() {
    let w = fixture();
    let mut e = policy_support::gate_event(true);
    e["pull_request"]["user"]["login"] = json!("a'$()\n`touch bad`");
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"none"}));
    policy_support::content(&mut p, "");
    p.reply(json!({}));
    p.reply(json!({}));
    run(
        &w.root,
        "pr-gate",
        "pull_request_target",
        &e.to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert_eq!(
        p.requests[0].args[3],
        "repos/fixture/project/collaborators/a'$()\n`touch bad`/permission"
    );
    for r in &p.requests {
        assert_eq!(r.program, "gh");
        assert_eq!(r.cwd, w.root);
        assert!(!r.args.contains(&"-c".into()));
    }
    assert!(!w.root.join("bad").exists());
}

#[test]
fn normal_issue_message_weekend_route_is_configured() {
    let w = fixture();
    for configured in [false, true] {
        if configured {
            policy_support::configure(&w, |v| {
                v["issue_gate"]["weekend_days"] = json!([5, 6, 0]);
                v["issue_gate"]["weekend_message"] = json!("Weekend reports wait until Monday.");
                v["issue_gate"]["weekend_labels"] = json!(["weekend"]);
            });
        }
        for (date, weekend) in [
            ("2026-10-09", true),
            ("2026-10-10", true),
            ("2026-10-11", true),
            ("2026-10-12", false),
            ("2026-10-13", false),
            ("2026-10-14", false),
            ("2026-10-15", false),
        ] {
            let mut e = event("opened");
            e["issue"]["created_at"] = json!(date);
            let mut p = RecordingProcess::default();
            p.reply(json!({"permission":"read"}));
            policy_support::content(&mut p, "");
            p.reply(json!({}));
            if configured && weekend {
                p.reply(json!({}));
            }
            p.reply(json!({}));
            run(
                &w.root,
                "issue-gate",
                "issues",
                &e.to_string(),
                None,
                &mut p,
            )
            .unwrap();
            let body = p.requests[2].input.as_ref().unwrap()["body"]
                .as_str()
                .unwrap();
            let prefix = if configured && weekend {
                "This issue was auto-closed. All issues from new contributors are auto-closed by default.\nWeekend reports wait until Monday.\n\n"
            } else {
                "This issue was auto-closed. All issues from new contributors are auto-closed by default.\n\n"
            };
            assert_eq!(
                body,
                format!(
                    "{prefix}Maintainers review auto-closed issues daily and reopen worthwhile ones. Issues that do not meet the quality bar in [CONTRIBUTING.md](https://github.com/fixture/project/blob/trunk/CONTRIBUTING.md) will not be reopened or receive a reply.\n\nIf a maintainer replies `lgtmi` on one of your issues, your future issues will stay open. If a maintainer replies `lgtm`, your future issues and PRs will stay open.\n\nSee [CONTRIBUTING.md](https://github.com/fixture/project/blob/trunk/CONTRIBUTING.md)."
                )
            );
            assert_eq!(p.requests.len(), if configured && weekend { 5 } else { 4 });
            if configured && weekend {
                assert_eq!(p.requests[3].input, Some(json!({"labels":["weekend"]})));
            }
        }
    }
}

#[test]
fn refactor_issue_message_labels_keep_order() {
    let w = fixture();
    policy_support::configure(&w, |v| {
        v["issue_gate"] = json!({"message_mode":"refactor","weekend_days":[5,6,0],"weekend_message":"Weekend","weekend_labels":["weekend"],"refactor_until":"2026-12-01","refactor_branch":"rebuild","refactor_reason":"Issues closed during this period will not be reviewed. Fixture maintenance.","refactor_labels":["maintenance"]});
        v["help_url"] = json!("https://example.invalid/help");
    });
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"none"}));
    policy_support::content(&mut p, "");
    p.reply(json!({}));
    p.reply(json!({}));
    p.reply(json!({}));
    run(
        &w.root,
        "issue-gate",
        "issues",
        &event("opened").to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert_eq!(
        p.requests[2].input.as_ref().unwrap()["body"],
        "This issue was auto-closed. All issues will be closed until 2026-12-01 because the project is undergoing a large refactor.\n\nSee the `rebuild` branch: https://github.com/fixture/project/tree/rebuild\n\nIssues closed during this period will not be reviewed. Fixture maintenance.\n\nIn case of emergency, ask here: https://example.invalid/help"
    );
    assert_eq!(
        p.requests[3].input,
        Some(json!({"labels":["weekend","maintenance"]}))
    );
    assert_eq!(p.requests[4].input, Some(json!({"state":"closed"})));
    policy_support::configure(&w, |v| {
        v["issue_gate"]["message_mode"] = json!("anything");
        v["issue_gate"]["weekend_labels"] = json!([]);
    });
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"none"}));
    policy_support::content(&mut p, "");
    p.reply(json!({}));
    p.reply(json!({}));
    run(
        &w.root,
        "issue-gate",
        "issues",
        &event("opened").to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert_eq!(p.requests.len(), 4);
    assert!(
        p.requests[2].input.as_ref().unwrap()["body"]
            .as_str()
            .unwrap()
            .starts_with("This issue was auto-closed. All issues from new contributors")
    );
}

#[test]
fn issue_dates_follow_utc_javascript_boundaries() {
    let w = fixture();
    for (date, day) in [
        ("2026-10-08T23:59:59.999Z", Some(4)),
        ("2026-10-09T00:00:00Z", Some(5)),
        ("2026-10-09T00:30:00+02:00", Some(4)),
        ("2026-10-11T23:59:59Z", Some(0)),
        ("2026-10-12T00:00:00Z", Some(1)),
        ("2026-10-09", Some(5)),
        ("2026-10-08T24:00:00Z", Some(5)),
        ("2026-02-30T00:00:00Z", Some(1)),
        ("2024-02-29", Some(4)),
        ("2026-13-01", None),
        ("2026-10-08T25:00:00Z", None),
        ("not-date", None),
        ("2026-10-09T00:00:00", Some(5)),
        ("2026-10-09T00:30+02:00", Some(4)),
        ("2026-10-09T00:00Z", Some(5)),
        ("2026-10-09T00:30", Some(5)),
        ("2026-10", Some(4)),
        ("2026", Some(4)),
        ("+002026-10-09T00:00:00Z", Some(5)),
        ("+262143-01-01T00:00:00Z", Some(2)),
        ("+275760-09-13T00:00:00Z", Some(6)),
        ("+275760-09-13T00:00:00.001Z", None),
        ("-271821-04-20T00:00:00Z", Some(2)),
        ("-271821-04-19T23:59:59.999Z", None),
        ("2026-10-09T00:00:00.1234Z", Some(5)),
        ("2026-10-09t00:00:00z", Some(5)),
        ("2026-10-09 00:00:00Z", Some(5)),
        ("2026T00:00Z", Some(4)),
        ("2026-10T00:00Z", Some(4)),
        ("2026-10-09T00:00+0200", Some(4)),
        ("2026-10-09 00:00+0200", Some(4)),
    ] {
        for selected in 0..=6 {
            policy_support::configure(&w, |v| {
                v["issue_gate"]["weekend_days"] = json!([selected]);
                v["issue_gate"]["weekend_labels"] = json!(["selected"]);
                v["issue_gate"]["weekend_message"] = json!("Selected weekend guidance.");
            });
            let mut e = event("opened");
            e["issue"]["created_at"] = json!(date);
            let mut p = RecordingProcess::default();
            p.reply(json!({"permission":"none"}));
            policy_support::content(&mut p, "");
            p.reply(json!({}));
            if day == Some(selected) {
                p.reply(json!({}));
            }
            p.reply(json!({}));
            run(
                &w.root,
                "issue-gate",
                "issues",
                &e.to_string(),
                None,
                &mut p,
            )
            .unwrap();
            assert_eq!(
                p.requests[2].input.as_ref().unwrap()["body"]
                    .as_str()
                    .unwrap()
                    .contains("Selected weekend guidance."),
                day == Some(selected),
                "{date} guidance day {selected}"
            );
            if day == Some(selected) {
                assert_eq!(p.requests[3].input, Some(json!({"labels":["selected"]})));
            }
            assert_eq!(
                p.requests.len(),
                if day == Some(selected) { 5 } else { 4 },
                "{date} day {selected}"
            );
        }
    }
}

#[test]
fn gate_read_and_action_failures_stop_in_order() {
    let w = fixture();
    policy_support::configure(&w, |v| {
        v["issue_gate"]["weekend_days"] = json!([5]);
        v["issue_gate"]["weekend_labels"] = json!(["weekend"]);
    });
    for pr in [false, true] {
        for failing in 1..=if pr { 3 } else { 4 } {
            let mut p = RecordingProcess::default();
            p.reply(json!({"permission":"none"}));
            if failing == 1 {
                p.fail();
            } else {
                policy_support::content(&mut p, "");
                for _ in 2..failing {
                    p.reply(json!({}));
                }
                p.fail();
            }
            assert!(
                run(
                    &w.root,
                    if pr { "pr-gate" } else { "issue-gate" },
                    if pr { "pull_request_target" } else { "issues" },
                    &policy_support::gate_event(pr).to_string(),
                    None,
                    &mut p
                )
                .is_err()
            );
            assert_eq!(p.requests.len(), failing + 1);
        }
    }
}

#[test]
fn activity_checks_approval_before_permissions() {
    let w = fixture();
    for pr in [false, true] {
        for cap in ["issue", "pr"] {
            let mut p = RecordingProcess::default();
            policy_support::content(&mut p, &format!("Alice {cap}\n"));
            run(
                &w.root,
                "contribution-policy",
                if pr { "pull_request_target" } else { "issues" },
                &policy_support::gate_event(pr).to_string(),
                None,
                &mut p,
            )
            .unwrap();
            assert_eq!(p.requests.len(), 1);
            assert!(p.requests[0].args[3].contains("contents/.github/APPROVED_CONTRIBUTORS"));
        }
    }
    let mut e = event("opened");
    e["issue"]["user"]["login"] = json!("tool[bot]");
    let mut p = RecordingProcess::default();
    run(
        &w.root,
        "contribution-policy",
        "issues",
        &e.to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert!(p.requests.is_empty());
}

#[test]
fn activity_read_and_permission_failures_continue() {
    let w = fixture();
    for response in [None, Some(json!([])), Some(json!({"content":false}))] {
        let mut p = RecordingProcess::default();
        if let Some(v) = response.clone() {
            p.reply(v);
        } else {
            p.fail();
        }
        p.reply(json!({"permission":"write"}));
        run(
            &w.root,
            "contribution-policy",
            "issues",
            &event("opened").to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(
            p.diagnostics,
            vec![
                format!(
                    "Could not read APPROVED_CONTRIBUTORS: {}",
                    if response.is_none() {
                        "controlled failure"
                    } else {
                        "Expected file content for .github/APPROVED_CONTRIBUTORS"
                    }
                ),
                "Alice is a collaborator (write), passing".into()
            ]
        );
        assert_eq!(p.requests.len(), 2);
        assert!(p.requests[1].args[3].contains("collaborators/Alice/permission"));
    }
}

#[test]
fn activity_search_matches_label_only() {
    let w = fixture();
    policy_support::configure(&w, |v| {
        v["activity_gate"] =
            json!({"repositories":["fixture/first","fixture/second"],"label":"activity"});
    });
    for pr in [false, true] {
        let mut e = policy_support::gate_event(pr);
        e[if pr { "pull_request" } else { "issue" }]["user"]["login"] = json!("MiXeD");
        let mut p = RecordingProcess::default();
        policy_support::content(&mut p, "");
        p.reply(json!({"permission":"none"}));
        p.reply(json!({"total_count":0}));
        p.reply(json!({"total_count":2}));
        p.reply(json!({}));
        run(
            &w.root,
            "contribution-policy",
            if pr { "pull_request_target" } else { "issues" },
            &e.to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(
            p.diagnostics,
            vec![
                "MiXeD has opened 2 issues/PRs on fixture/second",
                "MiXeD has configured activity, adding label"
            ]
        );
        assert_eq!(p.requests.len(), 5);
        assert_eq!(p.requests[2].args[3], "search/issues");
        assert!(
            p.requests[2]
                .args
                .windows(2)
                .any(|pair| pair == ["--raw-field", "q=repo:fixture/first author:MiXeD"])
        );
        assert!(
            p.requests[2]
                .args
                .windows(2)
                .any(|pair| pair == ["--raw-field", "per_page=1"])
        );
        assert_eq!(
            p.requests[2].input,
            Some(json!({"q":"repo:fixture/first author:MiXeD","per_page":1}))
        );
        assert_eq!(
            p.requests[3].input,
            Some(json!({"q":"repo:fixture/second author:MiXeD","per_page":1}))
        );
        assert_eq!(p.requests[4].input, Some(json!({"labels":["activity"]})));
        assert_eq!(
            p.requests[4].args[3],
            "repos/fixture/project/issues/7/labels"
        );
    }
}

#[test]
fn activity_search_failures_and_no_matches_pass() {
    let w = fixture();
    policy_support::configure(&w, |v| {
        v["activity_gate"] = json!({"repositories":["fixture/remote"],"label":"activity"});
    });
    for found in [None, Some(0), Some(1)] {
        let mut p = RecordingProcess::default();
        p.fail();
        p.fail();
        if let Some(count) = found {
            p.reply(json!({"total_count":count}));
        } else {
            p.fail();
        }
        if found == Some(1) {
            p.fail();
        }
        let result = run(
            &w.root,
            "contribution-policy",
            "issues",
            &event("opened").to_string(),
            None,
            &mut p,
        );
        assert!(
            p.diagnostics
                .contains(&"Could not read APPROVED_CONTRIBUTORS: controlled failure".into())
        );
        assert_eq!(
            p.diagnostics
                .contains(&"Search failed: controlled failure".into()),
            found.is_none()
        );
        if found == Some(1) {
            assert!(
                p.diagnostics
                    .contains(&"Alice has opened 1 issues/PRs on fixture/remote".into())
            );
        }
        assert_eq!(result.is_err(), found == Some(1));
        assert_eq!(p.requests.len(), if found == Some(1) { 4 } else { 3 });
    }
}

#[test]
fn activity_diagnostics_preserve_trimmed_gh_stderr() {
    let w = fixture();
    policy_support::configure(&w, |v| {
        v["activity_gate"] = json!({"repositories":["fixture/remote"],"label":"activity"});
    });
    let mut p = RecordingProcess::default();
    p.replies.push_back(Err("gh stderr".into()));
    p.reply(json!({"permission":"none"}));
    p.replies.push_back(Err("gh stderr".into()));
    run(
        &w.root,
        "contribution-policy",
        "issues",
        &event("opened").to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert_eq!(
        p.diagnostics,
        vec![
            "Could not read APPROVED_CONTRIBUTORS: controlled gh failure",
            "Search failed: controlled gh failure",
            "Alice has no configured activity, passing"
        ]
    );
}

#[test]
fn activity_empty_configuration_does_not_search() {
    let w = fixture();
    for gate in [
        json!({"repositories":[],"label":""}),
        json!({"repositories":[],"label":"activity"}),
        json!({"repositories":["fixture/remote"],"label":""}),
    ] {
        policy_support::configure(&w, |v| {
            v["activity_gate"] = gate;
        });
        let mut p = RecordingProcess::default();
        policy_support::content(&mut p, "");
        p.reply(json!({"permission":"none"}));
        run(
            &w.root,
            "contribution-policy",
            "issues",
            &event("opened").to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(p.requests.len(), 2);
    }
}

#[test]
fn approval_completion_requires_trusted_merged_request() {
    let w = fixture();
    let e = policy_support::completion_event();
    for (pointer, value) in [
        ("/merged", json!(false)),
        ("/user/login", json!("wrong[bot]")),
        ("/base/ref", json!("other")),
        ("/head/repo/full_name", json!("fork/project")),
        ("/base/repo/full_name", json!("other/project")),
        ("/head/ref", json!("feature/unrelated")),
        ("/body", json!("unrelated")),
        ("/changed_files", json!(2)),
    ] {
        let mut rejected = e.clone();
        *rejected["pull_request"].pointer_mut(pointer).unwrap() = value;
        let mut p = RecordingProcess::default();
        p.reply(rejected["pull_request"].clone());
        run(
            &w.root,
            "approve-contributor",
            "pull_request_target",
            &rejected.to_string(),
            Some("policy-app"),
            &mut p,
        )
        .unwrap();
        assert!(p.requests.iter().all(|r| r.args[2] == "GET"));
    }
    let mut p = RecordingProcess::default();
    policy_support::completion_replies(&mut p, &e, "Alice pr\n");
    run(
        &w.root,
        "approve-contributor",
        "pull_request_target",
        &e.to_string(),
        Some("policy-app"),
        &mut p,
    )
    .unwrap();
    assert_eq!(
        p.requests.last().unwrap().args[3],
        "repos/fixture/project/issues/7/comments"
    );
    let mut p = RecordingProcess::default();
    p.reply(e["pull_request"].clone());
    p.reply(json!([{"filename":"AGENTS.md"}]));
    run(
        &w.root,
        "approve-contributor",
        "pull_request_target",
        &e.to_string(),
        Some("policy-app"),
        &mut p,
    )
    .unwrap();
    assert_eq!(p.requests.len(), 2);
}

#[test]
fn approval_status_messages_follow_effective_capability() {
    let w = fixture();
    for (body, capability, guidance) in [
        (
            "LGTM then LGtMi",
            "issue",
            "@Alice approved for issues. Your future issues will not be auto-closed. PRs still require `lgtm`.",
        ),
        (
            "lgtmi\nlgtm",
            "issue",
            "@Alice approved for issues. Your future issues will not be auto-closed. PRs still require `lgtm`.",
        ),
        (
            "élgtmé",
            "pr",
            "@Alice approved for issues and PRs. Your future issues and PRs will not be auto-closed.",
        ),
        (
            "(LGTM)!",
            "pr",
            "@Alice approved for issues and PRs. Your future issues and PRs will not be auto-closed.",
        ),
    ] {
        let mut e = event("created");
        e["comment"]["body"] = json!(body);
        let mut p = RecordingProcess::default();
        policy_support::update_replies(&mut p);
        assert_eq!(
            run(
                &w.root,
                "approve-contributor",
                "issue_comment",
                &e.to_string(),
                Some("policy-app"),
                &mut p
            )
            .unwrap(),
            vec![
                ("status".into(), "added".into()),
                ("capability".into(), capability.into())
            ]
        );
        assert!(
            !p.requests
                .iter()
                .any(|r| r.args.iter().any(|s| s.ends_with("/comments")))
        );
        let mut e = policy_support::completion_event();
        e["comment"]["body"] = json!(body);
        e["pull_request"]["body"] = json!(format!(
            "<!-- maestro-approval:{{\"issue\":7,\"comment\":42,\"author\":\"Alice\",\"capability\":\"{capability}\"}} -->"
        ));
        let mut p = RecordingProcess::default();
        policy_support::completion_replies(&mut p, &e, &format!("Alice {capability}\n"));
        run(
            &w.root,
            "approve-contributor",
            "pull_request_target",
            &e.to_string(),
            Some("policy-app"),
            &mut p,
        )
        .unwrap();
        assert_eq!(
            p.requests.last().unwrap().input.as_ref().unwrap()["body"],
            format!(
                "{guidance}\n\nSee [CONTRIBUTING.md](https://github.com/fixture/project/blob/trunk/CONTRIBUTING.md)."
            )
        );
    }
}

#[test]
fn approval_completion_rechecks_comment_and_policy() {
    let w = fixture();
    let e = policy_support::completion_event();
    for invalid in [
        "author",
        "command",
        "permission",
        "effective",
        "issue-url",
        "pr-issue",
    ] {
        let mut p = RecordingProcess::default();
        p.reply(e["pull_request"].clone());
        p.reply(json!([{"filename":".github/APPROVED_CONTRIBUTORS","status":"modified"}]));
        let mut issue = e["issue"].clone();
        if invalid == "author" {
            issue["user"]["login"] = json!("Other");
        }
        if invalid == "pr-issue" {
            issue["pull_request"] = json!({});
        }
        p.reply(issue);
        let mut comment = e["comment"].clone();
        comment["issue_url"] = json!(if invalid == "issue-url" {
            "https://api.github.com/repos/fixture/project/issues/8"
        } else {
            "https://api.github.com/repos/fixture/project/issues/7"
        });
        if invalid == "command" {
            comment["body"] = json!("lgtmi");
        }
        p.reply(comment);
        if matches!(invalid, "permission" | "effective") {
            p.reply(json!({"permission":if invalid=="permission"{"read"}else{"write"}}));
        }
        if invalid == "effective" {
            policy_support::content(&mut p, "Alice issue\n");
        }
        run(
            &w.root,
            "approve-contributor",
            "pull_request_target",
            &e.to_string(),
            Some("policy-app"),
            &mut p,
        )
        .unwrap();
        assert!(p.requests.iter().all(|r| r.args[2] == "GET"));
    }
    let mut p = RecordingProcess::default();
    policy_support::completion_replies(&mut p, &e, "Alice pr\n");
    run(
        &w.root,
        "approve-contributor",
        "pull_request_target",
        &e.to_string(),
        Some("policy-app"),
        &mut p,
    )
    .unwrap();
    assert_eq!(p.requests[5].input, Some(json!({"ref":"trunk"})));
}

#[test]
fn approval_failures_never_claim_success() {
    let w = fixture();
    std::fs::remove_file(w.root.join(".github/APPROVED_CONTRIBUTORS")).unwrap();
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"write"}));
    assert!(
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &event("created").to_string(),
            Some("policy-app"),
            &mut p
        )
        .is_err()
    );
    assert_eq!(p.requests.len(), 1);
    std::fs::write(w.root.join(".github/APPROVED_CONTRIBUTORS"), "").unwrap();
    for failing in 1..=5 {
        let mut p = RecordingProcess::default();
        policy_support::update_replies(&mut p);
        p.replies.truncate(failing);
        p.fail();
        assert!(
            run(
                &w.root,
                "approve-contributor",
                "issue_comment",
                &event("created").to_string(),
                Some("policy-app"),
                &mut p
            )
            .is_err()
        );
        assert_eq!(p.requests.len(), failing + 1 + usize::from(failing >= 2));
        assert!(
            !p.requests
                .iter()
                .any(|r| r.args.iter().any(|s| s.ends_with("/comments")))
        );
    }
    std::fs::write(w.root.join(".github/APPROVED_CONTRIBUTORS"), "Alice pr\n").unwrap();
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"admin"}));
    p.fail();
    assert!(
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &event("created").to_string(),
            None,
            &mut p
        )
        .is_err()
    );
    assert_eq!(p.requests.len(), 2);
    let e = policy_support::completion_event();
    let mut p = RecordingProcess::default();
    policy_support::completion_replies(&mut p, &e, "Alice pr\n");
    p.replies.truncate(6);
    p.fail();
    assert!(
        run(
            &w.root,
            "approve-contributor",
            "pull_request_target",
            &e.to_string(),
            Some("policy-app"),
            &mut p
        )
        .is_err()
    );
    assert_eq!(p.requests.len(), 7);
}

#[test]
fn repository_policy_binary_uses_same_entrypoint() {
    use std::process::Command;
    let w = fixture();
    w.member("unused", "maestro-fixture", "");
    let payload = w.root.join("event.json");
    std::fs::write(&payload, "{}").unwrap();
    for dir in ["home", "config", "tmp", "cache", "data", "state"] {
        std::fs::create_dir(w.root.join(dir)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_repository_policy"))
        .arg("issue-gate")
        .current_dir(&w.root)
        .env_clear()
        .env("GITHUB_EVENT_NAME", "unrelated")
        .env("GITHUB_EVENT_PATH", &payload)
        .env("HOME", w.root.join("home"))
        .env("TMPDIR", w.root.join("tmp"))
        .env("XDG_CONFIG_HOME", w.root.join("config"))
        .env("XDG_CACHE_HOME", w.root.join("cache"))
        .env("XDG_DATA_HOME", w.root.join("data"))
        .env("XDG_STATE_HOME", w.root.join("state"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(
        run(
            &w.root,
            "issue-gate",
            "unrelated",
            "{}",
            None,
            &mut contribution_policy::SystemProcess
        )
        .unwrap()
        .is_empty()
    );
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin/repository_policy.rs"),
    )
    .unwrap();
    assert!(source.contains("contribution_policy::run("));
}

fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
fn yaml(path: &str) -> yaml_rust2::Yaml {
    yaml_rust2::YamlLoader::load_from_str(
        &std::fs::read_to_string(repository_root().join(path)).unwrap(),
    )
    .unwrap()
    .remove(0)
}

#[test]
fn workflow_routes_accept_only_intended_events() {
    for (file, events) in [
        (
            "approve-contributor",
            vec![
                ("issue_comment", "created"),
                ("pull_request_target", "closed"),
            ],
        ),
        ("issue-gate", vec![("issues", "opened")]),
        ("pr-gate", vec![("pull_request_target", "opened")]),
        (
            "contribution-policy",
            vec![("issues", "opened"), ("pull_request_target", "opened")],
        ),
    ] {
        let y = yaml(&format!(".github/workflows/{file}.yml"));
        assert_eq!(y["on"].as_hash().unwrap().len(), events.len());
        for (event, action) in events {
            assert_eq!(
                y["on"][event]["types"].as_vec().unwrap(),
                &vec![yaml_rust2::Yaml::String(action.into())]
            );
        }
        let w = fixture();
        for name in ["issues", "pull_request_target", "issue_comment", "push"] {
            for action in ["edited", "reopened", "synchronize", "deleted"] {
                let mut p = RecordingProcess::default();
                assert!(
                    run(
                        &w.root,
                        file,
                        name,
                        &event(action).to_string(),
                        Some("policy-app"),
                        &mut p
                    )
                    .unwrap()
                    .is_empty()
                );
                assert!(p.requests.is_empty());
            }
        }
    }
    let w = fixture();
    let mut e = event("created");
    e["issue"]["pull_request"] = json!({});
    let mut p = RecordingProcess::default();
    assert!(
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &e.to_string(),
            None,
            &mut p
        )
        .unwrap()
        .is_empty()
    );
    assert!(p.requests.is_empty());
    let condition = yaml(".github/workflows/approve-contributor.yml")["jobs"]["policy"]["if"]
        .as_str()
        .unwrap()
        .to_owned();
    for part in [
        "!github.event.issue.pull_request",
        "github.event.pull_request.merged",
        "github.event.pull_request.head.repo.full_name == github.repository",
        "github.event.pull_request.base.ref == github.event.repository.default_branch",
    ] {
        assert!(condition.contains(part));
    }
}

#[test]
fn workflow_adapters_keep_credentials_off_head_code() {
    for file in [
        "approve-contributor",
        "issue-gate",
        "pr-gate",
        "contribution-policy",
    ] {
        let y = yaml(&format!(".github/workflows/{file}.yml"));
        let job = &y["jobs"]["policy"];
        assert_eq!(job["permissions"]["contents"].as_str(), Some("read"));
        assert_eq!(job["env"]["TZ"].as_str(), Some("UTC"));
        for credential in ["GH_TOKEN", "GITHUB_TOKEN"] {
            assert!(job["env"][credential].is_badvalue());
            assert!(y["env"][credential].is_badvalue());
        }
        let steps = job["steps"].as_vec().unwrap();
        let checkout = &steps[0];
        assert_eq!(
            checkout["uses"].as_str(),
            Some("actions/checkout@11d5960a326750d5838078e36cf38b85af677262")
        );
        assert_eq!(
            checkout["with"]["ref"].as_str(),
            Some("${{ github.event.repository.default_branch }}")
        );
        assert_eq!(
            checkout["with"]["persist-credentials"].as_bool(),
            Some(false)
        );
        let mut build_index = None;
        let mut token_index = None;
        for (i, step) in steps.iter().enumerate() {
            if let Some(uses) = step["uses"].as_str() {
                let sha = uses.split('@').nth(1).unwrap();
                assert_eq!(sha.len(), 40);
                assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
                if uses.starts_with("actions/create-github-app-token@") {
                    assert_eq!(file, "approve-contributor");
                    token_index = Some(i);
                    assert_eq!(
                        uses,
                        "actions/create-github-app-token@bcd2ba49218906704ab6c1aa796996da409d3eb1"
                    );
                    assert_eq!(
                        step["with"]["client-id"].as_str(),
                        Some("${{ vars.RELEASE_APP_CLIENT_ID }}")
                    );
                    assert_eq!(
                        step["with"]["private-key"].as_str(),
                        Some("${{ secrets.RELEASE_APP_PRIVATE_KEY }}")
                    );
                    assert_eq!(
                        step["with"]["owner"].as_str(),
                        Some("${{ github.repository_owner }}")
                    );
                    assert_eq!(
                        step["with"]["repositories"].as_str(),
                        Some("${{ github.event.repository.name }}")
                    );
                    assert_eq!(step["with"]["permission-contents"].as_str(), Some("write"));
                    assert_eq!(
                        step["with"]["permission-pull-requests"].as_str(),
                        Some("write")
                    );
                    assert_eq!(step["with"]["permission-issues"].as_str(), Some("write"));
                }
            }
            if let Some(command) = step["run"].as_str() {
                assert!(!command.contains("${{"));
                assert!(!command.contains("github-script"));
                assert!(!command.contains("git checkout"));
                if command.contains("cargo build") {
                    build_index = Some(i);
                    assert!(command.contains("--locked"));
                    assert!(step["env"].is_badvalue());
                }
                if command.contains("target/debug/repository_policy") {
                    assert!(command.contains(file));
                    assert!(step["env"]["GH_TOKEN"].as_str().is_some());
                    if file == "approve-contributor" {
                        assert_eq!(
                            step["env"]["GH_TOKEN"].as_str(),
                            Some("${{ steps.app.outputs.token }}")
                        );
                        assert_eq!(step["id"].as_str(), Some("update"));
                        assert_eq!(
                            step["env"]["MAESTRO_APPROVAL_APP_SLUG"].as_str(),
                            Some("${{ steps.app.outputs.app-slug }}")
                        );
                    }
                }
            }
        }
        assert!(build_index.is_some());
        if file == "approve-contributor" {
            assert!(build_index.unwrap() < token_index.unwrap());
        } else {
            assert!(token_index.is_none());
        }
        if file != "approve-contributor" {
            assert_eq!(job["permissions"]["issues"].as_str(), Some("write"));
            assert_eq!(
                steps.last().unwrap()["env"]["GH_TOKEN"].as_str(),
                Some("${{ github.token }}")
            );
        }
        if matches!(file, "pr-gate" | "contribution-policy") {
            assert_eq!(job["permissions"]["pull-requests"].as_str(), Some("write"));
        }
    }
}

#[test]
fn policy_defaults_are_empty_maestro_data() {
    let root = repository_root();
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(".github/repository-policy.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        v,
        json!({"issue_gate":{"message_mode":"normal","weekend_days":[],"weekend_message":"","weekend_labels":[],"refactor_until":"","refactor_branch":"","refactor_reason":"","refactor_labels":[]},"help_url":"","activity_gate":{"repositories":[],"label":""}})
    );
    let approvals = std::fs::read_to_string(root.join(".github/APPROVED_CONTRIBUTORS")).unwrap();
    assert!(
        approvals
            .lines()
            .all(|line| line.trim().is_empty() || line.starts_with('#'))
    );
    assert!(approvals.contains("username capability"));
    assert!(approvals.contains("issue"));
    assert!(approvals.contains("pr"));
    let labels = std::fs::read_to_string(root.join("docs/agents/triage-labels.md")).unwrap();
    for label in [
        "needs-triage",
        "needs-info",
        "ready-for-agent",
        "ready-for-human",
        "wontfix",
    ] {
        assert!(labels.contains(label), "{label}");
    }
}

#[test]
fn templates_require_bug_and_proposal_fields() {
    for (file, required, optional) in [
        (
            "bug",
            vec!["description", "repro"],
            vec!["expected", "version"],
        ),
        ("contribution", vec!["what", "why"], vec!["how"]),
    ] {
        let y = yaml(&format!(".github/ISSUE_TEMPLATE/{file}.yml"));
        let body = y["body"].as_vec().unwrap();
        assert!(body[0]["attributes"]["value"].as_str().unwrap().contains(
            "https://github.com/Orchestration-Maestro/maestro/blob/main/CONTRIBUTING.md"
        ));
        for id in required {
            let field = body.iter().find(|v| v["id"].as_str() == Some(id)).unwrap();
            assert_eq!(field["validations"]["required"].as_bool(), Some(true));
            assert_eq!(field["type"].as_str(), Some("textarea"));
        }
        for id in optional {
            let field = body.iter().find(|v| v["id"].as_str() == Some(id)).unwrap();
            assert_eq!(field["validations"]["required"].as_bool(), Some(false));
        }
        if file == "bug" {
            assert_eq!(y["labels"][0].as_str(), Some("bug"));
        } else {
            assert!(y["labels"].as_vec().unwrap().is_empty());
        }
    }
    let config = yaml(".github/ISSUE_TEMPLATE/config.yml");
    assert_eq!(config["blank_issues_enabled"].as_bool(), Some(false));
    assert!(config["contact_links"].as_vec().unwrap().is_empty());
}

fn git(w: &support::Workspace, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .args(args)
        .current_dir(&w.root)
        .output()
        .unwrap()
}

#[test]
fn git_attributes_cover_text_and_binary() {
    let w = fixture();
    assert!(git(&w, &["init", "-q"]).status.success());
    std::fs::copy(
        repository_root().join(".gitattributes"),
        w.root.join(".gitattributes"),
    )
    .unwrap();
    for (file, eol) in [
        ("file.txt", "lf"),
        ("script.sh", "lf"),
        ("source.rs", "lf"),
        ("run.bat", "crlf"),
        ("run.cmd", "crlf"),
        ("run.ps1", "crlf"),
    ] {
        let output = git(&w, &["check-attr", "eol", "--", file]);
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{file}: eol: {eol}\n")
        );
    }
    for extension in [
        "png", "jpg", "jpeg", "gif", "webp", "ico", "pdf", "zip", "gz", "woff", "woff2",
    ] {
        let file = format!("asset.{extension}");
        let output = git(&w, &["check-attr", "text", "diff", "merge", "--", &file]);
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{file}: text: unset\n{file}: diff: unset\n{file}: merge: unset\n")
        );
    }
}

#[test]
fn ignore_patterns_hide_artifacts_not_sources() {
    let w = fixture();
    assert!(git(&w, &["init", "-q"]).status.success());
    std::fs::copy(
        repository_root().join(".gitignore"),
        w.root.join(".gitignore"),
    )
    .unwrap();
    for file in [
        "target/build",
        "dist/app",
        "node_modules/pkg",
        "packages/foo/dist/x",
        "packages/foo/dist-chrome/x",
        "packages/foo/dist-firefox/x",
        "run.log",
        ".DS_Store",
        "cache.tsbuildinfo",
        "cpu.cpuprofile",
        ".env",
        ".vscode/settings.json",
        ".zed/settings.json",
        ".idea/workspace.xml",
        "file.swp",
        "file.swo",
        "file~",
        ".npm/cache",
        "coverage/report",
        ".nyc_output/result",
        ".maestro_config/settings",
        "tui-debug.log",
        "compaction-results/result",
        "syntax.jsonl",
        "out.jsonl",
        "maestro-session.html",
        "out.html",
        "packages/maestro/binaries/exe",
        "todo.md",
        "plans/plan.md",
        ".maestro/hf-sessions/run",
        ".maestro/hf-sessions-backup/run",
        "collect.sh",
    ] {
        assert!(
            git(&w, &["check-ignore", "--no-index", file])
                .status
                .success(),
            "{file}"
        );
    }
    for file in [
        "src/lib.rs",
        "docs/readme.md",
        ".github/repository-policy.json",
        "Cargo.toml",
        "Cargo.lock",
        ".gitignore",
        ".gitattributes",
        ".maestro/prompts/example.md",
        ".env.local",
    ] {
        assert_eq!(
            git(&w, &["check-ignore", "--no-index", file]).status.code(),
            Some(1),
            "{file}"
        );
    }
}

#[test]
fn distribution_keeps_required_license_notices() {
    let w = fixture();
    assert!(git(&w, &["init", "-q"]).status.success());
    std::fs::create_dir(w.root.join("distribution")).unwrap();
    std::fs::copy(
        repository_root().join("LICENSE"),
        w.root.join("distribution/LICENSE"),
    )
    .unwrap();
    assert!(git(&w, &["add", "distribution/LICENSE"]).status.success());
    let output = git(&w, &["show", ":distribution/LICENSE"]);
    assert!(output.status.success());
    let notice = String::from_utf8(output.stdout).unwrap();
    let required = [
        "Copyright (c) 2026 Orchestration-Maestro contributors",
        "Copyright (c) 2025 Mario Zechner",
        "Permission is hereby granted, free of charge",
        "The above copyright notice and this permission notice shall be included",
        "THE SOFTWARE IS PROVIDED \"AS IS\"",
        "IN NO EVENT SHALL THE",
    ];
    for text in required {
        assert!(notice.contains(text), "{text}");
    }
    let missing = notice.replace("Copyright (c) 2025 Mario Zechner", "");
    std::fs::write(w.root.join("distribution/LICENSE"), missing).unwrap();
    assert!(git(&w, &["add", "distribution/LICENSE"]).status.success());
    let output = git(&w, &["show", ":distribution/LICENSE"]);
    assert!(output.status.success());
    let missing = String::from_utf8(output.stdout).unwrap();
    assert!(!required.iter().all(|text| missing.contains(text)));
}

#[test]
fn contributor_guidance_preserves_current_rules() {
    let root = repository_root();
    let contributing = std::fs::read_to_string(root.join("CONTRIBUTING.md")).unwrap();
    for text in [
        "understand your code",
        "AGENTS.md",
        "own voice",
        "why it matters",
        "lgtmi",
        "lgtm",
        "extension",
        "twice",
        "FAQ",
        "just check",
        "just test",
        "CHANGELOG.md",
    ] {
        assert!(contributing.contains(text), "{text}");
    }
    for paragraph in [
        "# Contributing to Maestro",
        "This guide exists to save both sides time.",
        "## The One Rule",
        "**You must understand your code.** If you cannot explain what your changes do and how they interact with the rest of the system, your PR will be closed.",
        "Using AI to write code is fine. Submitting AI-generated slop without understanding it is not.",
        "If you use an agent, run it from the repository root directory so it picks up `AGENTS.md` automatically. Your agent must follow the rules and guidelines in that file.",
        "## Contribution Gate",
        "All issues and PRs from new contributors are auto-closed by default.",
        "Maintainers review auto-closed issues daily and reopen worthwhile ones. Issues that do not meet the quality bar below will not be reopened or receive a reply.",
        "Approval happens through maintainer replies on issues:",
        "- `lgtmi`: your future issues will not be auto-closed\n- `lgtm`: your future issues and PRs will not be auto-closed",
        "`lgtmi` does not grant rights to submit PRs. Only `lgtm` grants rights to submit PRs.",
        "## Quality Bar For Issues",
        "If you open an issue, you must use one of the two GitHub issue templates.",
        "If you open an issue, keep it short, concrete, and worth reading.",
        "- Keep it concise. If it does not fit on one screen, it is too long.\n- Write in your own voice.\n- State the bug or request clearly.\n- Explain why it matters.\n- If you want to implement the change yourself, say so.",
        "If the issue is real and written well, a maintainer may reopen it, reply `lgtmi`, or reply `lgtm`.",
        "## Blocking",
        "If you ignore this document twice, or if you spam the tracker with agent-generated issues, your GitHub account will be permanently blocked.",
        "If you send a large volume of issues through automation, your GitHub account will be permanently blocked. No taksies backsies.",
        "## Before Submitting a PR",
        "Do not open a PR unless you have already been approved with `lgtm`.",
        "Before submitting a PR:",
        "```bash\njust check\njust test\n```",
        "Both must pass.",
        "Do not edit `CHANGELOG.md`. Changelog entries are added by maintainers.",
        "If you are adding a new provider to `crates/maestro-models`, see `AGENTS.md` for required tests.",
        "## Philosophy",
        "Maestro's core is minimal. If your feature does not belong in the core, it should be an extension. PRs that bloat the core will likely be rejected.",
        "## Questions?",
        "## FAQ",
        "### Why are new issues and PRs auto-closed?",
        "Maestro receives more issues than the maintainers can responsibly review in real time. Many reports do not meet the quality bar in this guide or do not follow CONTRIBUTING.md. Some are slung at the repository mindlessly via an agent instead of being reviewed and shaped by the person submitting them. Auto-closing creates a buffer so maintainers can review the tracker on their own schedule and reopen the issues that meet the quality bar.",
        "### Why are weekend issues not reviewed?",
        "The weekend route is configurable and currently off.",
        "### Why do some issues get no reply?",
        "A reply is maintenance work too. Low-signal issues, unclear reports, duplicates, and issues that do not follow this guide may be closed without discussion. This keeps time available for reproducible bugs, thoughtful requests, and contributors who have done the work to make their report actionable.",
        "### Why not let AI triage everything?",
        "AI can help group duplicates, summarize reports, and spot missing information. It is not trusted to make final maintainer decisions. Polished AI-generated issues can still be wrong, misleading, or expensive to investigate. Human review remains the final gate.",
        "### Is this hostile to contributors?",
        "No. It is a guardrail against burnout and tracker spam. Short, concrete, reproducible issues are welcome. Thoughtful contributions are welcome. Automated slop, entitlement, and large volumes of low-effort reports are not.",
    ] {
        assert!(contributing.contains(paragraph), "{paragraph}");
    }
    assert!(contributing.contains("Human review remains the final gate."));
    let agents = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
    for text in [
        "just check",
        "just test",
        "merge queue",
        "git commit -S",
        "git add <specific",
        "--body-file",
        "Preview",
        "Closes #",
        "80",
        "24",
        "tmux kill-session",
        "Escape",
        "C-o",
        "not implemented",
        "registration",
        "options",
        "authentication",
        "generated catalog",
        "representative",
        "token",
        "abort",
        "empty",
        "overflow",
        "image",
        "Unicode",
        "tool result",
        "cross-provider",
        "patch",
        "minor",
        "lockstep",
        "base64",
        "chrono",
        "yaml-rust2",
        "configurable keybindings",
        "external API",
        "intentional functionality",
    ] {
        assert!(agents.contains(text), "{text}");
    }
    let glossary = std::fs::read_to_string(root.join("CONTEXT.md")).unwrap();
    for text in ["Contributor capability", "Effective approval"] {
        assert!(glossary.contains(text));
    }
    let policy = std::fs::read_to_string(root.join("docs/repository_policy.md")).unwrap();
    assert!(policy.contains("merged"));
    for text in [
        "repository_policy",
        "GITHUB_OUTPUT",
        "MAESTRO_APPROVAL_APP_SLUG",
        "weekend_days",
        "refactor_until",
        "help_url",
        "App",
        "fixtures",
        "failure",
        "default branch",
    ] {
        assert!(policy.contains(text), "{text}");
    }
}

#[test]
fn approval_stale_checkout_fails_before_branch_creation() {
    let w = fixture();
    let mut p = RecordingProcess {
        git_head: "older-head".into(),
        ..RecordingProcess::default()
    };
    policy_support::update_replies(&mut p);
    let error = run(
        &w.root,
        "approve-contributor",
        "issue_comment",
        &event("created").to_string(),
        Some("policy-app"),
        &mut p,
    )
    .unwrap_err();
    assert_eq!(
        error,
        "Trusted checkout HEAD does not match default branch SHA"
    );
    assert_eq!(p.requests.len(), 3);
    assert_eq!(p.requests[2].program, "git");
    assert_eq!(p.requests[2].args, ["rev-parse", "HEAD"]);
    assert!(
        p.requests
            .iter()
            .all(|r| r.program == "git" || r.args[2] == "GET")
    );
}

#[test]
fn reference_branch_diagnostics_preserve_values_and_provenance() {
    let w = fixture();
    for (body, permission, expected) in [
        (
            "no command",
            Some("write"),
            "Comment does not match lgtm or lgtmi",
        ),
        ("lgtm", Some("read"), "Reviewer does not have write access"),
        ("lgtm", None, "Reviewer does not have collaborator access"),
    ] {
        let mut e = event("created");
        e["comment"]["body"] = json!(body);
        let mut p = RecordingProcess::default();
        if body == "lgtm" {
            if let Some(permission) = permission {
                p.reply(json!({"permission":permission}));
            } else {
                p.fail();
            }
        }
        run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &e.to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(p.diagnostics, vec![expected]);
        assert_eq!(p.requests.len(), usize::from(body == "lgtm"));
    }
    std::fs::write(
        w.root.join(".github/APPROVED_CONTRIBUTORS"),
        "  malformed  \n  Invalid bad  \nAlice pr\n",
    )
    .unwrap();
    let mut p = RecordingProcess::default();
    p.reply(json!({"permission":"write"}));
    p.reply(json!({}));
    run(
        &w.root,
        "approve-contributor",
        "issue_comment",
        &event("created").to_string(),
        None,
        &mut p,
    )
    .unwrap();
    assert_eq!(
        p.diagnostics,
        vec![
            "Skipping malformed line: malformed",
            "Skipping line with invalid capability: Invalid bad",
            "Alice is already approved for pr",
        ]
    );
    for (content, status) in [("", "added"), ("Alice issue\n", "updated")] {
        std::fs::write(w.root.join(".github/APPROVED_CONTRIBUTORS"), content).unwrap();
        let mut p = RecordingProcess::default();
        policy_support::update_replies(&mut p);
        let outputs = run(
            &w.root,
            "approve-contributor",
            "issue_comment",
            &event("created").to_string(),
            Some("policy-app"),
            &mut p,
        )
        .unwrap();
        assert_eq!(
            outputs,
            vec![
                ("status".into(), status.into()),
                ("capability".into(), "pr".into())
            ]
        );
        assert_eq!(p.diagnostics, vec!["Set Alice capability to pr"]);
    }
    for pr in [false, true] {
        let workflow = if pr { "pr-gate" } else { "issue-gate" };
        let event_name = if pr { "pull_request_target" } else { "issues" };
        for branch in ["bot", "collaborator", "approved", "unapproved"] {
            let mut e = policy_support::gate_event(pr);
            let mut p = RecordingProcess::default();
            let expected = match branch {
                "bot" => {
                    e[if pr { "pull_request" } else { "issue" }]["user"]["login"] =
                        json!("Helper[bot]");
                    vec!["Skipping bot: Helper[bot]"]
                }
                "collaborator" => {
                    p.reply(json!({"permission":"maintain"}));
                    vec!["Alice is a collaborator with maintain access"]
                }
                _ => {
                    p.reply(json!({"permission":"none"}));
                    policy_support::content(
                        &mut p,
                        if branch == "approved" {
                            "  malformed  \n  Invalid bad  \nAlice pr\n"
                        } else {
                            "  malformed  \n  Invalid bad  \n"
                        },
                    );
                    let mut expected = vec![
                        "Skipping malformed line:   malformed  ",
                        "Skipping line with invalid capability:   Invalid bad  ",
                    ];
                    if branch == "approved" {
                        expected.push(if pr {
                            "Alice is approved for PRs"
                        } else {
                            "Alice is approved for pr"
                        });
                    } else {
                        p.reply(json!({}));
                        p.reply(json!({}));
                        if pr {
                            expected.push("Alice is not approved, closing PR");
                        }
                    }
                    expected
                }
            };
            run(&w.root, workflow, event_name, &e.to_string(), None, &mut p).unwrap();
            assert_eq!(p.diagnostics, expected, "{workflow} {branch}");
            assert_eq!(
                p.requests.len(),
                match branch {
                    "bot" => 0,
                    "collaborator" => 1,
                    "approved" => 2,
                    _ => 4,
                }
            );
        }
        let mut p = RecordingProcess::default();
        p.reply(json!({"permission":"none"}));
        policy_support::content(&mut p, "Alice issue\n");
        if pr {
            p.reply(json!({}));
            p.reply(json!({}));
        }
        run(
            &w.root,
            workflow,
            event_name,
            &policy_support::gate_event(pr).to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(
            p.diagnostics,
            vec![if pr {
                "Alice is not approved, closing PR"
            } else {
                "Alice is approved for issue"
            }]
        );
    }
    policy_support::configure(&w, |v| {
        v["activity_gate"] = json!({"repositories":["fixture/remote"],"label":"activity"});
    });
    for branch in [
        "bot",
        "approved",
        "collaborator",
        "read-error",
        "match",
        "no-match",
        "search-error",
    ] {
        let mut e = event("opened");
        let mut p = RecordingProcess::default();
        let expected = match branch {
            "bot" => {
                e["issue"]["user"]["login"] = json!("Helper[bot]");
                vec!["Skipping bot: Helper[bot]"]
            }
            "approved" => {
                policy_support::content(&mut p, "Alice issue\n");
                vec!["Alice is in APPROVED_CONTRIBUTORS, passing"]
            }
            "collaborator" | "read-error" => {
                if branch == "read-error" {
                    p.fail();
                } else {
                    policy_support::content(&mut p, "");
                }
                p.reply(json!({"permission":"admin"}));
                if branch == "read-error" {
                    vec![
                        "Could not read APPROVED_CONTRIBUTORS: controlled failure",
                        "Alice is a collaborator (admin), passing",
                    ]
                } else {
                    vec!["Alice is a collaborator (admin), passing"]
                }
            }
            _ => {
                policy_support::content(&mut p, "malformed\nInvalid bad\n");
                p.fail();
                if branch == "search-error" {
                    p.fail();
                } else {
                    p.reply(json!({"total_count": if branch == "match" { 3 } else { 0 }}));
                }
                if branch == "match" {
                    p.reply(json!({}));
                    vec![
                        "Alice has opened 3 issues/PRs on fixture/remote",
                        "Alice has configured activity, adding label",
                    ]
                } else if branch == "search-error" {
                    vec![
                        "Search failed: controlled failure",
                        "Alice has no configured activity, passing",
                    ]
                } else {
                    vec!["Alice has no configured activity, passing"]
                }
            }
        };
        run(
            &w.root,
            "contribution-policy",
            "issues",
            &e.to_string(),
            None,
            &mut p,
        )
        .unwrap();
        assert_eq!(p.diagnostics, expected, "activity {branch}");
        assert_eq!(
            p.requests.len(),
            match branch {
                "bot" => 0,
                "approved" => 1,
                "collaborator" | "read-error" => 2,
                "match" => 4,
                _ => 3,
            }
        );
    }
}
