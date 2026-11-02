//! Recorded cache recipes replayed through the public resolver.
#![cfg(test)]
use super::{native_support, resource_output, support};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions, SourceScope};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, io, path::Path, rc::Rc};
/// An independently recorded input and complete observable result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recorded {
    /// Recipe name.
    case: String,
    /// Initial checkout location, observed independently.
    target: Option<String>,
    /// Input controls.
    recipe: Recipe,
    /// Entire observable tree.
    expected: Expected,
}
/// Returned observations; open JSON trees are compared in their entirety.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    /// First result tree.
    result: Option<Value>,
    /// Delegated error.
    error: Option<Value>,
    /// Subsequent invocation tree.
    second_result: Option<Value>,
    /// Ordered command records.
    commands: Vec<Value>,
    /// Ordered progress records.
    events: Vec<Value>,
    /// All surviving authored files.
    files: BTreeMap<String, String>,
}
/// Input kind chosen by the recipe.
#[derive(Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Npm,
    Git,
    Local,
}
/// Recorded input controls, none silently ignored.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Recipe {
    /// Source kind.
    kind: Kind,
    /// Requested scope.
    scope: SourceScope,
    /// Existing package setup.
    existing: Option<bool>,
    /// Existing cache-root setup.
    existing_root: Option<bool>,
    /// No files created by successful acquisition.
    no_contents: Option<bool>,
    /// Malformed acquired resources.
    bad_resources: Option<bool>,
    /// Updated upstream head.
    changed: Option<bool>,
    /// Append another missing source.
    second: Option<bool>,
    /// Repeat on same manager.
    twice: Option<bool>,
    /// Replace manager after first call.
    new_manager: Option<bool>,
    /// Root lookup failure.
    root_failure: Option<bool>,
    /// Authored npm spec.
    spec: Option<String>,
    /// Authored repository URL.
    git_source: Option<String>,
    /// Selected Git ref.
    #[serde(rename = "ref")]
    reference: Option<String>,
    /// Second manager's ref.
    second_manager_ref: Option<String>,
    /// Installed version.
    version: Option<String>,
    /// Version produced by installation.
    post_version: Option<String>,
    /// Existing raw manifest text.
    manifest_text: Option<String>,
    /// Missing or directory manifest setup.
    manifest_mode: Option<String>,
    /// Selected command failure.
    fail: Option<String>,
    /// Progress failure phase.
    callback_failure: Option<String>,
    /// Initial command configuration.
    npm_command: Option<Vec<String>>,
    /// Live configuration after cloning.
    npm_command_after_clone: Option<Vec<String>>,
    /// Initial offline value.
    offline: Option<String>,
    /// Startup offline selection supplied to the manager.
    selection: Option<bool>,
    /// Explicit source sequence, including an empty sequence.
    multiple: Option<Vec<String>>,
}
/// Places a recorded `$ROOT`-anchored path in `base`, joining its remainder by components.
fn recorded(base: &str, recorded: &str) -> String {
    native_support::under(base, recorded.trim_start_matches("$ROOT"))
        .to_string_lossy()
        .into_owned()
}
/// Creates package contents with distinct before/after markers.
fn package(target: &str, version: &str, marker: &str, bad: bool) {
    let manifest = if bad {
        json!({"maestro":{"extensions":[7]}})
    } else {
        json!({"version":version,"maestro":{"extensions":["entry.ts"],"skills":["skill.md"],"prompts":["prompt.md"],"themes":["theme.json"]}})
    };
    put(target, "package.json", &manifest.to_string());
    for name in ["entry.ts", "skill.md", "prompt.md", "theme.json"] {
        put(target, name, marker);
    }
}
/// Creates a recipe file without executing it.
fn put(root: &str, name: &str, text: &str) {
    let path = Path::new(root).join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
/// Source operands independent of expected observations.
fn source(recipe: &Recipe) -> String {
    match recipe.kind {
        Kind::Npm => format!("npm:{}", recipe.spec.as_deref().unwrap_or("pkg")),
        Kind::Git => recipe.git_source.clone().unwrap_or_else(|| {
            format!(
                "https://github.com/team/repo{}",
                recipe
                    .reference
                    .as_ref()
                    .map_or(String::new(), |r| format!("@{r}"))
            )
        }),
        Kind::Local => "./local.ts".into(),
    }
}
/// Collects surviving files in stable comparison order.
fn files(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    if !dir.exists() {
        return;
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(root, &path, out);
        } else {
            out.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                std::fs::read_to_string(path).unwrap(),
            );
        }
    }
}
/// Replaces only the owning disposable root in observations, then folds native separators.
fn normalize(value: Value, root: &str) -> Value {
    match value {
        Value::String(s) => s.replace(root, "$ROOT").replace('\\', "/").into(),
        Value::Array(a) => a.into_iter().map(|v| normalize(v, root)).collect(),
        Value::Object(o) => o
            .into_iter()
            .map(|(k, v)| (k, normalize(v, root)))
            .collect(),
        v => v,
    }
}
/// Serializes every returned resource field without dropping duplicates.
fn result(value: maestro_packages::ResolvedPaths, root: &str) -> Value {
    normalize(resource_output::output(value, "\0"), root)
}
/// Builds another manager retaining the same effect/configuration owners.
fn manager(
    base: &str,
    cwd: &str,
    settings: Rc<RefCell<maestro_settings::SettingsManager>>,
    effects: &Rc<support::Effects>,
    selection: Option<bool>,
) -> DefaultPackageManager<support::Controlled> {
    DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: cwd.into(),
            agent_dir: native_support::under(base, "user")
                .to_string_lossy()
                .into_owned(),
            settings_manager: settings,
        },
        support::Controlled(Rc::clone(effects)),
    )
    .with_offline(selection)
}
/// Replays each selected recipe exactly once.
pub async fn cases(names: &[&str]) {
    let all: Vec<Recorded> =
        serde_json::from_str(include_str!("../fixtures/cache_resolution.json")).unwrap();
    for name in names {
        let case = all.iter().find(|c| c.case == *name).unwrap();
        replay(case).await;
    }
}
/// Sets up only initial inputs before invoking the public resolver.
fn setup(case: &Recorded, base: &str) {
    let r = &case.recipe;
    if r.kind == Kind::Local {
        put(base, "local.ts", "fixture");
    }
    if r.existing == Some(true) {
        let target = recorded(base, case.target.as_ref().unwrap());
        package(
            &target,
            r.version.as_deref().unwrap_or("1.0.0"),
            "old",
            false,
        );
        if let Some(text) = &r.manifest_text {
            put(&target, "package.json", text);
        }
        if let Some(mode) = &r.manifest_mode {
            let path = Path::new(&target).join("package.json");
            std::fs::remove_file(&path).unwrap();
            if mode == "directory" {
                std::fs::create_dir(path).unwrap();
            } else {
                assert_eq!(mode, "missing");
            }
        }
    }
    if r.existing_root == Some(true) {
        let root = native_support::under(base, "temp/maestro-extensions/npm/f35b2129")
            .to_string_lossy()
            .into_owned();
        put(&root, ".gitignore", "keep-ignore\n");
        put(&root, "package.json", "{\"keep\":true}\n");
    }
}
/// Installs stateful command effects into the existing controlled adapter.
fn hooks(
    recipe: Rc<Recipe>,
    base: &str,
    effects: &Rc<support::Effects>,
    settings: &Rc<RefCell<maestro_settings::SettingsManager>>,
) {
    let settings = Rc::clone(settings);
    let root = base.to_owned();
    let r = Rc::clone(&recipe);
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        let outcome = run(&r, &root, &settings, (command, args, cwd));
        Box::pin(async move { outcome.map(|()| Some(0)) })
    }));
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |_, args, _| {
        let r = &recipe;
        let result = if r.fail.as_deref() == Some("upstream") {
            Err(io::Error::other("probe failed"))
        } else if r.fail.as_deref() == Some("head") && args == ["rev-parse", "HEAD"] {
            Err(io::Error::other("head failed"))
        } else if args == ["rev-parse", "--symbolic-full-name", "HEAD"] {
            Ok("refs/heads/main".into())
        } else if args.iter().any(|a| a == "--abbrev-ref") {
            Ok("origin/main\n".into())
        } else if args == ["rev-parse", "HEAD"] || r.changed != Some(true) {
            Ok("old\n".into())
        } else {
            Ok("new\n".into())
        };
        Box::pin(async move { result })
    }));
}
/// Applies completed child effects, respecting the admitted argv and cwd.
fn run(
    r: &Recipe,
    root: &str,
    settings: &RefCell<maestro_settings::SettingsManager>,
    call: (&str, &[String], Option<&str>),
) -> io::Result<()> {
    let (command, args, cwd) = call;
    if r.fail.as_deref() == Some("install")
        && (args[0] == "clone" || args.iter().any(|a| a == "install"))
    {
        return Err(io::Error::other("acquisition failed"));
    }
    if r.fail.as_deref() == Some(args[0].as_str()) {
        return Err(io::Error::other(format!("{} failed", args[0])));
    }
    if args[0] == "clone" {
        if r.no_contents != Some(true) {
            package(
                &args[2],
                r.post_version.as_deref().unwrap_or("1.0.0"),
                "new",
                false,
            );
        }
        if let Some(value) = &r.npm_command_after_clone {
            settings.borrow_mut().set_npm_command(Some(
                value
                    .iter()
                    .cloned()
                    .map(maestro_settings::SettingsListEntry::String)
                    .collect(),
            ));
        }
    } else if command == "git" && args[0] == "reset" {
        package(
            cwd.unwrap(),
            r.post_version.as_deref().unwrap_or("1.0.0"),
            if args[2] == "old" { "old" } else { "refreshed" },
            false,
        );
    } else if let Some(index) = args.iter().position(|a| a == "install") {
        if cwd.is_some() {
            if r.fail.as_deref() == Some("dependencies") {
                return Err(io::Error::other("dependencies failed"));
            }
        } else {
            install(r, root, args, index);
        }
    }
    Ok(())
}
/// Produces npm files at the command-selected prefix, never an expected output path.
fn install(r: &Recipe, root: &str, args: &[String], index: usize) {
    let spec = &args[index + 1 + usize::from(args[index + 1] == "-g")];
    let offset = usize::from(spec.starts_with('@'));
    let suffix = spec[offset..].find('@').map(|i| i + offset).filter(|&i| {
        !spec[i + 1..].is_empty() && !spec[i + 1..].contains(['\n', '\r', '\u{2028}', '\u{2029}'])
    });
    let name = suffix.map_or(spec.as_str(), |i| &spec[..i]);
    let version = suffix.map_or("1.0.0", |i| &spec[i + 1..]);
    let prefix = args.iter().position(|a| a == "--prefix").map_or_else(
        || {
            native_support::under(root, "global")
                .to_string_lossy()
                .into_owned()
        },
        |i| args[i + 1].clone(),
    );
    let target = maestro_path::join(&[&prefix, "node_modules", name]);
    if r.manifest_mode.as_deref() == Some("directory") {
        std::fs::remove_dir(Path::new(&target).join("package.json")).unwrap();
    }
    if r.no_contents != Some(true) {
        package(
            &target,
            r.post_version.as_deref().unwrap_or(version),
            "new",
            r.bad_resources == Some(true),
        );
    }
}
/// The child's `HOME`, inside its own tree.
pub fn home(base: &str) -> (&'static str, Option<String>) {
    (
        "HOME",
        Some(
            native_support::under(base, "home")
                .to_string_lossy()
                .into_owned(),
        ),
    )
}
/// Compares ordered effects, surviving files, errors and complete returned trees.
async fn replay(case: &Recorded) {
    let Some(tree) = super::resolution_support::case_in_child(&case.case, |base| {
        vec![home(base), ("MAESTRO_OFFLINE", case.recipe.offline.clone())]
    }) else {
        return;
    };
    let base = tree.to_string_lossy().into_owned();
    setup(case, &base);
    let (_, settings, _) = support::manager(&json!({}));
    let r = Rc::new(case.recipe.clone());
    replay_ready(case, &base, r, settings).await;
}
/// Initializes injected controls and hooks before command admission.
fn configure(
    r: &Rc<Recipe>,
    base: &str,
    effects: &Rc<support::Effects>,
    settings: &Rc<RefCell<maestro_settings::SettingsManager>>,
) {
    if let Some(value) = &r.npm_command {
        settings.borrow_mut().set_npm_command(Some(
            value
                .iter()
                .cloned()
                .map(maestro_settings::SettingsListEntry::String)
                .collect(),
        ));
    }
    effects
        .outputs
        .borrow_mut()
        .push_back(if r.root_failure == Some(true) {
            Err(io::Error::other("root lookup failed"))
        } else {
            Ok(support::output(
                &native_support::under(base, "global/node_modules").to_string_lossy(),
                "",
                Some(0),
            ))
        });
    hooks(Rc::clone(r), base, effects, settings);
}
/// Drives first and optional subsequent invocations against the same retained files.
async fn replay_ready(
    case: &Recorded,
    base: &str,
    r: Rc<Recipe>,
    settings: Rc<RefCell<maestro_settings::SettingsManager>>,
) {
    let effects = Rc::new(support::Effects::default());
    configure(&r, base, &effects, &settings);
    let events = Rc::new(RefCell::new(Vec::new()));
    let m = manager(base, base, Rc::clone(&settings), &effects, r.selection);
    progress(&m, &r, &events);
    let mut sources = r.multiple.clone().unwrap_or_else(|| vec![source(&r)]);
    if r.second == Some(true) {
        sources.push("npm:second".into());
    }
    let scope = (r.scope != SourceScope::User).then_some(r.scope);
    let first = m.resolve_extension_sources(&sources, scope).await;
    let (actual, error) = match first {
        Ok(v) => (Some(result(v, base)), None),
        Err(e) => (None, Some(json!({"message": e.to_string()}))),
    };
    let mut second = None;
    if error.is_none() {
        if r.twice == Some(true) {
            second = Some(result(
                m.resolve_extension_sources(&sources, Some(r.scope))
                    .await
                    .unwrap(),
                base,
            ));
        }
        if r.new_manager == Some(true) || r.second_manager_ref.is_some() {
            drop(m);
            let next = manager(
                base,
                &native_support::under(base, "other-cwd").to_string_lossy(),
                Rc::clone(&settings),
                &effects,
                r.selection,
            );
            progress(&next, &r, &events);
            let sources = r.second_manager_ref.as_ref().map_or(sources, |reference| {
                vec![format!("https://github.com/team/repo@{reference}")]
            });
            second = Some(result(
                next.resolve_extension_sources(&sources, Some(r.scope))
                    .await
                    .unwrap(),
                base,
            ));
        }
    }
    assert_packages_unedited(&settings);
    compare(case, base, &effects, &events, &(actual, error, second));
}
/// Observes both mutable settings scopes through the caller's surviving owner.
fn assert_packages_unedited(settings: &RefCell<maestro_settings::SettingsManager>) {
    assert!(
        !settings
            .borrow()
            .get_global_settings()
            .0
            .contains_key("packages")
    );
    assert!(
        !settings
            .borrow()
            .get_project_settings()
            .0
            .contains_key("packages")
    );
}
/// Records every progress payload before any injected callback failure.
fn progress(
    m: &DefaultPackageManager<support::Controlled>,
    r: &Recipe,
    events: &Rc<RefCell<Vec<Value>>>,
) {
    let phase = r.callback_failure.clone();
    let events = Rc::clone(events);
    m.set_progress_callback(Some(Rc::new(move |e| {
        let kind = match e.r#type {
            maestro_packages::ProgressEventType::Start => "start",
            maestro_packages::ProgressEventType::Complete => "complete",
            maestro_packages::ProgressEventType::Error => "error",
            maestro_packages::ProgressEventType::Progress => "progress",
        };
        let action = match e.action {
            maestro_packages::ProgressAction::Pull => "pull",
            maestro_packages::ProgressAction::Install => "install",
            maestro_packages::ProgressAction::Remove => "remove",
            maestro_packages::ProgressAction::Update => "update",
            maestro_packages::ProgressAction::Clone => "clone",
        };
        let mut event = json!({"type":kind,"action":action,"source":e.source});
        if let Some(message) = &e.message {
            event["message"] = message.clone().into();
        }
        events.borrow_mut().push(event);
        if phase.as_deref() == Some(kind) {
            return Err(io::Error::other(format!("callback {kind} failed")));
        }
        Ok(())
    })));
}
/// Checks the complete observations after resolver completion closes their producers.
fn compare(
    case: &Recorded,
    base: &str,
    effects: &support::Effects,
    events: &RefCell<Vec<Value>>,
    returned: &(Option<Value>, Option<Value>, Option<Value>),
) {
    assert_eq!(returned.0, case.expected.result, "{} result", case.case);
    assert_eq!(returned.1, case.expected.error, "{} error", case.case);
    assert_eq!(
        returned.2, case.expected.second_result,
        "{} second",
        case.case
    );
    let commands = normalize(json!(*effects.transcript.borrow()), base).to_string();
    assert_eq!(
        serde_json::from_str::<Value>(&support::recorded_stages(&commands)).unwrap(),
        json!(case.expected.commands),
        "{} commands",
        case.case
    );
    assert_eq!(
        normalize(json!(*events.borrow()), base),
        json!(case.expected.events),
        "{} events",
        case.case
    );
    let mut actual_files = BTreeMap::new();
    files(Path::new(base), Path::new(base), &mut actual_files);
    assert_eq!(actual_files, case.expected.files, "{} files", case.case);
}
#[cfg(unix)]
pub mod native;
