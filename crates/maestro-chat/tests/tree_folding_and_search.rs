//! Retained session-tree search and folding behavior.
#![cfg(test)]
use maestro_app::SessionTreeNode as InputNode;
use maestro_chat::components::tree_selector::TreeList;
use serde_json::json;

mod tree_folding_support;

/// Replays the complete recorded fold then unfold and jump observations.
#[test]
fn fold_then_unfold_and_jump() {
    tree_folding_support::group("fold_then_unfold_and_jump");
}

/// Replays the complete recorded alt navigation folds like control observations.
#[test]
fn alt_navigation_folds_like_control() {
    tree_folding_support::group("alt_navigation_folds_like_control");
}

/// Replays the complete recorded root unfold keeps nested folds observations.
#[test]
fn root_unfold_keeps_nested_folds() {
    tree_folding_support::group("root_unfold_keeps_nested_folds");
}

/// Replays the complete recorded inactive segments fold independently observations.
#[test]
fn inactive_segments_fold_independently() {
    tree_folding_support::group("inactive_segments_fold_independently");
}

/// Replays the complete recorded forest roots fold independently observations.
#[test]
fn forest_roots_fold_independently() {
    tree_folding_support::group("forest_roots_fold_independently");
}

/// Replays the complete recorded fold hides descendants through hidden nodes observations.
#[test]
fn fold_hides_descendants_through_hidden_nodes() {
    tree_folding_support::group("fold_hides_descendants_through_hidden_nodes");
}

/// Replays the complete recorded typing and cancel reset folds observations.
#[test]
fn typing_and_cancel_reset_folds() {
    tree_folding_support::group("typing_and_cancel_reset_folds");
}

/// Replays the complete recorded filter actions clear fold state observations.
#[test]
fn filter_actions_clear_fold_state() {
    tree_folding_support::group("filter_actions_clear_fold_state");
}

/// Replays the complete recorded branch segments skip chains and stop at leaf observations.
#[test]
fn branch_segments_skip_chains_and_stop_at_leaf() {
    tree_folding_support::group("branch_segments_skip_chains_and_stop_at_leaf");
}

/// Replays the complete recorded search fields cover each entry kind observations.
#[test]
fn search_fields_cover_each_entry_kind() {
    tree_folding_support::group("search_fields_cover_each_entry_kind");
}

/// Replays the complete recorded search requires all literal lowercase tokens observations.
#[test]
fn search_requires_all_literal_lowercase_tokens() {
    tree_folding_support::group("search_requires_all_literal_lowercase_tokens");
}

/// Replays the complete recorded search uses shared whitespace and unicode lowercase observations.
#[test]
fn search_uses_shared_whitespace_and_unicode_lowercase() {
    tree_folding_support::group("search_uses_shared_whitespace_and_unicode_lowercase");
}

/// Replays the complete recorded search preview limits blocks but not custom strings observations.
#[test]
fn search_preview_limits_blocks_but_not_custom_strings() {
    tree_folding_support::group("search_preview_limits_blocks_but_not_custom_strings");
}

/// Replays the complete recorded search and category filters intersect observations.
#[test]
fn search_and_category_filters_intersect() {
    tree_folding_support::group("search_and_category_filters_intersect");
}

/// Replays the complete recorded search preserves selection and empty bookmarks observations.
#[test]
fn search_preserves_selection_and_empty_bookmarks() {
    tree_folding_support::group("search_preserves_selection_and_empty_bookmarks");
}

/// Replays the complete recorded search input ignores control chunks observations.
#[test]
fn search_input_ignores_control_chunks() {
    tree_folding_support::group("search_input_ignores_control_chunks");
}

/// Replays the complete recorded backspace removes last scalar and resets folds observations.
#[test]
fn backspace_removes_last_scalar_and_resets_folds() {
    tree_folding_support::group("backspace_removes_last_scalar_and_resets_folds");
}

/// Replays the complete recorded cancel clears query before notifying observations.
#[test]
fn cancel_clears_query_before_notifying() {
    tree_folding_support::group("cancel_clears_query_before_notifying");
}

/// Replays the complete recorded bound keys keep action priority observations.
#[test]
fn bound_keys_keep_action_priority() {
    tree_folding_support::group("bound_keys_keep_action_priority");
}

/// Replays the complete recorded registry updates are used for new input observations.
#[test]
fn registry_updates_are_used_for_new_input() {
    tree_folding_support::group("registry_updates_are_used_for_new_input");
}

/// Replays the complete recorded confirm reports selected node from filtered view observations.
#[test]
fn confirm_reports_selected_node_from_filtered_view() {
    tree_folding_support::group("confirm_reports_selected_node_from_filtered_view");
}

/// Replays the complete recorded fold markers follow connector and root roles observations.
#[test]
fn fold_markers_follow_connector_and_root_roles() {
    tree_folding_support::group("fold_markers_follow_connector_and_root_roles");
}

/// Replays the complete recorded retained occurrences keep duplicate ids distinct observations.
#[test]
fn retained_occurrences_keep_duplicate_ids_distinct() {
    tree_folding_support::group("retained_occurrences_keep_duplicate_ids_distinct");
}

/// Replays the complete recorded search optional fields keep their own defaults observations.
#[test]
fn search_optional_fields_keep_their_own_defaults() {
    tree_folding_support::group("search_optional_fields_keep_their_own_defaults");
}

/// Replays the complete recorded printable chunks extend query at control boundaries observations.
#[test]
fn printable_chunks_extend_query_at_control_boundaries() {
    tree_folding_support::group("printable_chunks_extend_query_at_control_boundaries");
}

/// Replays the complete recorded label action reports raw selected label without searching observations.
#[test]
fn label_action_reports_raw_selected_label_without_searching() {
    tree_folding_support::group("label_action_reports_raw_selected_label_without_searching");
}

/// Replays the complete recorded owned projection keeps raw label presence observations.
#[test]
fn owned_projection_keeps_raw_label_presence() {
    tree_folding_support::group("owned_projection_keeps_raw_label_presence");
}

/// Selected records and ordered children survive the list and original containers.
#[test]
fn retained_node_handles_outlive_the_list() {
    let expected = tree_folding_support::retention().surviving;
    let selected = {
        let record = expected.entry.clone();
        let child = expected.children[0].clone();
        let label = json!(null);
        let tree = vec![InputNode {
            entry: &record,
            children: vec![InputNode {
                entry: &child,
                children: vec![],
                label: None,
                label_timestamp: None,
            }],
            label: Some(&label),
            label_timestamp: None,
        }];
        let list = TreeList::new(&tree, None, 30, Some("a"), None).unwrap();
        list.get_selected_node().unwrap()
    };
    assert_eq!(**selected.entry(), expected.entry);
    assert_eq!(
        selected
            .children()
            .map(|n| (**n.entry()).clone())
            .collect::<Vec<_>>(),
        expected.children
    );
    assert_eq!(*selected.label(), Some(expected.label));
    assert_eq!(
        selected.entry()["extension"]["ordered"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
}
/// Aliases share metadata and occurrence identity without merging record aliases.
#[test]
fn retained_node_handles_share_occurrence_identity() {
    use std::rc::Rc;
    let expected = tree_folding_support::retention();
    let metadata = json!(null);
    let child = &expected.surviving.children[0];
    let tree = tree_folding_support::retention_tree(&expected.surviving, Some(&metadata));
    let list = TreeList::new(&tree, None, 30, Some("a"), None).unwrap();
    let node = list.get_selected_node().unwrap();
    let alias = node.clone();
    let observed = &expected.identity;
    assert_eq!(json!(node == alias), observed.same_node);
    assert_eq!(
        json!(Rc::ptr_eq(node.entry(), alias.entry())),
        observed.same_entry
    );
    assert_eq!(
        node.children()
            .map(|n| n.entry()["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        observed.child_ids
    );
    assert_eq!(json!(node.label().is_some()), observed.label_present);
    assert_eq!(node.label().as_ref(), Some(&observed.label));
    *alias.label_mut() = Some(json!("edited"));
    assert_eq!(
        node.label().as_ref(),
        Some(&observed.label_after_alias_edit)
    );
    assert_eq!(node.label_timestamp().as_ref(), Some(&metadata));
    *alias.label_timestamp_mut() = Some(json!("changed"));
    assert_eq!(*node.label_timestamp(), Some(json!("changed")));
    let distinct = vec![
        InputNode {
            entry: child,
            children: vec![],
            label: None,
            label_timestamp: None,
        },
        InputNode {
            entry: child,
            children: vec![],
            label: Some(&observed.label),
            label_timestamp: None,
        },
    ];
    let list = TreeList::new(&distinct, None, 30, None, None).unwrap();
    let first = list.get_selected_node().unwrap();
    let (_guard, _restore) = tree_folding_support::registry();
    list.handle_input("\u{1b}[B").unwrap();
    let second = list.get_selected_node().unwrap();
    assert_eq!(
        json!(Rc::ptr_eq(first.entry(), second.entry())),
        observed.entry_alias_shared
    );
    assert_eq!(json!(first != second), observed.entry_alias_distinct_nodes);
    *first.label_mut() = Some(json!("independent"));
    assert_eq!(
        json!(second.label().is_none()),
        observed.entry_alias_labels_independent
    );
}
/// Confirmation releases its borrow before reentry and callback replacement.
#[test]
fn callbacks_can_reenter_and_replace_themselves() {
    use std::{cell::RefCell, rc::Rc};
    let (_guard, _restore) = tree_folding_support::registry();
    let fixture = tree_folding_support::retention();
    let tree = tree_folding_support::retention_tree(&fixture.surviving, None);
    let list = Rc::new(TreeList::new(&tree, None, 30, Some("a"), None).unwrap());
    let weak = Rc::downgrade(&list);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&observed);
    list.set_on_select(Some(Rc::new(move |id| {
        let list = weak.upgrade().unwrap();
        let before = list.get_selected_node().unwrap().entry()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        log.borrow_mut()
            .push(vec!["before".to_owned(), id.to_owned(), before]);
        list.handle_input("\u{1b}[B").unwrap();
        log.borrow_mut().push(vec![
            "after".to_owned(),
            list.get_selected_node().unwrap().entry()["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        ]);
        let capture = Rc::clone(&log);
        list.set_on_select(Some(Rc::new(move |id| {
            capture
                .borrow_mut()
                .push(vec!["replacement".to_owned(), id.to_owned()]);
        })));
    })));
    assert!(list.on_select().is_some());
    list.handle_input("\r").unwrap();
    list.handle_input("\r").unwrap();
    assert_eq!(*observed.borrow(), fixture.transcript);
    list.set_on_select(None);
    assert!(list.on_select().is_none());
    drop(list);
    assert_eq!(Rc::strong_count(&observed), 1);
}
/// Definitions retain both ordered fold aliases and the label action shape.
#[test]
fn fold_key_defaults_keep_both_aliases() {
    tree_folding_support::definitions();
}

/// A stored self-parent cannot make an upward segment walk loop.
#[test]
fn upward_segment_cycle_keeps_selection() {
    segment_cycle("\u{1b}[1;5D");
}
/// A stored self-child projection cannot make a downward segment walk loop.
#[test]
fn downward_segment_cycle_keeps_selection() {
    segment_cycle("\u{1b}[1;5C");
}
/// Exercises an admitted stored self-parent through public input.
fn segment_cycle(key: &str) {
    let (_guard, _restore) = tree_folding_support::registry();
    let entry = json!({"id":"self","parentId":"self","type":"message","message":{"role":"user","content":"text"}});
    let tree = vec![InputNode {
        entry: &entry,
        children: vec![],
        label: None,
        label_timestamp: None,
    }];
    let list = TreeList::new(&tree, None, 30, None, None).unwrap();
    let selected = list.get_selected_node().unwrap();
    list.handle_input(key).unwrap();
    assert!(list.get_selected_node().unwrap() == selected);
}
