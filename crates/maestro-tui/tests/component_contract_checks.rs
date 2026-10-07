#![allow(clippy::type_complexity)]
use maestro_tui::{Component, Container};
use std::{cell::RefCell, rc::Rc};
struct Mutable {
    text: String,
    log: Rc<RefCell<Vec<String>>>,
}
impl Component for Mutable {
    fn render(&mut self, _: usize) -> Vec<String> {
        vec![self.text.clone()]
    }
    fn invalidate(&mut self) {
        self.log.borrow_mut().push(self.text.clone());
    }
}
#[test]
fn container_observes_shared_children_in_order() {
    let log = Rc::new(RefCell::new(vec![]));
    let first = Rc::new(RefCell::new(Mutable {
        text: "first".into(),
        log: log.clone(),
    }));
    let shared: Rc<RefCell<dyn Component>> = first.clone();
    let other: Rc<RefCell<dyn Component>> = Rc::new(RefCell::new(Mutable {
        text: "other".into(),
        log: log.clone(),
    }));
    let mut container = Container::new();
    container.add_child(shared.clone());
    container.add_child(other.clone());
    container.add_child(shared.clone());
    first.borrow_mut().text = "changed".into();
    assert_eq!(container.render(10), ["changed", "other", "changed"]);
    container.invalidate();
    assert_eq!(*log.borrow(), ["changed", "other", "changed"]);
    container.remove_child(&shared);
    assert_eq!(container.render(10), ["other", "changed"]);
    container.remove_child(&other);
    container.remove_child(&other);
    let nested: Rc<RefCell<dyn Component>> = Rc::new(RefCell::new(container));
    let mut outer = Container::default();
    outer.add_child(nested);
    assert_eq!(outer.render(5), ["changed"]);
    outer.invalidate();
    assert_eq!(log.borrow().last().map(String::as_str), Some("changed"));

    let mut text = maestro_tui::TruncatedText::new("hello\nignored".into(), Some(1), Some(1));
    assert_eq!(text.render(8), ["        ", " hello  ", "        "]);
    text.invalidate();
    assert_eq!(text.render(8)[1], " hello  ");

    outer.clear();
    outer.clear();
    assert!(outer.render(5).is_empty());
}
#[test]
fn focus_and_overlay_contracts_preserve_optional_members() {
    use maestro_tui::components::tui::OverlayMarginValue;
    use maestro_tui::*;
    struct Focus(bool);
    impl Focusable for Focus {
        fn focused(&self) -> bool {
            self.0
        }
        fn set_focused(&mut self, f: bool) {
            self.0 = f;
        }
    }
    impl Component for Focus {
        fn render(&mut self, _: usize) -> Vec<String> {
            vec![]
        }
        fn invalidate(&mut self) {}
        fn focusable(&self) -> Option<&dyn Focusable> {
            Some(self)
        }
        fn focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
            Some(self)
        }
    }
    let mut f = Focus(false);
    assert!(!is_focusable(None));
    assert!(is_focusable(Some(&f)));
    assert!(!f.focusable().unwrap().focused());
    f.focusable_mut().unwrap().set_focused(true);
    assert!(f.focused());
    assert!(!is_focusable(Some(&Container::new())));
    assert_eq!(CURSOR_MARKER, "\x1b_maestro:c\x07");
    assert_eq!(maestro_tui::visible_width(CURSOR_MARKER), 0);
    for anchor in [
        OverlayAnchor::Center,
        OverlayAnchor::TopLeft,
        OverlayAnchor::TopRight,
        OverlayAnchor::BottomLeft,
        OverlayAnchor::BottomRight,
        OverlayAnchor::TopCenter,
        OverlayAnchor::BottomCenter,
        OverlayAnchor::LeftCenter,
        OverlayAnchor::RightCenter,
    ] {
        let options = OverlayOptions {
            width: Some(SizeValue::Percent("bad%".into())),
            min_width: Some(1),
            max_height: Some(SizeValue::Number(3)),
            anchor: Some(anchor),
            offset_x: Some(-2),
            offset_y: Some(4),
            row: None,
            col: Some(SizeValue::Percent("50%".into())),
            margin: Some(OverlayMarginValue::Sides(OverlayMargin {
                top: None,
                right: Some(2),
                bottom: Some(1),
                left: None,
            })),
            visible: Some(Box::new(|w, h| w > h)),
            non_capturing: Some(false),
        };
        assert!(matches!(options.width,Some(SizeValue::Percent(ref s)) if s=="bad%"));
        assert_eq!(options.offset_x, Some(-2));
        assert!((options.visible.unwrap())(8, 3));
        assert_eq!(options.non_capturing, Some(false));
    }
    let margins = [
        None,
        Some(OverlayMarginValue::Number(2)),
        Some(OverlayMarginValue::Sides(OverlayMargin {
            top: None,
            right: None,
            bottom: None,
            left: None,
        })),
    ];
    assert_eq!(margins.len(), 3);
    struct Handle {
        hidden: bool,
        focused: bool,
    }
    impl OverlayHandle for Handle {
        fn hide(&mut self) {
            self.hidden = true;
        }
        fn set_hidden(&mut self, h: bool) {
            self.hidden = h;
        }
        fn is_hidden(&self) -> bool {
            self.hidden
        }
        fn focus(&mut self) {
            self.focused = true;
        }
        fn unfocus(&mut self) {
            self.focused = false;
        }
        fn is_focused(&self) -> bool {
            self.focused
        }
    }
    let mut h = Handle {
        hidden: false,
        focused: false,
    };
    h.hide();
    assert!(h.is_hidden());
    h.set_hidden(false);
    assert!(!h.is_hidden());
    h.focus();
    assert!(h.is_focused());
    h.unfocus();
    assert!(!h.is_focused());
}
#[test]
fn terminal_contract_accepts_independent_implementations() {
    use maestro_tui::Terminal;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };
    #[derive(Default)]
    struct Fake<const DEFER: bool> {
        input: Option<Box<dyn FnMut(String)>>,
        resize: Option<Box<dyn FnMut()>>,
        log: Vec<String>,
    }
    impl<const D: bool> Terminal for Fake<D> {
        fn start(&mut self, i: Box<dyn FnMut(String)>, r: Box<dyn FnMut()>) {
            self.input = Some(i);
            self.resize = Some(r);
            self.log.push("start".into());
        }
        fn stop(&mut self) {
            self.log.push("stop".into());
        }
        fn drain_input<'a>(
            &'a mut self,
            max: Option<usize>,
            idle: Option<usize>,
        ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
            self.log.push(format!("drain:{max:?}:{idle:?}"));
            let mut first = D;
            Box::pin(std::future::poll_fn(move |_| {
                if first {
                    first = false;
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            }))
        }
        fn write(&mut self, s: &str) {
            self.log.push(s.into());
        }
        fn columns(&self) -> usize {
            80
        }
        fn rows(&self) -> usize {
            24
        }
        fn kitty_protocol_active(&self) -> bool {
            D
        }
        fn move_by(&mut self, n: isize) {
            self.log.push(format!("move:{n}"));
        }
        fn hide_cursor(&mut self) {
            self.log.push("hide".into());
        }
        fn show_cursor(&mut self) {
            self.log.push("show".into());
        }
        fn clear_line(&mut self) {
            self.log.push("line".into());
        }
        fn clear_from_cursor(&mut self) {
            self.log.push("from".into());
        }
        fn clear_screen(&mut self) {
            self.log.push("screen".into());
        }
        fn set_title(&mut self, s: &str) {
            self.log.push(format!("title:{s}"));
        }
        fn set_progress(&mut self, b: bool) {
            self.log.push(format!("progress:{b}"));
        }
    }
    fn drive(t: &mut dyn Terminal, events: Rc<RefCell<Vec<String>>>, deferred: bool) {
        let inputs = events.clone();
        let resize = events.clone();
        t.start(
            Box::new(move |s| inputs.borrow_mut().push(s)),
            Box::new(move || resize.borrow_mut().push("resize".into())),
        );
        assert_eq!(
            (t.columns(), t.rows(), t.kitty_protocol_active()),
            (80, 24, deferred)
        );
        t.write("raw");
        t.move_by(-2);
        t.hide_cursor();
        t.show_cursor();
        t.clear_line();
        t.clear_from_cursor();
        t.clear_screen();
        t.set_title("title");
        t.set_progress(true);
        t.set_progress(false);
        for (max, idle) in [(None, None), (Some(20), Some(3))] {
            let mut f = t.drain_input(max, idle);
            let mut cx = Context::from_waker(Waker::noop());
            if deferred {
                assert!(f.as_mut().poll(&mut cx).is_pending());
            }
            assert!(f.as_mut().poll(&mut cx).is_ready());
        }
        t.stop();
    }
    let events = Rc::new(RefCell::new(vec![]));
    let mut a = Fake::<false>::default();
    let mut b = Fake::<true>::default();
    drive(&mut a, events.clone(), false);
    drive(&mut b, events.clone(), true);
    (a.input.as_mut().unwrap())("input-a".into());
    (b.input.as_mut().unwrap())("input-b".into());
    (a.resize.as_mut().unwrap())();
    (b.resize.as_mut().unwrap())();
    assert_eq!(*events.borrow(), ["input-a", "input-b", "resize", "resize"]);
    assert_eq!(a.log, b.log);
    assert_eq!(
        a.log,
        [
            "start",
            "raw",
            "move:-2",
            "hide",
            "show",
            "line",
            "from",
            "screen",
            "title:title",
            "progress:true",
            "progress:false",
            "drain:None:None",
            "drain:Some(20):Some(3)",
            "stop"
        ]
    );
}
#[test]
fn editor_and_completion_contracts_keep_caller_callbacks() {
    use maestro_tui::editor::completion::autocomplete::AbortSignal;
    use maestro_tui::{
        AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, EditorComponent,
        SlashCommand,
    };
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };
    #[derive(Default)]
    struct Signal {
        aborted: std::cell::Cell<bool>,
        listeners: RefCell<Vec<(Rc<dyn Fn()>, bool)>>,
    }
    impl AbortSignal for Signal {
        fn aborted(&self) -> bool {
            self.aborted.get()
        }
        fn add_event_listener(&self, l: Rc<dyn Fn()>, once: bool) {
            self.listeners.borrow_mut().push((l, once));
        }
        fn remove_event_listener(&self, l: &Rc<dyn Fn()>) {
            self.listeners
                .borrow_mut()
                .retain(|(v, _)| !Rc::ptr_eq(v, l));
        }
    }
    impl Signal {
        fn abort(&self) {
            self.aborted.set(true);
            let listeners = self.listeners.borrow().clone();
            self.listeners.borrow_mut().retain(|(_, once)| !*once);
            for (l, _) in listeners {
                l();
            }
        }
    }
    struct Provider<const DEFER: bool> {
        log: Rc<RefCell<Vec<String>>>,
    }
    impl<const D: bool> AutocompleteProvider for Provider<D> {
        fn get_suggestions(
            &mut self,
            lines: Vec<String>,
            line: usize,
            col: usize,
            (signal, force): (Rc<dyn AbortSignal>, Option<bool>),
        ) -> Pin<Box<dyn Future<Output = Option<AutocompleteSuggestions>>>> {
            self.log.borrow_mut().push(format!(
                "{}:{line}:{col}:{force:?}:{}",
                lines[0],
                signal.aborted()
            ));
            let mut first = D;
            Box::pin(std::future::poll_fn(move |_| {
                if first {
                    first = false;
                    Poll::Pending
                } else if signal.aborted() {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(AutocompleteSuggestions {
                        items: vec![],
                        prefix: "😀".into(),
                    }))
                }
            }))
        }
        fn apply_completion(
            &mut self,
            mut lines: Vec<String>,
            line: usize,
            col: usize,
            item: AutocompleteItem,
            prefix: &str,
        ) -> (Vec<String>, usize, usize) {
            assert_eq!((line, col, prefix), (0, 2, "😀"));
            lines[line] = item.value;
            (lines, line, 3)
        }
        fn should_trigger_file_completion(
            &mut self,
        ) -> Option<Box<dyn FnMut(&[String], usize, usize) -> bool + '_>> {
            Some(Box::new(|l, row, col| l[row] == "😀" && col == 2))
        }
    }
    #[derive(Default)]
    struct Minimal {
        text: String,
        submit: Option<Rc<dyn Fn(String)>>,
        change: Option<Rc<dyn Fn(String)>>,
        border: Option<Rc<dyn Fn(&str) -> String>>,
    }
    impl Component for Minimal {
        fn render(&mut self, _: usize) -> Vec<String> {
            vec![self.text.clone()]
        }
        fn invalidate(&mut self) {}
        fn handle_input(&mut self, s: &str) {
            self.text += s;
            if let Some(cb) = &self.change {
                cb(self.text.clone());
            }
        }
    }
    impl EditorComponent for Minimal {
        fn get_text(&self) -> String {
            self.text.clone()
        }
        fn set_text(&mut self, t: String) {
            self.text = t;
        }
        fn on_submit(&mut self) -> &mut Option<Rc<dyn Fn(String)>> {
            &mut self.submit
        }
        fn on_change(&mut self) -> &mut Option<Rc<dyn Fn(String)>> {
            &mut self.change
        }
        fn border_color(&mut self) -> &mut Option<Rc<dyn Fn(&str) -> String>> {
            &mut self.border
        }
    }
    #[derive(Default)]
    struct Full {
        inner: Minimal,
        history: Vec<String>,
        provider: Option<Rc<RefCell<dyn AutocompleteProvider>>>,
        padding: usize,
        max: usize,
    }
    impl Component for Full {
        fn render(&mut self, w: usize) -> Vec<String> {
            self.inner.render(w)
        }
        fn invalidate(&mut self) {
            self.inner.invalidate();
        }
        fn handle_input(&mut self, s: &str) {
            self.inner.handle_input(s);
        }
    }
    impl EditorComponent for Full {
        fn get_text(&self) -> String {
            self.inner.get_text()
        }
        fn set_text(&mut self, t: String) {
            self.inner.set_text(t);
        }
        fn on_submit(&mut self) -> &mut Option<Rc<dyn Fn(String)>> {
            self.inner.on_submit()
        }
        fn on_change(&mut self) -> &mut Option<Rc<dyn Fn(String)>> {
            self.inner.on_change()
        }
        fn border_color(&mut self) -> &mut Option<Rc<dyn Fn(&str) -> String>> {
            self.inner.border_color()
        }
        fn add_to_history(&mut self) -> Option<Box<dyn FnMut(&str) + '_>> {
            Some(Box::new(|s| self.history.push(s.into())))
        }
        fn insert_text_at_cursor(&mut self) -> Option<Box<dyn FnMut(&str) + '_>> {
            Some(Box::new(|s| self.inner.text += s))
        }
        fn get_expanded_text(&self) -> Option<Box<dyn Fn() -> String + '_>> {
            Some(Box::new(|| format!("expanded:{}", self.inner.text)))
        }
        fn set_autocomplete_provider(
            &mut self,
        ) -> Option<Box<dyn FnMut(Rc<RefCell<dyn AutocompleteProvider>>) + '_>> {
            Some(Box::new(|p| self.provider = Some(p)))
        }
        fn set_padding_x(&mut self) -> Option<Box<dyn FnMut(usize) + '_>> {
            Some(Box::new(|v| self.padding = v))
        }
        fn set_autocomplete_max_visible(&mut self) -> Option<Box<dyn FnMut(usize) + '_>> {
            Some(Box::new(|v| self.max = v))
        }
    }
    fn drive(e: &mut dyn EditorComponent) {
        e.set_text("😀".into());
        assert_eq!(e.get_text(), "😀");
        e.handle_input("x");
        let text = e.get_text();
        (e.on_submit().as_ref().unwrap())(text);
        *e.border_color() = Some(Rc::new(|s| format!("[{s}]")));
        assert_eq!(e.border_color().as_ref().unwrap()("a"), "[a]");
    }
    let events = Rc::new(RefCell::new(vec![]));
    let mut a = Minimal::default();
    let mut b = Full::default();
    let submit = events.clone();
    *a.on_submit() = Some(Rc::new(move |s| {
        submit.borrow_mut().push(format!("submit:{s}"))
    }));
    let change = events.clone();
    *a.on_change() = Some(Rc::new(move |s| {
        change.borrow_mut().push(format!("change:{s}"))
    }));
    *b.on_submit() = a.on_submit().clone();
    *b.on_change() = a.on_change().clone();
    assert!(a.on_submit().is_some());
    assert!(a.on_change().is_some());
    drive(&mut a);
    drive(&mut b);
    assert_eq!(
        *events.borrow(),
        ["change:😀x", "submit:😀x", "change:😀x", "submit:😀x"]
    );
    assert!(a.get_expanded_text().is_none());
    let fallback = a.get_expanded_text().map_or_else(|| a.get_text(), |f| f());
    assert_eq!(fallback, "😀x");
    assert!(a.add_to_history().is_none());
    assert!(a.insert_text_at_cursor().is_none());
    assert!(a.set_autocomplete_provider().is_none());
    assert!(a.set_padding_x().is_none());
    assert!(a.set_autocomplete_max_visible().is_none());
    b.add_to_history().unwrap()("old");
    b.insert_text_at_cursor().unwrap()("!");
    assert_eq!(b.get_expanded_text().unwrap()(), "expanded:😀x!");
    b.set_padding_x().unwrap()(1);
    b.set_autocomplete_max_visible().unwrap()(7);
    assert_eq!(b.history, ["old"]);
    assert_eq!((b.padding, b.max), (1, 7));
    let log = Rc::new(RefCell::new(vec![]));
    let p1: Rc<RefCell<dyn AutocompleteProvider>> =
        Rc::new(RefCell::new(Provider::<false> { log: log.clone() }));
    let p2: Rc<RefCell<dyn AutocompleteProvider>> =
        Rc::new(RefCell::new(Provider::<true> { log: log.clone() }));
    let signal = Rc::new(Signal::default());
    let mut cx = Context::from_waker(Waker::noop());
    for (provider, deferred) in [(&p1, false), (&p2, true)] {
        b.set_autocomplete_provider().unwrap()(provider.clone());
        assert!(Rc::ptr_eq(b.provider.as_ref().unwrap(), provider));
        for force in [None, Some(false), Some(true)] {
            let mut first = provider.borrow_mut().get_suggestions(
                vec!["😀".into()],
                0,
                2,
                (signal.clone(), force),
            );
            let second = provider.borrow_mut().get_suggestions(
                vec!["😀".into()],
                0,
                2,
                (signal.clone(), force),
            );
            drop(second);
            if deferred {
                assert!(first.as_mut().poll(&mut cx).is_pending());
            }
            let Poll::Ready(Some(s)) = first.as_mut().poll(&mut cx) else {
                panic!("missing suggestions")
            };
            assert!(s.items.is_empty());
            assert_eq!(s.prefix, "😀");
        }
        assert!(provider
            .borrow_mut()
            .should_trigger_file_completion()
            .unwrap()(&["😀".into()], 0, 2));
        let item = AutocompleteItem {
            value: "abc".into(),
            label: "ABC".into(),
            description: Some("description".into()),
        };
        assert_eq!(item.label, "ABC");
        assert_eq!(item.description.as_deref(), Some("description"));
        assert_eq!(
            provider
                .borrow_mut()
                .apply_completion(vec!["😀".into()], 0, 2, item, "😀"),
            (vec!["abc".into()], 0, 3)
        );
    }
    assert_eq!(log.borrow().len(), 12);
    assert!(log.borrow().iter().any(|s| s.contains("None")));
    assert!(log.borrow().iter().any(|s| s.contains("Some(false)")));
    assert!(log.borrow().iter().any(|s| s.contains("Some(true)")));
    for deferred in [false, true] {
        let prefix = Rc::new(std::cell::Cell::new(false));
        let observed = prefix.clone();
        let command = SlashCommand {
            name: "command".into(),
            description: None,
            argument_hint: Some("hint".into()),
            get_argument_completions: Some(Rc::new(move |s| {
                assert_eq!(s, "arg");
                observed.set(true);
                let mut first = deferred;
                Box::pin(std::future::poll_fn(move |_| {
                    if first {
                        first = false;
                        Poll::Pending
                    } else {
                        Poll::Ready(Some(vec![AutocompleteItem {
                            value: "v".into(),
                            label: "l".into(),
                            description: None,
                        }]))
                    }
                }))
            })),
        };
        assert_eq!(command.name, "command");
        assert_eq!(command.argument_hint.as_deref(), Some("hint"));
        let mut f = command.get_argument_completions.unwrap()("arg".into());
        assert!(prefix.get());
        if deferred {
            assert!(f.as_mut().poll(&mut cx).is_pending());
        }
        let Poll::Ready(Some(items)) = f.as_mut().poll(&mut cx) else {
            panic!("missing arguments")
        };
        assert_eq!(items[0].value, "v");
    }
    let fired = Rc::new(std::cell::Cell::new(0));
    let n = fired.clone();
    let once: Rc<dyn Fn()> = Rc::new(move || n.set(n.get() + 1));
    let n = fired.clone();
    let repeat: Rc<dyn Fn()> = Rc::new(move || n.set(n.get() + 10));
    signal.add_event_listener(once, true);
    signal.add_event_listener(repeat.clone(), false);
    signal.abort();
    signal.abort();
    assert_eq!(fired.get(), 21);
    signal.remove_event_listener(&repeat);
    signal.abort();
    assert_eq!(fired.get(), 21);
    assert!(signal.aborted());
    let mut cancelled = p1
        .borrow_mut()
        .get_suggestions(vec!["😀".into()], 0, 2, (signal, None));
    assert!(matches!(
        cancelled.as_mut().poll(&mut cx),
        Poll::Ready(None)
    ));
}
