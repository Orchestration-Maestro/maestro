//! The footer's rendered behaviour against the recorded corpus.
#![cfg(not(target_arch = "wasm32"))]

use crate::footer_support::corpus::{
    self, Input, Lines, Render, Row, assert_frame, plain, rows_of,
};
use crate::footer_support::{Metadata, Roles, World};
use maestro_app::{ReadonlyFooterDataProvider, ReadonlySessionManager, ThinkingLevel};
use maestro_chat::FooterComponent;
use maestro_tui::Component;
use serde_json::{Value, json};
use std::rc::Rc;

/// The recorded role names of the two shipped palettes.
fn roles() -> Roles {
    Roles::new(
        &crate::footer_support::dark_theme(),
        &crate::footer_support::light_theme(),
    )
}

/// The recorded expectations of one behaviour group.
fn expectations(test: &str) -> Vec<(Render, Value)> {
    rows_of(test)
        .into_iter()
        .map(|row| {
            let Input::Render(render) = row.input else {
                panic!("{test} records no render")
            };
            (*render, row.expected)
        })
        .collect()
}

/// Replays one render group, comparing the rows the recorded state produces.
fn replay(test: &str) {
    replay_arranged(test, |_| {});
}

/// Replays one render group after `arrange` reshapes each recorded world.
fn replay_arranged(test: &str, arrange: impl Fn(&World)) {
    let roles = roles();
    for (render, expected) in expectations(test) {
        let world = render.world();
        arrange(&world);
        let footer = world.footer();
        render.apply_auto(&footer);
        match footer.try_render(render.width) {
            Ok(lines) => {
                let recorded: Lines =
                    serde_json::from_value(expected).expect("recorded successful lines");
                corpus::assert_lines(&lines, &render, &recorded, &roles);
            }
            Err(error) => panic!("{test} at width {} failed: {error}", render.width),
        }
    }
}

/// Builds a world whose context usage is the supplied reported total.
fn world_at(cwd: &str, tokens: f64, window: f64) -> World {
    let world = World::new(cwd, None);
    world.set_model("test-model", "test", false, window);
    world.set_context(tokens);
    world
}

/// Builds a world whose context usage is unknown after a compaction.
fn compacted_world(cwd: &str, window: f64) -> World {
    let world = World::new(cwd, None);
    world.set_model("test-model", "test", false, window);
    world.append_message(&corpus::unreported());
    world.append_compaction();
    world
}

#[test]
fn footer_token_units_cover_every_rounding_boundary() {
    for row in rows_of("footer_token_units_cover_every_rounding_boundary") {
        let Input::Tokens { count } = row.input else {
            panic!("a token row")
        };
        let world = World::new("/tmp/project", None);
        world.set_model("test-model", "test", false, count);
        let lines = world.footer().try_render(200).expect("a rendered frame");
        let recorded = row.expected.as_str().expect("a recorded total");
        let shown = plain(&lines[1]);
        assert!(
            shown.starts_with(&format!("0.0%/{recorded} (auto)")),
            "{count} rendered {shown}"
        );
    }
}

#[test]
fn footer_status_text_normalizes_only_the_owned_characters() {
    let roles = roles();
    for row in rows_of("footer_status_text_normalizes_only_the_owned_characters") {
        let Input::Sanitize { value } = row.input else {
            panic!("a sanitize row")
        };
        let world = World::new("/tmp/project", None);
        world.set_model("test-model", "test", false, 200_000.0);
        world
            .metadata
            .set_statuses(&[("key".to_owned(), value.clone())]);
        let lines = world.footer().try_render(300).expect("a rendered frame");
        let recorded = row.expected.as_str().expect("a recorded text");
        assert_eq!(
            roles.label(&lines[2]),
            roles.label(recorded),
            "{value:?} rendered {:?}",
            lines[2]
        );
    }
}

#[test]
fn footer_wide_session_name_stays_within_columns() {
    replay("footer_wide_session_name_stays_within_columns");
}

#[test]
fn footer_wide_model_and_provider_stay_within_columns() {
    replay("footer_wide_model_and_provider_stay_within_columns");
}

#[test]
fn footer_empty_history_uses_the_context_default() {
    replay("footer_empty_history_uses_the_context_default");
}

#[test]
fn footer_usage_comes_from_all_history_not_the_selected_branch() {
    replay_arranged(
        "footer_usage_comes_from_all_history_not_the_selected_branch",
        select_first_answer,
    );
}

/// Selects the first recorded answer, leaving every later record on an abandoned branch.
fn select_first_answer(world: &World) {
    let mut manager = world.session.session_manager().borrow_mut();
    let first = manager
        .get_entries()
        .into_iter()
        .find(|entry| entry["type"] == "message")
        .and_then(|entry| entry["id"].as_str())
        .map(str::to_owned)
        .expect("a recorded answer");
    manager.branch(&first).expect("an indexed answer");
}

/// A saved answer whose usage reports the categories 7/3/2/1 at cost 1, with `total` present
/// or omitted.
fn saved_answer(total: Option<f64>) -> Value {
    let mut usage = json!({
        "input": 7.0,
        "output": 3.0,
        "cacheRead": 2.0,
        "cacheWrite": 1.0,
        "cost": {"input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0, "total": 1.0},
    });
    if let Some(total) = total {
        usage["totalTokens"] = json!(total);
    }
    json!({
        "role": "assistant",
        "content": [],
        "api": "openai-completions",
        "provider": "test",
        "model": "test-model",
        "usage": usage,
        "stopReason": "stop",
        "timestamp": 0.0,
    })
}

#[test]
fn footer_renders_history_without_total_tokens() {
    for (total, context) in [(None, "13.0%/100 (auto)"), (Some(97.0), "97.0%/100 (auto)")] {
        let answer = saved_answer(total);
        let world = World::new("/tmp/project", None);
        world.set_model("test-model", "test", false, 100.0);
        world.append_message(&answer);
        world.push_answer(&answer);
        let lines = world
            .footer()
            .try_render(120)
            .expect("the footer paints a saved history");
        let text = lines
            .iter()
            .map(|line| plain(line))
            .collect::<Vec<_>>()
            .join("\n");
        for part in ["↑7", "↓3", "R2", "W1", "$1.000", context] {
            assert!(text.contains(part), "{part} is missing from {text}");
        }
    }
}

#[test]
fn footer_usage_fields_and_cost_rounding_remain_distinct() {
    replay("footer_usage_fields_and_cost_rounding_remain_distinct");
}

#[test]
fn footer_subscription_cost_marker_is_independent_of_usage() {
    replay("footer_subscription_cost_marker_is_independent_of_usage");
}

#[test]
fn footer_context_colors_use_strict_thresholds() {
    replay("footer_context_colors_use_strict_thresholds");
}

#[test]
fn footer_context_absence_with_zero_window() {
    replay("footer_context_absence_with_zero_window");
}

#[test]
fn footer_home_precedence_and_raw_prefix_are_preserved() {
    replay("footer_home_precedence_and_raw_prefix_are_preserved");
}

#[test]
fn footer_path_branch_and_session_name_have_separate_presence() {
    replay("footer_path_branch_and_session_name_have_separate_presence");
}

#[test]
fn footer_empty_model_id_uses_no_model() {
    replay("footer_empty_model_id_uses_no_model");
}

#[test]
fn footer_thinking_label_depends_on_model_reasoning() {
    replay("footer_thinking_label_depends_on_model_reasoning");
}

#[test]
fn footer_provider_prefix_uses_count_and_exact_fit() {
    replay("footer_provider_prefix_uses_count_and_exact_fit");
}

#[test]
fn footer_layout_clips_left_then_right_without_overflow() {
    replay("footer_layout_clips_left_then_right_without_overflow");
}

#[test]
fn footer_status_order_spacing_and_visible_styles_are_retained() {
    replay("footer_status_order_spacing_and_visible_styles_are_retained");
}

#[test]
fn footer_status_clipping_uses_display_columns() {
    replay("footer_status_clipping_uses_display_columns");
}

#[test]
fn footer_persisted_usage_field_domain() {
    replay("footer_persisted_usage_field_domain");
}

#[test]
fn footer_persisted_cost_field_domain() {
    replay("footer_persisted_cost_field_domain");
}

#[test]
fn footer_missing_usage_reports_a_reached_failure() {
    for (render, expected) in expectations("footer_missing_usage_reports_a_reached_failure") {
        let world = render.world();
        let error = world
            .footer()
            .try_render(render.width)
            .expect_err("a missing usage must fail the paint");
        assert_failure(&expected, error.as_ref());
    }
}

#[test]
fn footer_missing_cost_reports_a_reached_failure() {
    for (render, expected) in expectations("footer_missing_cost_reports_a_reached_failure") {
        let world = render.world();
        let error = world
            .footer()
            .try_render(render.width)
            .expect_err("a missing cost must fail the paint");
        assert_failure(&expected, error.as_ref());
    }
}

#[test]
fn footer_missing_usage_poisoning_is_not_null_zero() {
    replay("footer_missing_usage_poisoning_is_not_null_zero");
}

/// Asserts the recorded failure category of a render that must fail.
fn assert_failure(expected: &Value, error: &(dyn std::error::Error + 'static)) {
    let recorded = expected["error"]["name"]
        .as_str()
        .expect("a recorded failure");
    let reached = error
        .downcast_ref::<maestro_app::DiagnosticErrorInfo>()
        .expect("a recorded-field failure");
    assert_eq!(reached.name.as_deref(), Some(recorded));
}

#[test]
fn footer_setters_and_noop_lifecycle_keep_live_owners() {
    let roles = roles();
    let row = single("footer_setters_and_noop_lifecycle_keep_live_owners");
    let first = world_at("/tmp/project", 24_600.0, 200_000.0);
    first.set_name("first");
    let second = compacted_world("/tmp/project", 200_000.0);
    second.set_name("second");
    let footer = first.footer();
    let before = footer.try_render(100).expect("a rendered frame");
    footer.set_session(Rc::clone(&second.session));
    let replaced = footer.try_render(100).expect("a rendered frame");
    footer.set_auto_compact_enabled(false);
    let manual = footer.try_render(100).expect("a rendered frame");
    footer.invalidate();
    footer.dispose();
    let after = footer.try_render(100).expect("a rendered frame");
    for (name, lines) in [
        ("before", &before),
        ("replaced", &replaced),
        ("manual", &manual),
        ("after", &after),
    ] {
        assert_frame(&roles, lines, &corpus::lines_of(&row.expected, name), name);
    }
    assert_ne!(before, replaced);
    assert!(manual[1].contains("12.3%") || manual[1].contains('?'));
    assert!(!plain(&manual[1]).contains("(auto)"));
    assert_eq!(after, manual);
}

#[test]
fn footer_provider_callbacks_read_live_model_at_each_phase() {
    let roles = roles();
    let phases = rows_of("footer_provider_callbacks_read_live_model_at_each_phase");
    for row in phases {
        let Input::Callback { phase } = row.input else {
            panic!("a callback row")
        };
        let world = world_at("/tmp/project", 200.0, 1_000.0);
        world.set_model("before", "old", false, 1_000.0);
        let session = Rc::clone(&world.session);
        let replace = Rc::new(move || {
            let model: maestro_app::Model = serde_json::from_value(json!({
                "id": "after",
                "name": "after",
                "api": "openai-completions",
                "provider": "new",
                "baseUrl": "http://localhost",
                "reasoning": true,
                "input": ["text"],
                "cost": {"input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0},
                "contextWindow": 999_999.0,
                "maxTokens": 0.0
            }))
            .expect("a controlled descriptor");
            {
                let mut state = session
                    .state()
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.model = std::sync::Arc::new(std::sync::RwLock::new(model));
                state.thinking_level = ThinkingLevel::High;
            }
        });
        if phase == "branch" {
            world.metadata.set_count(Rc::new(|| 2.0));
            let replace = Rc::clone(&replace);
            world.metadata.set_branch(Rc::new(move || {
                replace();
                Some("changed".to_owned())
            }));
        } else {
            let replace = Rc::clone(&replace);
            world.metadata.set_count(Rc::new(move || {
                replace();
                2.0
            }));
        }
        let lines = world.footer().try_render(100).expect("a rendered frame");
        assert_frame(
            &roles,
            &lines,
            &corpus::lines_of(&row.expected, "lines"),
            &phase,
        );
    }
}

#[test]
fn footer_theme_is_sampled_at_each_paint_boundary() {
    let roles = roles();
    let row = single("footer_theme_is_sampled_at_each_paint_boundary");
    let world = world_at("/tmp/project", 182_000.0, 200_000.0);
    let publication = Rc::clone(&world.publication);
    world.metadata.set_count(Rc::new(move || {
        publication
            .set_theme_instance(crate::footer_support::light_theme())
            .expect("a published theme");
        2.0
    }));
    let lines = world.footer().try_render(100).expect("a rendered frame");
    assert_frame(
        &roles,
        &lines,
        &corpus::lines_of(&row.expected, "lines"),
        "theme",
    );
}

#[test]
fn footer_live_metadata_is_resampled_after_disposal() {
    let roles = roles();
    let row = single("footer_live_metadata_is_resampled_after_disposal");
    let world = world_at("/tmp/project", 24_600.0, 200_000.0);
    world
        .metadata
        .set_statuses(&[("b".to_owned(), "old".to_owned())]);
    let footer = world.footer();
    let first = footer.try_render(100).expect("a rendered frame");
    world
        .metadata
        .set_statuses(&[("a".to_owned(), "new".to_owned())]);
    world
        .metadata
        .set_branch(Rc::new(|| Some("next".to_owned())));
    footer.invalidate();
    footer.dispose();
    let second = footer.try_render(100).expect("a rendered frame");
    for (name, lines) in [("first", &first), ("second", &second)] {
        assert_frame(&roles, lines, &corpus::lines_of(&row.expected, name), name);
    }
    assert_ne!(first, second);
}

#[test]
fn footer_uninitialized_theme_fails_at_first_paint() {
    let row = single("footer_uninitialized_theme_fails_at_first_paint");
    let world = crate::footer_support::unpublished("/tmp/project");
    let error = world
        .footer()
        .try_render(80)
        .expect_err("an unpublished theme must fail the paint");
    let recorded = row.expected["error"]["message"]
        .as_str()
        .expect("a recorded message");
    assert_eq!(error.to_string(), recorded);
}

#[test]
fn footer_public_type_uses_the_default_execution() {
    let world = World::new("/tmp/project", None);
    world.set_model("test-model", "test", false, 200_000.0);
    let footer: FooterComponent = world.footer();
    assert_eq!(footer.render(80).len(), 2, "a path row and a stats row");
}

/// One metadata read per frame, counted at each metadata getter.
#[test]
fn footer_reads_metadata_once_per_frame() {
    for count in [0_usize, 1, 4] {
        let (world, pairs) = world_with_items(count);
        let branch = crate::footer_support::counter();
        let statuses = crate::footer_support::counter();
        let providers = crate::footer_support::counter();
        let branch_calls = Rc::clone(&branch);
        world.metadata.set_branch(Rc::new(move || {
            branch_calls.set(branch_calls.get() + 1);
            Some("main".to_owned())
        }));
        let provider_calls = Rc::clone(&providers);
        world.metadata.set_count(Rc::new(move || {
            provider_calls.set(provider_calls.get() + 1);
            1.0
        }));
        let counted = CountingMetadata {
            inner: Rc::clone(&world.metadata),
            calls: Rc::clone(&statuses),
        };
        let footer = FooterComponent::new(
            Rc::clone(&world.session),
            Rc::new(counted) as Rc<dyn ReadonlyFooterDataProvider>,
            world.theme.clone(),
            Rc::new(
                maestro_theme::Collator::new(&|_| Some("en_US.UTF-8".to_owned()))
                    .expect("the collator"),
            ),
            Rc::new(|_| None),
        );
        let lines = footer.try_render(200).expect("a rendered frame");
        assert_eq!(statuses.get(), 1, "one status read for {count} items");
        assert_eq!(branch.get(), 1, "one branch read for {count} items");
        assert_eq!(providers.get(), 1, "one provider read for {count} items");
        if count > 0 {
            let stats = plain(&lines[1]);
            assert!(
                stats.starts_with(&format!("↑{count}.0k ")),
                "{stats} totals {count} answers"
            );
            let status_row = plain(&lines[2]);
            for (_, value) in &pairs {
                assert!(
                    status_row.contains(value.as_str()),
                    "{status_row} holds {value}"
                );
            }
        }
    }
}

/// A world whose history holds `count` answers and whose metadata holds `count` statuses.
fn world_with_items(count: usize) -> (World, Vec<(String, String)>) {
    let world = World::new("/tmp/project", None);
    world.set_model("test-model", "test", false, 200_000.0);
    for _ in 0..count {
        world.append_message(&json!({
            "role": "assistant",
            "usage": {
                "input": 1000, "output": 0, "cacheRead": 0, "cacheWrite": 0,
                "cost": {"total": 0}
            }
        }));
    }
    let pairs: Vec<(String, String)> = (0..count)
        .map(|index| (format!("key{index}"), format!("value{index}")))
        .collect();
    world.metadata.set_statuses(&pairs);
    (world, pairs)
}

/// Metadata that counts how often its statuses were walked.
struct CountingMetadata {
    /// The metadata answering the reads.
    inner: Rc<Metadata>,
    /// How often the statuses were read.
    calls: Rc<std::cell::Cell<usize>>,
}

impl ReadonlyFooterDataProvider for CountingMetadata {
    fn get_git_branch(&self) -> Option<String> {
        self.inner.get_git_branch()
    }

    fn get_extension_statuses(&self) -> maestro_app::ExtensionStatuses {
        self.calls.set(self.calls.get() + 1);
        self.inner.get_extension_statuses()
    }

    fn get_available_provider_count(&self) -> f64 {
        self.inner.get_available_provider_count()
    }

    fn on_branch_change(&self, callback: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        self.inner.on_branch_change(callback)
    }
}

/// The only row of a recorded group.
fn single(test: &str) -> Row {
    let mut rows = rows_of(test);
    assert_eq!(rows.len(), 1, "{test} records one row");
    rows.remove(0)
}
