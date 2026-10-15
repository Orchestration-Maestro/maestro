//! Controlled effects and exact complete-frame assertions.
use super::*;
use maestro_tui::{
    KeybindingKeys, KeybindingsConfig, KeybindingsManager, get_keybindings, set_keybindings,
    tui::Focusable,
};
use serde_json::json;
use std::time::{Duration, SystemTime};

/// Converts authored instant text without sampling the clock.
fn instant(text: &str) -> SystemTime {
    let millis = maestro_app::parse_timestamp_millis(text).unwrap();
    if millis >= 0 {
        SystemTime::UNIX_EPOCH + Duration::from_millis(millis.unsigned_abs())
    } else {
        SystemTime::UNIX_EPOCH - Duration::from_millis(millis.unsigned_abs())
    }
}
/// Retains scalar and disabled-list overrides.
fn bindings(input: &std::collections::BTreeMap<String, Binding>) -> KeybindingsConfig {
    input
        .iter()
        .map(|(id, key)| {
            (
                id.clone(),
                Some(match key {
                    Binding::Single(s) => KeybindingKeys::Single(s.clone()),
                    Binding::Multiple(s) => KeybindingKeys::Multiple(s.clone()),
                }),
            )
        })
        .collect()
}
/// Maps recorded category names to their public variants.
fn mode(name: &str) -> maestro_chat::FilterMode {
    use maestro_chat::FilterMode::*;
    match name {
        "default" => Default,
        "no-tools" => NoTools,
        "user-only" => UserOnly,
        "labeled-only" => LabeledOnly,
        "all" => All,
        _ => panic!("unknown filter"),
    }
}
/// Retains missing, null and empty metadata independently.
fn metadata(panel: &TreeSelectorComponent) -> Value {
    let mut result = serde_json::Map::new();
    if let Some(node) = panel.get_tree_list().get_selected_node() {
        if let Some(label) = node.label().as_ref() {
            result.insert("label".into(), label.clone());
        }
        if let Some(timestamp) = node.label_timestamp().as_ref() {
            result.insert("timestamp".into(), timestamp.clone());
        }
    }
    Value::Object(result)
}
/// Installs one clock shared by row rendering and updates.
fn clock(effects: &Rc<Effects>) -> Rc<dyn Fn() -> SystemTime> {
    let effects = effects.clone();
    Rc::new(move || {
        effects.reads.set(effects.reads.get() + 1);
        let now = effects.instant.get();
        if effects.reenter.replace(false) {
            let panel = effects.panel.borrow().upgrade().unwrap();
            effects
                .events
                .borrow_mut()
                .push(json!({"kind":"clock_update", "observed": metadata(&panel)}));
            panel.get_tree_list().update_node_label(
                "a",
                Some("changed"),
                Some("2026-10-14T09:09:00Z"),
            );
        }
        now
    })
}
/// Records callbacks before closure, with optional caller reentry or failure.
fn callbacks(effects: &Rc<Effects>, policy: &str) -> TreeSelectorCallbacks {
    let select = effects.clone();
    let cancel = effects.clone();
    let label = effects.clone();
    let policy = policy.to_owned();
    TreeSelectorCallbacks {
        on_select: Rc::new(move |id| select.events.borrow_mut().push(json!({"kind":"select", "id":id}))),
        on_cancel: Rc::new(move || cancel.events.borrow_mut().push(json!({"kind":"cancel"}))),
        on_label_change: (policy != "none").then(|| Rc::new(move |id: &str, value: Option<&str>| {
            let panel = label.panel.borrow().upgrade().unwrap();
            let mut event = json!({"kind":"label", "id":id, "observed":{"lines":panel.render(200),"label":metadata(&panel).get("label").cloned().unwrap_or(Value::Null)}});
            if let Some(value) = value { event["label"] = json!(value); }
            label.events.borrow_mut().push(event);
            if policy == "reenter" { panel.handle_input("Z").unwrap(); }
            assert!(policy != "throws", "fixture callback failure");
        }) as Rc<dyn Fn(&str, Option<&str>)>),
    }
}
/// Observes state before rendering and effects after it, preserving oracle phases.
fn frame(panel: &TreeSelectorComponent, case: &Case, record: &Frame, context: &Context) {
    let list = panel.get_tree_list();
    assert!(Rc::ptr_eq(list, &context.original_list));
    assert_eq!(
        list.get_selected_node()
            .and_then(|n| n.entry()["id"].as_str().map(str::to_owned)),
        record.selected
    );
    assert_eq!(list.get_search_query(), record.query);
    assert_eq!(metadata(panel), record.metadata, "{}", case.name);
    let children = panel
        .children()
        .borrow()
        .iter()
        .map(|c| {
            let any = c.as_ref() as &dyn std::any::Any;
            any.downcast_ref::<maestro_tui::Container>()
                .map(|c| c.children().borrow().len())
        })
        .collect::<Vec<_>>();
    assert_eq!(children, record.children);
    assert_eq!(record.rendered.len(), case.input.widths.len());
    for (rendered, width) in record.rendered.iter().zip(&case.input.widths) {
        assert_eq!(rendered.width, *width);
        assert_eq!(panel.render(*width), rendered.panel, "{}", case.name);
        assert_eq!(
            list.render(*width, &context.theme, &context.host, case.input.styles)
                .unwrap(),
            rendered.list,
            "{}",
            case.name
        );
    }
    assert_eq!(*context.effects.events.borrow(), record.events);
    assert_eq!(context.effects.reads.get(), record.clock_reads);
    assert_eq!(context.host.pending(), record.pending);
}
/// Runtime and publication handles spanning each complete transaction.
struct Context {
    /// Original list retained across every replay operation.
    original_list: Rc<maestro_chat::TreeList>,
    host: manual_runtime::ManualRuntime,
    theme: maestro_theme::LiveTheme,
    publication: maestro_theme::ThemeState,
    effects: Rc<Effects>,
}
/// Applies one public operation with no test-owned scheduling machinery.
fn operation(panel: &TreeSelectorComponent, op: &Operation, context: &Context) {
    match op {
        Operation::Key(data) => panel.handle_input(data).unwrap(),
        Operation::Change(c) => {
            if let Some(b) = &c.set_bindings {
                get_keybindings().set_user_bindings(bindings(b));
            }
            if let Some(focus) = c.focus {
                panel.focus_flag().set(focus);
            }
            if let Some(palette) = &c.palette {
                context
                    .publication
                    .set_theme_instance(self::palette::palette(palette))
                    .unwrap();
            }
            if let Some(now) = &c.now {
                context.effects.instant.set(instant(now));
            }
            if let Some(u) = &c.update {
                panel.get_tree_list().update_node_label(
                    &u.id,
                    u.label.as_deref(),
                    u.timestamp.as_deref(),
                );
            }
            if let Some(data) = &c.list_key {
                panel.get_tree_list().handle_input(data).unwrap();
            }
            if let Some(ms) = c.advance {
                for result in context.host.advance_to(Duration::from_millis(ms)) {
                    result.unwrap();
                }
            }
            if let Some(clear) = c.clear_outer {
                assert!(clear);
                panel.clear();
            }
            if let Some(invalidate) = c.invalidate {
                assert!(invalidate);
                panel.invalidate();
            }
        }
    }
}
/// Replays construction and every ordered input in one controlled world.
pub(super) fn case(case: &Case) {
    let input = &case.input;
    set_keybindings(KeybindingsManager::new(
        maestro_app::presentation_data::keybindings::KEYBINDINGS.clone(),
        bindings(&input.bindings),
    ));
    let host = manual_runtime::ManualRuntime::new();
    let (publication, theme) = published::published(&palette::palette(&input.palette));
    let effects = Rc::new(Effects {
        panel: RefCell::default(),
        events: RefCell::default(),
        reads: Cell::new(0),
        instant: Cell::new(instant(&input.now)),
        reenter: Cell::new(input.clock_reenter),
    });
    let panel = Rc::new(
        TreeSelectorComponent::new(
            &tree::tree(&input.nodes, &input.roots),
            callbacks(&effects, &input.callback),
            TreeSelectorOptions {
                current_leaf_id: input.leaf.clone(),
                terminal_height: input.height,
                initial_selected_id: input.initial.clone(),
                initial_filter_mode: Some(mode(&input.mode)),
                theme: theme.clone(),
                runtime: host.handle(),
                styles_enabled: input.styles,
                now: Some(clock(&effects)),
            },
        )
        .unwrap(),
    );
    *effects.panel.borrow_mut() = Rc::downgrade(&panel);
    let context = Context {
        original_list: panel.get_tree_list().clone(),
        host,
        theme,
        publication,
        effects,
    };
    operations(&panel, case, &context);
}
/// Drives the exact operation transcript and its failure boundary.
fn operations(panel: &TreeSelectorComponent, case: &Case, context: &Context) {
    let input = &case.input;
    frame(panel, case, &case.expected.frames[0], context);
    let mut failure = None;
    for (step, op) in input.ops.iter().enumerate() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            operation(panel, op, context);
        }));
        frame(panel, case, &case.expected.frames[step + 1], context);
        if result.is_err() {
            failure = Some(step);
            break;
        }
    }
    assert_eq!(
        failure,
        case.expected.error.as_ref().map(|e| {
            assert_eq!(e.kind, "callback");
            e.step
        })
    );
    assert_eq!(
        case.expected.frames.len(),
        failure.map_or(input.ops.len() + 1, |s| s + 2)
    );
}
