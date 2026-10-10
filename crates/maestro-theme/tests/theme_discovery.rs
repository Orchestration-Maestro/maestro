//! Theme registration, discovery and shared source metadata.
use maestro_request::source_info::{SourceInfo, SourceOrigin, SourceScope};
use maestro_theme::{
    ColorMode, NativeThemeOperations, Theme, ThemeColor, ThemeDirectories, ThemeError, ThemeInfo,
    ThemeOperations, ThemeOptions, ThemeState, load_theme_from_path,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;

#[cfg(test)]
const SHIPPED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme");
#[cfg(test)]
const TRUECOLOR_ACCENT: &str = "\x1b[38;2;138;190;183m";

/// A scratch directory removed on drop.
#[cfg(test)]
struct Scratch(PathBuf);

#[cfg(test)]
impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("maestro-theme-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, relative: &str) -> String {
        format!("{}/{relative}", self.0.display())
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.path(relative);
        fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

#[cfg(test)]
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Real files with a controlled environment, recording each read and environment lookup.
#[cfg(test)]
struct Real {
    env: RefCell<HashMap<String, String>>,
    log: RefCell<Vec<String>>,
}

#[cfg(test)]
impl Real {
    fn new(env: &[(&str, &str)]) -> Rc<Self> {
        Rc::new(Self {
            env: RefCell::new(
                env.iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            ),
            log: RefCell::default(),
        })
    }

    fn reads(&self) -> Vec<String> {
        self.log
            .borrow()
            .iter()
            .filter_map(|entry| entry.strip_prefix("read:").map(str::to_owned))
            .collect()
    }

    fn env_reads(&self) -> usize {
        self.log
            .borrow()
            .iter()
            .filter(|entry| entry.starts_with("env:"))
            .count()
    }
}

#[cfg(test)]
impl ThemeOperations for Real {
    fn read_to_string(&self, path: &str) -> io::Result<String> {
        self.log.borrow_mut().push(format!("read:{path}"));
        NativeThemeOperations.read_to_string(path)
    }

    fn environment(&self, name: &str) -> Option<String> {
        self.log.borrow_mut().push(format!("env:{name}"));
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

/// In-memory files and directories; `sort_by_name` reverses its input.
#[cfg(test)]
#[derive(Default)]
struct Virtual {
    files: HashMap<String, String>,
    existing: HashSet<String>,
    dirs: HashMap<String, Vec<String>>,
    log: RefCell<Vec<String>>,
}

#[cfg(test)]
impl ThemeOperations for Virtual {
    fn read_to_string(&self, path: &str) -> io::Result<String> {
        self.log.borrow_mut().push(format!("read:{path}"));
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn environment(&self, name: &str) -> Option<String> {
        self.log.borrow_mut().push(format!("env:{name}"));
        None
    }

    fn exists(&self, path: &str) -> bool {
        self.log.borrow_mut().push(format!("exists:{path}"));
        self.existing.contains(path)
    }

    fn read_dir(&self, path: &str) -> io::Result<Vec<String>> {
        self.log.borrow_mut().push(format!("read_dir:{path}"));
        self.dirs
            .get(path)
            .cloned()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn sort_by_name(&self, themes: &mut [ThemeInfo]) -> io::Result<()> {
        themes.reverse();
        Ok(())
    }
}

#[cfg(test)]
impl Virtual {
    fn shipped() -> Self {
        let mut ops = Self::default();
        for name in ["dark", "light"] {
            let text = fs::read_to_string(format!("{SHIPPED}/{name}.json")).unwrap();
            ops.files.insert(format!("/themes/{name}.json"), text);
        }
        ops
    }
}

#[cfg(test)]
fn dirs(themes: &str, custom: &str) -> ThemeDirectories {
    ThemeDirectories {
        themes_dir: themes.to_owned(),
        custom_themes_dir: custom.to_owned(),
    }
}

#[cfg(test)]
fn make(ops: &Rc<impl ThemeOperations + 'static>, custom: &str) -> ThemeState {
    ThemeState::new(
        dirs(SHIPPED, custom),
        Rc::clone(ops) as Rc<dyn ThemeOperations>,
    )
}

#[cfg(test)]
fn shipped_dark() -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(format!("{SHIPPED}/dark.json")).unwrap()).unwrap()
}

/// The shipped dark document renamed, with its accent variable replaced.
fn theme_text(name: &str, accent: &str) -> String {
    let mut doc = shipped_dark();
    doc["name"] = name.into();
    doc["vars"]["accent"] = accent.into();
    doc.to_string()
}

#[cfg(test)]
fn registered(name: Option<&str>, path: Option<&str>) -> Rc<Theme> {
    let options = ThemeOptions {
        name: name.map(str::to_owned),
        source_path: path.map(str::to_owned),
        ..ThemeOptions::default()
    };
    Rc::new(Theme::new([], [], ColorMode::Truecolor, options).unwrap())
}

#[cfg(test)]
fn accent(theme: &Theme) -> String {
    theme.get_fg_ansi(&ThemeColor::Accent).unwrap().to_owned()
}

#[cfg(test)]
fn names(infos: &[ThemeInfo]) -> Vec<(&str, Option<&str>)> {
    infos
        .iter()
        .map(|info| (info.name.as_str(), info.path.as_deref()))
        .collect()
}

#[cfg(test)]
fn io_kind(error: &ThemeError) -> Option<io::ErrorKind> {
    error
        .source()
        .and_then(|cause| cause.downcast_ref::<io::Error>())
        .map(io::Error::kind)
}

#[cfg(test)]
fn record(
    scope: SourceScope,
    origin: SourceOrigin,
    base_dir: Option<&str>,
) -> Rc<RefCell<SourceInfo>> {
    Rc::new(RefCell::new(SourceInfo {
        path: "original".into(),
        source: "label".into(),
        scope,
        origin,
        base_dir: base_dir.map(str::to_owned),
    }))
}

#[test]
fn theme_source_metadata_keeps_alias_and_slot_identity() {
    for scope in [
        SourceScope::User,
        SourceScope::Project,
        SourceScope::Temporary,
    ] {
        for origin in [SourceOrigin::Package, SourceOrigin::TopLevel] {
            for base in [None, Some("/base")] {
                let first = record(scope, origin, base);
                let options = ThemeOptions {
                    source_info: Some(Rc::clone(&first)),
                    ..ThemeOptions::default()
                };
                let theme = Theme::new([], [], ColorMode::Truecolor, options).unwrap();
                assert!(Rc::ptr_eq(&theme.source_info().unwrap(), &first));
                first.borrow_mut().path = "edited".into();
                assert_eq!(theme.source_info().unwrap().borrow().path, "edited");
                assert_eq!(
                    theme.source_info().unwrap().borrow().base_dir.as_deref(),
                    base
                );

                let second = record(scope, origin, base);
                let weak_first = Rc::downgrade(&first);
                theme.set_source_info(Some(Rc::clone(&second)));
                assert!(Rc::ptr_eq(&theme.source_info().unwrap(), &second));
                assert_eq!(first.borrow().path, "edited");
                drop(first);
                assert!(weak_first.upgrade().is_none());

                theme.set_source_info(None);
                assert!(theme.source_info().is_none());
                assert_eq!(second.borrow().path, "original");
            }
        }
    }
    let bare = Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap();
    assert!(bare.source_info().is_none());
}

#[test]
fn theme_registration_replaces_without_reordering() {
    let scratch = Scratch::new("registration-order");
    let ops = Real::new(&[]);
    let mut state = make(&ops, &scratch.path("missing"));
    let decomposed = registered(Some("e\u{301}"), Some("second"));
    let replacement = registered(Some("é"), Some("replacement"));
    state.set_registered_themes(vec![
        registered(Some("é"), Some("first")),
        Rc::clone(&decomposed),
        registered(None, Some("nameless")),
        registered(Some(""), Some("empty")),
        Rc::clone(&replacement),
    ]);
    let listed = state.get_available_themes_with_paths().unwrap();
    let dark = format!("{SHIPPED}/dark.json");
    let light = format!("{SHIPPED}/light.json");
    assert_eq!(
        names(&listed),
        [
            ("dark", Some(dark.as_str())),
            ("é", Some("replacement")),
            ("e\u{301}", Some("second")),
            ("light", Some(light.as_str())),
        ]
    );
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("é").unwrap(),
        &replacement
    ));
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("e\u{301}").unwrap(),
        &decomposed
    ));

    state.set_registered_themes(vec![registered(Some("other"), None)]);
    assert!(state.get_theme_by_name("é").is_none());
    assert_eq!(
        state.get_available_themes().unwrap(),
        ["dark", "light", "other"]
    );
}

#[test]
fn theme_registration_clear_preserves_retained_instances() {
    let ops = Real::new(&[]);
    let mut state = make(&ops, "/nonexistent-custom");
    let info = record(SourceScope::User, SourceOrigin::TopLevel, None);
    let options = ThemeOptions {
        name: Some("mine".into()),
        source_info: Some(Rc::clone(&info)),
        ..ThemeOptions::default()
    };
    let theme = Rc::new(Theme::new([], [], ColorMode::Truecolor, options).unwrap());
    let weak = Rc::downgrade(&theme);
    state.set_registered_themes(vec![Rc::clone(&theme)]);
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("mine").unwrap(),
        &theme
    ));

    state.set_registered_themes(vec![]);
    assert!(state.get_theme_by_name("mine").is_none());
    assert_eq!(theme.name(), Some("mine"));
    assert!(Rc::ptr_eq(&theme.source_info().unwrap(), &info));
    drop(theme);
    assert!(weak.upgrade().is_none());
}

#[test]
fn theme_registered_lookup_avoids_unneeded_effects() {
    let ops = Rc::new(Virtual::default());
    let mut state = ThemeState::new(
        dirs("/themes", "/custom"),
        Rc::clone(&ops) as Rc<dyn ThemeOperations>,
    );
    let shadow = registered(Some("dark"), None);
    state.set_registered_themes(vec![Rc::clone(&shadow)]);
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("dark").unwrap(),
        &shadow
    ));
    assert!(ops.log.borrow().is_empty());
}

#[test]
fn maestro_theme_resolves_registered_and_file_precedence() {
    let scratch = Scratch::new("precedence");
    scratch.write("custom/dark.json", &theme_text("custom-dark", "#123456"));
    let ops = Real::new(&[("COLORTERM", "truecolor")]);
    let mut state = make(&ops, &scratch.path("custom"));
    let shadow = Rc::new(
        Theme::new(
            [(ThemeColor::Accent, maestro_theme::ColorValue::Index(24))],
            [],
            ColorMode::Truecolor,
            ThemeOptions {
                name: Some("dark".into()),
                ..ThemeOptions::default()
            },
        )
        .unwrap(),
    );
    state.set_registered_themes(vec![Rc::clone(&shadow)]);
    let found = state.get_theme_by_name("dark").unwrap();
    assert!(Rc::ptr_eq(&found, &shadow));
    assert_eq!(accent(&found), "\x1b[38;5;24m");

    state.set_registered_themes(vec![]);
    let shipped = state.get_theme_by_name("dark").unwrap();
    assert!(!Rc::ptr_eq(&shipped, &shadow));
    assert_eq!(accent(&shipped), TRUECOLOR_ACCENT);
    assert_eq!(shipped.name(), Some("dark"));
}

#[test]
fn theme_builtin_reads_are_lazy_and_retryable() {
    let scratch = Scratch::new("lazy");
    let ops = Real::new(&[("COLORTERM", "truecolor")]);
    let themes = ThemeDirectories {
        themes_dir: scratch.path("themes"),
        custom_themes_dir: scratch.path("custom"),
    };
    let state = ThemeState::new(themes, Rc::clone(&ops) as Rc<dyn ThemeOperations>);
    assert!(ops.log.borrow().is_empty());

    let dark = scratch.path("themes/dark.json");
    let light = scratch.path("themes/light.json");
    assert!(state.get_theme_by_name("dark").is_none());
    assert_eq!(ops.reads(), std::slice::from_ref(&dark));

    scratch.write("themes/dark.json", &theme_text("dark", "#111111"));
    scratch.write("themes/light.json", "{");
    assert!(state.get_theme_by_name("dark").is_none());
    assert_eq!(ops.reads(), [dark.clone(), dark.clone(), light.clone()]);

    scratch.write("themes/dark.json", &theme_text("dark", "#445566"));
    scratch.write("themes/light.json", &theme_text("light", "#778899"));
    let repaired = state.get_theme_by_name("dark").unwrap();
    assert_eq!(accent(&repaired), "\x1b[38;2;68;85;102m");
    assert_eq!(ops.reads()[3..], [dark, light]);
}

#[test]
fn theme_builtin_cache_keeps_data_not_instances() {
    let scratch = Scratch::new("cache");
    scratch.write("themes/dark.json", &theme_text("dark", "#445566"));
    scratch.write("themes/light.json", &theme_text("light", "#778899"));
    let ops = Real::new(&[("COLORTERM", "truecolor")]);
    let themes = ThemeDirectories {
        themes_dir: scratch.path("themes"),
        custom_themes_dir: scratch.path("custom"),
    };
    let state = ThemeState::new(themes, Rc::clone(&ops) as Rc<dyn ThemeOperations>);
    let first = state.get_theme_by_name("dark").unwrap();
    assert_eq!(accent(&first), "\x1b[38;2;68;85;102m");

    fs::remove_file(scratch.path("themes/dark.json")).unwrap();
    scratch.write("themes/light.json", "changed");
    ops.env
        .replace(HashMap::from([("TERM".to_owned(), "linux".to_owned())]));
    let second = state.get_theme_by_name("dark").unwrap();
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(second.get_color_mode(), ColorMode::Color256);
    assert_eq!(accent(&second), "\x1b[38;5;59m");
    assert_eq!(first.get_color_mode(), ColorMode::Truecolor);
    assert_eq!(accent(&first), "\x1b[38;2;68;85;102m");
}

#[test]
fn theme_lists_json_names_with_ordinal_order() {
    let scratch = Scratch::new("ordinal");
    for file in [
        "z.json",
        "A.json",
        "a.json",
        ".json",
        "ignored.JSON",
        "ignore.json.bak",
        "é.json",
        "e\u{301}.json",
        "\u{10000}.json",
        "\u{E000}.json",
        "dark.json",
        "custom-only.json",
    ] {
        scratch.write(&format!("custom/{file}"), "not json");
    }
    fs::create_dir(scratch.path("custom/directory.json")).unwrap();
    let ops = Real::new(&[]);
    let mut state = make(&ops, &scratch.path("custom"));
    state.set_registered_themes(vec![
        registered(Some("z"), None),
        registered(Some("registered-only"), None),
    ]);
    assert_eq!(
        state.get_available_themes().unwrap(),
        [
            "",
            "A",
            "a",
            "custom-only",
            "dark",
            "directory",
            "e\u{301}",
            "light",
            "registered-only",
            "z",
            "é",
            "\u{10000}",
            "\u{E000}",
        ]
    );
}

#[test]
fn theme_path_inventory_keeps_first_owner_and_locale_order() {
    let scratch = Scratch::new("owners");
    for file in [
        "a.json",
        "A.json",
        "dark.json",
        "z.json",
        "custom-only.json",
    ] {
        scratch.write(&format!("custom/{file}"), "not json");
    }
    let custom = scratch.path("custom");
    let ops = Real::new(&[]);
    let mut native = make(&ops, &custom);
    native.set_registered_themes(vec![
        registered(Some("z"), Some("registered-z")),
        registered(Some("reg-none"), None),
        registered(Some("reg-empty"), Some("")),
    ]);
    let listed = native.get_available_themes_with_paths().unwrap();
    let (dark, light) = (
        format!("{SHIPPED}/dark.json"),
        format!("{SHIPPED}/light.json"),
    );
    let (a, big_a) = (format!("{custom}/a.json"), format!("{custom}/A.json"));
    let (only, z) = (
        format!("{custom}/custom-only.json"),
        format!("{custom}/z.json"),
    );
    assert_eq!(
        names(&listed),
        [
            ("a", Some(a.as_str())),
            ("A", Some(big_a.as_str())),
            ("custom-only", Some(only.as_str())),
            ("dark", Some(dark.as_str())),
            ("light", Some(light.as_str())),
            ("reg-empty", Some("")),
            ("reg-none", None),
            ("z", Some(z.as_str())),
        ]
    );
}

#[test]
fn theme_path_inventory_sorts_with_the_supplied_operation() {
    let mut ops = Virtual::shipped();
    ops.existing.insert("/custom".into());
    ops.dirs
        .insert("/custom".into(), vec!["dark.json".into(), "x.json".into()]);
    let ops = Rc::new(ops);
    let mut controlled = ThemeState::new(
        dirs("/themes", "/custom"),
        Rc::clone(&ops) as Rc<dyn ThemeOperations>,
    );
    controlled.set_registered_themes(vec![
        registered(Some("x"), Some("ignored")),
        registered(Some("y"), Some("why")),
    ]);
    let listed = controlled.get_available_themes_with_paths().unwrap();
    assert_eq!(
        names(&listed),
        [
            ("y", Some("why")),
            ("x", Some("/custom/x.json")),
            ("light", Some("/themes/light.json")),
            ("dark", Some("/themes/dark.json")),
        ]
    );
}

#[test]
fn theme_inventory_handles_absent_custom_directory() {
    let ops = Rc::new(Virtual::shipped());
    let mut state = ThemeState::new(
        dirs("/themes", "/custom"),
        Rc::clone(&ops) as Rc<dyn ThemeOperations>,
    );
    state.set_registered_themes(vec![registered(Some("z"), Some("registered-z"))]);
    assert_eq!(
        state.get_available_themes().unwrap(),
        ["dark", "light", "z"]
    );
    let listed = state.get_available_themes_with_paths().unwrap();
    assert_eq!(
        names(&listed),
        [
            ("z", Some("registered-z")),
            ("light", Some("/themes/light.json")),
            ("dark", Some("/themes/dark.json"))
        ]
    );
    assert!(
        !ops.log
            .borrow()
            .iter()
            .any(|entry| entry.starts_with("read_dir:"))
    );
}

#[test]
fn theme_discovery_propagates_loading_failures() {
    let missing = Rc::new(Virtual::default());
    let state = ThemeState::new(
        dirs("/themes", "/custom"),
        Rc::clone(&missing) as Rc<dyn ThemeOperations>,
    );
    for error in [
        state.get_available_themes().unwrap_err(),
        state.get_available_themes_with_paths().unwrap_err(),
    ] {
        assert_eq!(io_kind(&error), Some(io::ErrorKind::NotFound));
    }

    let mut malformed = Virtual::shipped();
    malformed
        .files
        .insert("/themes/light.json".into(), "{".into());
    let malformed = Rc::new(malformed);
    let state = ThemeState::new(
        dirs("/themes", "/custom"),
        Rc::clone(&malformed) as Rc<dyn ThemeOperations>,
    );
    let error = state.get_available_themes().unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("Failed to parse theme /themes/light.json: ")
    );

    let scratch = Scratch::new("not-directory");
    scratch.write("custom", "a file");
    let ops = Real::new(&[]);
    let state = make(&ops, &scratch.path("custom"));
    for error in [
        state.get_available_themes().unwrap_err(),
        state.get_available_themes_with_paths().unwrap_err(),
    ] {
        assert_eq!(io_kind(&error), Some(io::ErrorKind::NotADirectory));
    }
}

#[test]
fn theme_custom_lookup_rereads_without_attaching_a_path() {
    let scratch = Scratch::new("reread");
    scratch.write("custom/mine.json", &theme_text("authored-name", "#ABCDEF"));
    let ops = Real::new(&[("COLORTERM", "truecolor")]);
    let state = make(&ops, &scratch.path("custom"));
    let first = state.get_theme_by_name("mine").unwrap();
    assert_eq!(first.name(), Some("authored-name"));
    assert_eq!(first.source_path(), None);
    assert_eq!(accent(&first), "\x1b[38;2;171;205;239m");

    scratch.write("custom/mine.json", &theme_text("changed-name", "#102030"));
    let second = state.get_theme_by_name("mine").unwrap();
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(second.name(), Some("changed-name"));
    assert_eq!(second.source_path(), None);
    assert_eq!(accent(&second), "\x1b[38;2;16;32;48m");

    let path = scratch.path("custom/mine.json");
    let direct = load_theme_from_path(&path, Some(ColorMode::Truecolor), &*ops).unwrap();
    assert_eq!(direct.source_path(), Some(path.as_str()));
}

#[test]
fn theme_lookup_suppresses_load_failures() {
    let scratch = Scratch::new("suppress");
    fs::create_dir_all(scratch.path("custom/directory.json")).unwrap();
    scratch.write("custom/malformed.json", "{");
    let mut schema = shipped_dark();
    schema.as_object_mut().unwrap().remove("colors");
    scratch.write("custom/schema.json", &schema.to_string());
    let mut alias = shipped_dark();
    alias["colors"]["accent"] = "no-such-variable".into();
    scratch.write("custom/alias.json", &alias.to_string());
    scratch.write("custom/hex.json", &theme_text("hex", "#12"));

    let ops = Real::new(&[]);
    let state = make(&ops, &scratch.path("custom"));
    for name in ["missing", "directory", "malformed", "schema"] {
        assert!(state.get_theme_by_name(name).is_none(), "{name}");
    }
    assert_eq!(ops.env_reads(), 0);
    for name in ["alias", "hex"] {
        assert!(state.get_theme_by_name(name).is_none(), "{name}");
    }

    let mut ops = Virtual::shipped();
    ops.files
        .insert("/custom/ghost.json".into(), theme_text("ghost", "#123456"));
    let ops = Rc::new(ops);
    let state = ThemeState::new(
        dirs("/themes", "/custom"),
        Rc::clone(&ops) as Rc<dyn ThemeOperations>,
    );
    assert!(state.get_theme_by_name("ghost").is_none());
    assert!(
        !ops.log
            .borrow()
            .contains(&"read:/custom/ghost.json".to_owned())
    );

    let light = Scratch::new("bad-light");
    light.write("themes/dark.json", &theme_text("dark", "#111111"));
    light.write("themes/light.json", "{");
    let broken = ThemeState::new(
        ThemeDirectories {
            themes_dir: light.path("themes"),
            custom_themes_dir: light.path("custom"),
        },
        Real::new(&[]),
    );
    assert!(broken.get_theme_by_name("dark").is_none());
}

#[test]
fn theme_lookup_joins_authored_names_without_trimming() {
    let scratch = Scratch::new("authored");
    scratch.write("user/.json", &theme_text("empty-request", "#000001"));
    scratch.write("user/ spaced .json", &theme_text("  stored  ", "#000002"));
    scratch.write("user/normalized.json", &theme_text("inside", "#000003"));
    scratch.write("custom/normalized.json", &theme_text("outside", "#000004"));
    let ops = Real::new(&[("COLORTERM", "truecolor")]);
    let state = make(&ops, &scratch.path("user"));
    let name_of = |requested: &str| {
        state
            .get_theme_by_name(requested)
            .unwrap()
            .name()
            .map(str::to_owned)
    };
    assert_eq!(name_of("").as_deref(), Some("empty-request"));
    assert_eq!(name_of(" spaced ").as_deref(), Some("  stored  "));
    assert_eq!(name_of("./normalized").as_deref(), Some("inside"));
    assert_eq!(name_of("../custom/normalized").as_deref(), Some("outside"));
}
