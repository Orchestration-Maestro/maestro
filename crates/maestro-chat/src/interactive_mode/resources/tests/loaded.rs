//! The extension, theme and diagnostic sections of the loaded-resource listing.
use super::super::diagnostics::{
    SourceInfoRef, SourceInfos, format_diagnostics, format_path_with_source,
    get_built_in_command_conflict_diagnostics, get_display_source_info,
};
use super::super::labels::{
    format_extension_display_path, get_compact_extension_labels, get_compact_package_source_label,
    get_compact_path_label, get_distinct_suffix_labels,
};
use super::super::{HomeLookup, ResourceItem};
use super::loaded_support::{
    Expected, Input, Listing, WIDTH, cases, check_listing, commands, diagnostics, identity,
    listings, world,
};
use super::{HOME, RecordedSource, expand_all, in_home, repainted, sections, unknown_home};
use crate::interactive_mode::tests::{Setup, World, dark, light, painted, roles};
use maestro_app::{ResourceDiagnostic, SourceInfo, home_directory};
use maestro_tui::Component;
use serde_json::json;
use std::{
    cell::Cell,
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hash, Hasher},
    rc::Rc,
};

/// The width the recorded theme changes render at.
const TOGGLE_WIDTH: usize = 120;

#[test]
fn theme_listing_omits_unlocated_and_keeps_empty_named_theme() {
    listings("theme_listing_omits_unlocated_and_keeps_empty_named_theme");
}

#[test]
fn theme_listing_uses_scope_and_package_paths() {
    listings("theme_listing_uses_scope_and_package_paths");
}

#[test]
fn empty_resources_and_diagnostics_append_nothing() {
    listings("empty_resources_and_diagnostics_append_nothing");
}

#[test]
fn compact_path_labels_keep_empty_root_and_separator_rules() {
    for (input, expected) in cases("compact_path_labels_keep_empty_root_and_separator_rules") {
        let (Input::CompactPath { path }, Expected::Value { value }) = (input, expected) else {
            panic!("a compact path case");
        };
        let item = ResourceItem {
            path: &path,
            source_info: None,
        };
        assert_eq!(get_compact_path_label(&item, RECORDED_HOME).unwrap(), value);
    }
}

#[test]
fn extension_and_theme_sections_keep_owned_content_across_theme_and_owner_changes() {
    let test = "extension_and_theme_sections_keep_owned_content_across_theme_and_owner_changes";
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            let (
                Input::ThemeToggle {
                    kind,
                    themes,
                    extensions,
                },
                Expected::Toggle {
                    before,
                    no_toggle,
                    expanded,
                    collapsed,
                },
            ) = (input, expected)
            else {
                panic!("a theme change case");
            };
            assert_eq!(kind == "theme", extensions.is_empty());
            let (world, request) = world(
                directory,
                Listing {
                    themes,
                    extensions,
                    ..Listing::default()
                },
            );
            request.list(&world, &home_directory).unwrap();
            let dark = world.themes.theme().get().unwrap();
            let chat = &world.mode.chat_container;
            assert_eq!(chat.render(TOGGLE_WIDTH), painted(&dark, &before));
            let light = light();
            world.themes.set_theme_instance(Rc::clone(&light)).unwrap();
            assert_eq!(chat.render(TOGGLE_WIDTH), painted(&dark, &no_toggle));
            expand_all(&world.mode, true);
            assert_eq!(
                chat.render(TOGGLE_WIDTH),
                repainted(&dark, &light, &expanded, 0)
            );
            expand_all(&world.mode, false);
            let rows = repainted(&dark, &light, &collapsed, 0);
            assert_eq!(chat.render(TOGGLE_WIDTH), rows);
            let [section] = <[_; 1]>::try_from(sections(&world.mode)).ok().unwrap();
            let World { mode, .. } = world;
            let released = Rc::downgrade(&mode);
            drop(mode);
            assert!(released.upgrade().is_none());
            assert_eq!(section.render(TOGGLE_WIDTH), rows[..rows.len() - 1]);
        }
    });
}

#[test]
fn resource_home_queries_follow_the_reached_formatting_branch() {
    let test = "resource_home_queries_follow_the_reached_formatting_branch";
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            let (Input::HomeRoutes(listing), Expected::Home { needs_home, before }) =
                (input, expected)
            else {
                panic!("a home route case");
            };
            let (failing, request) = world(directory, listing.clone());
            let result = request.list(&failing, &unknown_home);
            assert_eq!(result.is_err(), needs_home);
            if let Err(error) = result {
                assert_eq!(error.to_string(), "home directory is unknown");
                assert!(failing.mode.chat_container.children().borrow().is_empty());
            }
            let (world, request) = world(directory, listing);
            request.list(&world, &home_directory).unwrap();
            let theme = world.themes.theme().get().unwrap();
            assert_eq!(
                world.mode.chat_container.render(WIDTH),
                painted(&theme, &before)
            );
        }
    });
}

#[test]
fn extension_listing_uses_short_file_labels() {
    listings("extension_listing_uses_short_file_labels");
}

#[test]
fn extension_listing_keeps_mixed_compact_labels() {
    listings("extension_listing_keeps_mixed_compact_labels");
}

#[test]
fn extension_labels_add_only_needed_parent_segments() {
    listings("extension_labels_add_only_needed_parent_segments");
}

#[test]
fn extension_labels_shorten_typescript_indexes() {
    listings("extension_labels_shorten_typescript_indexes");
}

#[test]
fn extension_labels_shorten_javascript_indexes() {
    listings("extension_labels_shorten_javascript_indexes");
}

#[test]
fn extension_labels_mix_files_and_folder_indexes() {
    listings("extension_labels_mix_files_and_folder_indexes");
}

#[test]
fn extension_labels_keep_distinct_folder_names_short() {
    listings("extension_labels_keep_distinct_folder_names_short");
}

#[test]
fn extension_labels_disambiguate_equal_folder_names() {
    listings("extension_labels_disambiguate_equal_folder_names");
}

#[test]
fn extension_labels_keep_non_index_filenames() {
    listings("extension_labels_keep_non_index_filenames");
}

#[test]
fn extension_labels_preserve_duplicates_and_exclude_packages_from_uniqueness() {
    listings("extension_labels_preserve_duplicates_and_exclude_packages_from_uniqueness");
}

#[test]
fn extension_listing_groups_full_paths_by_scope() {
    listings("extension_listing_groups_full_paths_by_scope");
}

/// Looks up the home directory the recorded labels abbreviate.
const RECORDED_HOME: &HomeLookup<'static> = &|| Ok(HOME.to_owned());

#[test]
fn extension_labels_keep_package_identity() {
    let test = "extension_labels_keep_package_identity";
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            match (input, expected) {
                (Input::Listing(listing), expected) => {
                    check_listing(directory, listing, expected);
                }
                (Input::ExtensionLabel { extension }, Expected::Value { value }) => {
                    let (path, source_info) = extension.into_loaded();
                    let item = ResourceItem {
                        path: &path,
                        source_info: Some(&source_info),
                    };
                    let labels = get_compact_extension_labels(&[item], RECORDED_HOME).unwrap();
                    assert_eq!(labels, [value]);
                }
                _ => panic!("a package label case"),
            }
        }
    });
}

#[test]
fn extension_display_paths_strip_only_terminal_forward_slash_indexes() {
    let test = "extension_display_paths_strip_only_terminal_forward_slash_indexes";
    for (input, expected) in cases(test) {
        let (Input::ExtensionPath { path }, Expected::Value { value }) = (input, expected) else {
            panic!("an extension path case");
        };
        assert_eq!(
            format_extension_display_path(&path, RECORDED_HOME).unwrap(),
            value
        );
    }
}

#[test]
fn extension_labels_keep_package_source_forms() {
    for (input, expected) in cases("extension_labels_keep_package_source_forms") {
        let (Input::PackageSource { source }, Expected::Value { value }) = (input, expected) else {
            panic!("a package source case");
        };
        assert_eq!(get_compact_package_source_label(&source), value);
    }
}

/// A path segment that counts every hash and comparison made with it.
struct Counted<'a> {
    /// The segment.
    segment: &'a str,
    /// The shared count of hashes and comparisons.
    work: &'a Cell<usize>,
}

impl Hash for Counted<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.work.set(self.work.get() + 1);
        self.segment.hash(state);
    }
}

impl PartialEq for Counted<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.work.set(self.work.get() + 1);
        self.segment == other.segment
    }
}

impl Eq for Counted<'_> {}

impl AsRef<str> for Counted<'_> {
    fn as_ref(&self) -> &str {
        self.segment
    }
}

/// `count` local extension paths sharing every suffix but their second segment.
fn equal_suffixes(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("/root/{index:03}/same/tools/index.ts"))
        .collect()
}

/// The segments of `paths` as the labels see them, counting work into `work`.
fn counted<'a>(paths: &'a [String], work: &'a Cell<usize>) -> Vec<Vec<Counted<'a>>> {
    paths
        .iter()
        .map(|path| {
            let segments = path.trim_end_matches("/index.ts").split('/');
            segments
                .filter(|segment| !segment.is_empty())
                .map(|segment| Counted { segment, work })
                .collect()
        })
        .collect()
}

/// The work a selection comparing each candidate suffix with every other path does.
fn rescanned(paths: &[Vec<Counted<'_>>]) -> usize {
    for (index, segments) in paths.iter().enumerate() {
        for length in 1..=segments.len() {
            let candidate = &segments[segments.len() - length..];
            let shared = paths.iter().enumerate().any(|(other, others)| {
                other != index
                    && others.len() >= length
                    && others[others.len() - length..] == *candidate
            });
            if !shared {
                break;
            }
        }
    }
    paths[0][0].work.get()
}

#[test]
fn extension_label_work_stays_linear_in_total_path_segments() {
    let mut previous = None;
    for count in [32, 64, 128] {
        let paths = equal_suffixes(count);
        let work = Cell::new(0);
        let segments = counted(&paths, &work);
        let labels = get_distinct_suffix_labels(&segments);
        let total: usize = segments.iter().map(Vec::len).sum();
        assert!(work.get() <= 4 * total, "{} > 4 * {total}", work.get());
        let expected: Vec<_> = (0..count)
            .map(|index| format!("{index:03}/same/tools"))
            .collect();
        assert_eq!(labels, expected);
        let items: Vec<_> = paths
            .iter()
            .map(|path| ResourceItem {
                path,
                source_info: None,
            })
            .collect();
        let listed = get_compact_extension_labels(&items, RECORDED_HOME).unwrap();
        assert_eq!(listed, expected);
        if let Some(previous) = previous {
            assert!(work.get() <= 2 * previous + total, "work grows linearly");
        }
        previous = Some(work.get());
        let control = Cell::new(0);
        assert!(rescanned(&counted(&paths, &control)) > 4 * total);
    }
}

#[test]
fn extension_listing_work_stays_linear_through_the_listing() {
    let test = "extension_listing_work_stays_linear_through_the_listing";
    in_home(test, async |directory| {
        let mut previous = None;
        for count in [32, 64, 128] {
            let paths = equal_suffixes(count);
            let extensions: Vec<_> = paths
                .iter()
                .map(|path| json!({"path": path, "sourceInfo": {"source": "local", "scope": "project"}}))
                .collect();
            let listing = json!({"extensions": extensions, "width": 4096});
            let (world, request) = world(directory, serde_json::from_value(listing).unwrap());
            let lookups = Cell::new(0);
            let home = || {
                lookups.set(lookups.get() + 1);
                home_directory()
            };
            request.list(&world, &home).unwrap();
            let work = Cell::new(0);
            let labels = get_distinct_suffix_labels(&counted(&paths, &work)).join(", ");
            let theme = world.themes.theme().get().unwrap();
            let rows = world.mode.chat_container.render(request.width);
            let compact = painted(&theme, &[format!("@dim@  {labels}\u{1b}[39m")]);
            assert_eq!(rows[1].trim_end(), compact[0]);
            if let Some(previous) = previous {
                assert!(lookups.get() <= 2 * previous, "home lookups grow linearly");
            }
            previous = Some(lookups.get());
        }
    });
}

/// An index of the recorded paths and their provenance, in insertion order.
fn indexed(recorded: &[(String, SourceInfo)]) -> SourceInfos<'_> {
    let mut index = SourceInfos::default();
    for (path, source_info) in recorded {
        index.insert(path, SourceInfoRef::Resource(source_info));
    }
    index
}

/// The recorded paths with their provenance.
fn provenance(recorded: Vec<(String, RecordedSource)>) -> Vec<(String, SourceInfo)> {
    recorded
        .into_iter()
        .map(|(path, source)| {
            let source_info = source.into_source_info(&path);
            (path, source_info)
        })
        .collect()
}

#[test]
fn diagnostic_sources_preserve_local_cli_package_scope_labels() {
    for (input, expected) in cases("diagnostic_sources_preserve_local_cli_package_scope_labels") {
        let (Input::Source { source_info }, Expected::Display { value }) = (input, expected) else {
            panic!("a source label case");
        };
        let source_info = source_info.map(|source| source.into_source_info(""));
        let (label, scope) = get_display_source_info(source_info.as_ref());
        assert_eq!(
            (label, scope),
            (value.label.as_str(), value.scope_label.as_deref())
        );
    }
}

#[test]
fn diagnostic_source_lookup_selects_exact_then_nearest_forward_slash_parent() {
    let test = "diagnostic_source_lookup_selects_exact_then_nearest_forward_slash_parent";
    for (input, expected) in cases(test) {
        let (Input::Lookup { path, source_infos }, Expected::Found { value }) = (input, expected)
        else {
            panic!("a lookup case");
        };
        let recorded = provenance(source_infos);
        let index = indexed(&recorded);
        let found = index.find(&path).map(|found| match found {
            SourceInfoRef::Resource(info) => (info.source.clone(), info.scope),
            SourceInfoRef::Theme(_) => panic!("only resource provenance is indexed"),
        });
        let expected = value.as_ref().map(identity);
        assert_eq!(
            found
                .as_ref()
                .map(|(source, scope)| (source.as_str(), *scope)),
            expected
        );
    }
}

#[test]
fn diagnostic_paths_keep_source_scope_and_short_path() {
    for (input, expected) in cases("diagnostic_paths_keep_source_scope_and_short_path") {
        let (Input::PathSource { path, source_info }, Expected::Value { value }) =
            (input, expected)
        else {
            panic!("a path source case");
        };
        let source_info = source_info.map(|source| source.into_source_info(&path));
        let source = source_info.as_ref().map(SourceInfoRef::Resource);
        let shown = format_path_with_source(&path, source.as_ref(), RECORDED_HOME).unwrap();
        assert_eq!(shown, value);
    }
}

/// Checks every recorded diagnostic text case of `test`.
fn diagnostic_texts(test: &str) {
    let theme = dark();
    for (input, expected) in cases(test) {
        let (
            Input::Diagnostics {
                diagnostics: recorded,
                source_infos,
            },
            Expected::Value { value },
        ) = (input, expected)
        else {
            panic!("a diagnostics case");
        };
        let recorded = diagnostics(recorded);
        let borrowed: Vec<&ResourceDiagnostic> = recorded.iter().collect();
        let provenance = provenance(source_infos);
        let formatted = format_diagnostics(&borrowed, &indexed(&provenance), &theme, RECORDED_HOME);
        assert_eq!(formatted.unwrap(), roles(&theme, &value));
    }
}

#[test]
fn resource_collisions_group_before_other_diagnostics() {
    diagnostic_texts("resource_collisions_group_before_other_diagnostics");
}

#[test]
fn resource_diagnostics_keep_path_presence_kind_and_exact_lines() {
    diagnostic_texts("resource_diagnostics_keep_path_presence_kind_and_exact_lines");
}

#[test]
fn resource_diagnostics_remain_visible_when_quiet() {
    let test = "resource_diagnostics_remain_visible_when_quiet";
    in_home(test, async |directory| {
        let mut startup = None;
        for (input, expected) in cases(test) {
            let Input::Listing(listing) = input else {
                panic!("a listing case");
            };
            let quiet_request = listing.startup.quiet
                && !listing.startup.verbose
                && !listing.force
                && listing.diagnostics;
            if quiet_request {
                startup = Some((listing.clone(), expected));
                continue;
            }
            check_listing(directory, listing, expected);
        }
        let (mut listing, expected) = startup.expect("a quiet request for diagnostics");
        let Expected::Listing { before, .. } = expected else {
            panic!("a listing observation");
        };
        listing.diagnostics = false;
        let (world, _) = world(directory, listing);
        world.mode.init().await.unwrap();
        // Binding publishes the loaded themes before the listing reads its resources.
        assert_eq!(
            *world.loader.calls.borrow(),
            [
                "themes",
                "prompts",
                "skills",
                "skills",
                "prompts",
                "themes",
                "extensions",
                "extensions"
            ]
        );
        let theme = world.themes.theme().get().unwrap();
        assert_eq!(
            world.mode.chat_container.render(WIDTH),
            painted(&theme, &before)
        );
    });
}

#[test]
fn empty_extension_override_replaces_loader_listing_but_not_errors() {
    listings("empty_extension_override_replaces_loader_listing_but_not_errors");
}

#[test]
fn extension_override_supplies_own_provenance() {
    listings("extension_override_supplies_own_provenance");
}

#[test]
fn resource_source_index_keeps_snapshot_precedence() {
    listings("resource_source_index_keeps_snapshot_precedence");
}

#[test]
fn diagnostic_sections_keep_static_styles_spacing_and_order() {
    let test = "diagnostic_sections_keep_static_styles_spacing_and_order";
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            let Input::Listing(listing) = input else {
                panic!("a listing case");
            };
            let world = check_listing(directory, listing, expected);
            assert!(sections(&world.mode).is_empty());
        }
    });
}

/// The built-in conflicts of `recorded` registered for real, with the command
/// diagnostics the runner published before and after resolving them.
type Resolved = (Vec<ResourceDiagnostic>, Vec<String>, usize);

/// Registers `recorded` on a mode's extensions, publishes a command diagnostic and
/// resolves the built-in conflicts.
fn resolve(
    directory: &std::path::Path,
    recorded: Vec<super::loaded_support::RecordedCommand>,
) -> Resolved {
    let world = Setup {
        commands: commands(recorded),
        ..Setup::default()
    }
    .build(directory);
    let runner = world.mode.runtime_host.session().extension_runner();
    let retained = runner.get_command_diagnostics();
    retained
        .write()
        .unwrap()
        .push(diagnostics(retained_diagnostic()).remove(0));
    let conflicts = get_built_in_command_conflict_diagnostics(&runner);
    let messages = retained
        .read()
        .unwrap()
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    let current = runner.get_command_diagnostics().read().unwrap().len();
    (conflicts, messages, current)
}

/// The command diagnostic the runner published before resolution.
fn retained_diagnostic() -> Vec<super::loaded_support::RecordedDiagnostic> {
    serde_json::from_str(r#"[{"type": "warning", "message": "retained command diagnostic"}]"#)
        .unwrap()
}

/// Checks `actual` against the recorded diagnostics, field by field.
fn same_diagnostics(
    actual: &[ResourceDiagnostic],
    recorded: Vec<super::loaded_support::RecordedDiagnostic>,
) {
    let expected = diagnostics(recorded);
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(&expected) {
        assert_eq!(actual.r#type, expected.r#type);
        assert_eq!(actual.message, expected.message);
        assert_eq!(actual.path, expected.path);
        assert!(actual.collision.is_none() && expected.collision.is_none());
    }
}

#[test]
fn extension_issues_follow_registration_and_builtin_conflicts() {
    let test = "extension_issues_follow_registration_and_builtin_conflicts";
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            match (input, expected) {
                (
                    Input::RegisteredBuiltins { commands },
                    Expected::Registered {
                        value,
                        retained,
                        current,
                    },
                ) => {
                    let (conflicts, messages, now) = resolve(directory, commands);
                    same_diagnostics(&conflicts, value);
                    let retained: Vec<_> = diagnostics(retained)
                        .into_iter()
                        .map(|diagnostic| diagnostic.message)
                        .collect();
                    assert_eq!(messages, retained);
                    assert_eq!(now, current.len());
                }
                (Input::Builtins { commands }, Expected::Conflicts { value }) => {
                    let (conflicts, ..) = resolve(directory, commands);
                    same_diagnostics(&conflicts, value);
                }
                (Input::Listing(listing), expected) => {
                    check_listing(directory, listing, expected);
                }
                _ => panic!("an extension issue case"),
            }
        }
    });
}

/// A hasher state counting the bytes hashed into it.
struct CountingHasher<'a> {
    /// The standard state the bytes go on to.
    inner: std::collections::hash_map::DefaultHasher,
    /// The shared count of bytes hashed.
    bytes: &'a Cell<usize>,
}

impl Hasher for CountingHasher<'_> {
    fn finish(&self) -> u64 {
        self.inner.finish()
    }
    fn write(&mut self, bytes: &[u8]) {
        self.bytes.set(self.bytes.get() + bytes.len());
        self.inner.write(bytes);
    }
}

/// Builds standard hasher states counting into one shared byte count.
struct Counting<'a>(RandomState, &'a Cell<usize>);

impl<'a> BuildHasher for Counting<'a> {
    type Hasher = CountingHasher<'a>;
    fn build_hasher(&self) -> Self::Hasher {
        CountingHasher {
            inner: self.0.build_hasher(),
            bytes: self.1,
        }
    }
}

/// A path of `count` nested segments below the root.
fn nested(count: usize) -> String {
    let segments: Vec<_> = (0..count).map(|index| format!("/s{index}")).collect();
    segments.concat()
}

#[test]
fn diagnostic_lookup_work_stays_linear_in_total_path_bytes() {
    let source_info = SourceInfo {
        path: String::new(),
        source: "near".to_owned(),
        scope: maestro_app::SourceScope::Project,
        origin: maestro_app::SourceOrigin::TopLevel,
        base_dir: None,
    };
    let mut previous = None;
    for count in [32, 64, 128] {
        let path = nested(count);
        let parent = nested(count - 1);
        let bytes = Cell::new(0);
        let mut index = SourceInfos::with_hasher(Counting(RandomState::new(), &bytes));
        index.insert(&parent, SourceInfoRef::Resource(&source_info));
        let found = index.find(&format!("{path}/file"));
        assert!(matches!(found, Some(SourceInfoRef::Resource(info)) if info.source == "near"));
        // Each hashed segment adds its parent's position and an end marker; growing the
        // index hashes its stored segments again, at most doubling the total.
        let input = parent.len() + path.len() + "/file".len();
        let bound = 2 * (input + 16 * (2 * count + 2));
        let hashed = bytes.get();
        assert!(hashed <= bound, "{hashed} bytes for {input}");
        if let Some(previous) = previous {
            assert!(2 * hashed <= 5 * previous, "lookup work grows linearly");
        }
        previous = Some(hashed);
        // Hashing each whole parent prefix, as a lookup keyed by full paths does, exceeds
        // the bound once paths are long.
        let prefixes: usize = (1..=count).map(|depth| nested(depth).len()).sum();
        if count == 128 {
            assert!(prefixes > bound);
        }
    }
}
