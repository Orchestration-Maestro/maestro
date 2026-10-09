//! Retained state of the box and text widgets: caches, setters, child arrays and callbacks
//! that change a widget while it renders.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use maestro_tui::tui::ComponentHandle;
use maestro_tui::{Box, Component, Spacer, Text};

mod fixtures {
    pub mod widget_probes;
}
use fixtures::widget_probes::{Calls, background, live_background, tinted};

#[test]
fn text_cache_changes_only_after_width_setters_or_invalidation() {
    let calls = Calls::default();
    let text = Text::new("X".into(), 1, 1, Some(background("red:", 41, &calls)));
    let rows = |width: usize, color: u8| {
        let blank = " ".repeat(width);
        let content = format!(" X{}", " ".repeat(width - 2));
        tinted(color, &[&blank, &content, &blank])
    };

    assert_eq!(text.render(6), rows(6, 41));
    assert_eq!(calls.borrow().len(), 2, "content row and one padding row");
    assert_eq!(text.render(6), rows(6, 41));
    assert_eq!(
        calls.borrow().len(),
        2,
        "same text and width reuse the rows"
    );

    assert_eq!(text.render(7), rows(7, 41));
    assert_eq!(calls.borrow().len(), 4, "a new width renders again");

    text.set_text("X".into());
    assert_eq!(text.render(7), rows(7, 41));
    assert_eq!(calls.borrow().len(), 6, "setting equal text renders again");

    text.set_custom_bg_fn(Some(background("blue:", 44, &calls)));
    assert_eq!(text.render(7), rows(7, 44));
    assert_eq!(calls.borrow()[6..], ["blue: X     ", "blue:       "]);

    text.set_custom_bg_fn(None);
    assert_eq!(text.render(7), ["       ", " X     ", "       "]);
    assert_eq!(calls.borrow().len(), 8, "no background means no calls");

    text.set_custom_bg_fn(Some(background("red:", 41, &calls)));
    text.render(7);
    text.render(7);
    assert_eq!(calls.borrow().len(), 10);
    text.invalidate();
    text.render(7);
    assert_eq!(calls.borrow().len(), 12, "invalidate drops the cached rows");
}

#[test]
fn text_style_state_requires_explicit_invalidation() {
    let color = Rc::new(Cell::new(41));
    let styled = live_background("", &color, &Calls::default());
    let text = Text::new("X".into(), 0, 0, Some(styled));

    assert_eq!(text.render(5), tinted(41, &["X    "]));
    color.set(44);
    assert_eq!(
        text.render(5),
        tinted(41, &["X    "]),
        "captured style state is not sampled"
    );
    text.invalidate();
    assert_eq!(text.render(5), tinted(44, &["X    "]));
}

#[test]
fn text_callback_mutation_does_not_cache_old_output_for_new_text() {
    let text = Rc::new(Text::new("old".into(), 0, 0, None));
    let replace = {
        let text = Rc::downgrade(&text);
        Rc::new(move |row: &str| {
            if let Some(text) = text.upgrade() {
                text.set_text("new".into());
            }
            row.to_owned()
        })
    };
    text.set_custom_bg_fn(Some(replace));
    assert_eq!(
        text.render(5),
        ["old  "],
        "this render keeps the text it captured"
    );
    assert_eq!(
        text.render(5),
        ["new  "],
        "the next render shows the replacement"
    );

    let color = Rc::new(Cell::new(41));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let restyle = {
        let (text, color, calls) = (Rc::downgrade(&text), Rc::clone(&color), Rc::clone(&calls));
        Rc::new(move |row: &str| {
            let used = color.replace(44);
            calls.borrow_mut().push(used);
            if let Some(text) = text.upgrade() {
                text.invalidate();
            }
            format!("\x1b[{used}m{row}\x1b[49m")
        })
    };
    text.set_custom_bg_fn(Some(restyle));
    assert_eq!(text.render(5), tinted(41, &["new  "]));
    assert_eq!(text.render(5), tinted(44, &["new  "]));
    assert_eq!(*calls.borrow(), [41, 44]);
}

#[test]
fn text_callback_style_replacement_applies_to_later_rows() {
    let calls = Calls::default();
    let text = Rc::new(Text::new("A\nB".into(), 0, 1, None));
    let first = {
        let (text, record) = (Rc::downgrade(&text), background("first:", 41, &calls));
        let second = background("second:", 44, &calls);
        Rc::new(move |row: &str| {
            if let Some(text) = text.upgrade() {
                text.set_custom_bg_fn(Some(Rc::clone(&second)));
            }
            record(row)
        })
    };
    text.set_custom_bg_fn(Some(first));

    let blue = tinted(44, &["     "]);
    let expected = [
        &blue[..],
        &tinted(41, &["A    "]),
        &tinted(44, &["B    "]),
        &blue[..],
    ];
    assert_eq!(text.render(5), expected.concat());
    assert_eq!(
        *calls.borrow(),
        ["first:A    ", "second:B    ", "second:     "]
    );

    assert_eq!(
        text.render(5),
        tinted(44, &["     ", "A    ", "B    ", "     "])
    );
    assert_eq!(
        calls.borrow()[3..],
        ["second:A    ", "second:B    ", "second:     "]
    );
}

/// A child that returns rows it can be told to change, notes every render and invalidation in
/// a shared trace, and runs one action the first time it is visited.
struct Probe {
    /// Name written to the trace.
    name: &'static str,
    /// Rows every render returns.
    rows: RefCell<Vec<&'static str>>,
    /// Trace shared with the other probes and background functions of a test.
    trace: Calls,
    /// Action run, once, at the next render or invalidation.
    action: RefCell<Option<std::boxed::Box<dyn FnOnce()>>>,
}

impl Probe {
    /// A probe that renders its own name.
    fn new(name: &'static str, trace: &Calls) -> Rc<Self> {
        Rc::new(Self {
            name,
            rows: RefCell::new(vec![name]),
            trace: Rc::clone(trace),
            action: RefCell::default(),
        })
    }

    /// Runs `action` the next time the probe is rendered or invalidated.
    fn on_first_visit(&self, action: impl FnOnce() + 'static) {
        *self.action.borrow_mut() = Some(std::boxed::Box::new(action));
    }

    /// Adds `event` to the trace, then runs the pending action, if any.
    fn visit(&self, event: String) {
        self.trace.borrow_mut().push(event);
        let action = self.action.take();
        if let Some(action) = action {
            action();
        }
    }
}

impl Component for Probe {
    fn render(&self, width: usize) -> Vec<String> {
        self.visit(format!("render {}:{width}", self.name));
        self.rows
            .borrow()
            .iter()
            .map(|row| (*row).to_owned())
            .collect()
    }

    fn invalidate(&self) {
        self.visit(format!("invalidate {}", self.name));
    }
}

/// An unpadded text widget showing `content`.
fn text(content: &str) -> Rc<Text> {
    Rc::new(Text::new(content.into(), 0, 0, None))
}

#[test]
fn box_retains_order_duplicates_and_child_updates() {
    let (a, b) = (text("A"), text("B"));
    let first: ComponentHandle = a.clone();
    let container = Box::default();
    container.add_child(a.clone());
    container.add_child(b);
    container.add_child(a.clone());
    let blank = "      ";
    assert_eq!(
        container.render(6),
        [blank, " A    ", " B    ", " A    ", blank]
    );

    a.set_text("Z".into());
    assert_eq!(
        container.render(6),
        [blank, " Z    ", " B    ", " Z    ", blank]
    );

    container.remove_child(&first);
    assert_eq!(container.render(6), [blank, " B    ", " Z    ", blank]);
    let stranger: ComponentHandle = Rc::new(Spacer::default());
    container.remove_child(&stranger);
    assert_eq!(container.render(6), [blank, " B    ", " Z    ", blank]);
}

#[test]
fn box_child_array_aliases_survive_clear_and_assignment() {
    let (a, b) = (text("A"), text("B"));
    let container = Box::new(0, 0, None);
    let kept = container.children();
    kept.borrow_mut().extend([a.clone() as ComponentHandle, b]);
    assert_eq!(container.render(4), ["A   ", "B   "]);

    kept.borrow_mut().remove(0);
    assert_eq!(container.render(4), ["B   "]);

    container.clear();
    assert!(container.render(4).is_empty());
    assert_eq!(
        kept.borrow().len(),
        1,
        "clear leaves the old array to its holders"
    );
    assert!(!Rc::ptr_eq(&container.children(), &kept));

    kept.borrow_mut().push(a);
    assert!(
        container.render(4).is_empty(),
        "the old array is no longer rendered"
    );
    container.set_children(Rc::clone(&kept));
    assert_eq!(container.render(4), ["B   ", "A   "]);
    assert!(Rc::ptr_eq(&container.children(), &kept));
}

/// The edit probe `A` makes to the box's children the first time the walk reaches it.
fn edit_children(
    action: &str,
    container: Weak<Box>,
    a: ComponentHandle,
    c: ComponentHandle,
) -> impl FnOnce() + 'static {
    let action = action.to_owned();
    move || {
        let Some(container) = container.upgrade() else {
            return;
        };
        let array = container.children();
        match action.as_str() {
            "append" => container.add_child(c),
            "duplicate" => container.add_child(a),
            "remove-next" => {
                let next = array.borrow()[1].clone();
                container.remove_child(&next);
            }
            "remove-current" => container.remove_child(&a),
            "clear" => container.clear(),
            _ => container.set_children(Rc::new(RefCell::new(vec![c]))),
        }
    }
}

#[test]
fn box_walk_keeps_its_live_array_across_callbacks() {
    let cases: [(&str, &[&str], usize); 6] = [
        ("append", &["A", "B", "C"], 3),
        ("remove-next", &["A"], 1),
        ("remove-current", &["A"], 1),
        ("clear", &["A", "B"], 0),
        ("replace", &["A", "B"], 1),
        ("duplicate", &["A", "B", "A"], 3),
    ];
    for rendering in [true, false] {
        for (action, visited, remaining) in cases {
            let trace = Calls::default();
            let container = Rc::new(Box::new(0, 0, None));
            let (a, b, c) = (
                Probe::new("A", &trace),
                Probe::new("B", &trace),
                Probe::new("C", &trace),
            );
            container.add_child(a.clone());
            container.add_child(b);
            a.on_first_visit(edit_children(
                action,
                Rc::downgrade(&container),
                a.clone(),
                c,
            ));

            let rows = if rendering {
                container.render(5)
            } else {
                container.invalidate();
                Vec::new()
            };

            let event = if rendering { "render" } else { "invalidate" };
            let suffix = if rendering { ":5" } else { "" };
            let expected: Vec<String> = visited
                .iter()
                .map(|name| format!("{event} {name}{suffix}"))
                .collect();
            assert_eq!(*trace.borrow(), expected, "{event} then {action}");
            if rendering {
                let shown: Vec<String> = visited.iter().map(|name| format!("{name:<5}")).collect();
                assert_eq!(rows, shown, "rows of render then {action}");
            }
            assert_eq!(
                container.children().borrow().len(),
                remaining,
                "{event} then {action}"
            );
        }
    }
}

/// The strings added to `calls` since it held `seen` entries.
fn since(calls: &Calls, seen: usize) -> Vec<String> {
    calls.borrow()[seen..].to_vec()
}

#[test]
fn box_cache_compares_width_child_lines_and_live_style_sample() {
    let calls = Calls::default();
    let color = Rc::new(Cell::new(41));
    let container = Box::new(1, 1, Some(live_background("bg:", &color, &calls)));
    let child = Probe::new("X", &calls);
    container.add_child(child.clone());

    let blank = |width: usize| " ".repeat(width);
    let rows = |color: u8, width: usize, lines: &[&str]| {
        let mut shown = vec![blank(width)];
        shown.extend(
            lines
                .iter()
                .map(|line| format!(" {line:<w$}", w = width - 1)),
        );
        shown.push(blank(width));
        shown
            .iter()
            .map(|row| format!("\x1b[{color}m{row}\x1b[49m"))
            .collect::<Vec<_>>()
    };

    assert_eq!(container.render(5), rows(41, 5, &["X"]));
    assert_eq!(
        *calls.borrow(),
        ["render X:3", "bg:test", "bg:     ", "bg: X   ", "bg:     "]
    );

    let seen = calls.borrow().len();
    assert_eq!(container.render(5), rows(41, 5, &["X"]));
    assert_eq!(
        since(&calls, seen),
        ["render X:3", "bg:test"],
        "unchanged inputs reuse the rows"
    );

    let steps: [(usize, &[&'static str]); 4] =
        [(6, &["X"]), (6, &["Y"]), (6, &["Y", "Z"]), (6, &["Z", "Y"])];
    for (width, lines) in steps {
        *child.rows.borrow_mut() = lines.to_vec();
        let seen = calls.borrow().len();
        assert_eq!(container.render(width), rows(41, width, lines));
        let painted = since(&calls, seen);
        assert_eq!(
            painted.len(),
            2 + 2 + lines.len(),
            "{lines:?} at {width} is painted again"
        );
        assert_eq!(
            painted[..2],
            [format!("render X:{}", width - 2), "bg:test".to_owned()]
        );
    }

    color.set(44);
    let seen = calls.borrow().len();
    assert_eq!(container.render(6), rows(44, 6, &["Z", "Y"]));
    assert_eq!(
        since(&calls, seen).len(),
        6,
        "a changed sample paints the rows again"
    );
}

#[test]
fn box_mutators_preserve_distinct_cache_effects() {
    let calls = Calls::default();
    let container = Box::new(0, 0, Some(background("", 41, &calls)));
    let empty = Probe::new("empty", &calls);
    *empty.rows.borrow_mut() = Vec::new();
    container.add_child(Probe::new("X", &calls));
    let painted = ["render X:5", "test", "X    "];
    let reused = ["render X:5", "test"];

    assert_eq!(container.render(5), tinted(41, &["X    "]));
    assert_eq!(*calls.borrow(), painted);

    let missing: ComponentHandle = Rc::new(Spacer::default());
    container.remove_child(&missing);
    let seen = calls.borrow().len();
    container.render(5);
    assert_eq!(
        since(&calls, seen),
        reused,
        "removing a missing child keeps the rows"
    );

    container.add_child(empty.clone());
    let seen = calls.borrow().len();
    container.render(5);
    assert_eq!(
        since(&calls, seen),
        ["render X:5", "render empty:5", "test", "X    "],
        "adding a child that shows nothing still paints again"
    );

    let removable: ComponentHandle = empty;
    container.remove_child(&removable);
    let seen = calls.borrow().len();
    container.render(5);
    assert_eq!(since(&calls, seen), painted, "removing it paints again");

    container.invalidate();
    let seen = calls.borrow().len();
    container.render(5);
    assert_eq!(since(&calls, seen), painted, "invalidation paints again");
    assert_eq!(calls.borrow()[seen - 1], "invalidate X");

    container.clear();
    let seen = calls.borrow().len();
    assert!(container.render(5).is_empty());
    assert_eq!(calls.borrow().len(), seen, "no children means no calls");
}

/// A background that styles every row but the sample, so that two of them look alike to it.
fn sample_blind(color: u8) -> Rc<dyn Fn(&str) -> String> {
    Rc::new(move |row| {
        if row == "test" {
            return row.to_owned();
        }
        format!("\x1b[{color}m{row}\x1b[49m")
    })
}

#[test]
fn box_replacing_equal_sample_styles_repaints() {
    let container = Box::new(0, 0, Some(sample_blind(41)));
    container.add_child(text("X"));
    assert_eq!(container.render(5), tinted(41, &["X    "]));

    container.set_bg_fn(Some(sample_blind(44)));
    assert_eq!(container.render(5), tinted(44, &["X    "]));

    container.set_bg_fn(None);
    assert_eq!(container.render(5), ["X    "]);
}

#[test]
fn box_invalidation_drops_cache_before_child_callbacks() {
    let calls = Calls::default();
    let container = Rc::new(Box::new(0, 0, Some(background("bg:", 41, &calls))));
    let child = Probe::new("X", &calls);
    container.add_child(child.clone());
    container.render(5);
    calls.borrow_mut().clear();

    let weak = Rc::downgrade(&container);
    child.on_first_visit(move || {
        if let Some(container) = weak.upgrade() {
            container.render(5);
        }
    });
    container.invalidate();
    assert_eq!(
        *calls.borrow(),
        ["invalidate X", "render X:5", "bg:test", "bg:X    "],
        "the render inside the child's invalidation starts from scratch"
    );
}

#[test]
fn box_background_callback_can_clear_children() {
    let container = Rc::new(Box::default());
    container.add_child(text("X"));
    let weak = Rc::downgrade(&container);
    container.set_bg_fn(Some(Rc::new(move |row: &str| {
        if let Some(container) = weak.upgrade() {
            container.clear();
        }
        row.to_owned()
    })));

    assert_eq!(container.render(5), ["     ", " X   ", "     "]);
    assert!(container.render(5).is_empty());
}

#[test]
fn box_callback_style_replacement_applies_to_later_rows() {
    let calls = Calls::default();
    let container = Rc::new(Box::new(0, 1, None));
    container.add_child(Probe::new("A", &Calls::default()));
    let first = {
        let weak = Rc::downgrade(&container);
        let second = background("second:", 44, &calls);
        let record = background("first:", 41, &calls);
        Rc::new(move |row: &str| {
            if let Some(container) = weak.upgrade() {
                container.set_bg_fn(Some(Rc::clone(&second)));
            }
            record(row)
        })
    };
    container.set_bg_fn(Some(first));

    let blank = "     ";
    assert_eq!(container.render(5), tinted(44, &[blank, "A    ", blank]));
    assert_eq!(
        *calls.borrow(),
        ["first:test", "second:     ", "second:A    ", "second:     "]
    );

    let seen = calls.borrow().len();
    assert_eq!(container.render(5), tinted(44, &[blank, "A    ", blank]));
    assert_eq!(
        since(&calls, seen).len(),
        4,
        "a changed sample paints again"
    );

    let seen = calls.borrow().len();
    container.render(5);
    assert_eq!(
        since(&calls, seen),
        ["second:test"],
        "then the rows are reused"
    );
}

#[test]
fn box_rows_painted_across_a_background_replacement_are_not_reused() {
    let container = Rc::new(Box::new(0, 1, None));
    container.add_child(text("A"));
    let swapping = {
        let weak = Rc::downgrade(&container);
        let (red, blue) = (sample_blind(41), sample_blind(44));
        Rc::new(move |row: &str| {
            if row != "test"
                && let Some(container) = weak.upgrade()
            {
                container.set_bg_fn(Some(Rc::clone(&blue)));
            }
            red(row)
        })
    };
    container.set_bg_fn(Some(swapping));
    let blank = "     ";
    let mixed = [
        &tinted(41, &[blank])[..],
        &tinted(44, &["A    ", blank])[..],
    ]
    .concat();
    assert_eq!(container.render(5), mixed);
    assert_eq!(
        container.render(5),
        tinted(44, &[blank, "A    ", blank]),
        "rows painted across a replacement are not reused"
    );
}

#[test]
fn box_rows_composed_across_a_child_change_are_not_reused() {
    let calls = Calls::default();
    let container = Rc::new(Box::new(0, 0, None));
    container.add_child(text("X"));
    let record = background("bg:", 41, &calls);
    let adding = {
        let weak = Rc::downgrade(&container);
        let added = Cell::new(false);
        Rc::new(move |row: &str| {
            if let Some(container) = weak.upgrade()
                && !added.replace(true)
            {
                container.add_child(Rc::new(Spacer::new(0)));
            }
            record(row)
        })
    };
    container.set_bg_fn(Some(adding));

    assert_eq!(container.render(5), tinted(41, &["X    "]));
    let seen = calls.borrow().len();
    assert_eq!(container.render(5), tinted(41, &["X    "]));
    assert_eq!(
        since(&calls, seen),
        ["bg:test", "bg:X    "],
        "the child added during the first render showed nothing, yet its rows are painted again"
    );
}
