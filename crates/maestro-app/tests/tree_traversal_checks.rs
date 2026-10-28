//! Session-tree appends, branches, projections and forks observed on an application
//! session, compared with the recorded traversal cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, harness, on_task_set, recorded, recorded_number, user,
};
use maestro_session::Presence;
use maestro_session::session_manager::{
    CompactionOptions, ReadonlySessionManager, parse_session_entries,
};
use maestro_session::{SessionEntry, SessionManager};
use serde_json::{Value, json};

/// The position of `entry`'s parent among the appended identities, or `-1` for a root.
fn parent_position(ids: &[String], entry: &SessionEntry) -> Value {
    entry["parentId"].as_str().map_or(json!(-1), |parent| {
        json!(
            ids.iter()
                .position(|id| id == parent)
                .expect("appended parent")
        )
    })
}

/// The positions of `entries` among the appended identities, in order.
fn positions(ids: &[String], entries: impl IntoIterator<Item = String>) -> Vec<Value> {
    entries
        .into_iter()
        .map(|id| {
            json!(
                ids.iter()
                    .position(|known| *known == id)
                    .expect("appended identity")
            )
        })
        .collect()
}

/// The identities of the entries the session retains now.
fn retained(manager: &dyn ReadonlySessionManager) -> Vec<String> {
    manager
        .get_entries()
        .iter()
        .map(|entry| entry["id"].as_str().expect("entry identity").to_owned())
        .collect()
}

/// Whether the session retains the same entries, in order, as `ids`.
fn retains(manager: &dyn ReadonlySessionManager, ids: &[String]) -> bool {
    retained(manager) == ids
}

/// Appends `messages` to the session history and returns their identities.
fn append(h: &Harness, messages: &[Value]) -> Vec<String> {
    let mut manager = h.session.session_manager().borrow_mut();
    messages
        .iter()
        .map(|message| manager.append_message(message.clone()).expect("append"))
        .collect()
}

/// The recorded form of `error`'s message.
fn message(error: &std::io::Error) -> Value {
    json!(error.to_string())
}

/// The three plain messages the append and path cases build.
fn linear_messages() -> Vec<Value> {
    vec![
        user(&json!("1"), 1),
        crate::support::session_harness::assistant("2", 2, &json!({})),
        user(&json!("3"), 3),
    ]
}

/// The session over the harness, borrowed mutably.
fn history(h: &Harness) -> std::cell::RefMut<'_, SessionManager> {
    h.session.session_manager().borrow_mut()
}

/// The session over the harness, borrowed.
fn history_view(h: &Harness) -> std::cell::Ref<'_, SessionManager> {
    h.session.session_manager().borrow()
}

/// The entry of `kind` in the retained entries.
fn of_type<'a>(manager: &'a dyn ReadonlySessionManager, kind: &str) -> &'a SessionEntry {
    manager
        .get_entries()
        .into_iter()
        .find(|entry| entry["type"] == kind)
        .unwrap_or_else(|| panic!("no {kind} entry"))
}

/// Whether `entry`'s identity is the appended identity at `index`.
fn identity_matches(ids: &[String], entry: &SessionEntry, index: usize) -> Value {
    json!(entry["id"] == json!(ids[index]))
}

/// Whether `entry`'s parent is the appended identity at `index`.
fn parent_matches(ids: &[String], entry: &SessionEntry, index: usize) -> Value {
    json!(entry["parentId"] == json!(ids[index]))
}

#[test]
fn message_appends_form_the_parent_chain() {
    let expected = recorded("model_navigation", "message_appends_form_the_parent_chain");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &linear_messages());
        let manager = history_view(&h);
        let observed: Vec<Value> = manager
            .get_entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                json!({
                    "id_matches": identity_matches(&ids, entry, index),
                    "parent": parent_position(&ids, entry),
                    "type": entry["type"],
                })
            })
            .collect();
        assert_eq!(json!(observed), expected);
    });
}

#[test]
fn thinking_entry_is_part_of_the_message_chain() {
    let expected = recorded(
        "model_navigation",
        "thinking_entry_is_part_of_the_message_chain",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut manager = history(&h);
        let first = manager
            .append_message(user(&json!("hello"), 1))
            .expect("append");
        let thinking = manager
            .append_thinking_level_change("high")
            .expect("append");
        manager
            .append_message(crate::support::session_harness::assistant(
                "response",
                2,
                &json!({}),
            ))
            .expect("append");
        let ids = vec![first, thinking];
        drop(manager);
        let manager = h.session.session_manager().borrow();
        let entry = of_type(&*manager, "thinking_level_change");
        let observed = json!({
            "length": manager.get_entries().len(),
            "id_matches": identity_matches(&ids, entry, 1),
            "parent_matches": parent_matches(&ids, entry, 0),
            "next_parent_matches": json!(manager.get_entries()[2]["parentId"] == json!(ids[1])),
            "thinking": entry["thinkingLevel"],
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn model_entry_is_part_of_the_message_chain() {
    let expected = recorded(
        "model_navigation",
        "model_entry_is_part_of_the_message_chain",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut manager = history(&h);
        let first = manager
            .append_message(user(&json!("hello"), 1))
            .expect("append");
        let change = manager
            .append_model_change("openai", "gpt-4")
            .expect("append");
        manager
            .append_message(crate::support::session_harness::assistant(
                "response",
                2,
                &json!({}),
            ))
            .expect("append");
        let ids = vec![first, change];
        drop(manager);
        let manager = h.session.session_manager().borrow();
        let entry = of_type(&*manager, "model_change");
        let observed = json!({
            "type": entry["type"],
            "provider": entry["provider"],
            "model": entry["modelId"],
            "id_matches": identity_matches(&ids, entry, 1),
            "parent_matches": parent_matches(&ids, entry, 0),
            "next_parent_matches": json!(manager.get_entries()[2]["parentId"] == json!(ids[1])),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn compaction_entry_is_part_of_the_message_chain() {
    let expected = recorded(
        "model_navigation",
        "compaction_entry_is_part_of_the_message_chain",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut manager = history(&h);
        let first = manager
            .append_message(user(&json!("1"), 1))
            .expect("append");
        let second = manager
            .append_message(crate::support::session_harness::assistant(
                "2",
                2,
                &json!({}),
            ))
            .expect("append");
        let compaction = manager
            .append_compaction("summary", &first, 1000.0, CompactionOptions::default())
            .expect("append");
        manager
            .append_message(user(&json!("3"), 3))
            .expect("append");
        let ids = vec![first, second, compaction];
        drop(manager);
        let manager = h.session.session_manager().borrow();
        let entry = of_type(&*manager, "compaction");
        let observed = json!({
            "type": entry["type"],
            "summary": entry["summary"],
            "tokens": recorded_number(entry["tokensBefore"].as_f64().expect("token count")),
            "kept_matches": json!(entry["firstKeptEntryId"] == json!(ids[0])),
            "parent_matches": parent_matches(&ids, entry, 1),
            "id_matches": identity_matches(&ids, entry, 2),
            "next_parent_matches": json!(manager.get_entries()[3]["parentId"] == json!(ids[2])),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn custom_entry_is_part_of_the_message_chain() {
    let expected = recorded(
        "model_navigation",
        "custom_entry_is_part_of_the_message_chain",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut manager = history(&h);
        let first = manager
            .append_message(user(&json!("hello"), 1))
            .expect("append");
        let data = json!({"key": "value"});
        let custom = manager
            .append_custom_entry("my_data", Presence::Present(data))
            .expect("append");
        manager
            .append_message(crate::support::session_harness::assistant(
                "response",
                2,
                &json!({}),
            ))
            .expect("append");
        let ids = vec![first, custom];
        drop(manager);
        let manager = h.session.session_manager().borrow();
        let entry = of_type(&*manager, "custom");
        let observed = json!({
            "type": entry["type"],
            "custom_type": entry["customType"],
            "data": entry["data"],
            "id_matches": identity_matches(&ids, entry, 1),
            "parent_matches": parent_matches(&ids, entry, 0),
            "next_parent_matches": json!(manager.get_entries()[2]["parentId"] == json!(ids[1])),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn each_append_advances_the_active_leaf() {
    let expected = recorded("model_navigation", "each_append_advances_the_active_leaf");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut manager = history(&h);
        let mut advanced = vec![json!(manager.get_leaf_id().is_none())];
        let id = manager
            .append_message(user(&json!("1"), 1))
            .expect("append");
        advanced.push(json!(manager.get_leaf_id() == Some(id.as_str())));
        let id = manager
            .append_message(crate::support::session_harness::assistant(
                "2",
                2,
                &json!({}),
            ))
            .expect("append");
        advanced.push(json!(manager.get_leaf_id() == Some(id.as_str())));
        let id = manager
            .append_thinking_level_change("high")
            .expect("append");
        advanced.push(json!(manager.get_leaf_id() == Some(id.as_str())));
        assert_eq!(json!(advanced), expected);
    });
}

#[test]
fn one_message_has_a_single_entry_branch() {
    let expected = recorded("model_navigation", "one_message_has_a_single_entry_branch");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &[user(&json!("hello"), 1)]);
        let manager = history_view(&h);
        let branch = manager.get_branch(None).expect("branch");
        let observed = json!({
            "length": branch.len(),
            "identity": json!(branch[0]["id"] == json!(ids[0])),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn active_branch_lists_root_to_leaf_in_order() {
    let expected = recorded(
        "model_navigation",
        "active_branch_lists_root_to_leaf_in_order",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut ids = append(&h, &linear_messages()[..2]);
        let thinking = history(&h)
            .append_thinking_level_change("high")
            .expect("append");
        ids.push(thinking);
        ids.extend(append(&h, &linear_messages()[2..]));
        let manager = history_view(&h);
        let branch: Vec<String> = manager
            .get_branch(None)
            .expect("branch")
            .iter()
            .map(|entry| entry["id"].as_str().expect("identity").to_owned())
            .collect();
        assert_eq!(json!(positions(&ids, branch)), expected);
    });
}

#[test]
fn specified_branch_stops_at_the_requested_entry() {
    let expected = recorded(
        "model_navigation",
        "specified_branch_stops_at_the_requested_entry",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let messages = vec![
            user(&json!("1"), 1),
            crate::support::session_harness::assistant("2", 2, &json!({})),
            user(&json!("3"), 3),
            crate::support::session_harness::assistant("4", 4, &json!({})),
        ];
        let ids = append(&h, &messages);
        let manager = history_view(&h);
        let branch: Vec<String> = manager
            .get_branch(Some(&ids[1]))
            .expect("branch")
            .iter()
            .map(|entry| entry["id"].as_str().expect("identity").to_owned())
            .collect();
        assert_eq!(json!(positions(&ids, branch)), expected);
    });
}

/// The recorded shape of one tree node: its identity position and its children.
fn node(ids: &[String], node: &maestro_session::session_manager::SessionTreeNode<'_>) -> Value {
    json!({
        "id": json!(ids.iter().position(|id| node.entry["id"] == json!(id)).expect("identity")),
        "children": node.children.len(),
    })
}

#[test]
fn linear_tree_retains_one_root_and_nested_children() {
    let expected = recorded(
        "model_navigation",
        "linear_tree_retains_one_root_and_nested_children",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &linear_messages());
        let manager = history_view(&h);
        let tree = manager.get_tree();
        let root = node(&ids, &tree[0]);
        let child = node(&ids, &tree[0].children[0]);
        let grandchild = node(&ids, &tree[0].children[0].children[0]);
        let observed = json!({
            "roots": tree.len(),
            "root": root["id"],
            "children": root["children"],
            "child": child["id"],
            "grandchildren": child["children"],
            "grandchild": grandchild["id"],
            "end": grandchild["children"],
        });
        assert_eq!(observed, expected);
    });
}

/// The recorded position of `entry`'s identity among `ids`.
fn position_of(ids: &[String], entry: &SessionEntry) -> Value {
    json!(
        ids.iter()
            .position(|id| entry["id"] == json!(id))
            .expect("appended identity")
    )
}

#[test]
fn branching_retains_both_sibling_paths() {
    let expected = recorded("model_navigation", "branching_retains_both_sibling_paths");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &linear_messages());
        let mut manager = history(&h);
        manager.branch(&ids[1]).expect("branch");
        let alternate = manager
            .append_message(user(&json!("4-branch"), 4))
            .expect("append");
        let mut all = ids;
        all.push(alternate);
        drop(manager);
        let manager = h.session.session_manager().borrow();
        let tree = manager.get_tree();
        let second = &tree[0].children[0];
        let mut siblings: Vec<Value> = second
            .children
            .iter()
            .map(|c| position_of(&all, c.entry))
            .collect();
        siblings.sort_by_key(Value::as_i64);
        let observed = json!({
            "roots": tree.len(),
            "root": position_of(&all, tree[0].entry),
            "children": tree[0].children.len(),
            "node": position_of(&all, second.entry),
            "siblings": siblings,
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn several_branches_share_the_same_parent() {
    let expected = recorded("model_navigation", "several_branches_share_the_same_parent");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut ids = append(
            &h,
            &[
                user(&json!("root"), 1),
                crate::support::session_harness::assistant("response", 2, &json!({})),
            ],
        );
        let mut manager = history(&h);
        let mut branches = Vec::new();
        for text in ["branch-A", "branch-B", "branch-C"] {
            manager.branch(&ids[1]).expect("branch");
            branches.push(
                manager
                    .append_message(user(&json!(text), 3))
                    .expect("append"),
            );
        }
        ids.extend(branches);
        drop(manager);
        let manager = h.session.session_manager().borrow();
        let node = &manager.get_tree()[0].children[0];
        let mut shared: Vec<Value> = node
            .children
            .iter()
            .map(|c| position_of(&ids, c.entry))
            .collect();
        shared.sort_by_key(Value::as_i64);
        assert_eq!(json!(shared), expected);
    });
}

#[test]
fn nested_branches_keep_descendants_on_their_own_paths() {
    let expected = recorded(
        "model_navigation",
        "nested_branches_keep_descendants_on_their_own_paths",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let messages = vec![
            user(&json!("1"), 1),
            crate::support::session_harness::assistant("2", 2, &json!({})),
            user(&json!("3"), 3),
            crate::support::session_harness::assistant("4", 4, &json!({})),
        ];
        let mut ids = append(&h, &messages);
        let mut manager = history(&h);
        manager.branch(&ids[1]).expect("branch");
        let five = manager
            .append_message(user(&json!("5"), 5))
            .expect("append");
        let six = manager
            .append_message(crate::support::session_harness::assistant(
                "6",
                6,
                &json!({}),
            ))
            .expect("append");
        manager.branch(&five).expect("branch");
        let seven = manager
            .append_message(user(&json!("7"), 7))
            .expect("append");
        ids.extend([five, six, seven]);
        drop(manager);
        let manager = h.session.session_manager().borrow();
        let second = &manager.get_tree()[0].children[0];
        let sorted = |nodes: &[maestro_session::session_manager::SessionTreeNode<'_>]| {
            let mut values: Vec<Value> = nodes.iter().map(|c| position_of(&ids, c.entry)).collect();
            values.sort_by_key(Value::as_i64);
            values
        };
        let node5 = second
            .children
            .iter()
            .find(|c| c.entry["id"] == json!(ids[4]))
            .expect("branch node");
        let node3 = second
            .children
            .iter()
            .find(|c| c.entry["id"] == json!(ids[2]))
            .expect("main node");
        let observed = json!({
            "children": sorted(&second.children),
            "main_children": sorted(&node3.children),
            "alternate_children": sorted(&node5.children),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn branch_selection_moves_only_the_leaf() {
    let expected = recorded("model_navigation", "branch_selection_moves_only_the_leaf");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &linear_messages());
        let mut manager = history(&h);
        let before = manager
            .get_leaf_entry()
            .map(|entry| position_of(&ids, entry));
        manager.branch(&ids[0]).expect("branch");
        let after = manager
            .get_leaf_entry()
            .map(|entry| position_of(&ids, entry));
        let observed = json!({
            "before": before,
            "after": after,
            "count": manager.get_entries().len(),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn unknown_branch_target_reports_its_identity() {
    let expected = recorded(
        "model_navigation",
        "unknown_branch_target_reports_its_identity",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &[user(&json!("hello"), 1)]);
        let mut manager = history(&h);
        let leaf = manager.get_leaf_id().map(str::to_owned);
        let error = manager.branch("nonexistent").expect_err("unknown entry");
        let observed = json!({
            "error": message(&error),
            "same_leaf": json!(manager.get_leaf_id() == leaf.as_deref()),
            "same_entries": retains(&*manager, &ids),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn append_after_branch_uses_the_selected_parent() {
    let expected = recorded(
        "model_navigation",
        "append_after_branch_uses_the_selected_parent",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(
            &h,
            &[
                user(&json!("1"), 1),
                crate::support::session_harness::assistant("2", 2, &json!({})),
            ],
        );
        let mut manager = history(&h);
        manager.branch(&ids[0]).expect("branch");
        let branched = manager
            .append_message(user(&json!("branched"), 3))
            .expect("append");
        let entry = manager.get_entry(&branched).expect("branched entry");
        assert_eq!(json!({"parent": parent_position(&ids, entry)}), expected);
    });
}

#[test]
fn branch_summary_is_stored_at_target_and_becomes_leaf() {
    let expected = recorded(
        "model_navigation",
        "branch_summary_is_stored_at_target_and_becomes_leaf",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &linear_messages());
        let mut manager = history(&h);
        let summary = manager
            .branch_with_summary(
                Some(&ids[0]),
                "Summary of abandoned work",
                Presence::Missing,
                Presence::Missing,
            )
            .expect("summary");
        let entry = manager.get_entry(&summary).expect("summary entry");
        let observed = json!({
            "leaf_is_summary": json!(manager.get_leaf_id() == Some(summary.as_str())),
            "parent": parent_position(&ids, entry),
            "summary": entry["summary"],
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn unknown_summary_target_reports_its_identity() {
    let expected = recorded(
        "model_navigation",
        "unknown_summary_target_reports_its_identity",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &[user(&json!("hello"), 1)]);
        let mut manager = history(&h);
        let leaf = manager.get_leaf_id().map(str::to_owned);
        let error = manager
            .branch_with_summary(
                Some("nonexistent"),
                "summary",
                Presence::Missing,
                Presence::Missing,
            )
            .expect_err("unknown entry");
        let observed = json!({
            "error": message(&error),
            "same_leaf": json!(manager.get_leaf_id() == leaf.as_deref()),
            "same_entries": retains(&*manager, &ids),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn leaf_read_returns_the_latest_entry() {
    let expected = recorded("model_navigation", "leaf_read_returns_the_latest_entry");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(
            &h,
            &[
                user(&json!("1"), 1),
                crate::support::session_harness::assistant("2", 2, &json!({})),
            ],
        );
        let manager = history_view(&h);
        let leaf = position_of(&ids, manager.get_leaf_entry().expect("leaf"));
        assert_eq!(json!({"leaf": leaf}), expected);
    });
}

/// The recorded content text of a stored message entry: its text blocks, or its plain text.
fn stored_text(entry: &SessionEntry) -> Value {
    match &entry["message"]["content"] {
        Value::String(text) => json!(text),
        blocks => blocks[0]["text"].clone(),
    }
}

#[test]
fn identity_lookup_keeps_message_role_and_content() {
    let expected = recorded(
        "model_navigation",
        "identity_lookup_keeps_message_role_and_content",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(
            &h,
            &[
                user(&json!("first"), 1),
                crate::support::session_harness::assistant("second", 2, &json!({})),
            ],
        );
        let manager = history_view(&h);
        let observed: Vec<Value> = ids
            .iter()
            .map(|id| {
                let entry = manager.get_entry(id).expect("entry");
                json!({
                    "type": entry["type"],
                    "role": entry["message"]["role"],
                    "text": stored_text(entry),
                })
            })
            .collect();
        assert_eq!(json!(observed), expected);
    });
}

#[test]
fn context_projection_excludes_the_abandoned_sibling() {
    let expected = recorded(
        "model_navigation",
        "context_projection_excludes_the_abandoned_sibling",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        append(&h, &[user(&json!("msg1"), 1)]);
        let second = append(
            &h,
            &[crate::support::session_harness::assistant(
                "msg2",
                2,
                &json!({}),
            )],
        );
        append(&h, &[user(&json!("msg3"), 3)]);
        let mut manager = history(&h);
        manager.branch(&second[0]).expect("branch");
        manager
            .append_message(crate::support::session_harness::assistant(
                "msg4-branch",
                4,
                &json!({}),
            ))
            .expect("append");
        let context = manager.build_session_context().expect("context");
        let texts: Vec<Value> = context
            .messages
            .iter()
            .map(crate::support::session_harness::message_text)
            .collect();
        assert_eq!(json!(texts), expected);
    });
}

#[test]
fn unknown_fork_target_is_rejected_without_replacing_history() {
    let expected = recorded(
        "model_navigation",
        "unknown_fork_target_is_rejected_without_replacing_history",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(&h, &[user(&json!("hello"), 1)]);
        let mut manager = history(&h);
        let leaf = manager.get_leaf_id().map(str::to_owned);
        let error = manager
            .create_branched_session("nonexistent")
            .expect_err("unknown entry");
        let observed = json!({
            "error": message(&error),
            "same_leaf": json!(manager.get_leaf_id() == leaf.as_deref()),
            "same_entries": retains(&*manager, &ids),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn memory_fork_keeps_only_the_selected_prefix() {
    let expected = recorded(
        "model_navigation",
        "memory_fork_keeps_only_the_selected_prefix",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let ids = append(
            &h,
            &[
                user(&json!("1"), 1),
                crate::support::session_harness::assistant("2", 2, &json!({})),
                user(&json!("3"), 3),
                crate::support::session_harness::assistant("4", 4, &json!({})),
            ],
        );
        let mut manager = history(&h);
        manager.branch(&ids[2]).expect("branch");
        manager
            .append_message(user(&json!("5"), 5))
            .expect("append");
        let file = manager
            .create_branched_session(&ids[1])
            .expect("branched session");
        let observed = json!({
            "no_file": file.is_none(),
            "entries": positions(&ids, retained(&*manager)),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn fork_extracts_the_selected_alternate_path() {
    let expected = recorded(
        "model_navigation",
        "fork_extracts_the_selected_alternate_path",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let mut ids = append(
            &h,
            &[
                user(&json!("1"), 1),
                crate::support::session_harness::assistant("2", 2, &json!({})),
            ],
        );
        let mut manager = history(&h);
        let three = manager
            .append_message(user(&json!("3"), 3))
            .expect("append");
        ids.push(three);
        manager.branch(&ids[1]).expect("branch");
        let four = manager
            .append_message(user(&json!("4"), 4))
            .expect("append");
        let five = manager
            .append_message(crate::support::session_harness::assistant(
                "5",
                5,
                &json!({}),
            ))
            .expect("append");
        ids.extend([four, five.clone()]);
        manager
            .create_branched_session(&five)
            .expect("branched session");
        assert_eq!(json!(positions(&ids, retained(&*manager))), expected);
    });
}

/// A file-backed session below `root`.
fn file_session(root: &str) -> SessionManager {
    SessionManager::create(
        &format!("{root}/project"),
        Some(&format!("{root}/sessions")),
        root,
    )
    .expect("file session")
}

/// The recorded shape of the JSONL file at `path`: whether it exists, its headers and
/// whether its identities repeat.
fn published(path: &str) -> Value {
    let Ok(content) = std::fs::read_to_string(path) else {
        return json!({"exists": false, "headers": 0, "unique_ids": true});
    };
    let records = parse_session_entries(&content);
    let headers = records
        .iter()
        .filter(|record| record["type"] == "session")
        .count();
    let mut identities: Vec<&str> = records
        .iter()
        .filter(|record| record["type"] != "session")
        .filter_map(|record| record["id"].as_str())
        .collect();
    let total = identities.len();
    identities.sort_unstable();
    identities.dedup();
    json!({
        "exists": std::path::Path::new(path).exists(),
        "headers": headers,
        "unique_ids": identities.len() == total,
    })
}

#[test]
fn first_user_fork_delays_file_and_writes_each_entry_once() {
    let expected = recorded(
        "model_navigation",
        "first_user_fork_delays_file_and_writes_each_entry_once",
    );
    on_task_set(|_| async {
        let root = maestro_test_directory::Scratch::new().expect("scratch");
        let root = root.path().display().to_string();
        let h = harness(HarnessOptions {
            session_manager: Some(file_session(&root)),
            ..HarnessOptions::default()
        })
        .await;
        let mut manager = history(&h);
        let first = manager
            .append_message(user(&json!("first question"), 1))
            .expect("append");
        manager
            .append_message(crate::support::session_harness::assistant(
                "first answer",
                2,
                &json!({}),
            ))
            .expect("append");
        manager
            .append_message(user(&json!("second question"), 3))
            .expect("append");
        manager
            .append_message(crate::support::session_harness::assistant(
                "second answer",
                4,
                &json!({}),
            ))
            .expect("append");
        let file = manager
            .create_branched_session(&first)
            .expect("branched session")
            .expect("file-backed fork");
        let before = published(&file);
        manager
            .append_custom_entry("preset-state", Presence::Present(json!({"name": "plan"})))
            .expect("append");
        manager
            .append_message(crate::support::session_harness::assistant(
                "new answer",
                5,
                &json!({}),
            ))
            .expect("append");
        let after = published(&file);
        let observed = json!({
            "exists_before": before["exists"],
            "exists_after": after["exists"],
            "headers": after["headers"],
            "unique_ids": after["unique_ids"],
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn fork_with_an_assistant_writes_the_file_immediately() {
    let expected = recorded(
        "model_navigation",
        "fork_with_an_assistant_writes_the_file_immediately",
    );
    on_task_set(|_| async {
        let root = maestro_test_directory::Scratch::new().expect("scratch");
        let root = root.path().display().to_string();
        let h = harness(HarnessOptions {
            session_manager: Some(file_session(&root)),
            ..HarnessOptions::default()
        })
        .await;
        let mut manager = history(&h);
        manager
            .append_message(user(&json!("first question"), 1))
            .expect("append");
        let second = manager
            .append_message(crate::support::session_harness::assistant(
                "first answer",
                2,
                &json!({}),
            ))
            .expect("append");
        manager
            .append_message(user(&json!("second question"), 3))
            .expect("append");
        manager
            .append_message(crate::support::session_harness::assistant(
                "second answer",
                4,
                &json!({}),
            ))
            .expect("append");
        let file = manager
            .create_branched_session(&second)
            .expect("branched session")
            .expect("file-backed fork");
        let after = published(&file);
        let observed = json!({
            "exists_before": after["exists"],
            "exists_after": after["exists"],
            "headers": after["headers"],
            "unique_ids": after["unique_ids"],
        });
        assert_eq!(observed, expected);
    });
}
