//! The loaded-resource listing through the mode's startup and retained sections.
use super::{
    HomeLookup, ResourceItem, ShowLoadedResourcesOptions, build_scope_groups, format_context_path,
    format_display_path, format_scope_groups, get_short_path,
};
use crate::interactive_mode::startup::ExpandableText;
use crate::interactive_mode::tests::{
    Setup, World, dark, isolated_case, light, painted, roles, start,
};
use crate::{InteractiveMode, InteractiveModeOptions};
use maestro_app::{
    ContextFile, PromptTemplate, Skill, SourceInfo, SourceOrigin, SourceScope, home_directory,
};
use maestro_theme::{Collator, Theme, ThemeColor};
use maestro_tui::Component;
use serde::Deserialize;
use std::{any::Any, cell::Cell, io, path::Path, rc::Rc};

mod loaded;
mod loaded_support;

/// Every recorded listing case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_resources.json");

/// The neutral home directory the recorded listings abbreviate.
const HOME: &str = "/home/user";

/// The width recorded listings render at unless a case names one.
const WIDTH: usize = 220;

/// The recorded operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Input {
    /// A listing at startup, or forced afterwards.
    Listing(Listing),
    /// A listing whose theme changes before its sections toggle.
    ThemeToggle {
        /// The context files.
        #[serde(default)]
        context: Vec<Located>,
        /// The skills.
        #[serde(default)]
        skills: Vec<Recorded>,
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
    /// One package resource path displayed with a supplied home directory.
    Short {
        /// The path.
        path: String,
        /// Where the resource came from.
        #[serde(rename = "sourceInfo")]
        source_info: Option<RecordedSource>,
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
    /// The skills.
    skills: Vec<Recorded>,
    /// The prompt templates the session lists.
    templates: Vec<Recorded>,
    /// The prompt templates of the loader's first read, when they differ.
    #[serde(rename = "loaderTemplates")]
    loader_templates: Option<Vec<Recorded>>,
    /// The render width.
    width: Option<usize>,
}

/// How the mode starts.
#[derive(Deserialize, Default, Clone, Copy)]
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

/// One recorded skill or prompt template.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Recorded {
    /// The resource file.
    #[serde(rename = "filePath")]
    file_path: String,
    /// The name.
    name: String,
    /// Where the resource came from, local project material when absent.
    #[serde(rename = "sourceInfo")]
    source_info: Option<RecordedSource>,
}

/// Where a recorded resource came from.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RecordedSource {
    /// The source label.
    source: String,
    /// The scope.
    scope: SourceScope,
    /// The package root.
    #[serde(rename = "baseDir")]
    base_dir: Option<String>,
}

impl RecordedSource {
    /// The source of the resource at `path`.
    fn into_source_info(self, path: &str) -> SourceInfo {
        SourceInfo {
            path: path.to_owned(),
            source: self.source,
            scope: self.scope,
            origin: SourceOrigin::TopLevel,
            base_dir: self.base_dir,
        }
    }
}

impl Recorded {
    /// The recorded source, local project material when absent.
    fn source_info(&mut self) -> SourceInfo {
        self.source_info
            .take()
            .unwrap_or_else(|| RecordedSource {
                source: "local".to_owned(),
                scope: SourceScope::Project,
                base_dir: None,
            })
            .into_source_info(&self.file_path)
    }

    /// The recorded skill.
    fn into_skill(mut self) -> Skill {
        Skill {
            source_info: self.source_info(),
            name: self.name,
            description: String::new(),
            file_path: self.file_path,
            base_dir: String::new(),
            disable_model_invocation: false,
        }
    }

    /// The recorded prompt template.
    fn into_template(mut self) -> PromptTemplate {
        PromptTemplate {
            source_info: self.source_info(),
            name: self.name,
            description: String::new(),
            argument_hint: None,
            content: String::new(),
            file_path: self.file_path,
        }
    }
}

/// The recorded skills.
fn skills(recorded: Vec<Recorded>) -> Vec<Skill> {
    recorded.into_iter().map(Recorded::into_skill).collect()
}

/// The recorded prompt templates.
fn templates(recorded: Vec<Recorded>) -> Vec<PromptTemplate> {
    recorded.into_iter().map(Recorded::into_template).collect()
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
    crate::interactive_mode::tests::cases(FIXTURES, test)
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

/// Runs `body` in a fresh child whose home directory and locale are the recorded ones.
fn in_home(test: &str, body: impl AsyncFnOnce(&Path)) {
    let environment = [("HOME", HOME), ("LANG", "C.UTF-8")];
    isolated_case(test, "child", &environment, body);
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
async fn check_listing(directory: &Path, listing: Listing, expected: Expected) -> World {
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
    let mut prompts: Vec<_> = listing
        .loader_templates
        .into_iter()
        .map(templates)
        .collect();
    prompts.push(templates(listing.templates));
    let world = Setup {
        options: InteractiveModeOptions {
            verbose,
            ..InteractiveModeOptions::default()
        },
        configure: Box::new(move |settings| settings.set_quiet_startup(quiet)),
        cwd: listing.cwd,
        context: context_files(listing.context),
        skills: skills(listing.skills),
        prompts,
        ..Setup::default()
    }
    .build(directory);
    world.mode.tool_output_expanded.set(tool_output_expanded);
    // A forced listing is a later call; startup's own listing goes through `init`.
    if listing.force {
        let options = ShowLoadedResourcesOptions {
            force: true,
            ..ShowLoadedResourcesOptions::default()
        };
        world.mode.show_loaded_resources(options).unwrap();
    } else {
        start(&world).await;
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
fn listings(test: &str, extra: &impl AsyncFn(&World)) {
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            let Input::Listing(listing) = input else {
                panic!("a listing case");
            };
            extra(&check_listing(directory, listing, expected).await).await;
        }
    });
}

#[test]
fn loaded_resources_use_compact_defaults() {
    listings("loaded_resources_use_compact_defaults", &async |world| {
        let chat = &world.mode.chat_container;
        assert_eq!(
            sections(&world.mode).len(),
            chat.children().borrow().len() / 2
        );
        start(world).await;
        let children = chat.children().borrow().len();
        start(world).await;
        assert_eq!(chat.children().borrow().len(), children);
        assert_eq!(world.terminal.events(), ["start"]);
    });
}

#[test]
fn verbose_resources_override_quiet_startup() {
    listings("verbose_resources_override_quiet_startup", &async |_| {});
}

#[test]
fn quiet_resources_require_force() {
    listings("quiet_resources_require_force", &async |_| {});
}

#[test]
fn empty_resources_do_not_add_chat_children() {
    listings("empty_resources_do_not_add_chat_children", &async |_| {});
}

#[test]
fn context_resources_keep_discovery_order_and_relative_paths() {
    listings(
        "context_resources_keep_discovery_order_and_relative_paths",
        &async |_| {},
    );
}

#[test]
fn loaded_resources_keep_heading_roles_and_section_spacing() {
    listings(
        "loaded_resources_keep_heading_roles_and_section_spacing",
        &async |world| {
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
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            let Input::Listing(listing) = input else {
                panic!("a listing case");
            };
            let width = listing.width.unwrap();
            let leading = usize::from(!listing.context.is_empty());
            let world = check_listing(directory, listing, expected).await;
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
            assert_eq!(section.render(width), rows[leading..rows.len() - 1]);
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
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            let (
                Input::ThemeToggle {
                    context,
                    skills: recorded,
                    width,
                },
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
            let heading = usize::from(!context.is_empty());
            let world = Setup {
                context: context_files(context),
                skills: skills(recorded),
                ..Setup::default()
            }
            .build(directory);
            start(&world).await;
            let dark = world.themes.theme().get().unwrap();
            let chat = &world.mode.chat_container;
            assert_eq!(chat.render(width), painted(&dark, &before));
            let light = light();
            world.themes.set_theme_instance(Rc::clone(&light)).unwrap();
            assert_eq!(chat.render(width), painted(&dark, &no_toggle));
            expand_all(&world.mode, true);
            assert_eq!(
                chat.render(width),
                repainted(&dark, &light, &expanded, heading)
            );
            expand_all(&world.mode, false);
            assert_eq!(
                chat.render(width),
                repainted(&dark, &light, &collapsed, heading)
            );
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
            format_context_path(&path, &cwd, &|| Ok(home.clone())).unwrap(),
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
        assert_eq!(
            format_display_path(&path, &|| Ok(home.clone())).unwrap(),
            value
        );
    }
}

#[test]
fn resource_listing_keeps_root_home_paths_absolute() {
    let test = "resource_listing_keeps_root_home_paths_absolute";
    in_home(test, async |directory| {
        for (home, value) in [
            ("/", "/tmp/supplied-home/file"),
            ("", "/tmp/supplied-home/file"),
            ("/tmp/supplied-home", "~/file"),
        ] {
            let setup = Setup {
                context: context_files(vec![Located {
                    path: "/tmp/supplied-home/file".to_owned(),
                }]),
                ..Setup::default()
            };
            let rows = listed(directory, setup, &|| Ok(home.to_owned())).unwrap();
            let row = dark().fg(&ThemeColor::Dim, &format!("  {value}")).unwrap();
            assert_eq!(rows[2].trim_end(), row, "{home:?} home");
        }
    });
}

#[test]
fn resource_home_lookup_preserves_absent_and_supplied_home_and_corrects_empty_prefix() {
    let test = "resource_home_lookup_preserves_absent_and_supplied_home_and_corrects_empty_prefix";
    for (input, expected) in cases(test) {
        let (Input::NativeHome { mode, path }, Expected::Value { value }) = (input, expected)
        else {
            panic!("a native home case");
        };
        let environment: &[(&str, &str)] = match mode.as_str() {
            "supplied" => &[("HOME", "/tmp/supplied-home")],
            "empty" => &[("HOME", "")],
            other => panic!("an unknown home mode {other}"),
        };
        isolated_case(test, &mode, environment, async |directory| {
            assert_native_home(directory, &path, &value).await;
        });
    }
    isolated_case(test, "absent", &[], async |directory| {
        let (path, value) = native_absent_case();
        assert_native_home(directory, &path, &value).await;
    });
}

/// Checks `path` shows as `value` under the child's home, both formatted and listed.
async fn assert_native_home(directory: &Path, path: &str, value: &str) {
    assert_eq!(format_display_path(path, &home_directory).unwrap(), value);
    let world = Setup {
        context: context_files(vec![Located {
            path: path.to_owned(),
        }]),
        ..Setup::default()
    }
    .build(directory);
    world.mode.tool_output_expanded.set(true);
    world.mode.init().await.unwrap();
    let theme = world.themes.theme().get().unwrap();
    let row = theme.fg(&ThemeColor::Dim, &format!("  {value}")).unwrap();
    assert_eq!(world.mode.chat_container.render(WIDTH)[2].trim_end(), row);
}

/// The path and the display it shows when the account home stands in for an absent HOME.
fn native_absent_case() -> (String, String) {
    let home = home_directory().unwrap();
    if home.is_empty() {
        let path = "/tmp/supplied-home/file".to_owned();
        (path.clone(), path)
    } else if home == "/" {
        (home.clone(), home)
    } else {
        (home, "~".to_owned())
    }
}

/// A home lookup failing as on a host with no home variable and no account entry.
fn unknown_home() -> io::Result<String> {
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "home directory is unknown",
    ))
}

/// The rows a forced, expanded listing over `setup` appends, looking home up through
/// `home`, or its error.
fn listed(directory: &Path, setup: Setup, home: &HomeLookup<'_>) -> Result<Vec<String>, String> {
    let world = setup.build(directory);
    world.mode.tool_output_expanded.set(true);
    world
        .mode
        .list_loaded_resources(
            ShowLoadedResourcesOptions {
                force: true,
                ..ShowLoadedResourcesOptions::default()
            },
            home,
        )
        .map(|()| world.mode.chat_container.render(WIDTH))
        .map_err(|error| error.to_string())
}

/// One resource of `name` below the recorded home directory.
fn under_home(path: &str, name: &str) -> Vec<Recorded> {
    vec![Recorded {
        file_path: format!("{HOME}/{path}"),
        name: name.to_owned(),
        source_info: None,
    }]
}

#[test]
fn resource_listing_looks_up_home_only_to_abbreviate_a_path() {
    let test = "resource_listing_looks_up_home_only_to_abbreviate_a_path";
    in_home(test, async |directory| {
        let prompts = || Setup {
            prompts: vec![templates(under_home("prompt.md", "prompt"))],
            ..Setup::default()
        };
        let rows = listed(directory, prompts(), &home_directory).unwrap();
        assert!(rows.iter().any(|row| row.contains("/prompt")));
        assert_eq!(
            (
                listed(directory, Setup::default(), &unknown_home),
                listed(directory, prompts(), &unknown_home)
            ),
            (Ok(Vec::new()), Ok(rows))
        );
        let context = Setup {
            context: context_files(vec![Located {
                path: format!("{HOME}/AGENTS.md"),
            }]),
            ..Setup::default()
        };
        let skills = Setup {
            skills: skills(under_home("skill/SKILL.md", "skill")),
            ..Setup::default()
        };
        for abbreviating in [context, skills] {
            assert_eq!(
                listed(directory, abbreviating, &unknown_home),
                Err("home directory is unknown".to_owned())
            );
        }
    });
}

#[test]
fn loaded_resources_expand_each_retained_section() {
    listings(
        "loaded_resources_expand_each_retained_section",
        &async |_| {},
    );
}

#[test]
fn resource_scopes_keep_project_user_path_order() {
    listings(
        "resource_scopes_keep_project_user_path_order",
        &async |_| {},
    );
}

#[test]
fn resource_groups_keep_stable_locale_order() {
    listings("resource_groups_keep_stable_locale_order", &async |_| {});
}

#[test]
fn compact_resource_labels_use_ecmascript_space_rules() {
    listings(
        "compact_resource_labels_use_ecmascript_space_rules",
        &async |_| {},
    );
}

#[test]
fn prompt_resources_use_session_templates_and_last_path_lookup() {
    listings(
        "prompt_resources_use_session_templates_and_last_path_lookup",
        &async |world| {
            let rows = world.mode.chat_container.render(WIDTH).join("\n");
            assert!(!rows.contains("NOT_SELECTED"));
        },
    );
}

#[test]
fn package_resources_shorten_from_roots_before_legacy_fallbacks() {
    for (input, expected) in cases("package_resources_shorten_from_roots_before_legacy_fallbacks") {
        let (
            Input::Short {
                path,
                source_info,
                home,
            },
            Expected::Value { value },
        ) = (input, expected)
        else {
            panic!("a package path case");
        };
        let source_info = source_info.map(|source| source.into_source_info(&path));
        let short = get_short_path(&path, source_info.as_ref(), &|| Ok(home.clone())).unwrap();
        assert_eq!(short, value);
    }
}

/// The paths visited while grouping `count` resources and the paths emitted, and the
/// lookups a formatter that rescans every resource per emitted path would make.
fn traversal(count: usize) -> (usize, usize, usize) {
    let paths: Vec<_> = (0..count)
        .rev()
        .map(|index| format!("/r/{index:04}"))
        .collect();
    let items: Vec<_> = paths
        .iter()
        .map(|path| ResourceItem {
            path,
            source_info: None,
        })
        .collect();
    let visited = Cell::new(0);
    let groups = build_scope_groups(items.iter().inspect(|_| visited.set(visited.get() + 1)));
    let emitted = Cell::new(0);
    let rescanned = Cell::new(0);
    let theme = crate::interactive_mode::tests::dark();
    let collator = Collator::new(&|_| None).unwrap();
    format_scope_groups(
        &groups,
        &theme,
        &collator,
        |item| {
            emitted.set(emitted.get() + 1);
            let position = items
                .iter()
                .position(|other| other.path == item.path)
                .unwrap();
            rescanned.set(rescanned.get() + count - position);
            Ok(item.path.to_owned())
        },
        |_| unreachable!("no package resources"),
    )
    .unwrap();
    (visited.get(), emitted.get(), rescanned.get())
}

#[test]
fn resource_listing_reads_each_input_once_before_ordering() {
    for count in [64, 128, 256] {
        let (visited, emitted, rescanned) = traversal(count);
        assert_eq!((visited, emitted), (count, count));
        assert_eq!(rescanned, count * (count + 1) / 2);
        assert!(rescanned > 2 * count);
    }
}
