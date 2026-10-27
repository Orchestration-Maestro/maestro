//! Skill discovery, validation, provenance and prompt behavior.
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_resources::{
    LoadSkillsFromDirOptions, SourceOrigin, SourceScope, load_skills_from_dir,
};
use support::{Directory, strings};

/// A described skill retains complete local provenance.
#[test]
fn maestro_skills_load_valid_metadata_and_source() {
    let dir = Directory::new();
    let file = dir.file(
        "calendar/SKILL.md",
        "---\nname: calendar\ndescription: Schedule meetings\n---\nRead this.",
    );
    let base = file.parent().unwrap();
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(base),
        source: "user",
    });
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.skills.len(), 1);
    let skill = &result.skills[0];
    assert_eq!(skill.name, "calendar");
    assert_eq!(skill.description, "Schedule meetings");
    assert_eq!(skill.file_path, file);
    assert_eq!(skill.base_dir, support::text(base));
    assert!(!skill.disable_model_invocation);
    assert_eq!(skill.source_info.path, file);
    assert_eq!(
        skill.source_info.base_dir.as_deref(),
        Some(support::text(base))
    );
    assert_eq!(skill.source_info.source, "local");
    assert_eq!(skill.source_info.scope, SourceScope::User);
    assert_eq!(skill.source_info.origin, SourceOrigin::TopLevel);
}

/// Parent mismatch warns without omitting the described resource.
#[test]
fn maestro_skills_warn_on_parent_mismatch() {
    let dir = Directory::new();
    let file = dir.file(
        "calendar/SKILL.md",
        "---\nname: other\ndescription: Meetings\n---",
    );
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].message,
        "name \"other\" does not match parent directory \"calendar\""
    );
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(support::text(&file))
    );
    assert_eq!(
        result.diagnostics[0].r#type,
        maestro_resources::DiagnosticType::Warning
    );
    assert!(result.diagnostics[0].collision.is_none());
}

/// Name warnings retain their authored order and count complete characters.
#[test]
fn maestro_skills_keep_name_warning_order() {
    let dir = Directory::new();
    let name = format!("-{}--", "A".repeat(62));
    let file = dir.file(
        "parent/SKILL.md",
        &format!("---\nname: {name}\ndescription: Useful\n---"),
    );
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "custom",
    });
    assert_eq!(result.skills.len(), 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        vec![
            format!("name \"{name}\" does not match parent directory \"parent\""),
            "name exceeds 64 characters (65)".into(),
            "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)".into(),
            "name must not start or end with a hyphen".into(),
            "name must not contain consecutive hyphens".into(),
        ]
    );
    for (name, expected) in [
        ("a0-b".to_string(), 0),
        ("a".repeat(64), 0),
        ("a".repeat(65), 1),
        ("😀".repeat(64), 2),
        ("😀".repeat(65), 3),
        ("a.b".into(), 1),
    ] {
        let parent = if name.contains('😀') {
            "parent"
        } else {
            &name
        };
        let file = dir.file(
            &format!("{parent}/SKILL.md"),
            &format!("---\nname: {name}\ndescription: Useful\n---"),
        );
        let result = load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd: support::text(&std::env::temp_dir()),
            dir: support::text(file.parent().unwrap()),
            source: "path",
        });
        assert_eq!(result.diagnostics.len(), expected, "{name}");
    }
}

/// Missing description warnings precede name warnings before omission.
#[test]
fn maestro_skills_validate_description_before_name() {
    let dir = Directory::new();
    for header in [
        "",
        "---\nname: Other\n---",
        "---\nname: Other\ndescription: '  '\n---",
        "---\nname: Other\ndescription: '\u{feff}'\n---",
    ] {
        let file = dir.file("parent/SKILL.md", header);
        let result = load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd: support::text(&std::env::temp_dir()),
            dir: support::text(file.parent().unwrap()),
            source: "path",
        });
        assert!(result.skills.is_empty());
        assert_eq!(result.diagnostics[0].message, "description is required");
        if header.is_empty() {
            assert_eq!(result.diagnostics.len(), 1);
        } else {
            assert_eq!(
                result
                    .diagnostics
                    .iter()
                    .map(|d| d.message.as_str())
                    .collect::<Vec<_>>(),
                vec![
                    "description is required",
                    "name \"Other\" does not match parent directory \"parent\"",
                    "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)"
                ]
            );
        }
    }
    let file = dir.file("parent/SKILL.md", "---\ndescription: \"\\u0085\"\n---");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    assert_eq!(result.skills[0].description, "\u{85}");
    assert!(result.diagnostics.is_empty());
}

/// Overlong descriptions remain intact and use Unicode character counts.
#[test]
fn maestro_skills_keep_overlong_descriptions() {
    let dir = Directory::new();
    for c in ['a', '😀'] {
        for count in [1024, 1025] {
            let description = c.to_string().repeat(count);
            let file = dir.file(
                "parent/SKILL.md",
                &format!("---\ndescription: {description}\n---"),
            );
            let result = load_skills_from_dir(LoadSkillsFromDirOptions {
                cwd: support::text(&std::env::temp_dir()),
                dir: support::text(file.parent().unwrap()),
                source: "path",
            });
            assert_eq!(result.skills[0].description, description);
            assert_eq!(
                result
                    .diagnostics
                    .iter()
                    .map(|d| d.message.as_str())
                    .collect::<Vec<_>>(),
                if count == 1025 {
                    vec!["description exceeds 1024 characters (1025)"]
                } else {
                    vec![]
                }
            );
        }
    }
}

/// Wrong-typed supported fields produce one path-bearing failure.
#[test]
fn maestro_skills_reject_nonstring_metadata_fields() {
    let dir = Directory::new();
    for field in ["name", "description"] {
        for value in ["true", "42", "[a]", "{a: b}"] {
            let content = if field == "name" {
                format!("name: {value}\ndescription: Useful")
            } else {
                format!("description: {value}")
            };
            let file = dir.file("parent/SKILL.md", &format!("---\n{content}\n---"));
            let result = load_skills_from_dir(LoadSkillsFromDirOptions {
                cwd: support::text(&std::env::temp_dir()),
                dir: support::text(file.parent().unwrap()),
                source: "path",
            });
            assert!(result.skills.is_empty());
            assert_eq!(result.diagnostics.len(), 1);
            assert_eq!(
                result.diagnostics[0].path.as_deref(),
                Some(support::text(&file))
            );
            assert!(result.diagnostics[0].message.contains(field));
            assert!(result.diagnostics[0].message.contains("string"));
        }
    }
    let file = dir.file("parent/SKILL.md", "---\nname: true\ndescription: 42\n---");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].message,
        "description must be a string"
    );
    let value = maestro_resources::FrontmatterValue::Mapping(vec![(
        "name".into(),
        maestro_resources::FrontmatterValue::String("typed".into()),
    )]);
    let projected = maestro_resources::SkillFrontmatter::try_from(&value).unwrap();
    assert_eq!(projected.name.as_deref(), Some("typed"));
    assert!(projected.description.is_none());
}

/// Unrelated nested metadata is retained by projection without loader warnings.
#[test]
fn maestro_skills_ignore_unrelated_metadata() {
    let dir = Directory::new();
    let text = "---\ndescription: Useful\ncustom: {nested: [true, 4]}\n---";
    let file = dir.file("parent/SKILL.md", text);
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "custom",
    });
    assert_eq!(result.skills.len(), 1);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.skills[0].source_info.source, "custom");
    let parsed = maestro_resources::parse_frontmatter(text).unwrap();
    let projected = maestro_resources::SkillFrontmatter::try_from(&parsed.frontmatter).unwrap();
    assert_eq!(
        projected.extra,
        vec![(
            "custom".into(),
            maestro_resources::FrontmatterValue::Mapping(vec![(
                "nested".into(),
                maestro_resources::FrontmatterValue::Sequence(vec![
                    maestro_resources::FrontmatterValue::Bool(true),
                    maestro_resources::FrontmatterValue::Number(4.0)
                ])
            )])
        )]
    );
}

/// Directory scans descend to nested entry files.
#[test]
fn maestro_skills_find_nested_entry_files() {
    let dir = Directory::new();
    let file = dir.file(
        "nested/calendar/SKILL.md",
        "---\ndescription: Meetings\n---",
    );
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "project",
    });
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, file);
    assert_eq!(result.skills[0].source_info.scope, SourceScope::Project);
    assert!(result.diagnostics.is_empty());
}

/// A root entry file is preferred even when a child was created first.
#[test]
fn maestro_skills_prefer_the_root_candidate() {
    let dir = Directory::new();
    let _ = dir.file("parent/child/SKILL.md", "---\ndescription: Child\n---");
    let file = dir.file("parent/SKILL.md", "---\ndescription: Parent\n---");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, file);
    assert!(result.diagnostics.is_empty());
}

/// Any attempted root entry prevents descent, including loading failures.
#[test]
fn maestro_skills_root_failure_stops_descent() {
    let dir = Directory::new();
    let _ = dir.file("parent/child/SKILL.md", "---\ndescription: Child\n---");
    for text in [
        "---\ndescription: [broken\n---",
        "---\nname: parent\n---",
        "---\ndescription: Unreadable\n---",
    ] {
        let file = dir.file("parent/SKILL.md", text);
        #[cfg(unix)]
        let (_guard, denied) = if text.contains("Unreadable") {
            let Some((guard, message)) = support::deny(&file) else {
                continue; // a privileged run still reads it
            };
            (Some(guard), Some(message))
        } else {
            (None, None)
        };
        #[cfg(not(unix))]
        let denied: Option<String> = if text.contains("Unreadable") {
            continue;
        } else {
            None
        };
        let result = load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd: support::text(&std::env::temp_dir()),
            dir: support::text(file.parent().unwrap()),
            source: "path",
        });
        assert!(result.skills.is_empty());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].path.as_deref(),
            Some(support::text(&file))
        );
        if let Some(message) = &denied {
            assert_eq!(&result.diagnostics[0].message, message);
        } else if text.contains("name:") {
            assert_eq!(result.diagnostics[0].message, "description is required");
        } else {
            assert!(result.diagnostics[0].message.contains("line"));
        }
    }
}

/// Nonfiles, ignored roots and dangling links allow other children to load.
#[test]
fn maestro_skills_skip_nonfile_or_ignored_root_candidates() {
    for branch in ["directory", "ignored", "dangling"] {
        let dir = Directory::new();
        let child = dir.file("child/SKILL.md", "---\ndescription: Child\n---");
        match branch {
            "directory" => std::fs::create_dir(dir.0.path().join("SKILL.md")).unwrap(),
            "ignored" => {
                let _ = dir.file("SKILL.md", "---\ndescription: Root\n---");
                let _ = dir.file(".ignore", "SKILL.md\n!child/SKILL.md\n");
            }
            _ => {
                #[cfg(unix)]
                std::os::unix::fs::symlink("missing", dir.0.path().join("SKILL.md")).unwrap();
            }
        }
        let result = load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd: support::text(&std::env::temp_dir()),
            dir: support::text(dir.0.path()),
            source: "path",
        });
        assert_eq!(result.skills.len(), 1, "{branch}");
        assert_eq!(result.skills[0].file_path, child, "{branch}");
        assert!(result.diagnostics.is_empty());
    }
}

/// Preserve multiline descriptions.
#[test]
fn maestro_skills_preserve_multiline_descriptions() {
    let dir = Directory::new();
    let file = dir.file(
        "parent/SKILL.md",
        "---\ndescription: |\n  First line\n  Second line\n---",
    );
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    assert_eq!(result.skills[0].description, "First line\nSecond line\n");
    assert!(result.diagnostics.is_empty());
}

/// Return empty for missing directories.
#[test]
fn maestro_skills_return_empty_for_missing_directories() {
    let dir = Directory::new();
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(&dir.0.path().join("absent")),
        source: "path",
    });
    assert!(result.skills.is_empty());
    assert!(result.diagnostics.is_empty());
}

/// Use basename when name is absent.
#[test]
fn maestro_skills_use_basename_when_name_is_absent() {
    let dir = Directory::new();
    for name in ["", "name: null\n", "name: ''\n"] {
        let file = dir.file(
            "parent/SKILL.md",
            &format!("---\n{name}description: Useful\n---"),
        );
        let result = load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd: support::text(&std::env::temp_dir()),
            dir: support::text(file.parent().unwrap()),
            source: "path",
        });
        assert_eq!(result.skills[0].name, "parent");
        assert!(result.diagnostics.is_empty());
    }
}

/// Disable only explicit boolean true.
#[test]
fn maestro_skills_disable_only_explicit_boolean_true() {
    let dir = Directory::new();
    for flag in ["true", "false", "'true'", "1", "null"] {
        let file = dir.file(
            "parent/SKILL.md",
            &format!("---\ndescription: Useful\ndisable-model-invocation: {flag}\n---"),
        );
        let result = load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd: support::text(&std::env::temp_dir()),
            dir: support::text(file.parent().unwrap()),
            source: "path",
        });
        assert_eq!(result.skills[0].disable_model_invocation, flag == "true");
        assert!(result.diagnostics.is_empty());
    }
}

/// Empty and fully disabled skills produce no prompt text.
#[test]
fn maestro_skills_empty_or_hidden_lists_have_no_prompt() {
    assert_eq!(maestro_resources::format_skills_for_prompt(&[]), "");
    let dir = Directory::new();
    let file = dir.file(
        "parent/SKILL.md",
        "---\ndescription: Useful\ndisable-model-invocation: true\n---",
    );
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    assert_eq!(
        maestro_resources::format_skills_for_prompt(&result.skills),
        ""
    );
}

/// Prompt instructions, indentation and file location are exact.
#[test]
fn maestro_skills_render_exact_prompt_instructions() {
    let dir = Directory::new();
    let file = dir.file("parent/SKILL.md", "---\ndescription: Useful\n---");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    let expected = format!(
        "\n\nThe following skills provide specialized instructions for specific tasks.\nUse the read tool to load a skill's file when the task matches its description.\nWhen a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.\n\n<available_skills>\n  <skill>\n    <name>parent</name>\n    <description>Useful</description>\n    <location>{}</location>\n  </skill>\n</available_skills>",
        file.display()
    );
    assert_eq!(
        maestro_resources::format_skills_for_prompt(&result.skills),
        expected
    );
}

/// XML fields escape all special characters without rescanning replacements.
#[test]
fn maestro_skills_escape_every_xml_field() {
    let dir = Directory::new();
    let file = dir.file("parent/SKILL.md", "---\ndescription: Useful\n---");
    let mut result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    let skill = &mut result.skills[0];
    skill.name = "&<>\"'😀&amp;".into();
    skill.description = "&<>\"'😀&amp;".into();
    skill.file_path = "&<>\"'😀&amp;".into();
    let prompt = maestro_resources::format_skills_for_prompt(&result.skills);
    for field in ["name", "description", "location"] {
        assert!(prompt.contains(&format!(
            "<{field}>&amp;&lt;&gt;&quot;&apos;😀&amp;amp;</{field}>"
        )));
    }
}

/// Preserve prompt order.
#[test]
fn maestro_skills_preserve_prompt_order() {
    let dir = Directory::new();
    let mut skills = Vec::new();
    for name in ["second", "first"] {
        let file = dir.file(&format!("{name}/SKILL.md"), "---\ndescription: Useful\n---");
        skills.extend(
            load_skills_from_dir(LoadSkillsFromDirOptions {
                cwd: support::text(&std::env::temp_dir()),
                dir: support::text(file.parent().unwrap()),
                source: "path",
            })
            .skills,
        );
    }
    let prompt = maestro_resources::format_skills_for_prompt(&skills);
    assert!(
        prompt.find("<name>second</name>").unwrap() < prompt.find("<name>first</name>").unwrap()
    );
    assert_eq!(prompt.matches("  <skill>").count(), 2);
}

/// Filter hidden prompt members.
#[test]
fn maestro_skills_filter_hidden_prompt_members() {
    let dir = Directory::new();
    let mut skills = Vec::new();
    for (name, hidden) in [("visible", false), ("secret", true)] {
        let file = dir.file(
            &format!("{name}/SKILL.md"),
            &format!(
                "---\ndescription: {name} instructions\ndisable-model-invocation: {hidden}\n---"
            ),
        );
        skills.extend(
            load_skills_from_dir(LoadSkillsFromDirOptions {
                cwd: support::text(&std::env::temp_dir()),
                dir: support::text(file.parent().unwrap()),
                source: "path",
            })
            .skills,
        );
    }
    let prompt = maestro_resources::format_skills_for_prompt(&skills);
    assert!(prompt.contains("<name>visible</name>"));
    assert!(!prompt.contains("secret"));
    assert_eq!(prompt.matches("  <skill>").count(), 1);
}

/// Explicit files and directories load without ambient defaults.
#[test]
fn maestro_skills_accept_explicit_files_and_directories() {
    let dir = Directory::new();
    let first = dir.file("calendar/SKILL.md", "---\ndescription: Meetings\n---");
    let second = dir.file("notes/notes.md", "---\ndescription: Notes\n---");
    let paths = vec![first.parent().unwrap().into(), second.clone()];
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(&dir.0.path().join("agent")),
        config_dir_name: ".maestro",
        skill_paths: &strings(&paths),
        include_defaults: false,
    })
    .unwrap();
    assert_eq!(
        result
            .skills
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>(),
        vec![&first, &second]
    );
    assert!(result.diagnostics.is_empty());
    assert!(
        result
            .skills
            .iter()
            .all(|s| s.source_info.scope == SourceScope::Temporary)
    );
}

/// Explicit wrong-kind, missing and stat failures retain resolved paths.
#[test]
fn maestro_skills_report_explicit_path_failures() {
    let dir = Directory::new();
    let upper = dir.file("upper.MD", "---\ndescription: Not loaded\n---");
    let missing = dir.0.path().join("absent");
    let paths = vec![missing.clone(), upper.clone()];
    #[cfg(unix)]
    let (paths, _socket) = {
        let mut paths = paths;
        let other = dir.0.path().join("other.md");
        let listener = std::os::unix::net::UnixListener::bind(&other).unwrap();
        paths.push(other);
        (paths, listener)
    };
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(dir.0.path()),
        config_dir_name: ".maestro",
        skill_paths: &strings(&paths),
        include_defaults: false,
    })
    .unwrap();
    assert!(result.skills.is_empty());
    let expected = [
        ("skill path does not exist", missing),
        ("skill path is not a markdown file", upper),
        #[cfg(unix)]
        (
            "skill path is not a markdown file",
            dir.0.path().join("other.md"),
        ),
    ];
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| (d.message.as_str(), d.path.as_deref().unwrap()))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(message, path)| (*message, support::text(path)))
            .collect::<Vec<_>>()
    );
    assert!(
        result.diagnostics.iter().all(
            |d| d.r#type == maestro_resources::DiagnosticType::Warning && d.collision.is_none()
        )
    );
}

/// All home forms and relative components resolve to populated resources.
#[test]
fn maestro_skills_expand_all_home_path_forms() {
    let dir = Directory::new();
    let file = dir.file("home/calendar/SKILL.md", "---\ndescription: Meetings\n---");
    let home = dir.0.path().join("home");
    for (raw, target) in [
        ("~".into(), file.clone()),
        ("~/calendar".into(), file.clone()),
        ("~//calendar".into(), file.clone()),
        ("~calendar".into(), file.clone()),
        ("home/../home/calendar".into(), file.clone()),
        ("\u{feff}~/calendar\u{feff}".into(), file.clone()),
        (home.join("calendar/../calendar"), file),
    ] {
        let paths = [raw];
        let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
            cwd: support::text(dir.0.path()),
            home: support::text(&home),
            agent_dir: support::text(dir.0.path()),
            config_dir_name: ".maestro",
            skill_paths: &strings(&paths),
            include_defaults: false,
        })
        .unwrap();
        assert_eq!(result.skills.len(), 1);
        assert_eq!(result.skills[0].file_path.as_str(), support::text(&target));
        assert!(result.diagnostics.is_empty());
    }
    let paths = [std::path::PathBuf::from("\u{85}~/calendar")];
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(&home),
        agent_dir: support::text(dir.0.path()),
        config_dir_name: ".maestro",
        skill_paths: &strings(&paths),
        include_defaults: false,
    })
    .unwrap();
    assert!(result.skills.is_empty());
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(support::text(&support::under(
            dir.0.path(),
            "\u{85}~/calendar"
        )))
    );
}

/// The complete fixture tree yields exact retained files and diagnostic cardinality.
#[test]
fn maestro_skills_load_the_complete_fixture_tree() {
    let root = support::under(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "tests/fixtures/skills",
    );
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(&root),
        source: "fixture",
    });
    let mut names = result
        .skills
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    let mut expected = vec![
        "bad--name",
        "disable-model-invocation",
        "Invalid_Name",
        "multiline-description",
        "different-name",
        "child-skill",
        "unknown-field",
        "valid-skill",
        "root-skill-preferred",
        "this-is-a-very-long-skill-name-that-exceeds-the-sixty-four-character-limit-set-by-the-standard",
    ];
    expected.sort_unstable();
    assert_eq!(names, expected);
    assert_eq!(result.diagnostics.len(), 10);
    assert!(result.skills.iter().all(
        |s| s.file_path.starts_with(support::text(&root)) && s.source_info.source == "fixture"
    ));
}

/// Public loading keeps the first name and appends complete collision details last.
#[test]
fn maestro_skills_keep_first_discovered_name() {
    let root = support::under(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "tests/fixtures/skills-collision",
    );
    let paths = [
        support::under(&root, "first/calendar"),
        support::under(&root, "second/calendar"),
    ];
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(&root),
        home: support::text(&root),
        agent_dir: support::text(&root),
        config_dir_name: ".maestro",
        skill_paths: &strings(&paths),
        include_defaults: false,
    })
    .unwrap();
    assert_eq!(result.skills.len(), 1);
    let winner = paths[0].join("SKILL.md");
    let loser = paths[1].join("SKILL.md");
    assert_eq!(result.skills[0].file_path, winner);
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(
        diagnostic.r#type,
        maestro_resources::DiagnosticType::Collision
    );
    assert_eq!(diagnostic.message, "name \"calendar\" collision");
    assert_eq!(diagnostic.path.as_deref(), Some(support::text(&loser)));
    let collision = diagnostic.collision.as_ref().unwrap();
    assert_eq!(collision.resource_type, "skill");
    assert_eq!(collision.name, "calendar");
    assert_eq!(collision.winner_path, winner);
    assert_eq!(collision.loser_path, loser);
    assert!(collision.winner_source.is_none());
    assert!(collision.loser_source.is_none());
}

/// User then project then explicit paths determine winners in controlled order.
#[test]
fn maestro_skills_load_defaults_before_explicit_paths() {
    let dir = Directory::new();
    let user = dir.file(
        "agent/skills/calendar/SKILL.md",
        "---\ndescription: User\n---",
    );
    let project = dir.file(
        ".maestro/skills/calendar/SKILL.md",
        "---\ndescription: Project\n---",
    );
    let explicit = dir.file(
        "explicit/calendar/SKILL.md",
        "---\ndescription: Explicit\n---",
    );
    let other = dir.file(
        "agent/skills/other/SKILL.md",
        "---\ndescription: Other\n---",
    );
    let paths = [explicit.clone()];
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(&dir.0.path().join("agent")),
        config_dir_name: ".maestro",
        skill_paths: &strings(&paths),
        include_defaults: true,
    })
    .unwrap();
    assert_eq!(
        result
            .skills
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>(),
        vec![&user, &other]
    );
    assert_eq!(result.skills[0].source_info.scope, SourceScope::User);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.path.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec![&project, &explicit]
    );
}

/// Explicit source scopes use component containment and user precedence.
#[test]
fn maestro_skills_classify_explicit_source_scopes() {
    let dir = Directory::new();
    for (relative, scope) in [
        ("agent/skills/user/SKILL.md", SourceScope::User),
        (".maestro/skills/project/SKILL.md", SourceScope::Project),
        ("agent/skills-other/temp/SKILL.md", SourceScope::Temporary),
    ] {
        let file = dir.file(relative, "---\ndescription: Useful\n---");
        let paths = [file.clone()];
        let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
            cwd: support::text(dir.0.path()),
            home: support::text(dir.0.path()),
            agent_dir: support::text(&dir.0.path().join("agent")),
            config_dir_name: ".maestro",
            skill_paths: &strings(&paths),
            include_defaults: false,
        })
        .unwrap();
        assert_eq!(result.skills[0].source_info.scope, scope);
        assert_eq!(result.skills[0].source_info.source, "local");
        assert_eq!(result.skills[0].source_info.path, file);
    }
    let file = dir.file(
        "overlap/skills/item/SKILL.md",
        "---\ndescription: Useful\n---",
    );
    let paths = [file];
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(&dir.0.path().join("overlap")),
        config_dir_name: "overlap",
        skill_paths: &strings(&paths),
        include_defaults: false,
    })
    .unwrap();
    assert_eq!(result.skills[0].source_info.scope, SourceScope::User);
}

/// Canonical aliases deduplicate before name collision, with spelling fallback.
#[test]
fn maestro_skills_deduplicate_paths_before_names() {
    let dir = Directory::new();
    let file = dir.file("parent/SKILL.md", "---\ndescription: Useful\n---");
    let alias = dir.0.path().join("parent/alias.md");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&file, &alias).unwrap();
    #[cfg(not(unix))]
    std::fs::copy(&file, &alias).unwrap();
    let paths = [file.clone(), file, alias];
    let authored_paths = strings(&paths);
    let options = || maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(dir.0.path()),
        config_dir_name: ".maestro",
        skill_paths: &authored_paths,
        include_defaults: false,
    };
    let result = maestro_resources::load_skills(options()).unwrap();
    assert_eq!(result.skills.len(), 1);
    #[cfg(unix)]
    assert!(result.diagnostics.is_empty());
}

/// Repeated losing paths repeat collisions after all ordinary diagnostics.
#[test]
fn maestro_skills_repeat_collisions_for_losing_paths() {
    let dir = Directory::new();
    let winner = dir.file("first/calendar/SKILL.md", "---\ndescription: Winner\n---");
    let loser = dir.file("second/calendar/SKILL.md", "---\ndescription: Loser\n---");
    let missing = dir.0.path().join("missing");
    let paths = [winner, loser.clone(), missing.clone(), loser.clone()];
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(dir.0.path()),
        config_dir_name: ".maestro",
        skill_paths: &strings(&paths),
        include_defaults: false,
    })
    .unwrap();
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.diagnostics.len(), 3);
    assert_eq!(result.diagnostics[0].message, "skill path does not exist");
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(support::text(&missing))
    );
    for diagnostic in &result.diagnostics[1..] {
        assert_eq!(diagnostic.path.as_deref(), Some(support::text(&loser)));
        assert_eq!(
            diagnostic.r#type,
            maestro_resources::DiagnosticType::Collision
        );
    }
}

/// Loose markdown belongs only to the scan root; links follow targets.
#[test]
fn maestro_skills_limit_loose_markdown_to_the_scan_root() {
    let dir = Directory::new();
    let root = dir.file("loose.md", "---\nname: loose\ndescription: Root\n---");
    let nested = dir.file("nested/loose.md", "---\ndescription: Nested loose\n---");
    for relative in [
        ".hidden/SKILL.md",
        "node_modules/SKILL.md",
        ".secret.md",
        "upper.MD",
    ] {
        let _ = dir.file(relative, "---\ndescription: Hidden\n---");
    }
    let target = dir.file("outside/calendar/SKILL.md", "---\ndescription: Linked\n---");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "path",
    });
    assert_eq!(
        result
            .skills
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>(),
        vec![&root, &target]
    );
    assert!(!result.skills.iter().any(|s| s.file_path == nested));
}

/// Native traversal follows file and directory links while skipping broken ones.
#[cfg(unix)]
#[test]
fn maestro_skills_follow_native_links_and_skip_broken_links() {
    let dir = Directory::new();
    let target = dir.file("outside/calendar/SKILL.md", "---\ndescription: Linked\n---");
    let scan = dir.0.path().join("scan");
    std::fs::create_dir(&scan).unwrap();
    std::os::unix::fs::symlink(&target, scan.join("linked.md")).unwrap();
    let Some(_locked) = support::denied_link(&dir, &scan.join("denied.md")) else {
        return;
    };
    std::os::unix::fs::symlink(target.parent().unwrap(), scan.join("calendar")).unwrap();
    std::os::unix::fs::symlink("absent", scan.join("broken")).unwrap();
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(&scan),
        source: "path",
    });
    assert_eq!(
        result
            .skills
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>(),
        vec![&scan.join("calendar/SKILL.md"), &scan.join("linked.md")]
    );
    assert!(result.diagnostics.is_empty());
}

/// A linked root entry file is the only skill; every other outcome leaves the child.
#[cfg(unix)]
#[test]
fn skill_entry_links_preserve_precedence_and_failure_fallback() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    for (branch, expect_root) in [
        ("file", true),
        ("directory", false),
        ("socket", false),
        ("dangling", false),
        ("denied", false),
    ] {
        let dir = Directory::new();
        let scan = dir.0.path().join("scan");
        let child = dir.file("scan/child/SKILL.md", "---\ndescription: Child\n---");
        let target = dir.0.path().join("target");
        let _socket;
        let mut _locked = None;
        match branch {
            "file" => drop(dir.file("target", "---\ndescription: Root\n---")),
            "directory" => std::fs::create_dir(&target).unwrap(),
            "socket" => _socket = UnixListener::bind(&target).unwrap(),
            "denied" => match support::denied_link(&dir, &scan.join("SKILL.md")) {
                Some(guard) => _locked = Some(guard),
                None => continue,
            },
            _ => {}
        }
        if branch != "denied" {
            symlink(&target, scan.join("SKILL.md")).unwrap();
        }
        let result = load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd: support::text(&std::env::temp_dir()),
            dir: support::text(&scan),
            source: "path",
        });
        let paths = result
            .skills
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>();
        if expect_root {
            assert_eq!(paths, vec![&scan.join("SKILL.md")], "{branch}");
        } else {
            assert_eq!(paths, vec![&child], "{branch}");
        }
        assert!(result.diagnostics.is_empty(), "{branch}");
    }
}

/// The shared classifier reads metadata only for links and returns `None` when that read fails.
#[cfg(unix)]
#[test]
fn entry_classification_uses_supplied_observations() {
    use maestro_resources::ResourceFileType as Kind;
    use maestro_resources::skills::observed_type;
    use std::os::unix::{fs::symlink, net::UnixListener};
    let dir = Directory::new();
    let file = dir.file("file", "x");
    let sub = dir.0.path().join("dir");
    std::fs::create_dir(&sub).unwrap();
    let socket = dir.0.path().join("sock");
    let _listener = UnixListener::bind(&socket).unwrap();
    symlink(&file, dir.0.path().join("link-file")).unwrap();
    symlink(&sub, dir.0.path().join("link-dir")).unwrap();
    symlink(&socket, dir.0.path().join("link-sock")).unwrap();
    symlink("absent", dir.0.path().join("link-dangling")).unwrap();
    let Some(_locked) = support::denied_link(&dir, &dir.0.path().join("link-denied")) else {
        return;
    };
    symlink("link-loop", dir.0.path().join("link-loop")).unwrap();
    let entries = maestro_resources::read_entries(dir.0.path()).unwrap();
    let observed = |name: &str| {
        let entry = entries.iter().find(|e| e.name == name).unwrap();
        observed_type(entry, support::text(&dir.0.path().join(name)))
    };
    let cases = [
        ("file", Some(Kind::File)),
        ("dir", Some(Kind::Directory)),
        ("sock", Some(Kind::Other)),
        ("link-file", Some(Kind::File)),
        ("link-dir", Some(Kind::Directory)),
        ("link-sock", Some(Kind::Other)),
        ("link-dangling", None),
        ("link-denied", None),
        ("link-loop", None),
    ];
    for (name, expected) in cases {
        assert_eq!(observed(name), expected, "{name}");
    }
}

/// Three ignore files share ordered rules and later negations reopen files.
#[test]
fn maestro_skills_apply_all_ignore_files_in_order() {
    let dir = Directory::new();
    let a = dir.file("a/SKILL.md", "---\ndescription: A\n---");
    let _ = dir.file("b/SKILL.md", "---\ndescription: B\n---");
    let c = dir.file("c/SKILL.md", "---\ndescription: C\n---");
    let _ = dir.file(".gitignore", "a/SKILL.md\nb/SKILL.md\nc/SKILL.md\n");
    let _ = dir.file(".ignore", "!a/SKILL.md\n!b/SKILL.md\n");
    let _ = dir.file(".fdignore", "b/SKILL.md\n!c/SKILL.md\n");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "path",
    });
    assert_eq!(
        result
            .skills
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>(),
        vec![&a, &c]
    );
    assert!(result.diagnostics.is_empty());
}

/// Nested ignore rules are directory-prefixed and scan-local.
#[test]
fn maestro_skills_scope_nested_ignore_rules() {
    let dir = Directory::new();
    let _ = dir.file("nested/drop/SKILL.md", "---\ndescription: Drop\n---");
    let keep = dir.file("sibling/drop/SKILL.md", "---\ndescription: Keep\n---");
    let _ = dir.file("nested/#literal/SKILL.md", "---\ndescription: Hash\n---");
    let _ = dir.file("nested/.ignore", "\n  \n# comment\n/drop/\n\\#literal/\n");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "path",
    });
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, keep);
    assert!(result.diagnostics.is_empty());
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(&dir.0.path().join("sibling")),
        source: "path",
    });
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, keep);
}

/// Escaped exclamations remain literal at root and nested ignore scopes.
#[test]
fn maestro_skills_preserve_escaped_exclamation_rules() {
    let dir = Directory::new();
    let _ = dir.file(
        "!literal.md",
        "---\nname: literal\ndescription: Hidden\n---",
    );
    let ordinary = dir.file(
        "ordinary.md",
        "---\nname: ordinary\ndescription: Visible\n---",
    );
    let _ = dir.file("nested/!literal/SKILL.md", "---\ndescription: Hidden\n---");
    let nested = dir.file("nested/ordinary/SKILL.md", "---\ndescription: Visible\n---");
    let _ = dir.file(".ignore", "\\!literal.md\n");
    let _ = dir.file("nested/.ignore", "\\!literal/\n");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "path",
    });
    assert_eq!(
        result
            .skills
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>(),
        vec![&nested, &ordinary]
    );
}

/// Child negation cannot reopen excluded parents and matching ignores case.
#[test]
fn maestro_skills_respect_excluded_parent_directories() {
    let dir = Directory::new();
    let _ = dir.file("excluded/child/SKILL.md", "---\ndescription: Excluded\n---");
    let keep = dir.file("kept/child/SKILL.md", "---\ndescription: Kept\n---");
    let _ = dir.file("excluded/.ignore", "!child/SKILL.md\n");
    let _ = dir.file(".ignore", "EXCLUDED/\n!excluded/child/SKILL.md\n");
    let local = dir.file(
        "self-blocked/SKILL.md",
        "---\ndescription: Blocked at entry\n---",
    );
    let _ = dir.file("self-blocked/.ignore", "/\n!SKILL.md\n");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "path",
    });
    assert_eq!(result.skills.len(), 2);
    assert_eq!(result.skills[0].file_path, keep);
    assert_eq!(result.skills[1].file_path, local);
    assert!(result.diagnostics.is_empty());
}

/// Invalid ignore rules remain inert while valid rules still apply.
#[test]
fn maestro_skills_ignore_invalid_rules_without_panicking() {
    let dir = Directory::new();
    let keep = dir.file("keep/SKILL.md", "---\ndescription: Keep\n---");
    let _ = dir.file("drop/SKILL.md", "---\ndescription: Drop\n---");
    let _ = dir.file(".ignore", "[unclosed\ndrop/\n");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "path",
    });
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, keep);
    assert!(result.diagnostics.is_empty());
    let _ = dir.file(".ignore", &"?".repeat(250_000));
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "path",
    });
    assert_eq!(result.skills.len(), 2);
    assert!(result.diagnostics.is_empty());
}

/// I/O failures preserve earlier discoveries and the quiet-versus-warning routes.
#[cfg(unix)]
#[test]
fn maestro_skills_keep_partial_results_after_io_failures() {
    let dir = Directory::new();
    let keep = dir.file("keep/SKILL.md", "---\ndescription: Keep\n---");
    let unreadable = dir.file("unreadable/SKILL.md", "---\ndescription: Unreadable\n---");
    let _ = dir.file("blocked/SKILL.md", "---\ndescription: Blocked\n---");
    let ignore = dir.file(".ignore", "keep/\n");
    let (Some(_ignore), Some(_blocked), Some((_unreadable, message)), Some(_link)) = (
        support::deny(&ignore),
        support::deny(&dir.0.path().join("blocked")),
        support::deny(&unreadable),
        support::denied_link(&dir, &dir.0.path().join("link")),
    ) else {
        return;
    };
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "path",
    });
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, keep);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(support::text(&unreadable))
    );
    assert_eq!(result.diagnostics[0].message, message);
    std::fs::write(&keep, b"---\ndescription: Replaced \xff byte\n---").unwrap();
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(keep.parent().unwrap()),
        source: "path",
    });
    assert_eq!(result.skills[0].description, "Replaced \u{fffd} byte");
}

/// Skills of a custom directory carry its source metadata, in name order.
#[test]
fn custom_skill_directories_keep_source_metadata_and_name_order() {
    let dir = Directory::new();
    let second = dir.file("second/SKILL.md", "---\ndescription: Second\n---");
    let first = dir.file("first/SKILL.md", "---\ndescription: First\n---");
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "custom",
    });
    assert!(result.diagnostics.is_empty());
    assert!(
        result
            .skills
            .iter()
            .all(|s| s.source_info.source == "custom"
                && s.source_info.scope == SourceScope::Temporary
                && s.source_info.origin == SourceOrigin::TopLevel
                && s.source_info.path == s.file_path
                && s.source_info.base_dir.as_ref() == Some(&s.base_dir))
    );
    assert_eq!(
        result
            .skills
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>(),
        vec![&first, &second]
    );
}

/// Relative dot roots retain their literal basename after child normalization.
#[test]
fn maestro_skills_use_literal_dot_root_names() {
    if !maestro_test_process::selection().is_child() {
        let dir = Directory::new();
        let _ = dir.file("SKILL.md", "---\ndescription: Parent\n---");
        let _ = dir.file("child/SKILL.md", "---\ndescription: Child\n---");
        let output =
            maestro_test_process::TestProcess::current("maestro_skills_use_literal_dot_root_names")
                .unwrap()
                .nocapture()
                .case("child-directory")
                .current_dir(dir.0.path().join("child"))
                .output()
                .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    for (root, expected_file) in [(".", "SKILL.md"), ("..", "../SKILL.md")] {
        let result = load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd: support::text(&std::env::temp_dir()),
            dir: support::text(std::path::Path::new(root)),
            source: "path",
        });
        assert_eq!(result.skills.len(), 1);
        let skill = &result.skills[0];
        assert_eq!(skill.name, root);
        assert_eq!(skill.base_dir.as_str(), root);
        // The file is joined natively, so Windows spells `../SKILL.md` with `\`.
        let expected_file = expected_file.replace('/', std::path::MAIN_SEPARATOR_STR);
        assert_eq!(skill.file_path.as_str(), expected_file);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].message,
            "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)"
        );
    }
}

#[test]
fn maestro_skills_resolve_authored_project_roots() {
    let dir = Directory::new();
    let file = dir.file("config/skills/SKILL.md", "---\ndescription: Project\n---");
    let config = "./config/../config";
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(&dir.0.path().join("absent")),
        config_dir_name: config,
        skill_paths: &[],
        include_defaults: true,
    })
    .unwrap();
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path.as_str(), support::text(&file));
    assert_eq!(result.skills[0].source_info.scope, SourceScope::Project);
}

/// Preferred defaults skip explicit children, whose enabled-default scope is temporary.
#[test]
fn maestro_skills_classify_skipped_default_children_as_temporary() {
    let dir = Directory::new();
    let agent = dir.0.path().join("agent");
    let _ = dir.file(
        "agent/skills/SKILL.md",
        "---\nname: user-root\ndescription: User\n---",
    );
    let _ = dir.file(
        ".maestro/skills/SKILL.md",
        "---\nname: project-root\ndescription: Project\n---",
    );
    let user = dir.file(
        "agent/skills/user/SKILL.md",
        "---\ndescription: Explicit user\n---",
    );
    let project = dir.file(
        ".maestro/skills/project/SKILL.md",
        "---\ndescription: Explicit project\n---",
    );
    let outside = dir.file("elsewhere/item/SKILL.md", "---\ndescription: Outside\n---");
    let paths = [user.clone(), project.clone(), outside.clone()];
    for include_defaults in [false, true] {
        let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
            cwd: support::text(dir.0.path()),
            home: support::text(dir.0.path()),
            agent_dir: support::text(&agent),
            config_dir_name: ".maestro",
            skill_paths: &strings(&paths),
            include_defaults,
        })
        .unwrap();
        assert_eq!(result.skills.len(), if include_defaults { 5 } else { 3 });
        for (file, disabled_scope) in [
            (&user, SourceScope::User),
            (&project, SourceScope::Project),
            (&outside, SourceScope::Temporary),
        ] {
            let skill = result
                .skills
                .iter()
                .find(|skill| &skill.file_path == file)
                .unwrap();
            assert_eq!(
                skill.source_info.scope,
                if include_defaults {
                    SourceScope::Temporary
                } else {
                    disabled_scope
                }
            );
            assert_eq!(skill.source_info.path, *file);
            assert_eq!(skill.source_info.source, "local");
        }
    }
}

/// Explicit dot components remain part of fallback names and provenance.
#[test]
fn maestro_skills_preserve_authored_dot_basename_and_provenance() {
    use maestro_resources::{LoadSkillsOptions, load_skills};
    let dir = Directory::new();
    let _ = dir.file("calendar/SKILL.md", "---\ndescription: Meetings\n---");
    let paths = [dir.0.path().join("calendar/./SKILL.md")];
    let result = load_skills(LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(dir.0.path()),
        config_dir_name: ".maestro",
        skill_paths: &strings(&paths),
        include_defaults: false,
    })
    .unwrap();
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].name, ".");
    assert_eq!(
        result.skills[0].base_dir.as_str(),
        support::text(&dir.0.path().join("calendar/."))
    );
    assert_eq!(
        result.skills[0]
            .source_info
            .base_dir
            .as_ref()
            .unwrap()
            .as_str(),
        support::text(&dir.0.path().join("calendar/."))
    );
}

/// Repeated separators do not enter normalized default scope prefixes.
#[test]
fn maestro_skills_keep_repeated_separator_paths_temporary() {
    use maestro_resources::{LoadSkillsOptions, load_skills};
    let dir = Directory::new();
    for root in ["agent/skills", ".maestro/skills"] {
        let _ = dir.file(
            &format!("{root}/calendar/SKILL.md"),
            "---\nname: calendar\ndescription: Meetings\n---",
        );
        let paths = [dir.0.path().join(format!(
            "{}/calendar/SKILL.md",
            root.replace("/skills", "//skills")
        ))];
        let result = load_skills(LoadSkillsOptions {
            cwd: support::text(dir.0.path()),
            home: support::text(dir.0.path()),
            agent_dir: support::text(&dir.0.path().join("agent")),
            config_dir_name: ".maestro",
            skill_paths: &strings(&paths),
            include_defaults: false,
        })
        .unwrap();
        assert_eq!(result.skills.len(), 1);
        assert_eq!(
            result.skills[0].source_info.scope,
            SourceScope::Temporary,
            "{root}"
        );
    }
}

/// A scan root spelled with dot components still matches ignore rules by the paths below it.
#[test]
fn maestro_skills_match_ignore_rules_below_a_dotted_scan_root() {
    let dir = Directory::new();
    let _ = dir.file("skills/.gitignore", "nested/drop/\n");
    let _ = dir.file(
        "skills/nested/drop/SKILL.md",
        "---\nname: drop\ndescription: Omitted\n---",
    );
    let keep = dir.file(
        "skills/nested/keep/SKILL.md",
        "---\nname: keep\ndescription: Retained\n---",
    );
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(&dir.0.path().join("skills/../skills")),
        source: "path",
    });
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, keep);
}

/// Absolute project configuration roots replace the working directory prefix.
#[test]
fn maestro_skills_resolve_absolute_project_configuration() {
    use maestro_resources::{LoadSkillsOptions, load_skills};
    let dir = Directory::new();
    let config = dir.0.path().join("config");
    let file = dir.file(
        "config/skills/calendar/SKILL.md",
        "---\nname: calendar\ndescription: Meetings\n---",
    );
    let result = load_skills(LoadSkillsOptions {
        cwd: support::text(&dir.0.path().join("work")),
        home: support::text(dir.0.path()),
        agent_dir: support::text(&dir.0.path().join("agent")),
        config_dir_name: support::text(&config),
        skill_paths: &[],
        include_defaults: true,
    })
    .unwrap();
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, support::text(&file));
    assert_eq!(result.skills[0].source_info.scope, SourceScope::Project);
}

/// Explicit directories equal to normalized default roots retain their scope.
#[test]
fn maestro_skills_classify_exact_default_roots() {
    use maestro_resources::{LoadSkillsOptions, load_skills};
    let dir = Directory::new();
    for (root, scope) in [
        ("agent/skills", SourceScope::User),
        (".maestro/skills", SourceScope::Project),
    ] {
        let _ = dir.file(
            &format!("{root}/SKILL.md"),
            "---\nname: skills\ndescription: Meetings\n---",
        );
        let paths = [support::text(&support::under(dir.0.path(), root)).to_owned()];
        let result = load_skills(LoadSkillsOptions {
            cwd: support::text(dir.0.path()),
            home: support::text(dir.0.path()),
            agent_dir: support::text(&dir.0.path().join("agent/../agent")),
            config_dir_name: ".maestro",
            skill_paths: &paths,
            include_defaults: false,
        })
        .unwrap();
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.skills.len(), 1);
        assert_eq!(result.skills[0].source_info.scope, scope);
    }
}

/// Native directory order chooses the first conflicting skill name.
#[cfg(unix)]
#[test]
fn native_skill_discovery_uses_ordered_directory_entries() {
    let dir = Directory::new();
    let loser = dir.file("z/SKILL.md", "---\nname: common\ndescription: Last\n---");
    let winner = dir.file("a/SKILL.md", "---\nname: common\ndescription: First\n---");
    let result = maestro_resources::load_skills(maestro_resources::LoadSkillsOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: "",
        config_dir_name: ".maestro",
        skill_paths: &[support::text(dir.0.path()).into()],
        include_defaults: false,
    })
    .unwrap();
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].file_path, winner);
    assert_eq!(result.skills[0].description, "First");
    let collision = result
        .diagnostics
        .iter()
        .find_map(|d| d.collision.as_ref())
        .unwrap();
    assert_eq!(collision.winner_path, winner);
    assert_eq!(collision.loser_path, loser);
}

/// Recorded complete prompt for one skill with special characters.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedPrompt {
    input: RecordedSkill,
    expected: String,
}

/// The skill fields the prompt reads.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedSkill {
    name: String,
    description: String,
    #[serde(rename = "filePath")]
    file_path: String,
}

/// The whole prompt escapes every field once.
#[test]
fn maestro_skills_render_the_recorded_escaped_prompt() {
    let recorded: RecordedPrompt =
        serde_json::from_str(include_str!("fixtures/skill_prompt.json")).unwrap();
    let dir = Directory::new();
    let file = dir.file("parent/SKILL.md", "---\ndescription: Useful\n---");
    let mut result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(file.parent().unwrap()),
        source: "path",
    });
    let skill = &mut result.skills[0];
    skill.name = recorded.input.name;
    skill.description = recorded.input.description;
    skill.file_path = recorded.input.file_path;
    assert_eq!(
        maestro_resources::format_skills_for_prompt(&result.skills),
        recorded.expected
    );
}

/// Skills load in the name order of their directories, whatever order the file system lists.
#[test]
fn maestro_skills_load_in_directory_name_order() {
    let dir = Directory::new();
    // NTFS lists these case-insensitively (a, b, C, _x); name order is byte order.
    for name in ["b", "_x", "a", "C"] {
        let _ = dir.file(
            &format!("{name}/SKILL.md"),
            &format!("---\nname: {name}\ndescription: Order\n---\n"),
        );
    }
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(&std::env::temp_dir()),
        dir: support::text(dir.0.path()),
        source: "user",
    });
    let names: Vec<_> = result
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect();
    assert_eq!(names, ["C", "_x", "a", "b"]);
}
