#![cfg(not(target_arch = "wasm32"))]
//! Session, metadata, theme and corpus support for the footer tests.
use crate::published;
pub mod corpus;
pub mod session;

use session::session;

use maestro_app::{
    AgentSession, CompactionOptions, ExtensionStatuses, Local, Model, Presence,
    ReadonlyFooterDataProvider, ThinkingLevel,
};
use maestro_chat::FooterComponent;
use maestro_chat::components::footer::Environment;

use maestro_theme::{Collator, LiveTheme, Theme, ThemeColor, ThemeState};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};

/// What a metadata getter answers, including any change it makes while answering.
type Answer<T> = Rc<dyn Fn() -> T>;

/// The read-only footer metadata the footer reads, with live branch, count and statuses.
pub struct Metadata {
    /// The branch getter, which may change the world before it answers.
    branch: RefCell<Answer<Option<String>>>,
    /// The provider-count getter, which may change the world before it answers.
    count: RefCell<Answer<f64>>,
    /// The extension statuses in insertion order.
    statuses: ExtensionStatuses,
}

impl Metadata {
    /// Metadata that answers `branch` and `count` without side effects.
    pub fn new(branch: Option<&str>, count: f64) -> Rc<Self> {
        let branch = branch.map(str::to_owned);
        Rc::new(Self {
            branch: RefCell::new(Rc::new(move || branch.clone())),
            count: RefCell::new(Rc::new(move || count)),
            statuses: ExtensionStatuses::default(),
        })
    }

    /// Replaces the branch getter.
    pub fn set_branch(&self, answer: Answer<Option<String>>) {
        *self.branch.borrow_mut() = answer;
    }

    /// Replaces the provider-count getter.
    pub fn set_count(&self, answer: Answer<f64>) {
        *self.count.borrow_mut() = answer;
    }

    /// Inserts the status pairs in the supplied order.
    pub fn set_statuses(&self, pairs: &[(String, String)]) {
        self.statuses.clear();
        for (key, value) in pairs {
            self.statuses.insert(key.clone(), value.clone());
        }
    }
}

impl ReadonlyFooterDataProvider for Metadata {
    fn get_git_branch(&self) -> Option<String> {
        (self.branch.borrow())()
    }

    fn get_extension_statuses(&self) -> ExtensionStatuses {
        self.statuses.clone()
    }

    fn get_available_provider_count(&self) -> f64 {
        (self.count.borrow())()
    }

    fn on_branch_change(&self, _callback: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        panic!("the footer must not subscribe")
    }
}

/// The roles a recorded expectation names, mapped onto the published theme's escapes.
pub struct Roles {
    /// Each foreground escape with the recorded role name.
    entries: Vec<(String, String)>,
}

impl Roles {
    /// Reads the escapes of the supplied themes, dark first.
    pub fn new(dark: &Theme, light: &Theme) -> Self {
        let mut entries = Vec::new();
        for (mode, theme) in [("dark", dark), ("light", light)] {
            for (role, name) in [
                (ThemeColor::Dim, "dim"),
                (ThemeColor::Error, "error"),
                (ThemeColor::Warning, "warning"),
            ] {
                add(&mut entries, mode, theme, &role, name);
            }
        }
        Self { entries }
    }

    /// Replaces every role escape with its recorded name and every reset with one marker.
    pub fn label(&self, text: &str) -> String {
        let mut labelled = text.to_owned();
        for (escape, label) in &self.entries {
            labelled = labelled.replace(escape.as_str(), label);
        }
        labelled
            .replace("\u{1b}[39m", "</>")
            .replace("\u{1b}[0m", "</>")
    }
}

/// Records one role's escape, keeping the first name that escape answered to.
fn add(
    entries: &mut Vec<(String, String)>,
    mode: &str,
    theme: &Theme,
    role: &ThemeColor,
    name: &str,
) {
    let Ok(escape) = theme.get_fg_ansi(role) else {
        return;
    };
    if entries.iter().any(|(known, _)| known == escape) {
        return;
    }
    entries.push((escape.to_owned(), format!("<{mode}:{name}>")));
}

/// A footer over a controlled session, its metadata and a published theme.
pub struct World {
    /// The session the footer renders.
    pub session: Rc<AgentSession<Local>>,
    /// The metadata the footer reads.
    pub metadata: Rc<Metadata>,
    /// The live slot the published theme sits in.
    pub publication: Rc<ThemeState>,
    /// The handle the footer reads its theme through.
    pub theme: LiveTheme,
    /// Reads `HOME` and `USERPROFILE` for the path row.
    environment: RefCell<Environment>,
}

impl World {
    /// A world over `cwd` with the dark palette published and one provider.
    pub fn new(cwd: &str, oauth_provider: Option<&str>) -> Self {
        let session = session(cwd, oauth_provider, |loader| loader).session;
        let (publication, theme) = published::published(&dark_theme());
        Self {
            session,
            metadata: Metadata::new(Some("main"), 1.0),
            publication: Rc::new(publication),
            theme,
            environment: RefCell::new(Rc::new(|_| None)),
        }
    }

    /// Sets the `HOME` and `USERPROFILE` values the path row reads.
    pub fn set_environment(&self, home: Option<&Value>, userprofile: Option<&Value>) {
        let home = text(home);
        let userprofile = text(userprofile);
        *self.environment.borrow_mut() = Rc::new(move |variable| match variable {
            "HOME" => home.clone(),
            "USERPROFILE" => userprofile.clone(),
            _ => None,
        });
    }

    /// Builds a footer over this world.
    pub fn footer(&self) -> FooterComponent<Local> {
        FooterComponent::new(
            Rc::clone(&self.session),
            Rc::clone(&self.metadata) as Rc<dyn ReadonlyFooterDataProvider>,
            self.theme.clone(),
            Rc::new(
                Collator::new(&|variable| match variable {
                    "LC_ALL" => Some("en_US.UTF-8".to_owned()),
                    _ => None,
                })
                .expect("the collator"),
            ),
            Rc::clone(&*self.environment.borrow()),
        )
    }

    /// Selects the model the footer reports.
    pub fn set_model(&self, id: &str, provider: &str, reasoning: bool, window: f64) {
        let document = serde_json::json!({
            "id": id,
            "name": id,
            "api": "openai-completions",
            "provider": provider,
            "baseUrl": "http://localhost",
            "reasoning": reasoning,
            "input": ["text"],
            "cost": {"input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0},
            "contextWindow": window,
            "maxTokens": 0.0
        });
        let model: Model = serde_json::from_value(document).expect("a controlled model descriptor");
        self.session
            .state()
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .model = Arc::new(RwLock::new(model));
    }

    /// Selects the thinking level the footer reports.
    pub fn set_thinking(&self, level: ThinkingLevel) {
        self.session
            .state()
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .thinking_level = level;
    }

    /// Appends the supplied raw records in order, keeping message and other records apart.
    pub fn append_entries(&self, entries: &[Value]) {
        let mut manager = self.session.session_manager().borrow_mut();
        for entry in entries {
            match entry.get("type").and_then(Value::as_str) {
                Some("message") => {
                    let message = entry.get("message").cloned().unwrap_or(Value::Null);
                    manager
                        .append_message(message)
                        .expect("an in-memory append");
                }
                Some(record) => {
                    manager
                        .append_custom_entry(record, Presence::Present(entry.clone()))
                        .expect("an in-memory append");
                }
                None => {}
            }
        }
    }

    /// Appends one raw message record.
    pub fn append_message(&self, message: &Value) {
        self.append_entries(&[serde_json::json!({"type": "message", "message": message})]);
    }

    /// Puts one already-decoded answer into the conversation the session derives its context
    /// usage from.
    pub fn push_answer(&self, document: &Value) {
        let message = serde_json::from_value(document.clone()).expect("a controlled answer");
        self.session
            .messages()
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(message);
    }

    /// Puts one answer into the conversation the session derives its context usage from.
    pub fn set_context(&self, tokens: f64) {
        let document = serde_json::json!({
            "role": "assistant",
            "content": [],
            "api": "openai-completions",
            "provider": "test",
            "model": "test-model",
            "usage": {
                "input": 0.0,
                "output": 0.0,
                "cacheRead": 0.0,
                "cacheWrite": 0.0,
                "totalTokens": tokens,
                "cost": {
                    "input": 0.0,
                    "output": 0.0,
                    "cacheRead": 0.0,
                    "cacheWrite": 0.0,
                    "total": 0.0
                }
            },
            "stopReason": "stop",
            "timestamp": 0.0
        });
        self.push_answer(&document);
    }

    /// Appends a compaction record, which leaves context tokens unknown until the next answer.
    pub fn append_compaction(&self) {
        let mut manager = self.session.session_manager().borrow_mut();
        manager
            .append_compaction("summary", "parent", 0.0, CompactionOptions::default())
            .expect("an in-memory compaction");
    }

    /// Records the session name the footer shows.
    pub fn set_name(&self, name: &str) {
        self.session
            .session_manager()
            .borrow_mut()
            .append_session_info(name)
            .expect("an in-memory session name");
    }
}

/// A world whose live slot holds no published theme yet.
pub fn unpublished(cwd: &str) -> World {
    let state = published::unpublished();
    let theme = state.theme();
    World {
        session: session(cwd, None, |loader| loader).session,
        metadata: Metadata::new(Some("main"), 1.0),
        publication: Rc::new(state),
        theme,
        environment: RefCell::new(Rc::new(|_| None)),
    }
}

/// The palette a footer test starts with: one distinct colour per role.
pub fn dark_theme() -> Rc<Theme> {
    published::theme(0)
}

/// The palette a paint swap publishes: the same roles in other colours.
pub fn light_theme() -> Rc<Theme> {
    published::theme(100)
}

/// The text of a recorded environment value.
fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

/// A counter a test bumps to observe how often a getter answered.
pub fn counter() -> Rc<Cell<usize>> {
    Rc::new(Cell::new(0))
}
