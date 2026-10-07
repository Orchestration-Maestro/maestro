use maestro_resources::{
    LoadSkillsFromDirOptions, NativeResourceOperations, format_skills_for_prompt,
    load_skills_from_dir,
};

#[test]
fn supplied_directory_loads_into_prompt() {
    let dir = std::env::temp_dir().join(format!("maestro-skill-slice-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\ndescription: Demonstration.\n---\n# Demo\n",
    )
    .unwrap();
    let text = dir.to_str().unwrap();
    let result = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: text,
            source: "example",
        },
        &NativeResourceOperations,
    )
    .unwrap();
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].source_info.source, "example");
    assert_eq!(result.skills[0].source_info.scope, "temporary");
    assert!(
        format_skills_for_prompt(&result.skills)
            .contains("<description>Demonstration.</description>")
    );
    let mut controlled = Controlled::default();
    controlled.dir(text, &[("SKILL.md", 'f')]);
    controlled.file(
        &format!("{text}/SKILL.md"),
        "---\ndescription: Demonstration.\n---\n# Demo\n",
    );
    assert_eq!(
        load_skills_from_dir(
            LoadSkillsFromDirOptions {
                dir: text,
                source: "example"
            },
            &controlled
        )
        .unwrap(),
        result
    );
    assert_eq!(
        format_skills_for_prompt(&result.skills),
        format!(
            "\n\nThe following skills provide specialized instructions for specific tasks.\nUse the read tool to load a skill's file when the task matches its description.\nWhen a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.\n\n<available_skills>\n  <skill>\n    <name>{}</name>\n    <description>Demonstration.</description>\n    <location>{text}/SKILL.md</location>\n  </skill>\n</available_skills>",
            dir.file_name().unwrap().to_str().unwrap()
        )
    );
    std::fs::remove_file(dir.join("SKILL.md")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

fn fixture(name: &str) -> maestro_resources::LoadSkillsResult {
    let dir = format!(
        "{}/tests/fixtures/skills/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: &dir,
            source: "test",
        },
        &NativeResourceOperations,
    )
    .unwrap()
}
#[test]
fn valid_skill_retains_source() {
    let result = fixture("valid-skill");
    assert_eq!(result.skills[0].name, "valid-skill");
    assert_eq!(
        result.skills[0].description,
        "A valid skill for testing purposes."
    );
    assert_eq!(result.skills[0].source_info.source, "test");
    assert!(result.diagnostics.is_empty());
}
#[test]
fn parent_mismatch_warns_without_omission() {
    let result = fixture("name-mismatch");
    assert_eq!(result.skills[0].name, "different-name");
    assert_eq!(
        result.diagnostics[0].message,
        "name \"different-name\" does not match parent directory \"name-mismatch\""
    );
}
#[test]
fn description_then_name_warning_order() {
    let r = metadata(&format!(
        "name: '-{}--'\ndescription: {}",
        "A".repeat(65),
        "x".repeat(1025)
    ));
    assert_eq!(
        r.diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        vec![
            "description exceeds 1024 characters (1025)",
            &format!(
                "name \"-{}--\" does not match parent directory \"parent\"",
                "A".repeat(65)
            ),
            "name exceeds 64 characters (68)",
            "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)",
            "name must not start or end with a hyphen",
            "name must not contain consecutive hyphens"
        ]
    );
}
#[test]
fn missing_description_warns_and_omits() {
    let r = fixture("missing-description");
    assert!(r.skills.is_empty());
    assert_eq!(r.diagnostics[0].message, "description is required");
}
#[test]
fn nested_skill_is_discovered() {
    let r = fixture("nested");
    assert_eq!(r.skills[0].name, "child-skill");
    assert!(r.diagnostics.is_empty());
}
#[path = "support/resource_operations.rs"]
mod support;
use support::Controlled;
fn metadata(yaml: &str) -> maestro_resources::LoadSkillsResult {
    load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: "/parent",
            source: "test",
        },
        &Controlled::skill(&format!("---\n{yaml}\n---")),
    )
    .unwrap()
}
#[test]
fn typed_metadata_preserves_truthiness() {
    for name in ["null", "false", "0", "-0", ".nan", "''"] {
        let r = metadata(&format!("name: {name}\ndescription: text"));
        assert_eq!(r.skills[0].name, "parent");
        assert!(r.diagnostics.is_empty());
    }
    for name in ["true", "1", "[]", "{}"] {
        let r = metadata(&format!("name: {name}\ndescription: text"));
        assert!(r.skills.is_empty());
        assert_eq!(
            r.diagnostics.last().unwrap().message,
            "skill name must be a string"
        );
        assert_eq!(r.diagnostics.len(), 1);
    }
    let r = metadata("name: {toString: 1}\ndescription: text");
    assert_eq!(r.diagnostics[0].message, "skill name must be a string");
    for description in ["null", "false", "0", "-0", ".nan", "''"] {
        let r = metadata(&format!("description: {description}"));
        assert!(r.skills.is_empty());
        assert_eq!(r.diagnostics[0].message, "description is required");
    }
    for description in ["true", "1", "[]", "{}"] {
        let r = metadata(&format!("name: Invalid\ndescription: {description}"));
        assert_eq!(r.diagnostics.len(), 1);
        assert_eq!(
            r.diagnostics[0].message,
            "skill description must be a string"
        );
    }
    let r = metadata("name: true");
    assert_eq!(
        r.diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        vec!["description is required", "skill name must be a string"]
    );
    assert!(
        metadata("description: text\ndisable-model-invocation: true").skills[0]
            .disable_model_invocation
    );
    assert!(
        !metadata("description: text\ndisable-model-invocation: 'true'").skills[0]
            .disable_model_invocation
    );
}

#[test]
fn invalid_name_characters_warn_without_omission() {
    let r = fixture("invalid-name-chars");
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.skills[0].name, "Invalid_Name");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("invalid characters"))
    );
}

#[test]
fn long_name_warns_without_omission() {
    let r = fixture("long-name");
    assert_eq!(r.skills.len(), 1);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("exceeds 64 characters"))
    );
}

#[test]
fn extra_metadata_is_ignored_by_loader() {
    let r = fixture("unknown-field");
    assert_eq!(r.skills.len(), 1);
    assert!(r.diagnostics.is_empty());
}

#[test]
fn root_skill_prevents_child_discovery() {
    let r = fixture("root-skill-preferred");
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.skills[0].name, "root-skill-preferred");
    assert_eq!(r.skills[0].description, "Root skill should win.");
    assert!(r.diagnostics.is_empty());
}

#[test]
fn plain_markdown_without_description_is_omitted() {
    let r = fixture("no-frontmatter");
    assert!(r.skills.is_empty());
    assert_eq!(r.diagnostics[0].message, "description is required");
}

#[test]
fn bad_yaml_warns_and_omits() {
    let r = fixture("invalid-yaml");
    assert!(r.skills.is_empty());
    assert!(r.diagnostics[0].message.contains("line"));
}

#[test]
fn multiline_skill_description_is_preserved() {
    let r = fixture("multiline-description");
    assert_eq!(r.skills.len(), 1);
    assert!(r.skills[0].description.contains("\n"));
    assert!(
        r.skills[0]
            .description
            .contains("This is a multiline description.")
    );
    assert!(r.diagnostics.is_empty());
}

#[test]
fn double_hyphen_name_warns_without_omission() {
    let r = fixture("consecutive-hyphens");
    assert_eq!(r.skills.len(), 1);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("consecutive hyphens"))
    );
}

#[test]
fn fixture_tree_loads_described_skills() {
    let r = fixture("");
    assert!(r.skills.len() >= 6);
    let mut names = r.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>();
    names.sort();
    let mut expected = vec![
        "bad--name",
        "disable-model-invocation",
        "Invalid_Name",
        "this-is-a-very-long-skill-name-that-exceeds-the-sixty-four-character-limit-set-by-the-standard",
        "multiline-description",
        "different-name",
        "child-skill",
        "root-skill-preferred",
        "unknown-field",
        "valid-skill",
    ];
    expected.sort();
    assert_eq!(names, expected);
}

#[test]
fn absent_scan_directory_is_quiet() {
    let r = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: "/non/existent/path",
            source: "test",
        },
        &NativeResourceOperations,
    )
    .unwrap();
    assert!(r.skills.is_empty());
    assert!(r.diagnostics.is_empty());
}

#[test]
fn valid_fixture_retains_declared_name() {
    let r = fixture("valid-skill");
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.skills[0].name, "valid-skill");
}

#[test]
fn boolean_disable_flag_is_read() {
    let r = fixture("disable-model-invocation");
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.skills[0].name, "disable-model-invocation");
    assert!(r.skills[0].disable_model_invocation);
    assert!(
        !r.diagnostics
            .iter()
            .any(|d| d.message.contains("unknown frontmatter field"))
    );
}

#[test]
fn missing_disable_flag_is_false() {
    let r = fixture("valid-skill");
    assert_eq!(r.skills.len(), 1);
    assert!(!r.skills[0].disable_model_invocation);
}

#[test]
fn absent_name_uses_directory_basename() {
    let r = metadata("description: text");
    assert_eq!(r.skills[0].name, "parent");
    assert!(r.diagnostics.is_empty());
}

#[test]
fn empty_skill_list_has_empty_prompt() {
    assert_eq!(format_skills_for_prompt(&[]), "");
}

#[test]
fn prompt_xml_uses_skill_file_location() {
    let mut s = fixture("valid-skill").skills.remove(0);
    s.name = "test-skill".into();
    s.description = "A test skill.".into();
    s.file_path = "/path/to/skill/SKILL.md".into();
    s.base_dir = "/path/to/skill".into();
    let p = format_skills_for_prompt(&[s]);
    for text in [
        "<available_skills>",
        "</available_skills>",
        "<skill>",
        "<name>test-skill</name>",
        "<description>A test skill.</description>",
        "<location>/path/to/skill/SKILL.md</location>",
    ] {
        assert!(p.contains(text));
    }
}

#[test]
fn prompt_intro_precedes_xml() {
    let p = format_skills_for_prompt(&fixture("valid-skill").skills);
    let intro = p.split("<available_skills>").next().unwrap();
    assert!(intro.contains("The following skills provide specialized instructions"));
    assert!(intro.contains("Use the read tool to load a skill's file"));
    assert_eq!(
        intro,
        "\n\nThe following skills provide specialized instructions for specific tasks.\nUse the read tool to load a skill's file when the task matches its description.\nWhen a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.\n\n"
    );
}

#[test]
fn prompt_xml_escapes_special_characters() {
    let mut s = fixture("valid-skill").skills.remove(0);
    s.name = "<&>\"'".into();
    s.description = "A skill with <special> & \"characters\". '".into();
    s.file_path = "<&>\"'".into();
    let p = format_skills_for_prompt(&[s]);
    for t in [
        "&lt;special&gt;",
        "&amp;",
        "&quot;characters&quot;",
        "&apos;",
        "<name>&lt;&amp;&gt;&quot;&apos;</name>",
        "<location>&lt;&amp;&gt;&quot;&apos;</location>",
    ] {
        assert!(p.contains(t));
    }
}

#[test]
fn prompt_keeps_multiple_skill_order() {
    let mut a = fixture("valid-skill").skills.remove(0);
    let mut b = a.clone();
    a.name = "skill-one".into();
    b.name = "skill-two".into();
    let p = format_skills_for_prompt(&[a, b]);
    assert!(p.contains("<name>skill-one</name>"));
    assert!(p.contains("<name>skill-two</name>"));
    assert!(p.find("skill-one") < p.find("skill-two"));
    assert_eq!(p.matches("<skill>").count(), 2);
}

#[test]
fn prompt_omits_disabled_member() {
    let mut a = fixture("valid-skill").skills.remove(0);
    let mut b = a.clone();
    a.name = "visible-skill".into();
    b.name = "hidden-skill".into();
    b.disable_model_invocation = true;
    let p = format_skills_for_prompt(&[a, b]);
    assert!(p.contains("<name>visible-skill</name>"));
    assert!(!p.contains("hidden-skill"));
    assert_eq!(p.matches("<skill>").count(), 1);
}

#[test]
fn fully_disabled_list_has_empty_prompt() {
    assert_eq!(
        format_skills_for_prompt(&fixture("disable-model-invocation").skills),
        ""
    );
}

#[test]
fn utf16_limits_and_trim_membership() {
    for (n, warn) in [(64, false), (65, true)] {
        let r = metadata(&format!("name: {}\ndescription: text", "a".repeat(n)));
        assert_eq!(
            r.diagnostics
                .iter()
                .any(|d| d.message.contains("exceeds 64")),
            warn
        );
    }
    for (n, warn) in [(1024, false), (1025, true)] {
        let r = metadata(&format!("description: '{}'", "a".repeat(n)));
        assert_eq!(
            r.diagnostics
                .iter()
                .any(|d| d.message.contains("exceeds 1024")),
            warn
        );
    }
    let r = metadata(&format!(
        "name: {}\ndescription: '{}'",
        "😀".repeat(65),
        "😀".repeat(1025)
    ));
    assert_eq!(
        r.diagnostics[0].message,
        "description exceeds 1024 characters (1025)"
    );
    assert_eq!(r.diagnostics[2].message, "name exceeds 64 characters (65)");
    assert_eq!(
        metadata("description: '\u{feff}'").skills[0].description,
        "\u{feff}"
    );
    assert!(metadata("description: '\u{85}'").skills.is_empty());
    assert_eq!(
        metadata("description: ' text '").skills[0].description,
        " text "
    );
}
fn explicit(
    paths: &[String],
    ops: &dyn maestro_resources::ResourceOperations,
) -> Result<maestro_resources::LoadSkillsResult, maestro_resources::ResourceError> {
    maestro_resources::load_skills(
        maestro_resources::LoadSkillsOptions {
            cwd: "/work",
            agent_dir: "/agent",
            skill_paths: paths,
            include_defaults: true,
            config_dir_name: ".maestro",
            home: "/home",
        },
        ops,
    )
}
#[test]
fn explicit_skill_path_has_temporary_scope() {
    let path = format!(
        "{}/tests/fixtures/skills/valid-skill",
        env!("CARGO_MANIFEST_DIR")
    );
    let r = explicit(&[path], &NativeResourceOperations).unwrap();
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.skills[0].source_info.scope, "temporary");
    assert!(r.diagnostics.is_empty());
}
#[test]
fn explicit_path_kinds_and_errors() {
    use maestro_resources::{ResourceError, Stats};
    let mut ops = Controlled::default();
    ops.file("/work/file.md", "---\ndescription: text\n---");
    ops.file("/work/file.MD", "text");
    ops.stats.insert(
        "/work/other".into(),
        Ok(Stats {
            is_file: false,
            is_directory: false,
        }),
    );
    ops.stats.insert(
        "/work/bad".into(),
        Err(ResourceError {
            message: Some("stat failed".into()),
        }),
    );
    ops.stats.insert(
        "/work/nonerror".into(),
        Err(ResourceError { message: None }),
    );
    ops.file("/work/read.md", "");
    ops.files
        .insert("/work/read.md".into(), Err(ResourceError { message: None }));
    ops.file("/work/yaml.md", "---\nfoo: [bar\n---");
    let paths = [
        "missing", "file.md", "file.MD", "other", "bad", "nonerror", "read.md", "yaml.md",
    ]
    .map(str::to_owned);
    let r = explicit(&paths, &ops).unwrap();
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.skills[0].file_path, "/work/file.md");
    let messages = r
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        &messages[..5],
        &[
            "skill path does not exist",
            "skill path is not a markdown file",
            "skill path is not a markdown file",
            "stat failed",
            "failed to read skill path"
        ]
    );
    assert_eq!(messages[5], "failed to parse skill file");
    assert!(messages[6].contains("line"));
}
#[test]
fn default_scopes_precede_explicit_paths() {
    let mut ops = Controlled::default();
    for (dir, name) in [
        ("/agent/skills", "user"),
        ("/work/.maestro/skills", "project"),
        ("/explicit", "explicit"),
    ] {
        ops.dir(dir, &[("SKILL.md", 'f')]);
        ops.file(
            &format!("{dir}/SKILL.md"),
            &format!("---\nname: {name}\ndescription: text\n---"),
        );
    }
    let r = explicit(&["/explicit".into()], &ops).unwrap();
    assert_eq!(
        r.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["user", "project", "explicit"]
    );
    assert_eq!(
        r.skills
            .iter()
            .map(|s| s.source_info.scope.as_str())
            .collect::<Vec<_>>(),
        vec!["user", "project", "temporary"]
    );
    for (dir, scope) in [
        ("/agent/skills", "user"),
        ("/work/.maestro/skills", "project"),
        ("/explicit", "temporary"),
    ] {
        let r = maestro_resources::load_skills(
            maestro_resources::LoadSkillsOptions {
                cwd: "/work",
                agent_dir: "/agent",
                skill_paths: &[dir.into()],
                include_defaults: false,
                config_dir_name: ".maestro",
                home: "/home",
            },
            &ops,
        )
        .unwrap();
        assert_eq!(r.skills[0].source_info.scope, scope);
    }
}
#[test]
fn canonical_deduplication_precedes_name_collisions() {
    let mut ops = Controlled::default();
    for p in ["/a.md", "/alias.md", "/b.md"] {
        ops.file(p, "---\nname: same\ndescription: text\n---");
    }
    ops.real.insert("/alias.md".into(), Ok("/a.md".into()));
    let r = explicit(
        &[
            "/a.md".into(),
            "/alias.md".into(),
            "/b.md".into(),
            "/b.md".into(),
        ],
        &ops,
    )
    .unwrap();
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.skills[0].file_path, "/a.md");
    let collisions = r
        .diagnostics
        .iter()
        .filter(|d| d.r#type == "collision")
        .collect::<Vec<_>>();
    assert_eq!(collisions.len(), 2);
    assert_eq!(collisions[0].message, "name \"same\" collision");
    assert_eq!(
        collisions[0].collision.as_ref().unwrap().winner_path,
        "/a.md"
    );
}
#[test]
fn ignore_files_share_ordered_matcher() {
    let mut ops = Controlled::default();
    ops.dir(
        "/scan",
        &[
            ("a.md", 'f'),
            ("b.md", 'f'),
            ("c.md", 'f'),
            ("one", 'd'),
            ("two", 'd'),
        ],
    );
    for name in ["a", "b", "c"] {
        ops.file(
            &format!("/scan/{name}.md"),
            &format!("---\nname: {name}\ndescription: text\n---"),
        );
    }
    ops.file("/scan/.gitignore", "a.md\nb.md\nc.md");
    ops.file("/scan/.ignore", "!a.md\n!b.md");
    ops.file("/scan/.fdignore", "b.md");
    ops.dir("/scan/one", &[]);
    ops.file("/scan/one/.ignore", "/../../two/\n");
    ops.dir("/scan/two", &[("SKILL.md", 'f')]);
    ops.file("/scan/two/SKILL.md", "---\ndescription: two\n---");
    let r = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: "/scan",
            source: "test",
        },
        &ops,
    )
    .unwrap();
    assert_eq!(
        r.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["a", "two"]
    );
    let reset = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: "/scan/two",
            source: "test",
        },
        &ops,
    )
    .unwrap();
    assert_eq!(reset.skills.len(), 1);
}

#[test]
fn missing_explicit_path_warns() {
    let r = explicit(&["/non/existent/path".into()], &NativeResourceOperations).unwrap();
    assert!(r.skills.is_empty());
    assert_eq!(r.diagnostics[0].message, "skill path does not exist");
    assert_eq!(r.diagnostics[0].path.as_deref(), Some("/non/existent/path"));
}

#[test]
fn tilde_path_matches_populated_home_path() {
    let mut ops = Controlled::default();
    ops.dir("/home/skills", &[("SKILL.md", 'f')]);
    ops.file("/home/skills/SKILL.md", "---\ndescription: text\n---");
    let a = explicit(&["~/skills".into()], &ops).unwrap();
    let b = explicit(&["/home/skills".into()], &ops).unwrap();
    assert_eq!(a.skills.len(), b.skills.len());
    assert_eq!(a, b);
    assert_eq!(a.skills.len(), 1);
}

#[test]
fn loader_collisions_follow_other_diagnostics() {
    let base = format!(
        "{}/tests/fixtures/skills-collision",
        env!("CARGO_MANIFEST_DIR")
    );
    let r = explicit(
        &[
            format!("{base}/first"),
            "/missing".into(),
            format!("{base}/second"),
            format!(
                "{}/tests/fixtures/skills/invalid-yaml",
                env!("CARGO_MANIFEST_DIR")
            ),
        ],
        &NativeResourceOperations,
    )
    .unwrap();
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.skills[0].name, "calendar");
    assert_eq!(r.diagnostics[0].message, "skill path does not exist");
    assert!(r.diagnostics[1].message.contains("line"));
    let d = &r.diagnostics[2];
    assert_eq!(d.r#type, "collision");
    assert_eq!(d.message, "name \"calendar\" collision");
    let c = d.collision.as_ref().unwrap();
    assert_eq!(c.resource_type, "skill");
    assert_eq!(c.name, "calendar");
    assert_eq!(c.winner_path, format!("{base}/first/calendar/SKILL.md"));
    assert_eq!(c.loser_path, format!("{base}/second/calendar/SKILL.md"));
    assert_eq!(d.path.as_deref(), Some(c.loser_path.as_str()));
    assert_eq!(c.winner_source, None);
    assert_eq!(c.loser_source, None);
}

#[test]
fn inline_collision_simulation_keeps_first() {
    let base = format!(
        "{}/tests/fixtures/skills-collision",
        env!("CARGO_MANIFEST_DIR")
    );
    let first = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: &format!("{base}/first"),
            source: "first",
        },
        &NativeResourceOperations,
    )
    .unwrap();
    let second = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: &format!("{base}/second"),
            source: "second",
        },
        &NativeResourceOperations,
    )
    .unwrap();
    let mut map = std::collections::HashMap::new();
    let mut warnings = vec![];
    for s in first.skills {
        map.insert(s.name.clone(), s);
    }
    for s in second.skills {
        if let Some(existing) = map.get(&s.name) {
            warnings.push(format!(
                "name collision: \"{}\" already loaded from {}",
                s.name, existing.file_path
            ));
        } else {
            map.insert(s.name.clone(), s);
        }
    }
    assert_eq!(map.len(), 1);
    assert_eq!(map["calendar"].source_info.source, "first");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("name collision"));
}

#[test]
fn source_metadata_keeps_optional_fields() {
    use maestro_resources::*;
    let metadata = PathMetadata {
        source: "arbitrary".into(),
        scope: "custom".into(),
        origin: "new".into(),
        base_dir: Some("".into()),
    };
    let s = create_source_info("p", &metadata);
    assert_eq!(
        s,
        SourceInfo {
            path: "p".into(),
            source: "arbitrary".into(),
            scope: "custom".into(),
            origin: "new".into(),
            base_dir: Some("".into())
        }
    );
    let s = create_synthetic_source_info("p", "", None, None, None);
    assert_eq!(s.scope, "temporary");
    assert_eq!(s.origin, "top-level");
    assert_eq!(s.base_dir, None);
    let s = create_synthetic_source_info("p", "", Some(""), Some(""), Some(""));
    assert_eq!(
        (s.scope, s.origin, s.base_dir),
        ("".into(), "".into(), Some("".into()))
    );
    let d = ResourceDiagnostic {
        r#type: "custom".into(),
        message: "text".into(),
        path: None,
        collision: Some(ResourceCollision {
            resource_type: "new".into(),
            name: "n".into(),
            winner_path: "a".into(),
            loser_path: "b".into(),
            winner_source: Some("".into()),
            loser_source: None,
        }),
    };
    assert_eq!(d.path, None);
    assert_eq!(d.collision.unwrap().winner_source, Some("".into()));
}

#[test]
fn root_candidate_attempt_controls_descent() {
    for kind in ['f', 'd', 'l'] {
        let mut ops = Controlled::default();
        ops.dir("/scan", &[("SKILL.md", kind), ("child", 'd')]);
        ops.dir("/scan/child", &[("SKILL.md", 'f')]);
        ops.file("/scan/child/SKILL.md", "---\ndescription: child\n---");
        ops.file("/scan/SKILL.md", "---\nx: [\n---");
        let r = load_skills_from_dir(
            LoadSkillsFromDirOptions {
                dir: "/scan",
                source: "test",
            },
            &ops,
        )
        .unwrap();
        if kind == 'f' {
            assert!(r.skills.is_empty());
            assert_eq!(r.diagnostics.len(), 1);
        } else if kind == 'd' {
            assert_eq!(r.skills[0].description, "child");
        } else {
            assert!(r.skills.is_empty());
        }
    }
    let mut ops = Controlled::skill("---\ndescription: root\n---");
    ops.file("/parent/.ignore", "SKILL.md");
    let r = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: "/parent",
            source: "test",
        },
        &ops,
    )
    .unwrap();
    assert!(r.skills.is_empty());
}

#[test]
fn root_markdown_and_symlink_traversal() {
    use maestro_resources::{ResourceError, Stats};
    let mut ops = Controlled::default();
    ops.dir(
        "/scan",
        &[
            ("z.md", 'f'),
            (".hidden", 'd'),
            ("node_modules", 'd'),
            ("nested", 'd'),
            ("file-link.md", 'l'),
            ("dir-link", 'l'),
            ("broken", 'l'),
        ],
    );
    ops.file("/scan/z.md", "---\nname: z\ndescription: z\n---");
    ops.dir("/scan/nested", &[("ignored.md", 'f')]);
    ops.file("/scan/nested/ignored.md", "---\ndescription: ignored\n---");
    ops.stats.insert(
        "/scan/file-link.md".into(),
        Ok(Stats {
            is_file: true,
            is_directory: false,
        }),
    );
    ops.file(
        "/scan/file-link.md",
        "---\nname: linked\ndescription: linked\n---",
    );
    ops.stats.insert(
        "/scan/dir-link".into(),
        Ok(Stats {
            is_file: false,
            is_directory: true,
        }),
    );
    ops.dir("/scan/dir-link", &[("SKILL.md", 'f')]);
    ops.file(
        "/scan/dir-link/SKILL.md",
        "---\ndescription: directory\n---",
    );
    ops.stats
        .insert("/scan/broken".into(), Err(ResourceError { message: None }));
    let r = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: "/scan",
            source: "test",
        },
        &ops,
    )
    .unwrap();
    assert_eq!(
        r.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["z", "linked", "dir-link"]
    );
}

#[test]
fn read_failures_keep_partial_results() {
    use maestro_resources::ResourceError;
    let mut ops = Controlled::default();
    ops.dir(
        "/scan",
        &[("good.md", 'f'), ("bad.md", 'f'), ("unreadable", 'd')],
    );
    ops.file("/scan/good.md", "---\ndescription: good\n---");
    ops.files.insert(
        "/scan/bad.md".into(),
        Err(ResourceError {
            message: Some("read failed".into()),
        }),
    );
    ops.files
        .insert("/scan/.ignore".into(), Err(ResourceError { message: None }));
    ops.dirs.insert(
        "/scan/unreadable".into(),
        Err(ResourceError { message: None }),
    );
    let r = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            dir: "/scan",
            source: "test",
        },
        &ops,
    )
    .unwrap();
    assert_eq!(r.skills.len(), 1);
    assert_eq!(r.diagnostics[0].message, "read failed");
    let path = std::env::temp_dir().join(format!("maestro-utf8-{}", std::process::id()));
    std::fs::write(&path, b"a\xffb").unwrap();
    assert_eq!(
        maestro_resources::ResourceOperations::read_file(
            &NativeResourceOperations,
            path.to_str().unwrap()
        )
        .unwrap(),
        "a�b"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn ignore_prefix_and_parent_rules_match() {
    for (pattern, name, ignored) in [
        ("FILE.md", "file.md", true),
        ("/file.md", "file.md", true),
        ("# comment\n", "file.md", false),
        ("[abc", "[abc.md", false),
        ("*.md\n!file.md", "file.md", false),
        ("\\#file.md", "#file.md", true),
        ("[ab].md", "a.md", true),
        ("**/*.md", "file.md", true),
    ] {
        let mut ops = Controlled::default();
        ops.dir("/scan", &[(name, 'f')]);
        ops.file(&format!("/scan/{name}"), "---\ndescription: text\n---");
        ops.file("/scan/.ignore", pattern);
        let r = load_skills_from_dir(
            LoadSkillsFromDirOptions {
                dir: "/scan",
                source: "test",
            },
            &ops,
        )
        .unwrap();
        assert_eq!(r.skills.is_empty(), ignored, "{pattern}");
    }
    let mut ops = Controlled::default();
    ops.dir("/scan", &[("child", 'd')]);
    ops.dir("/scan/child", &[("SKILL.md", 'f')]);
    ops.file("/scan/child/SKILL.md", "---\ndescription: child\n---");
    ops.file("/scan/.ignore", "child/\n!child/SKILL.md");
    assert!(
        load_skills_from_dir(
            LoadSkillsFromDirOptions {
                dir: "/scan",
                source: "test"
            },
            &ops
        )
        .unwrap()
        .skills
        .is_empty()
    );
}

#[test]
fn fixture_text_is_preserved() {
    assert_eq!(
        include_str!("fixtures/skills/consecutive-hyphens/SKILL.md"),
        r###"---
name: bad--name
description: A skill with consecutive hyphens in the name.
---

# Consecutive Hyphens

This skill has consecutive hyphens in its name.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/disable-model-invocation/SKILL.md"),
        r###"---
name: disable-model-invocation
description: A skill that cannot be invoked by the model.
disable-model-invocation: true
---

# Manual Only Skill

This skill can only be invoked via /skill:disable-model-invocation.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/invalid-name-chars/SKILL.md"),
        r###"---
name: Invalid_Name
description: A skill with invalid characters in the name.
---

# Invalid Name

This skill has uppercase and underscore in the name.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/invalid-yaml/SKILL.md"),
        r###"---
name: invalid-yaml
description: [unclosed bracket
---

# Invalid YAML Skill

This skill has invalid YAML in the frontmatter.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/long-name/SKILL.md"),
        r###"---
name: this-is-a-very-long-skill-name-that-exceeds-the-sixty-four-character-limit-set-by-the-standard
description: A skill with a name that exceeds 64 characters.
---

# Long Name

This skill's name is too long.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/missing-description/SKILL.md"),
        r###"---
name: missing-description
---

# Missing Description

This skill has no description field.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/multiline-description/SKILL.md"),
        r###"---
name: multiline-description
description: |
  This is a multiline description.
  It spans multiple lines.
  And should be normalized.
---

# Multiline Description Skill

This skill tests that multiline YAML descriptions are normalized to single lines.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/name-mismatch/SKILL.md"),
        r###"---
name: different-name
description: A skill with a name that doesn't match the directory.
---

# Name Mismatch

This skill's name doesn't match its parent directory.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/nested/child-skill/SKILL.md"),
        r###"---
name: child-skill
description: A nested skill in a subdirectory.
---

# Child Skill

This skill is nested in a subdirectory.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/no-frontmatter/SKILL.md"),
        r###"# No Frontmatter

This skill has no YAML frontmatter at all.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/root-skill-preferred/SKILL.md"),
        r###"---
description: Root skill should win.
---
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/root-skill-preferred/nested-child/SKILL.md"),
        r###"---
description: Nested skill should be ignored.
---
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/unknown-field/SKILL.md"),
        r###"---
name: unknown-field
description: A skill with an unknown frontmatter field.
author: someone
version: 1.0
---

# Unknown Field

This skill has non-standard frontmatter fields.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills/valid-skill/SKILL.md"),
        r###"---
name: valid-skill
description: A valid skill for testing purposes.
---

# Valid Skill

This is a valid skill that follows the Agent Skills standard.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills-collision/first/calendar/SKILL.md"),
        r###"---
name: calendar
description: First calendar skill.
---

# Calendar (First)

This is the first calendar skill.
"###
    );
    assert_eq!(
        include_str!("fixtures/skills-collision/second/calendar/SKILL.md"),
        r###"---
name: calendar
description: Second calendar skill.
---

# Calendar (Second)

This is the second calendar skill.
"###
    );
}

#[test]
fn skills_doc_matches_delivered_interface() {
    let doc = include_str!("../../../docs/skills.md");
    assert!(doc.contains(r###"# Skill resources"###));
    assert!(doc.contains(r###"Load a supplied directory with `load_skills_from_dir`. Each retained `Skill` contains its name, description, file path, base directory, source information and model-invocation flag. Filesystem operations are supplied by the caller."###));
    assert!(doc.contains(r###"`load_skills` reads user skills before project skills, followed by explicit paths in input order. With defaults enabled, explicit paths have temporary scope. With defaults disabled, explicit paths beneath the supplied user or project skill directory retain that scope. Canonical file duplicates are omitted before name collisions; the first name wins."###));
    assert!(doc.contains(r###"Validation warnings do not prevent loading unless the description is missing or blank, or reading, parsing or a required string operation fails. Description warnings precede name warnings. Collision diagnostics follow all other diagnostics. An unignored file named `SKILL.md` ends discovery in its directory even when that file fails to load."###));
    assert!(doc.contains(r###"`parse_frontmatter` normalizes line endings and returns typed YAML metadata with the body. A leading three-dash opener and the first newline followed by three dashes delimit the metadata; only a delimited body is trimmed. Empty or null metadata becomes an empty object. `strip_frontmatter` returns the same body and propagates parsing errors."###));
    assert!(doc.contains(r###"`format_skills_for_prompt` renders visible skills in input order, using the skill file path as the location and XML-escaping each field. Only boolean `disable-model-invocation: true` hides a loaded skill. Native callers can supply `NativeResourceOperations`; portable callers provide `ResourceOperations`. Neither loading nor formatting executes a skill."###));
    assert!(doc.contains("let result = load_skills_from_dir("));
    assert!(doc.contains("&NativeResourceOperations"));
    assert!(doc.contains("format_skills_for_prompt(&result.skills)"));
}
