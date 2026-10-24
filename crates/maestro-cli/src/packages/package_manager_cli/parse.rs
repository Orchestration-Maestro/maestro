//! Package command grammar scanned left to right.

/// A recognized package command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageCommand {
    /// Install a source and record it.
    Install,
    /// Remove a source and its record.
    Remove,
    /// Update the application or installed packages.
    Update,
    /// List configured packages.
    List,
}

impl PackageCommand {
    /// The command name used in diagnostics and usage text.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Remove => "remove",
            Self::Update => "update",
            Self::List => "list",
        }
    }
}

/// What an update selects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum UpdateTarget {
    /// The application and every installed package.
    All,
    /// The application only.
    SelfOnly,
    /// Installed packages, optionally limited to one source.
    Extensions(Option<String>),
}

impl UpdateTarget {
    /// Whether this target updates the application.
    pub(super) fn includes_self(&self) -> bool {
        matches!(self, Self::All | Self::SelfOnly)
    }

    /// Whether this target updates installed packages.
    pub(super) fn includes_extensions(&self) -> bool {
        matches!(self, Self::All | Self::Extensions(_))
    }
}

/// Parsed options keeping the first value of every diagnostic category.
#[derive(Debug)]
pub(super) struct PackageCommandOptions {
    /// The recognized command.
    pub(super) command: PackageCommand,
    /// The retained positional source, which may be empty.
    pub(super) source: Option<String>,
    /// The update selection of an update command.
    pub(super) update_target: Option<UpdateTarget>,
    /// Whether the command works on project settings.
    pub(super) local: bool,
    /// Whether the update reinstalls the application.
    pub(super) force: bool,
    /// Whether help was requested.
    pub(super) help: bool,
    /// The first unknown option.
    pub(super) invalid_option: Option<String>,
    /// The first unexpected argument.
    pub(super) invalid_argument: Option<String>,
    /// The first option with a missing value.
    pub(super) missing_option_value: Option<String>,
    /// The first conflicting combination.
    pub(super) conflicting_options: Option<String>,
}

impl PackageCommandOptions {
    /// The source this command operates on; empty text counts as absent.
    pub(super) fn required_source(&self) -> Option<&str> {
        self.source.as_deref().filter(|source| !source.is_empty())
    }
}

/// Recognize the command named by the first argument.
fn command_of(token: &str) -> Option<PackageCommand> {
    match token {
        "install" => Some(PackageCommand::Install),
        "remove" | "uninstall" => Some(PackageCommand::Remove),
        "update" => Some(PackageCommand::Update),
        "list" => Some(PackageCommand::List),
        _ => None,
    }
}

/// Keep the first value recorded in one diagnostic category.
fn first(slot: &mut Option<String>, value: &str) {
    if slot.is_none() {
        *slot = Some(value.to_owned());
    }
}

/// A flag one scan collected.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Flag {
    /// Project-local scope.
    Local,
    /// Reinstall the application.
    Force,
    /// Print the help document.
    Help,
    /// Update only the application.
    SelfOnly,
    /// Update only installed packages.
    Extensions,
}

/// Parse one lead-to-tail scan of the arguments after the command.
pub(super) fn parse_command(args: &[String]) -> Option<PackageCommandOptions> {
    let (command, rest) = args.split_first()?;
    let command = command_of(command)?;
    let mut scan = Scan::new(command);
    let mut index = 0;
    while let Some(argument) = rest.get(index) {
        index += scan.consume(argument, rest, index);
    }
    Some(scan.finish())
}

/// Options accumulated while scanning, with their separate diagnostic slots.
struct Scan {
    /// The recognized command.
    command: PackageCommand,
    /// The flags collected so far.
    flags: std::collections::BTreeSet<Flag>,
    /// The operand of the first accepted `--extension`.
    extension_source: Option<String>,
    /// The retained positional source.
    source: Option<String>,
    /// The first unknown option.
    invalid_option: Option<String>,
    /// The first unexpected argument.
    invalid_argument: Option<String>,
    /// The first option with a missing value.
    missing_option_value: Option<String>,
    /// The first conflicting combination.
    conflicting_options: Option<String>,
}

impl Scan {
    /// Start scanning for `command` with no option recorded.
    fn new(command: PackageCommand) -> Self {
        Self {
            command,
            flags: std::collections::BTreeSet::new(),
            extension_source: None,
            source: None,
            invalid_option: None,
            invalid_argument: None,
            missing_option_value: None,
            conflicting_options: None,
        }
    }

    /// Take one argument and report how many arguments the scan advances.
    fn consume(&mut self, argument: &str, rest: &[String], index: usize) -> usize {
        match argument {
            "-h" | "--help" => drop(self.flags.insert(Flag::Help)),
            "-l" | "--local" => match self.command {
                PackageCommand::Install | PackageCommand::Remove => {
                    self.flags.insert(Flag::Local);
                }
                PackageCommand::Update | PackageCommand::List => self.unknown(argument),
            },
            "--extension" => return self.extension(rest, index),
            "--self" | "--extensions" | "--force" => self.update_flag(argument),
            _ if argument.starts_with('-') => self.unknown(argument),
            _ => self.positional(argument),
        }
        1
    }

    /// Record the update-only flags, rejecting them elsewhere.
    fn update_flag(&mut self, argument: &str) {
        if self.command != PackageCommand::Update {
            return self.unknown(argument);
        }
        self.flags.insert(match argument {
            "--self" => Flag::SelfOnly,
            "--extensions" => Flag::Extensions,
            _ => Flag::Force,
        });
    }

    /// Record an unknown option without replacing an earlier one.
    fn unknown(&mut self, argument: &str) {
        first(&mut self.invalid_option, argument);
    }

    /// Record a positional source, then any token after one.
    fn positional(&mut self, argument: &str) {
        match self.source.as_deref() {
            Some(source) if !source.is_empty() => first(&mut self.invalid_argument, argument),
            _ => self.source = Some(argument.to_owned()),
        }
    }

    /// Take the operand of `--extension`, advancing past it when it was consumed.
    fn extension(&mut self, rest: &[String], index: usize) -> usize {
        if self.command != PackageCommand::Update {
            self.unknown("--extension");
            return 1;
        }
        let value = rest.get(index + 1);
        if value.is_none_or(|value| value.is_empty() || value.starts_with('-')) {
            first(&mut self.missing_option_value, "--extension");
            return 1;
        }
        match self.extension_source {
            Some(_) => first(
                &mut self.conflicting_options,
                "--extension can only be provided once",
            ),
            None => self.extension_source = value.cloned(),
        }
        2
    }

    /// Resolve the update selection and keep every conflict collected on the way.
    fn target(&mut self) -> UpdateTarget {
        if let Some(extension) = self.extension_source.take() {
            if self.flags.contains(&Flag::SelfOnly) || self.flags.contains(&Flag::Extensions) {
                first(
                    &mut self.conflicting_options,
                    "--extension cannot be combined with --self or --extensions",
                );
            }
            if self
                .source
                .as_deref()
                .is_some_and(|source| !source.is_empty())
            {
                first(
                    &mut self.conflicting_options,
                    "--extension cannot be combined with a positional source",
                );
            }
            return UpdateTarget::Extensions(Some(extension));
        }
        if let Some(source) = self.source.as_deref().filter(|source| !source.is_empty()) {
            if source == "self" || source == "maestro" {
                return self.application_target();
            }
            if self.flags.contains(&Flag::Extensions) || self.flags.contains(&Flag::SelfOnly) {
                first(
                    &mut self.conflicting_options,
                    "positional update targets cannot be combined with --self or --extensions",
                );
            }
            return UpdateTarget::Extensions(Some(source.to_owned()));
        }
        match (
            self.flags.contains(&Flag::SelfOnly),
            self.flags.contains(&Flag::Extensions),
        ) {
            (true, false) => UpdateTarget::SelfOnly,
            (false, true) => UpdateTarget::Extensions(None),
            (true, true) | (false, false) => UpdateTarget::All,
        }
    }

    /// The selection of the application's own name, with extensions when asked.
    fn application_target(&self) -> UpdateTarget {
        if self.flags.contains(&Flag::Extensions) {
            UpdateTarget::All
        } else {
            UpdateTarget::SelfOnly
        }
    }

    /// Finish the scan, resolving the update selection of an update command.
    fn finish(mut self) -> PackageCommandOptions {
        let update_target = (self.command == PackageCommand::Update).then(|| self.target());
        PackageCommandOptions {
            command: self.command,
            source: self.source,
            update_target,
            local: self.flags.contains(&Flag::Local),
            force: self.flags.contains(&Flag::Force),
            help: self.flags.contains(&Flag::Help),
            invalid_option: self.invalid_option,
            invalid_argument: self.invalid_argument,
            missing_option_value: self.missing_option_value,
            conflicting_options: self.conflicting_options,
        }
    }
}
