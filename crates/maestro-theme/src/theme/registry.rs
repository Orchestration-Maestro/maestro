//! Theme registration, discovery and lookup over supplied directories.
use super::lifecycle::Lifecycle;
use super::loading::{build_theme, parse_custom};
use super::{Theme, ThemeError, ThemeOperations, ThemeOptions};
use indexmap::IndexMap;
use maestro_path::join;
use serde_json::Value;
use std::borrow::Cow;
use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

/// Shipped theme names in discovery order.
const BUILTIN_NAMES: [&str; 2] = ["dark", "light"];

/// Directories supplied by the application; the theme owner does not choose them.
#[derive(Debug, Clone)]
pub struct ThemeDirectories {
    /// Directory holding the shipped `dark.json` and `light.json`.
    pub themes_dir: String,
    /// Directory holding user theme entries named `<theme>.json`.
    pub custom_themes_dir: String,
}
/// One inventory entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeInfo {
    /// Theme name.
    pub name: String,
    /// Source path of the first owner of the name, when it has one.
    pub path: Option<String>,
}
/// Lazy shipped data, registrations and effects for one set of theme directories.
pub struct ThemeState {
    /// Supplied roots.
    pub(super) directories: ThemeDirectories,
    /// File, environment and sorting effects.
    pub(super) operations: Rc<dyn ThemeOperations>,
    /// Shipped documents, filled only after every one was read and parsed.
    builtins: OnceCell<[Value; 2]>,
    /// Registered instances by name, in first-registration order.
    pub(super) registered: RefCell<IndexMap<String, Rc<Theme>>>,
    /// Published theme, selected name, change callback and watch.
    pub(super) lifecycle: Lifecycle,
}
impl ThemeState {
    /// Retain the roots and effects; nothing is read until a query needs it.
    #[must_use]
    pub fn new(directories: ThemeDirectories, operations: Rc<dyn ThemeOperations>) -> Self {
        Self {
            directories,
            operations,
            builtins: OnceCell::new(),
            registered: RefCell::default(),
            lifecycle: Lifecycle::default(),
        }
    }
    /// Replace every registration; unnamed themes are ignored and a repeated name
    /// keeps its first position with the last instance.
    pub fn set_registered_themes(&self, themes: Vec<Rc<Theme>>) {
        let mut registered = self.registered.borrow_mut();
        registered.clear();
        for theme in themes {
            if let Some(name) = theme.name().filter(|name| !name.is_empty()) {
                registered.insert(name.to_owned(), Rc::clone(&theme));
            }
        }
    }
    /// List every shipped, custom and registered name once, in UTF-16 code-unit order.
    ///
    /// # Errors
    /// Returns shipped-data or custom-directory failures.
    pub fn get_available_themes(&self) -> Result<Vec<String>, ThemeError> {
        let mut names: Vec<String> = self.owners()?.into_keys().collect();
        names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        Ok(names)
    }
    /// List every name once with its first owner's path, in stable locale order.
    ///
    /// # Errors
    /// Returns shipped-data, custom-directory or sorting failures.
    pub fn get_available_themes_with_paths(&self) -> Result<Vec<ThemeInfo>, ThemeError> {
        let mut themes: Vec<ThemeInfo> = self
            .owners()?
            .into_iter()
            .map(|(name, path)| ThemeInfo { name, path })
            .collect();
        self.operations
            .sort_by_name(&mut themes)
            .map_err(ThemeError::io)?;
        Ok(themes)
    }
    /// Return the registered instance, else a new instance from shipped or custom data.
    ///
    /// Any loading failure is absence.
    #[must_use]
    pub fn get_theme_by_name(&self, name: &str) -> Option<Rc<Theme>> {
        self.load_named(name).ok()
    }

    /// Return the registered instance, else a new instance from shipped or custom data.
    pub(super) fn load_named(&self, name: &str) -> Result<Rc<Theme>, ThemeError> {
        if let Some(theme) = self.registered.borrow().get(name) {
            return Ok(Rc::clone(theme));
        }
        let document = self.document(name)?;
        let options = ThemeOptions::default();
        build_theme(&document, None, self.operations.as_ref(), options).map(Rc::new)
    }

    /// Select each name's first owner: shipped, then custom entry, then registration.
    fn owners(&self) -> Result<IndexMap<String, Option<String>>, ThemeError> {
        self.builtins()?;
        let mut owners = IndexMap::new();
        for name in BUILTIN_NAMES {
            let path = join(&[&self.directories.themes_dir, &format!("{name}.json")]);
            owners.insert(name.to_owned(), Some(path));
        }
        self.add_custom_owners(&mut owners)?;
        for (name, theme) in &*self.registered.borrow() {
            owners
                .entry(name.clone())
                .or_insert_with(|| theme.source_path().map(str::to_owned));
        }
        Ok(owners)
    }
    /// Add each custom `.json` entry name not owned yet, when the custom directory exists.
    fn add_custom_owners(
        &self,
        owners: &mut IndexMap<String, Option<String>>,
    ) -> Result<(), ThemeError> {
        let custom = &self.directories.custom_themes_dir;
        if !self.operations.exists(custom) {
            return Ok(());
        }
        for file in self.operations.read_dir(custom).map_err(ThemeError::io)? {
            if let Some(name) = file.strip_suffix(".json") {
                owners
                    .entry(name.to_owned())
                    .or_insert_with(|| Some(join(&[custom, &file])));
            }
        }
        Ok(())
    }
    /// Read and parse both shipped files, caching them only when both succeed.
    fn builtins(&self) -> Result<&[Value; 2], ThemeError> {
        if let Some(builtins) = self.builtins.get() {
            return Ok(builtins);
        }
        let dark = self.read_builtin(BUILTIN_NAMES[0])?;
        let light = self.read_builtin(BUILTIN_NAMES[1])?;
        Ok(self.builtins.get_or_init(|| [dark, light]))
    }
    /// Read one shipped file without custom-file admission.
    fn read_builtin(&self, name: &str) -> Result<Value, ThemeError> {
        let path = join(&[&self.directories.themes_dir, &format!("{name}.json")]);
        let content = self
            .operations
            .read_to_string(&path)
            .map_err(ThemeError::io)?;
        serde_json::from_str(&content)
            .map_err(|cause| ThemeError::caused_by(cause.to_string(), cause))
    }
    /// Select the document of a name: shipped data, then a registration's source file,
    /// then the custom entry. Shipped data is borrowed from the cache; files are reread.
    ///
    /// # Errors
    /// Returns a registration without a source path, a missing custom file or a read,
    /// parse or admission failure.
    pub(super) fn document(&self, name: &str) -> Result<Cow<'_, Value>, ThemeError> {
        let builtins = self.builtins()?;
        if let Some(index) = BUILTIN_NAMES.iter().position(|builtin| *builtin == name) {
            return Ok(Cow::Borrowed(&builtins[index]));
        }
        let source = self
            .registered
            .borrow()
            .get(name)
            .map(|theme| theme.source_path().unwrap_or_default().to_owned());
        let (path, label) = match source {
            Some(path) if path.is_empty() => {
                return Err(ThemeError::message(format!(
                    "Theme \"{name}\" does not have a source path for export"
                )));
            }
            Some(path) => (path, None),
            None => {
                let path = join(&[&self.directories.custom_themes_dir, &format!("{name}.json")]);
                if !self.operations.exists(&path) {
                    return Err(ThemeError::message(format!("Theme not found: {name}")));
                }
                (path, Some(name))
            }
        };
        let content = self
            .operations
            .read_to_string(&path)
            .map_err(ThemeError::io)?;
        parse_custom(label.unwrap_or(&path), &content).map(Cow::Owned)
    }
}
