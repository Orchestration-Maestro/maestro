//! The recorded cases of the extension, theme and diagnostic witnesses, and the mode
//! each listing case runs on.
use super::super::{HomeLookup, ResourceItem, ShowLoadedResourcesOptions};
use super::{Recorded, RecordedSource, Startup, expand_all, in_home, skills, templates};
use crate::InteractiveModeOptions;
use crate::interactive_mode::tests::{LoadedDiagnostics, Setup, World, painted};
use maestro_app::{
    DiagnosticType, ExtensionLoadError, ResourceCollision, ResourceDiagnostic, SourceInfo,
    SourceScope, home_directory,
};
use maestro_theme::{ColorMode, Theme, ThemeOptions};
use maestro_tui::Component;
use serde::Deserialize;
use std::{cell::RefCell, error::Error, mem::take, path::Path, rc::Rc};

/// Every recorded case of these witnesses.
const FIXTURES: &str = include_str!("../../../../tests/fixtures/interactive_loaded_resources.json");

/// The width recorded listings render at unless a case names one.
pub(super) const WIDTH: usize = 240;

/// The recorded operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Input {
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
        /// The package source.
        source: String,
    },
    /// The source label of one provenance, or of none.
    Source {
        /// The provenance.
        #[serde(rename = "sourceInfo")]
        source_info: Option<RecordedSource>,
    },
    /// The provenance a diagnostic path finds among indexed paths.
    Lookup {
        /// The diagnostic path.
        path: String,
        /// The indexed paths and their provenance, in insertion order.
        #[serde(rename = "sourceInfos")]
        source_infos: Vec<(String, RecordedSource)>,
    },
    /// One path shown with its provenance, or with none.
    PathSource {
        /// The path.
        path: String,
        /// The provenance.
        #[serde(rename = "sourceInfo")]
        source_info: Option<RecordedSource>,
    },
    /// Diagnostic text over indexed paths.
    Diagnostics {
        /// The diagnostics.
        diagnostics: Vec<RecordedDiagnostic>,
        /// The indexed paths and their provenance, in insertion order.
        #[serde(rename = "sourceInfos")]
        source_infos: Vec<(String, RecordedSource)>,
    },
    /// Built-in conflicts of registered commands, with the command diagnostics the
    /// runner had published before.
    RegisteredBuiltins {
        /// The registered commands.
        commands: Vec<RecordedCommand>,
    },
    /// Built-in conflicts of registered commands.
    Builtins {
        /// The registered commands.
        commands: Vec<RecordedCommand>,
    },
}

/// The settings and resources of one listing.
#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields, default)]
pub(super) struct Listing {
    /// How the mode starts.
    pub(super) startup: Startup,
    /// Whether the listing is forced.
    pub(super) force: bool,
    /// Whether diagnostics are asked for even when the listing is not.
    pub(super) diagnostics: bool,
    /// The loaded themes.
    pub(super) themes: Vec<RecordedTheme>,
    /// The loaded extensions.
    pub(super) extensions: Vec<RecordedExtension>,
    /// The extensions supplied in place of the loaded ones.
    #[serde(rename = "override")]
    pub(super) replacement: Option<Vec<RecordedExtension>>,
    /// The extension paths that failed to load.
    pub(super) errors: Vec<RecordedError>,
    /// The skills.
    pub(super) skills: Vec<Recorded>,
    /// The prompt templates the session lists.
    pub(super) prompts: Vec<Recorded>,
    /// The prompt templates of the loader's first read, when they differ.
    #[serde(rename = "loaderPrompts")]
    pub(super) loader_prompts: Option<Vec<Recorded>>,
    /// The registered commands.
    pub(super) commands: Vec<RecordedCommand>,
    /// The skill diagnostics.
    #[serde(rename = "skillDiagnostics")]
    pub(super) skill_diagnostics: Vec<RecordedDiagnostic>,
    /// The prompt template diagnostics.
    #[serde(rename = "promptDiagnostics")]
    pub(super) prompt_diagnostics: Vec<RecordedDiagnostic>,
    /// The theme diagnostics.
    #[serde(rename = "themeDiagnostics")]
    pub(super) theme_diagnostics: Vec<RecordedDiagnostic>,
    /// The command diagnostics the extension runner published.
    #[serde(rename = "commandDiagnostics")]
    pub(super) command_diagnostics: Vec<RecordedDiagnostic>,
    /// The shortcut diagnostics the extension runner published.
    #[serde(rename = "shortcutDiagnostics")]
    pub(super) shortcut_diagnostics: Vec<RecordedDiagnostic>,
    /// The render width.
    pub(super) width: Option<usize>,
}

/// One recorded loaded extension.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordedExtension {
    /// The extension path.
    path: String,
    /// Where it came from.
    #[serde(rename = "sourceInfo")]
    source_info: RecordedSource,
}

impl RecordedExtension {
    /// The extension's path and provenance.
    pub(super) fn into_loaded(self) -> (String, SourceInfo) {
        let source_info = self.source_info.into_source_info(&self.path);
        (self.path, source_info)
    }
}

/// One recorded path that failed to load.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordedError {
    /// The path.
    path: String,
    /// The failure.
    error: String,
}

/// One recorded command registration.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordedCommand {
    /// The command name.
    name: String,
    /// The registering extension's path.
    path: String,
}

/// The recorded registrations by name and path.
pub(super) fn commands(recorded: Vec<RecordedCommand>) -> Vec<(String, String)> {
    recorded
        .into_iter()
        .map(|command| (command.name, command.path))
        .collect()
}

/// One recorded diagnostic.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordedDiagnostic {
    /// The category.
    r#type: String,
    /// The message.
    message: String,
    /// The path.
    path: Option<String>,
    /// The collision details.
    collision: Option<RecordedCollision>,
}

/// The two paths of a recorded collision.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RecordedCollision {
    /// The colliding name.
    name: String,
    /// The retained path.
    #[serde(rename = "winnerPath")]
    winner_path: String,
    /// The skipped path.
    #[serde(rename = "loserPath")]
    loser_path: String,
}

impl RecordedDiagnostic {
    /// The recorded diagnostic.
    pub(super) fn into_diagnostic(self) -> ResourceDiagnostic {
        let r#type = match self.r#type.as_str() {
            "warning" => DiagnosticType::Warning,
            "error" => DiagnosticType::Error,
            "collision" => DiagnosticType::Collision,
            other => panic!("an unknown diagnostic type {other}"),
        };
        let collision = self.collision.map(|collision| ResourceCollision {
            resource_type: "skill".to_owned(),
            name: collision.name,
            winner_path: collision.winner_path,
            loser_path: collision.loser_path,
            winner_source: None,
            loser_source: None,
        });
        ResourceDiagnostic {
            r#type,
            message: self.message,
            path: self.path,
            collision,
        }
    }
}

/// The recorded diagnostics.
pub(super) fn diagnostics(recorded: Vec<RecordedDiagnostic>) -> Vec<ResourceDiagnostic> {
    recorded
        .into_iter()
        .map(RecordedDiagnostic::into_diagnostic)
        .collect()
}

/// One recorded loaded theme.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordedTheme {
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

/// A recorded source label.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DisplaySource {
    /// The label.
    pub(super) label: String,
    /// The scope shown after it.
    #[serde(rename = "scopeLabel")]
    pub(super) scope_label: Option<String>,
}

/// The recorded observations.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(super) enum Expected {
    /// Transcript rows after the listing, with every section expanded, then collapsed,
    /// the transcript's children and the loader reads.
    Listing {
        /// Rows after the listing.
        before: Vec<String>,
        /// Rows with every section expanded.
        expanded: Vec<String>,
        /// Rows with every section collapsed.
        collapsed: Vec<String>,
        /// The transcript's children.
        children: usize,
        /// The loader getters read, in order.
        calls: Vec<String>,
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
    /// The built-in conflicts, then the published command diagnostics before and after.
    Registered {
        /// The built-in conflicts.
        value: Vec<RecordedDiagnostic>,
        /// The command diagnostics published before.
        retained: Vec<RecordedDiagnostic>,
        /// The command diagnostics published after.
        current: Vec<RecordedDiagnostic>,
    },
    /// Built-in conflicts.
    Conflicts {
        /// The diagnostics.
        value: Vec<RecordedDiagnostic>,
    },
    /// One label or text.
    Value {
        /// The label or text.
        value: String,
    },
    /// One source label.
    Display {
        /// The label.
        value: DisplaySource,
    },
    /// The provenance found, or none.
    Found {
        /// The provenance.
        value: Option<RecordedSource>,
    },
}

/// The recorded cases of `test`.
pub(super) fn cases(test: &str) -> Vec<(Input, Expected)> {
    crate::interactive_mode::tests::cases(FIXTURES, test)
}

/// One listing call over a built mode.
pub(super) struct Request {
    /// The extensions supplied in place of the loaded ones.
    replacement: Option<Vec<(String, SourceInfo)>>,
    /// Whether the listing is forced.
    force: bool,
    /// Whether diagnostics are asked for even when the listing is not.
    diagnostics: bool,
    /// The render width.
    pub(super) width: usize,
}

impl Request {
    /// Lists the resources on `world`, looking the home directory up through `home`.
    pub(super) fn list(&self, world: &World, home: &HomeLookup<'_>) -> Result<(), Box<dyn Error>> {
        let items: Option<Vec<_>> = self.replacement.as_ref().map(|extensions| {
            extensions
                .iter()
                .map(|(path, source_info)| ResourceItem {
                    path,
                    source_info: Some(source_info),
                })
                .collect()
        });
        let options = ShowLoadedResourcesOptions {
            extensions: items.as_deref(),
            force: self.force,
            show_diagnostics_when_quiet: self.diagnostics,
        };
        world.mode.list_loaded_resources(options, home)
    }
}

/// The mode inputs of the recorded resources of `listing`, taken out of it.
fn setup(listing: &mut Listing) -> Setup {
    let startup = listing.startup;
    let loader_prompts = listing.loader_prompts.take();
    let mut prompts: Vec<_> = loader_prompts.into_iter().map(templates).collect();
    prompts.push(templates(take(&mut listing.prompts)));
    let errors = take(&mut listing.errors)
        .into_iter()
        .map(|error| ExtensionLoadError {
            path: error.path,
            error: error.error,
        });
    Setup {
        options: InteractiveModeOptions {
            verbose: startup.verbose,
            ..InteractiveModeOptions::default()
        },
        configure: Box::new(move |settings| settings.set_quiet_startup(startup.quiet)),
        skills: skills(take(&mut listing.skills)),
        prompts,
        themes: take(&mut listing.themes)
            .into_iter()
            .map(RecordedTheme::into_theme)
            .collect(),
        extensions: take(&mut listing.extensions)
            .into_iter()
            .map(RecordedExtension::into_loaded)
            .collect(),
        errors: errors.collect(),
        commands: commands(take(&mut listing.commands)),
        diagnostics: LoadedDiagnostics {
            skills: diagnostics(take(&mut listing.skill_diagnostics)),
            prompts: diagnostics(take(&mut listing.prompt_diagnostics)),
            themes: diagnostics(take(&mut listing.theme_diagnostics)),
        },
        ..Setup::default()
    }
}

/// A mode over the recorded resources of `listing`, with the runner's recorded
/// diagnostics published and the loader reads so far forgotten, and its listing call.
pub(super) fn world(directory: &Path, mut listing: Listing) -> (World, Request) {
    let world = setup(&mut listing).build(directory);
    world
        .mode
        .tool_output_expanded
        .set(listing.startup.expanded);
    let runner = world.mode.runtime_host.session().extension_runner();
    let published = [
        (
            runner.get_command_diagnostics(),
            listing.command_diagnostics,
        ),
        (
            runner.get_shortcut_diagnostics(),
            listing.shortcut_diagnostics,
        ),
    ];
    for (target, recorded) in published {
        target.write().unwrap().extend(diagnostics(recorded));
    }
    world.loader.calls.borrow_mut().clear();
    let replacement = listing.replacement.map(|extensions| {
        let loaded = extensions.into_iter().map(RecordedExtension::into_loaded);
        loaded.collect()
    });
    let request = Request {
        replacement,
        force: listing.force,
        diagnostics: listing.diagnostics,
        width: listing.width.unwrap_or(WIDTH),
    };
    (world, request)
}

/// Lists `listing` and checks the recorded rows after the listing, with every section
/// expanded and collapsed again, the transcript's children and the loader reads.
pub(super) fn check_listing(directory: &Path, listing: Listing, expected: Expected) -> World {
    let Expected::Listing {
        before,
        expanded,
        collapsed,
        children,
        calls,
    } = expected
    else {
        panic!("a listing observation");
    };
    let (world, request) = world(directory, listing);
    request.list(&world, &home_directory).unwrap();
    assert_eq!(*world.loader.calls.borrow(), calls);
    let theme = world.themes.theme().get().unwrap();
    let chat = &world.mode.chat_container;
    let width = request.width;
    assert_eq!(chat.render(width), painted(&theme, &before));
    assert_eq!(chat.children().borrow().len(), children);
    expand_all(&world.mode, true);
    assert_eq!(chat.render(width), painted(&theme, &expanded));
    expand_all(&world.mode, false);
    assert_eq!(chat.render(width), painted(&theme, &collapsed));
    world
}

/// Checks every recorded listing case of `test`.
pub(super) fn listings(test: &str) {
    in_home(test, async |directory| {
        for (input, expected) in cases(test) {
            let Input::Listing(listing) = input else {
                panic!("a listing case");
            };
            check_listing(directory, listing, expected);
        }
    });
}

/// The source and scope of `recorded`, for comparison with a found provenance.
pub(super) fn identity(recorded: &RecordedSource) -> (&str, SourceScope) {
    (&recorded.source, recorded.scope)
}
