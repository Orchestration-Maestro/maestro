//! Content-package integrity through native package resolution and skill loading.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::{scratch::Scratch, support};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use maestro_resources::{LoadSkillsOptions, NativeResourceOperations, load_skills};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

/// Complete skill inventory with human-only invocation flags.
const EXPECTED_SKILLS: [(&str, bool); 37] = [
    ("maestro-ask", true),
    ("maestro-code-review", false),
    ("maestro-codebase-design", false),
    ("maestro-diagnose", false),
    ("maestro-documentation", false),
    ("maestro-domain-modeling", false),
    ("maestro-frontend-ui", false),
    ("maestro-git-workflow", false),
    ("maestro-grilling", false),
    ("maestro-handoff", true),
    ("maestro-humanizer", false),
    ("maestro-idea-refine", true),
    ("maestro-implement", true),
    ("maestro-implement-spec", true),
    ("maestro-improve-codebase-architecture", true),
    ("maestro-observability", false),
    ("maestro-performance", false),
    ("maestro-pr", false),
    ("maestro-prototype", false),
    ("maestro-quality-gates", false),
    ("maestro-receive-review", false),
    ("maestro-research", false),
    ("maestro-retro", true),
    ("maestro-security", false),
    ("maestro-ship", false),
    ("maestro-show-options", false),
    ("maestro-tdd", false),
    ("maestro-teach", true),
    ("maestro-to-questionnaire", true),
    ("maestro-to-spec", true),
    ("maestro-to-tickets", true),
    ("maestro-triage", true),
    ("maestro-verify", false),
    ("maestro-wait-what", true),
    ("maestro-wayfinder", true),
    ("maestro-wizard", false),
    ("maestro-writing-for-agents", false),
];

/// Resolve the checkout package before loading its enabled resources.
#[test]
fn skill_pack_loads_with_closed_links_and_router() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/maestro-skills");
    let scratch = Scratch::new().unwrap();
    let root = scratch.0.to_str().unwrap();
    let paths = resolve_paths(&package, root);
    let loaded = load_skills(
        LoadSkillsOptions {
            cwd: root,
            home: root,
            agent_dir: root,
            config_dir_name: ".maestro",
            skill_paths: &paths,
            include_defaults: false,
        },
        &NativeResourceOperations,
    )
    .unwrap();
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let names: BTreeSet<_> = loaded.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(loaded.skills.len(), EXPECTED_SKILLS.len());
    assert_eq!(names.len(), loaded.skills.len(), "duplicate skill name");
    assert_eq!(
        loaded
            .skills
            .iter()
            .filter(|s| s.disable_model_invocation)
            .count(),
        14
    );
    assert_eq!(loaded.skills.len() - 14, 23);
    for (name, type_only) in EXPECTED_SKILLS {
        let skill = loaded.skills.iter().find(|s| s.name == name).unwrap();
        assert_eq!(skill.disable_model_invocation, type_only, "{name}");
        assert!(skill.description.starts_with("Use when"), "{name}");
        assert!(skill.description.chars().count() < 500, "{name}");
    }
    assert!(names.iter().all(|name| name.starts_with("maestro-")));
    let entries: BTreeSet<_> = fs::read_dir(package.join("skills"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, entries.iter().map(String::as_str).collect());
    check_files(&package, &names);
    check_controls(&package, &scratch.0, &names);
}

/// Resolve only local skill resources while observing acquisition and spawning.
fn resolve_paths(package: &Path, root: &str) -> Vec<String> {
    let (unused, _, mut effects) = support::manager(&serde_json::json!({}));
    drop(unused);
    Rc::get_mut(&mut effects).unwrap().native_exists = true;
    let settings = Rc::new(std::cell::RefCell::new(
        maestro_settings::SettingsManager::create(root, root, ".maestro"),
    ));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: root.into(),
            agent_dir: root.into(),
            settings_manager: settings,
        },
        support::Controlled(Rc::clone(&effects)),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let resolved = runtime
        .block_on(manager.resolve_extension_sources(&[package.to_str().unwrap().into()], None))
        .unwrap();
    assert!(resolved.extensions.is_empty());
    assert!(resolved.prompts.is_empty());
    assert!(resolved.themes.is_empty());
    assert!(!resolved.skills.is_empty(), "package must resolve skills");
    assert!(resolved.skills.iter().all(|s| s.enabled));
    assert!(effects.calls.borrow().is_empty());
    assert_eq!(effects.spawns.get(), 0);
    resolved
        .skills
        .into_iter()
        .filter(|s| s.enabled)
        .map(|s| s.path)
        .collect()
}

/// Collect the authored package once, without following directory symlinks.
fn markdown_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            markdown_files(&entry.path(), files);
        } else if entry.path().extension().is_some_and(|s| s == "md") {
            files.push(entry.path());
        }
    }
}

/// Read concrete invocation names, stopping before punctuation or arguments.
fn targets(text: &str) -> BTreeSet<&str> {
    text.split("/skill:")
        .skip(1)
        .map(|tail| {
            tail.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .next()
                .unwrap()
        })
        .collect()
}

/// Recognize only the package's ordinary inline Markdown file links.
fn internal_link(package: &Path, from: &Path, target: &str) -> bool {
    if target.starts_with('#') || target.contains("://") || target.starts_with("mailto:") {
        return true;
    }
    let file = target.split('#').next().unwrap();
    from.parent()
        .unwrap()
        .join(file)
        .canonicalize()
        .is_ok_and(|path| path.is_file() && path.starts_with(package.canonicalize().unwrap()))
}

/// A skill document belongs to a loaded directory, never the loose skills root.
fn owned_document(relative: &Path, names: &BTreeSet<&str>) -> bool {
    let mut parts = relative.iter();
    if parts.next().is_none_or(|part| part != "skills") {
        return true;
    }
    parts
        .next()
        .and_then(|p| p.to_str())
        .is_some_and(|name| names.contains(name))
        && parts.next().is_some()
}

/// Check each collected file's pointers, inline links and skill ownership.
fn check_files(package: &Path, names: &BTreeSet<&str>) {
    let mut files = Vec::new();
    markdown_files(package, &mut files);
    assert!(!files.is_empty());
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        let file_targets = targets(&text);
        if file == package.join("skills/maestro-ask/SKILL.md") {
            assert_eq!(
                file_targets,
                names
                    .iter()
                    .copied()
                    .filter(|s| *s != "maestro-ask")
                    .collect()
            );
        }
        for name in file_targets {
            assert!(
                names.contains(name),
                "{}: unknown skill {name}",
                file.display()
            );
        }
        for tail in text.split("](").skip(1) {
            let target = tail.split(')').next().unwrap();
            assert!(
                internal_link(package, &file, target),
                "{}: invalid link {target}",
                file.display()
            );
        }
        assert!(
            owned_document(file.strip_prefix(package).unwrap(), names),
            "{}: loose document",
            file.display()
        );
    }
}

/// Positive and negative controls distinguish valid content from broken admission.
fn check_controls(package: &Path, scratch: &Path, names: &BTreeSet<&str>) {
    assert_eq!(
        targets("/skill:maestro-ask, /skill:maestro-ask args."),
        BTreeSet::from(["maestro-ask"])
    );
    assert!(targets("/skill:maestro-ask").is_subset(names));
    assert!(!targets("/skill:maestro-missing").is_subset(names));
    assert!(targets("/lean /focus /lean-review /lean-audit security-audit").is_empty());
    let from = package.join("skills/maestro-ask/SKILL.md");
    assert!(internal_link(package, &from, "SKILL.md"));
    assert!(internal_link(package, &from, "SKILL.md#flow"));
    assert!(!internal_link(package, &from, "missing.md"));
    let outside = scratch.join("outside.md");
    fs::write(&outside, "outside").unwrap();
    assert!(!internal_link(package, &from, outside.to_str().unwrap()));
    assert!(!internal_link(package, &from, "../../../../Cargo.toml"));
    assert!(internal_link(package, &from, "https://example.com/missing"));
    assert!(internal_link(package, &from, "#flow"));
    assert!(owned_document(
        Path::new("skills/maestro-ask/support.md"),
        names
    ));
    assert!(!owned_document(Path::new("skills/loose.md"), names));
    #[cfg(unix)]
    {
        let control = scratch.join("control");
        fs::create_dir(&control).unwrap();
        std::os::unix::fs::symlink(&outside, control.join("escape.md")).unwrap();
        assert!(!internal_link(
            &control,
            &control.join("from.md"),
            "escape.md"
        ));
    }
}
