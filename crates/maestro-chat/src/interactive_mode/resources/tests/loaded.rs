//! The extension, theme and diagnostic sections of the loaded-resource listing.
use super::super::labels::{
    format_extension_display_path, get_compact_extension_labels, get_compact_package_source_label,
    get_compact_path_label, get_distinct_suffix_labels,
};
use super::super::{HomeLookup, ResourceItem};
use super::{HOME, RecordedSource, expand_all, in_home, repainted, sections, unknown_home};
use crate::InteractiveModeOptions;
use crate::interactive_mode::tests::{Setup, World, light, painted};
use maestro_app::{SourceInfo, home_directory};
use maestro_theme::{ColorMode, Theme, ThemeOptions};
use maestro_tui::Component;
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    hash::{Hash, Hasher},
    path::Path,
    rc::Rc,
};

/// Every recorded case of these witnesses.
const FIXTURES: &str = include_str!("../../../../tests/fixtures/interactive_loaded_resources.json");

/// The width recorded listings render at unless a case names one.
const WIDTH: usize = 240;

/// The width the recorded theme changes render at.
const TOGGLE_WIDTH: usize = 120;

/// One recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: Input,
    /// The recorded observation.
    expected: Expected,
}

/// The recorded operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Input {
    /// A listing over the recorded resources.
    Listing(Listing),
    /// A listing whose theme changes before its section toggles.
    ThemeToggle {
        /// The section kind.
        kind: String,
        /// The loaded themes.
        #[serde(default)]
        themes: Vec<RecordedTheme>,
        /// The loaded extensions.
        #[serde(default)]
        extensions: Vec<RecordedExtension>,
    },
    /// A listing whose home lookup fails, then succeeds.
    HomeRoutes(Listing),
    /// One compact path label.
    CompactPath {
        /// The resource path.
        path: String,
    },
    /// One extension's compact label.
    ExtensionLabel {
        /// The extension.
        extension: RecordedExtension,
    },
    /// One extension's expanded path.
    ExtensionPath {
        /// The extension path.
        path: String,
    },
    /// One package source's compact label.
    PackageSource {
        /// Where the extension came from.
        #[serde(rename = "sourceInfo")]
        source_info: RecordedSource,
    },
}

/// The settings and resources of one listing.
#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields, default)]
struct Listing {
    /// Whether tool output is expanded.
    expanded: bool,
    /// The loaded themes.
    themes: Vec<RecordedTheme>,
    /// The loaded extensions.
    extensions: Vec<RecordedExtension>,
}

/// One recorded loaded extension.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RecordedExtension {
    /// The extension path.
    path: String,
    /// Where it came from.
    #[serde(rename = "sourceInfo")]
    source_info: RecordedSource,
}

impl RecordedExtension {
    /// The extension's path and provenance.
    fn into_loaded(self) -> (String, SourceInfo) {
        let source_info = self.source_info.into_source_info(&self.path);
        (self.path, source_info)
    }
}

/// One recorded loaded theme.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RecordedTheme {
    /// The authored name.
    name: Option<String>,
    /// The file it was loaded from.
    #[serde(rename = "sourcePath")]
    source_path: Option<String>,
    /// Where it came from.
    #[serde(rename = "sourceInfo")]
    source_info: Option<RecordedSource>,
}

impl RecordedTheme {
    /// The recorded theme, without colours of its own.
    fn into_theme(self) -> Rc<Theme> {
        let path = self.source_path.clone().unwrap_or_default();
        let source_info = self
            .source_info
            .map(|source| Rc::new(RefCell::new(source.into_source_info(&path))));
        let options = ThemeOptions {
            name: self.name,
            source_path: self.source_path,
            source_info,
        };
        Rc::new(Theme::new([], [], ColorMode::Truecolor, options).unwrap())
    }
}

/// The recorded observations.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// Transcript rows after the listing, with every section expanded, then collapsed.
    Listing {
        /// Rows after the listing.
        before: Vec<String>,
        /// Rows with every section expanded.
        expanded: Vec<String>,
        /// Rows with every section collapsed.
        collapsed: Vec<String>,
        /// The transcript's children.
        children: usize,
    },
    /// Transcript rows around a theme change.
    Toggle {
        /// Rows after the listing.
        before: Vec<String>,
        /// Rows after the theme change.
        #[serde(rename = "noToggle")]
        no_toggle: Vec<String>,
        /// Rows with the section expanded.
        expanded: Vec<String>,
        /// Rows with the section collapsed again.
        collapsed: Vec<String>,
    },
    /// Whether the listing needed the home directory, and its rows.
    Home {
        /// Whether a path display looked the home directory up.
        #[serde(rename = "needsHome")]
        needs_home: bool,
        /// Rows after the listing.
        before: Vec<String>,
    },
    /// One label.
    Value {
        /// The label.
        value: String,
    },
}

/// The recorded cases of `test`.
fn cases(test: &str) -> Vec<(Input, Expected)> {
    let all: Vec<Case> = serde_json::from_str(FIXTURES).unwrap();
    let cases: Vec<_> = all
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| (case.input, case.expected))
        .collect();
    assert!(!cases.is_empty());
    cases
}

/// A mode over the recorded resources of `listing`, its tool output expanded as recorded.
fn world(directory: &Path, listing: Listing) -> World {
    let world = Setup {
        options: InteractiveModeOptions::default(),
        themes: listing
            .themes
            .into_iter()
            .map(RecordedTheme::into_theme)
            .collect(),
        extensions: listing
            .extensions
            .into_iter()
            .map(RecordedExtension::into_loaded)
            .collect(),
        ..Setup::default()
    }
    .build(directory);
    world.mode.tool_output_expanded.set(listing.expanded);
    world
}

/// Lists `listing` and checks the recorded rows after the listing, with every section
/// expanded and collapsed again, and the transcript's children.
fn check_listing(directory: &Path, listing: Listing, expected: Expected) -> World {
    let Expected::Listing {
        before,
        expanded,
        collapsed,
        children,
    } = expected
    else {
        panic!("a listing observation");
    };
    let world = world(directory, listing);
    world.mode.show_loaded_resources(false).unwrap();
    let theme = world.themes.theme().get().unwrap();
    let chat = &world.mode.chat_container;
    assert_eq!(chat.render(WIDTH), painted(&theme, &before));
    assert_eq!(chat.children().borrow().len(), children);
    expand_all(&world.mode, true);
    assert_eq!(chat.render(WIDTH), painted(&theme, &expanded));
    expand_all(&world.mode, false);
    assert_eq!(chat.render(WIDTH), painted(&theme, &collapsed));
    world
}

/// Checks every recorded listing case of `test`.
fn listings(test: &str) {
    in_home(test, |directory| {
        for (input, expected) in cases(test) {
            let Input::Listing(listing) = input else {
                panic!("a listing case");
            };
            check_listing(directory, listing, expected);
        }
    });
}

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
    in_home(test, |directory| {
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
            let world = world(
                directory,
                Listing {
                    themes,
                    extensions,
                    ..Listing::default()
                },
            );
            world.mode.show_loaded_resources(false).unwrap();
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
    in_home(test, |directory| {
        for (input, expected) in cases(test) {
            let (Input::HomeRoutes(listing), Expected::Home { needs_home, before }) =
                (input, expected)
            else {
                panic!("a home route case");
            };
            let failing = world(directory, listing.clone());
            let result = failing.mode.list_loaded_resources(false, &unknown_home);
            assert_eq!(result.is_err(), needs_home);
            if let Err(error) = result {
                assert_eq!(error.to_string(), "home directory is unknown");
                assert!(failing.mode.chat_container.children().borrow().is_empty());
            }
            let world = world(directory, listing);
            world
                .mode
                .list_loaded_resources(false, &home_directory)
                .unwrap();
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
    in_home(test, |directory| {
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
        let (Input::PackageSource { source_info }, Expected::Value { value }) = (input, expected)
        else {
            panic!("a package source case");
        };
        assert_eq!(get_compact_package_source_label(&source_info.source), value);
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
        .map(|index| format!("/root/{index}/same/tools/index.ts"))
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
            .map(|index| format!("{index}/same/tools"))
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
