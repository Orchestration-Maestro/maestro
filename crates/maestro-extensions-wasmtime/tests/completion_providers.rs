//! Authored completion providers stack on the native provider through the production runtime.
#![cfg(test)]

use maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for,
    types::{AutocompleteProviderFactory, ExtensionUIContextPort},
};
use maestro_tui::autocomplete::{
    CompletionError, CompletionOptions, CompletionResult, CursorPosition,
};
use maestro_tui::{AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, Cancellation};
use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc};

/// A native completion provider.
type Native = Rc<dyn AutocompleteProvider<Signal = Cancellation>>;

/// The application's own provider: `base` for the text before the cursor, file completion on
/// a line that starts with `@`.
struct Base(Rc<RefCell<Vec<String>>>);

impl AutocompleteProvider for Base {
    type Signal = Cancellation;
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a, Cancellation>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AutocompleteSuggestions>, CompletionError>> + 'a>>
    {
        let seen = format!(
            "base {lines:?} {} {:?}",
            options.signal.is_aborted(),
            options.force
        );
        self.0.borrow_mut().push(seen);
        let prefix = lines[cursor.line][..cursor.col].to_owned();
        Box::pin(async move {
            Ok(
                (options.force != Some(false)).then(|| AutocompleteSuggestions {
                    items: vec![AutocompleteItem {
                        value: "base".into(),
                        label: "Base".into(),
                        description: None,
                    }],
                    prefix,
                }),
            )
        })
    }
    fn apply_completion(
        &self,
        _: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        CompletionResult {
            lines: vec![format!("{prefix}={}", item.value)],
            cursor_line: cursor.line,
            cursor_col: cursor.col,
        }
    }
    fn should_trigger_file_completion(&self, lines: &[String], _: CursorPosition) -> Option<bool> {
        lines[0].starts_with('@').then_some(true)
    }
}

/// The application's provider stack, rebuilt over its own provider after every addition.
struct Ui {
    /// The application's own provider.
    base: Native,
    /// The stacked factories, in order.
    wrappers: RefCell<Vec<AutocompleteProviderFactory>>,
    /// The provider the stack built.
    current: RefCell<Option<Native>>,
}

impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn add_autocomplete_provider(
        &self,
        factory: AutocompleteProviderFactory,
    ) -> Result<(), EventError> {
        self.wrappers.borrow_mut().push(factory);
        let mut provider = self.base.clone();
        let wrappers = self.wrappers.borrow().clone();
        for wrapper in wrappers {
            provider = wrapper.call(provider)?;
        }
        self.current.replace(Some(provider));
        Ok(())
    }
}

/// Stacks the authored provider `name` through the bridge.
async fn stack(runner: &ExtensionRunner<Local>, name: &str) {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let outcome = handler(
        format!("ui provider {name}"),
        runner.create_command_context(),
    )
    .await;
    assert_eq!(outcome.unwrap_err().to_string(), "provider stacked");
}

/// Suggestions of `provider` for one line, with a cursor at its end.
async fn suggest(
    provider: &Native,
    line: &str,
    signal: &Cancellation,
    force: Option<bool>,
) -> Option<(Vec<String>, String)> {
    let lines = vec![line.to_owned()];
    let cursor = CursorPosition {
        line: 0,
        col: line.len(),
    };
    let options = CompletionOptions { signal, force };
    let found = provider
        .get_suggestions(&lines, cursor, options)
        .await
        .unwrap();
    found.map(|found| {
        let values = found.items.into_iter().map(|item| item.value).collect();
        (values, found.prefix)
    })
}

/// The decision of `provider` for one line.
fn decide(provider: &Native, line: &str) -> Option<bool> {
    let cursor = CursorPosition { line: 0, col: 0 };
    provider.should_trigger_file_completion(&[line.to_owned()], cursor)
}

/// Each wrapper receives the previous provider and adds its candidate; every wrapper keeps the
/// caller's own signal after the request returns.
async fn suggestions(provider: &Native, seen: &RefCell<Vec<String>>) {
    let signal = Cancellation::new();
    let stacked = Some((
        vec!["base".into(), "first".into(), "second".into()],
        "src/".into(),
    ));
    assert_eq!(suggest(provider, "src/", &signal, None).await, stacked);
    assert_eq!(suggest(provider, "src/", &signal, Some(false)).await, None);
    let own = Some((vec!["second".into()], "mine".into()));
    let kept = Cancellation::new();
    assert_eq!(suggest(provider, "mine", &kept, Some(true)).await, own);
    assert_eq!(decide(provider, "kept"), Some(false));
    kept.abort();
    assert_eq!(decide(provider, "kept"), Some(true));
    let seen = std::mem::take(&mut *seen.borrow_mut());
    assert_eq!(
        seen,
        [
            r#"base ["src/"] false None"#,
            r#"base ["src/"] false Some(false)"#,
        ]
    );
}

#[test]
fn canonical_completion_providers_stack_and_keep_signals() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let ui = Rc::new(Ui {
            base: Rc::new(Base(seen.clone())),
            wrappers: RefCell::default(),
            current: RefCell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        stack(&runner, "first").await;
        stack(&runner, "second").await;
        let provider = ui.current.borrow().clone().unwrap();
        suggestions(&provider, &seen).await;
        let cursor = CursorPosition { line: 0, col: 4 };
        let item = |value: &str| AutocompleteItem {
            value: value.into(),
            label: String::new(),
            description: None,
        };
        let lines = ["src/".to_owned()];
        let base = provider.apply_completion(&lines, cursor, &item("base"), "src/");
        assert_eq!(base.lines, ["src/=base"]);
        let first = provider.apply_completion(&lines, cursor, &item("first"), "src/");
        assert_eq!(
            (first.lines, first.cursor_col),
            (vec!["src/->first".into()], 5)
        );
        assert_eq!(decide(&provider, "@x"), Some(true));
        assert_eq!(decide(&provider, "x"), None);
        drop(runner);
        bus.clear();
    });
}
