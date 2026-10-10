//! Shared fixtures for the live-theme tests: scratch directories, controlled
//! environment and custom theme documents.
use maestro_theme::{
    NativeThemeOperations, ThemeDirectories, ThemeInfo, ThemeOperations, ThemeState,
};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Directory of the shipped theme documents.
const SHIPPED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme");
/// The shipped dark theme document.
const DARK: &str = include_str!("../../assets/theme/dark.json");

/// A uniquely named scratch directory whose removal is attempted on drop.
pub struct Scratch(PathBuf);

impl Scratch {
    /// Create an empty directory with a custom-themes subdirectory.
    pub fn new(tag: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-live-{}-{}-{tag}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("custom")).unwrap();
        Self(path)
    }

    /// Path of an entry below the scratch directory.
    pub fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }

    /// Write a file below the scratch directory.
    pub fn write(&self, relative: &str, content: &str) {
        let path = self.path(relative);
        fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Native file operations with a controlled environment that records each lookup.
pub struct Ops {
    /// Environment variables that are set.
    env: RefCell<HashMap<String, String>>,
    /// Names looked up, in call order.
    pub env_reads: RefCell<Vec<String>>,
}

impl Ops {
    /// Truecolor terminal plus the given variables.
    pub fn new(env: &[(&str, &str)]) -> Rc<Self> {
        let mut vars: HashMap<_, _> = [("COLORTERM".to_owned(), "truecolor".to_owned())].into();
        vars.extend(env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())));
        Rc::new(Self {
            env: RefCell::new(vars),
            env_reads: RefCell::default(),
        })
    }

    /// Set or replace one variable.
    #[allow(
        dead_code,
        reason = "The native notification test never changes the environment."
    )]
    pub fn set(&self, name: &str, value: &str) {
        self.env
            .borrow_mut()
            .insert(name.to_owned(), value.to_owned());
    }

    /// Whether a variable was ever looked up.
    #[allow(
        dead_code,
        reason = "The native notification test never inspects lookups."
    )]
    pub fn read(&self, name: &str) -> bool {
        self.env_reads.borrow().iter().any(|read| read == name)
    }
}

impl ThemeOperations for Ops {
    fn read_to_string(&self, path: &str) -> io::Result<String> {
        NativeThemeOperations.read_to_string(path)
    }

    fn environment(&self, name: &str) -> Option<String> {
        self.env_reads.borrow_mut().push(name.to_owned());
        self.env.borrow().get(name).cloned()
    }

    fn exists(&self, path: &str) -> bool {
        NativeThemeOperations.exists(path)
    }

    fn read_dir(&self, path: &str) -> io::Result<Vec<String>> {
        NativeThemeOperations.read_dir(path)
    }

    fn sort_by_name(&self, themes: &mut [ThemeInfo]) -> io::Result<()> {
        NativeThemeOperations.sort_by_name(themes)
    }
}

/// A state over the shipped themes and the scratch custom directory.
pub fn state(scratch: &Scratch, ops: &Rc<Ops>) -> Rc<ThemeState> {
    Rc::new(ThemeState::new(
        ThemeDirectories {
            themes_dir: SHIPPED.to_owned(),
            custom_themes_dir: scratch.path("custom"),
        },
        Rc::clone(ops) as Rc<dyn ThemeOperations>,
    ))
}

/// The shipped dark document under another name with a literal accent.
pub fn custom_json(name: &str, accent: &str) -> Value {
    let mut json: Value = serde_json::from_str(DARK).unwrap();
    json["name"] = name.into();
    json["colors"]["accent"] = accent.into();
    json
}
