//! Area order, listed areas, forbidden members and the combined settings and locks.
use crate::areas::{Composition, ReleaseFiles, read};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// A release held in memory: root-relative file paths and their text.
#[derive(Default)]
struct Release(BTreeMap<String, String>);

impl Release {
    /// Adds a settings document to the area at `area`; the common area is `""`.
    fn settings(self, area: &str, document: &Value) -> Self {
        self.file(&join(area, "settings/settings.json"), &document.to_string())
    }

    /// Adds a policy document to the area at `area`.
    fn policy(self, area: &str, document: &Value) -> Self {
        self.file(&join(area, "settings/policy.json"), &document.to_string())
    }

    /// Adds a file by its root-relative path.
    fn file(mut self, path: &str, text: &str) -> Self {
        self.0.insert(path.to_owned(), text.to_owned());
        self
    }

    /// Reads the release with the supplied `manifestAreas` request.
    fn read(&self, requested: Option<&Value>) -> Result<Composition, String> {
        read(self, requested)
    }
}

/// Joins an area folder and a path below it.
fn join(area: &str, path: &str) -> String {
    if area.is_empty() {
        path.to_owned()
    } else {
        format!("{area}/{path}")
    }
}

impl ReleaseFiles for Release {
    fn folders(&self, folder: &str) -> Vec<String> {
        let prefix = if folder.is_empty() {
            String::new()
        } else {
            format!("{folder}/")
        };
        let names: BTreeSet<_> = self
            .0
            .keys()
            .filter_map(|path| path.strip_prefix(&prefix))
            .filter_map(|rest| rest.split_once('/').map(|(name, _)| name.to_owned()))
            .collect();
        names.into_iter().collect()
    }

    fn text(&self, path: &str) -> Result<Option<String>, String> {
        Ok(self.0.get(path).cloned())
    }
}

/// The effective value of one top-level settings key, null when absent.
fn value(composition: &Composition, key: &str) -> Value {
    composition
        .settings
        .get(key)
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn an_empty_manifest_root_is_valid_and_contributes_nothing() {
    let composition = Release::default().read(None).unwrap();
    assert!(composition.settings.is_empty());
    assert!(composition.locked.is_empty());
}

#[test]
fn common_standards_and_core_combine_in_that_order_with_a_later_area_winning() {
    let release = Release::default()
        .settings(
            "",
            &json!({"order": "common", "images": {"blockImages": true, "autoResize": true}, "only-common": 1}),
        )
        .settings("standards/b-security", &json!({"order": "b", "images": {"autoResize": false}}))
        .settings("standards/a-engineering", &json!({"order": "a", "from-a": true}))
        .settings("core", &json!({"order": "core", "from-core": true}))
        .file("standards/README.md", "not a domain");
    let composition = release.read(None).unwrap();
    assert_eq!(value(&composition, "order"), "core");
    assert_eq!(value(&composition, "only-common"), 1);
    assert_eq!(value(&composition, "from-a"), true);
    assert_eq!(value(&composition, "from-core"), true);
    assert_eq!(
        value(&composition, "images"),
        json!({"blockImages": true, "autoResize": false})
    );
}

#[test]
fn standards_domains_load_in_directory_name_order() {
    let release = Release::default()
        .settings("standards/b", &json!({"winner": "b"}))
        .settings("standards/a", &json!({"winner": "a"}))
        .settings("standards/c", &json!({"winner": "c"}));
    assert_eq!(value(&release.read(None).unwrap(), "winner"), "c");
}

#[test]
fn listed_languages_load_before_listed_capabilities_each_in_list_order() {
    let release = Release::default()
        .settings("core", &json!({"winner": "core"}))
        .settings("languages/go", &json!({"winner": "go", "go": true}))
        .settings("languages/rust", &json!({"winner": "rust", "rust": true}))
        .settings(
            "capabilities/team/one",
            &json!({"winner": "one", "one": true}),
        )
        .settings(
            "capabilities/team/two",
            &json!({"winner": "two", "two": true}),
        );
    let request = json!([
        "capabilities/team/two",
        "languages/rust",
        "capabilities/team/one",
        "languages/go"
    ]);
    let composition = release.read(Some(&request)).unwrap();
    assert_eq!(value(&composition, "winner"), "one");
    for loaded in ["go", "rust", "one", "two"] {
        assert_eq!(value(&composition, loaded), true, "{loaded}");
    }
    let swapped = json!(["languages/rust", "languages/go"]);
    assert_eq!(
        value(&release.read(Some(&swapped)).unwrap(), "winner"),
        "go"
    );
}

#[test]
fn a_duplicate_entry_loads_once_and_unlisted_areas_are_never_read() {
    let release = Release::default()
        .settings("languages/rust", &json!({"count": 1}))
        .settings("languages/unlisted", &json!({"unlisted": true}))
        .settings(
            "capabilities/team/unlisted",
            &json!({"unlisted-capability": true}),
        )
        .policy("languages/unlisted", &json!({"locked": ["unlisted-lock"]}));
    let request = json!(["languages/rust", "languages/rust"]);
    let composition = release.read(Some(&request)).unwrap();
    assert_eq!(value(&composition, "count"), 1);
    assert!(!composition.settings.contains_key("unlisted"));
    assert!(!composition.settings.contains_key("unlisted-capability"));
    assert!(composition.locked.is_empty());
}

#[test]
fn the_manifests_own_area_list_is_the_default_and_a_request_replaces_it() {
    let release = Release::default()
        .settings("", &json!({"manifestAreas": ["languages/rust"]}))
        .settings("languages/rust", &json!({"rust": true}))
        .settings("languages/go", &json!({"go": true}));
    let default = release.read(None).unwrap();
    assert_eq!(value(&default, "rust"), true);
    assert!(!default.settings.contains_key("go"));
    let replaced = release.read(Some(&json!(["languages/go"]))).unwrap();
    assert_eq!(value(&replaced, "go"), true);
    assert!(!replaced.settings.contains_key("rust"));
    let emptied = release.read(Some(&json!([]))).unwrap();
    assert!(!emptied.settings.contains_key("rust") && !emptied.settings.contains_key("go"));
}

#[test]
fn an_unknown_area_or_a_foreign_prefix_makes_the_manifest_invalid() {
    let release = Release::default().settings("languages/rust", &json!({}));
    let long = format!("languages/{}", "x".repeat(65));
    let cases = [
        (json!(["languages/missing"]), "does not exist"),
        (json!(["capabilities/team/missing"]), "does not exist"),
        (json!(["core"]), "languages/<name>"),
        (json!(["standards/security"]), "languages/<name>"),
        (json!(["languages/Rust"]), "languages/<name>"),
        (json!(["languages/rust/extra"]), "languages/<name>"),
        (json!(["languages/"]), "languages/<name>"),
        (json!(["languages/../core"]), "languages/<name>"),
        (json!(["capabilities/team"]), "capabilities/<group>/<name>"),
        (json!([long]), "languages/<name>"),
        (json!(["languages/rust", 7]), "list of area paths"),
        (json!("languages/rust"), "list of area paths"),
    ];
    for (request, expected) in cases {
        let problem = release
            .read(Some(&request))
            .expect_err(&request.to_string());
        assert!(problem.contains(expected), "{request}: {problem}");
    }
}

#[test]
fn a_selection_member_in_a_language_or_capability_makes_the_manifest_invalid() {
    for key in [
        "manifestAreas",
        "manifest",
        "minimumRelease",
        "updateCheckMinutes",
    ] {
        for area in ["languages/rust", "capabilities/team/one"] {
            let release = Release::default().settings(area, &json!({ key: 1 }));
            let problem = release
                .read(Some(&json!([area])))
                .expect_err(&format!("{area} {key}"));
            assert!(problem.contains(area) && problem.contains(key), "{problem}");
        }
    }
}

#[test]
fn a_selection_constraint_in_a_language_or_capability_makes_the_manifest_invalid() {
    let policies = [
        json!({"minimumRelease": "1.0.0"}),
        json!({"updateCheckMinutes": 5}),
        json!({"locked": ["manifest"]}),
        json!({"locked": ["defaultModel", "manifestAreas"]}),
    ];
    for policy in policies {
        let release = Release::default().policy("languages/rust", &policy);
        let problem = release
            .read(Some(&json!(["languages/rust"])))
            .expect_err(&policy.to_string());
        assert!(
            problem.contains("languages/rust/settings/policy.json"),
            "{problem}"
        );
    }
}

#[test]
fn the_common_standards_and_core_areas_may_set_selection_members() {
    let release = Release::default()
        .settings(
            "",
            &json!({"manifestAreas": [], "manifestAutoUpdate": false}),
        )
        .policy(
            "",
            &json!({"locked": ["manifest", "manifestAreas"], "updateCheckMinutes": 30}),
        )
        .policy("core", &json!({"minimumRelease": "1.5.1"}));
    let composition = release.read(None).unwrap();
    assert_eq!(value(&composition, "manifestAreas"), json!([]));
    assert!(composition.locked.contains("manifest"));
}

#[test]
fn a_resource_path_key_in_any_area_makes_the_manifest_invalid() {
    for key in ["extensions", "skills", "prompts", "themes"] {
        for area in ["", "standards/security", "core", "languages/rust"] {
            let release = Release::default().settings(area, &json!({ key: ["x"] }));
            let problem = release
                .read(Some(&json!(["languages/rust"])))
                .expect_err(&format!("{area:?} {key}"));
            assert!(problem.contains(key), "{problem}");
            assert!(
                problem.contains(&join(area, "settings/settings.json")),
                "{problem}"
            );
        }
    }
}

#[test]
fn a_malformed_file_names_its_path() {
    let cases = [
        ("core/settings/settings.json", "{", "not valid JSON"),
        ("core/settings/settings.json", "[]", "JSON object"),
        ("settings/policy.json", "7", "JSON object"),
        (
            "standards/a/settings/policy.json",
            "{\"locked\": \"manifest\"}",
            "`locked`",
        ),
        (
            "standards/a/settings/policy.json",
            "{\"locked\": [1]}",
            "`locked`",
        ),
    ];
    for (path, text, expected) in cases {
        let problem = Release::default()
            .file(path, text)
            .read(None)
            .expect_err(path);
        assert!(
            problem.contains(path) && problem.contains(expected),
            "{problem}"
        );
    }
}

#[test]
fn locks_join_across_areas_without_duplicates() {
    let release = Release::default()
        .policy("", &json!({"locked": ["manifest", "defaultModel"]}))
        .policy(
            "core",
            &json!({"locked": ["defaultModel", "images.blockImages"]}),
        )
        .policy("languages/rust", &json!({"locked": ["compaction.enabled"]}));
    let composition = release.read(Some(&json!(["languages/rust"]))).unwrap();
    assert_eq!(
        composition
            .locked
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "compaction.enabled",
            "defaultModel",
            "images.blockImages",
            "manifest"
        ]
    );
}
