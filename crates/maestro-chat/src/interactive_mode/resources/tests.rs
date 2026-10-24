//! The loaded-resource listing through the mode's startup and retained sections.
use super::{format_context_path, format_display_path};
use crate::interactive_mode::startup::ExpandableText;
use crate::interactive_mode::tests::{Setup, World, isolated_case, light, painted, roles};
use crate::{InteractiveMode, InteractiveModeOptions};
use maestro_app::{ContextFile, home_directory};
use maestro_theme::{Theme, ThemeColor};
use maestro_tui::Component;
use serde::Deserialize;
use std::{any::Any, path::Path, rc::Rc};

/// Every recorded listing case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_resources.json");

/// The neutral home directory the recorded listings abbreviate.
const HOME: &str = "/home/user";

/// The width recorded listings render at unless a case names one.
const WIDTH: usize = 220;

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
    /// A listing at startup, or forced afterwards.
    Listing(Listing),
    /// A listing whose theme changes before its sections toggle.
    ThemeToggle {
        /// The context files.
        context: Vec<Located>,
        /// The render width.
        width: usize,
    },
    /// One context path displayed against a working directory.
    Context {
        /// The working directory.
        cwd: String,
        /// The context path.
        path: String,
        /// The home directory.
        home: String,
    },
    /// One path displayed with a supplied home directory.
    Display {
        /// The path.
        path: String,
        /// The home directory.
        home: String,
    },
    /// One path displayed with the native home directory.
    NativeHome {
        /// How the home variable is set.
        mode: String,
        /// The path.
        path: String,
    },
}

/// The settings and resources of one listing.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Listing {
    /// How the mode starts.
    startup: Startup,
    /// Whether the listing is forced.
    force: bool,
    /// The working directory, when not the disposable one.
    cwd: Option<String>,
    /// The context files.
    context: Vec<Located>,
    /// The render width.
    width: Option<usize>,
}

/// How the mode starts.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Startup {
    /// Whether startup is quiet.
    quiet: bool,
    /// Whether the mode is verbose.
    verbose: bool,
    /// Whether tool output is expanded.
    expanded: bool,
}

/// One resource path.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Located {
    /// The path.
    path: String,
}

/// The recorded observations.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// Transcript rows at startup, with every section expanded, then collapsed again.
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
    /// One displayed path.
    Value {
        /// The path.
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

/// Context files at the recorded paths.
fn context_files(located: Vec<Located>) -> Vec<ContextFile> {
    located
        .into_iter()
        .map(|file| ContextFile {
            path: file.path,
            content: String::new(),
        })
        .collect()
}

/// Runs `body` in a fresh child whose home directory is the recorded one.
fn in_home(test: &str, body: impl FnOnce(&Path)) {
    isolated_case(test, "child", (&[("HOME", HOME)], &[]), body);
}

/// The retained expandable sections of the transcript.
fn sections(mode: &InteractiveMode) -> Vec<Rc<ExpandableText>> {
    mode.chat_container
        .children()
        .borrow()
        .iter()
        .filter_map(|child| (Rc::clone(child) as Rc<dyn Any>).downcast().ok())
        .collect()
}

/// Expands or collapses every retained section.
fn expand_all(mode: &InteractiveMode, expanded: bool) {
    for section in sections(mode) {
        section.set_expanded(expanded).unwrap();
    }
}

/// Starts a mode over `listing` and checks the recorded rows after startup, with every
/// section expanded and collapsed again, and the transcript's children.
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
    let Startup {
        quiet,
        verbose,
        expanded: tool_output_expanded,
    } = listing.startup;
    let world = Setup {
        options: InteractiveModeOptions { verbose },
        configure: Box::new(move |settings| settings.set_quiet_startup(quiet)),
        cwd: listing.cwd,
        context: context_files(listing.context),
        ..Setup::default()
    }
    .build(directory);
    world.mode.tool_output_expanded.set(tool_output_expanded);
    world.mode.init().unwrap();
    if listing.force {
        world.mode.show_loaded_resources(true).unwrap();
    }
    let width = listing.width.unwrap_or(WIDTH);
    let theme = world.themes.theme().get().unwrap();
    let chat = &world.mode.chat_container;
    assert_eq!(chat.render(width), painted(&theme, &before));
    assert_eq!(chat.children().borrow().len(), children);
    expand_all(&world.mode, true);
    assert_eq!(chat.render(width), painted(&theme, &expanded));
    expand_all(&world.mode, false);
    assert_eq!(chat.render(width), painted(&theme, &collapsed));
    world
}

/// Checks every recorded listing case of `test`, then `extra` on each started mode.
fn listings(test: &str, extra: impl Fn(&World)) {
    in_home(test, |directory| {
        for (input, expected) in cases(test) {
            let Input::Listing(listing) = input else {
                panic!("a listing case");
            };
            extra(&check_listing(directory, listing, expected));
        }
    });
}

#[test]
fn loaded_resources_use_compact_defaults() {
    listings("loaded_resources_use_compact_defaults", |world| {
        let children = world.mode.chat_container.children().borrow().len();
        assert_eq!(sections(&world.mode).len(), children / 2);
        world.mode.init().unwrap();
        assert_eq!(
            world.mode.chat_container.children().borrow().len(),
            children
        );
        assert_eq!(world.terminal.events(), ["start"]);
    });
}

#[test]
fn verbose_resources_override_quiet_startup() {
    listings("verbose_resources_override_quiet_startup", |_| {});
}

#[test]
fn quiet_resources_require_force() {
    listings("quiet_resources_require_force", |_| {});
}

#[test]
fn empty_resources_do_not_add_chat_children() {
    listings("empty_resources_do_not_add_chat_children", |_| {});
}

#[test]
fn context_resources_keep_discovery_order_and_relative_paths() {
    listings(
        "context_resources_keep_discovery_order_and_relative_paths",
        |_| {},
    );
}

#[test]
fn loaded_resources_keep_heading_roles_and_section_spacing() {
    listings(
        "loaded_resources_keep_heading_roles_and_section_spacing",
        |world| {
            let rows = world.mode.chat_container.render(WIDTH);
            let header = world.mode.header_container.render(WIDTH).len();
            assert_eq!(
                world.mode.ui.render(WIDTH)[header..header + rows.len()],
                rows
            );
        },
    );
}

#[test]
fn loaded_resource_sections_retain_content_across_redraws() {
    let test = "loaded_resource_sections_retain_content_across_redraws";
    in_home(test, |directory| {
        for (input, expected) in cases(test) {
            let Input::Listing(listing) = input else {
                panic!("a listing case");
            };
            let width = listing.width.unwrap();
            let world = check_listing(directory, listing, expected);
            let rows = world.mode.chat_container.render(width);
            world.mode.chat_container.invalidate();
            assert_eq!(world.mode.chat_container.render(width), rows);
            *world.loader.context.borrow_mut() = context_files(vec![Located {
                path: "/replaced.md".to_owned(),
            }])
            .into();
            expand_all(&world.mode, false);
            assert_eq!(world.mode.chat_container.render(width), rows);
            let [section] = <[_; 1]>::try_from(sections(&world.mode)).ok().unwrap();
            let World { mode, .. } = world;
            let released = Rc::downgrade(&mode);
            drop(mode);
            assert!(released.upgrade().is_none());
            assert_eq!(section.render(width), rows[1..rows.len() - 1]);
        }
    });
}

/// Expected rows with the heading at `heading` painted by `current` and every other row
/// by `initial`.
fn repainted(initial: &Theme, current: &Theme, rows: &[String], heading: usize) -> Vec<String> {
    let mut painted = painted(initial, rows);
    painted[heading] = roles(current, &rows[heading]);
    painted
}

#[test]
fn resource_section_heading_refreshes_on_toggle_without_restyling_body() {
    let test = "resource_section_heading_refreshes_on_toggle_without_restyling_body";
    in_home(test, |directory| {
        for (input, expected) in cases(test) {
            let (
                Input::ThemeToggle { context, width },
                Expected::Toggle {
                    before,
                    no_toggle,
                    expanded,
                    collapsed,
                },
            ) = (input, expected)
            else {
                panic!("a theme case");
            };
            let world = Setup {
                context: context_files(context),
                ..Setup::default()
            }
            .build(directory);
            world.mode.init().unwrap();
            let dark = world.themes.theme().get().unwrap();
            let chat = &world.mode.chat_container;
            assert_eq!(chat.render(width), painted(&dark, &before));
            let light = light();
            world.themes.set_theme_instance(Rc::clone(&light)).unwrap();
            assert_eq!(chat.render(width), painted(&dark, &no_toggle));
            expand_all(&world.mode, true);
            assert_eq!(chat.render(width), repainted(&dark, &light, &expanded, 1));
            expand_all(&world.mode, false);
            assert_eq!(chat.render(width), repainted(&dark, &light, &collapsed, 1));
        }
    });
}

#[test]
fn context_resources_resolve_lexically_without_touching_files() {
    for (input, expected) in cases("context_resources_resolve_lexically_without_touching_files") {
        let (Input::Context { cwd, path, home }, Expected::Value { value }) = (input, expected)
        else {
            panic!("a context case");
        };
        assert_eq!(
            format_context_path(&path, &cwd, Some(&home)).unwrap(),
            value
        );
    }
}

#[test]
fn resource_paths_abbreviate_supplied_home_prefix() {
    for (input, expected) in cases("resource_paths_abbreviate_supplied_home_prefix") {
        let (Input::Display { path, home }, Expected::Value { value }) = (input, expected) else {
            panic!("a display case");
        };
        assert_eq!(format_display_path(&path, Some(&home)), value);
    }
}

#[test]
fn resource_home_lookup_preserves_absent_and_supplied_home_and_corrects_empty_prefix() {
    let test = "resource_home_lookup_preserves_absent_and_supplied_home_and_corrects_empty_prefix";
    for (input, expected) in cases(test) {
        let (Input::NativeHome { mode, path }, Expected::Value { value }) = (input, expected)
        else {
            panic!("a native home case");
        };
        let environment: (&[(&str, &str)], &[&str]) = match mode.as_str() {
            "absent" => (&[], &["HOME"]),
            "supplied" => (&[("HOME", "/tmp/supplied-home")], &[]),
            "empty" => (&[("HOME", "")], &[]),
            other => panic!("an unknown home mode {other}"),
        };
        isolated_case(test, &mode, environment, |directory| {
            let home = home_directory().ok();
            assert_eq!(format_display_path(&path, home.as_deref()), value);
            let world = Setup {
                context: context_files(vec![Located { path }]),
                ..Setup::default()
            }
            .build(directory);
            world.mode.tool_output_expanded.set(true);
            world.mode.init().unwrap();
            let theme = world.themes.theme().get().unwrap();
            let row = theme.fg(&ThemeColor::Dim, &format!("  {value}")).unwrap();
            assert_eq!(world.mode.chat_container.render(WIDTH)[2].trim_end(), row);
        });
    }
}
