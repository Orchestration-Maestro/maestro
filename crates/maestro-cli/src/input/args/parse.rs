//! Consumption of the authored prompt-first argument grammar.

use super::{Args, Diagnostic, DiagnosticType, ListModels, Mode};
use maestro_app::{FlagValue, ThinkingLevel, is_whitespace_scalar};
use std::collections::VecDeque;

/// The accepted reasoning levels in their documented order.
const VALID_THINKING_LEVELS: [&str; 6] = ["off", "minimal", "low", "medium", "high", "xhigh"];

/// A flag-only option and the presence bit it sets.
type FlagOption = (&'static str, fn(&mut Args));

/// An option and the consumer of the argument text following it.
type ValueOption = (&'static str, fn(&mut Args, String));

/// Flag-only options with the presence bit each one sets.
const FLAG_OPTIONS: &[FlagOption] = &[
    ("--help", |args| args.help = Some(true)),
    ("-h", |args| args.help = Some(true)),
    ("--version", |args| args.version = Some(true)),
    ("-v", |args| args.version = Some(true)),
    ("--continue", |args| args.r#continue = Some(true)),
    ("-c", |args| args.r#continue = Some(true)),
    ("--resume", |args| args.resume = Some(true)),
    ("-r", |args| args.resume = Some(true)),
    ("--no-session", |args| args.no_session = Some(true)),
    ("--no-tools", |args| args.no_tools = Some(true)),
    ("-nt", |args| args.no_tools = Some(true)),
    ("--no-builtin-tools", |args| {
        args.no_builtin_tools = Some(true);
    }),
    ("-nbt", |args| args.no_builtin_tools = Some(true)),
    ("--no-extensions", |args| args.no_extensions = Some(true)),
    ("-ne", |args| args.no_extensions = Some(true)),
    ("--no-skills", |args| args.no_skills = Some(true)),
    ("-ns", |args| args.no_skills = Some(true)),
    ("--no-prompt-templates", |args| {
        args.no_prompt_templates = Some(true);
    }),
    ("-np", |args| args.no_prompt_templates = Some(true)),
    ("--no-themes", |args| args.no_themes = Some(true)),
    ("--no-context-files", |args| {
        args.no_context_files = Some(true);
    }),
    ("-nc", |args| args.no_context_files = Some(true)),
    ("--verbose", |args| args.verbose = Some(true)),
    ("--offline", |args| args.offline = Some(true)),
];

/// Options that retain the argument text following them.
const VALUE_OPTIONS: &[ValueOption] = &[
    ("--provider", |args, text| args.provider = Some(text)),
    ("--model", |args, text| args.model = Some(text)),
    ("--api-key", |args, text| args.api_key = Some(text)),
    ("--system-prompt", |args, text| {
        args.system_prompt = Some(text);
    }),
    ("--session", |args, text| args.session = Some(text)),
    ("--fork", |args, text| args.fork = Some(text)),
    ("--session-dir", |args, text| args.session_dir = Some(text)),
    ("--export", |args, text| args.export = Some(text)),
    ("--mode", |args, text| take_mode(args, &text)),
    ("--thinking", |args, text| take_thinking(args, &text)),
    ("--append-system-prompt", |args, text| {
        repeat(&mut args.append_system_prompt, text);
    }),
    ("--extension", |args, text| {
        repeat(&mut args.extensions, text);
    }),
    ("-e", |args, text| repeat(&mut args.extensions, text)),
    ("--skill", |args, text| repeat(&mut args.skills, text)),
    ("--prompt-template", |args, text| {
        repeat(&mut args.prompt_templates, text);
    }),
    ("--theme", |args, text| repeat(&mut args.themes, text)),
    ("--models", |args, text| {
        args.models = Some(split_list(&text, List::Keeping));
    }),
    ("--tools", |args, text| {
        args.tools = Some(split_list(&text, List::Dropping));
    }),
    ("-t", |args, text| {
        args.tools = Some(split_list(&text, List::Dropping));
    }),
];

/// Report whether `level` names an accepted reasoning level.
#[must_use]
pub fn is_valid_thinking_level(level: &str) -> bool {
    thinking_level(level).is_some()
}

/// Parse one reasoning level through the application's shared level owner.
fn thinking_level(level: &str) -> Option<ThinkingLevel> {
    level.parse().ok()
}

/// Trim the authored whitespace set from both ends of `value`.
fn trim_authored(value: &str) -> &str {
    value.trim_matches(is_whitespace_scalar)
}

/// Report whether `next` may be consumed as the value of a `--print` prompt.
fn print_prompt(next: &str) -> bool {
    !next.starts_with('@') && (!next.starts_with('-') || next.starts_with("---"))
}

/// Report whether `next` may be consumed as an unknown flag's string value.
fn flag_value(next: &str) -> bool {
    !next.starts_with('-') && !next.starts_with('@')
}

/// The comma-list kinds whose empty segments differ.
#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    /// Keep every segment, including empty ones.
    Keeping,
    /// Drop names that are empty after trimming.
    Dropping,
}

/// Split one comma list with the authored whitespace policy.
fn split_list(value: &str, list: List) -> Vec<String> {
    value
        .split(',')
        .map(|segment| trim_authored(segment).to_owned())
        .filter(|name| list == List::Keeping || !name.is_empty())
        .collect()
}

/// Append one value to a repeated option.
fn repeat(slot: &mut Option<Vec<String>>, value: String) {
    slot.get_or_insert_with(Vec::new).push(value);
}

/// Parse the authored command-line arguments.
#[must_use]
pub fn parse_args(args: &[String]) -> Args {
    let mut result = Args {
        messages: VecDeque::new(),
        ..Args::default()
    };
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        let value = args.get(index + 1).map(String::as_str);
        index += 1;
        if consumes_next(&mut result, arg, value) {
            index += 1;
        }
    }
    result
}

/// Consume one token and report whether its following token also belonged to it.
fn consumes_next(result: &mut Args, arg: &str, value: Option<&str>) -> bool {
    if take_flag_option(result, arg) {
        return false;
    }
    if take_value_option(result, arg, value) {
        return true;
    }
    take_special(result, arg, value)
}

/// Set the presence bit of one flag-only option.
fn take_flag_option(result: &mut Args, arg: &str) -> bool {
    let Some((_, set)) = FLAG_OPTIONS.iter().find(|(name, _)| *name == arg) else {
        return false;
    };
    set(result);
    true
}

/// Retain the following token as one option's value.
fn take_value_option(result: &mut Args, arg: &str, value: Option<&str>) -> bool {
    let Some((_, set)) = VALUE_OPTIONS.iter().find(|(name, _)| *name == arg) else {
        return false;
    };
    match value {
        Some(text) => {
            set(result, text.to_owned());
            true
        }
        None => false,
    }
}

/// Select one configured output mode, keeping an earlier valid choice.
fn take_mode(result: &mut Args, text: &str) {
    let mode = match text {
        "text" => Some(Mode::Text),
        "json" => Some(Mode::Json),
        "rpc" => Some(Mode::Rpc),
        _ => None,
    };
    if mode.is_some() {
        result.mode = mode;
    }
}

/// Retain the reasoning level, warning about an unrecognized one.
fn take_thinking(result: &mut Args, level: &str) {
    match thinking_level(level) {
        Some(level) => result.thinking = Some(level),
        None => result.diagnostics.push(Diagnostic {
            r#type: DiagnosticType::Warning,
            message: format!(
                "Invalid thinking level \"{level}\". Valid values: {}",
                VALID_THINKING_LEVELS.join(", ")
            ),
        }),
    }
}

/// Consume the options whose value depends on the following token.
fn take_special(result: &mut Args, arg: &str, value: Option<&str>) -> bool {
    match (arg, value) {
        ("--print" | "-p", Some(next)) => {
            result.print = Some(true);
            if !print_prompt(next) {
                return false;
            }
            result.messages.push_back(next.to_owned());
            return true;
        }
        ("--print" | "-p", None) => result.print = Some(true),
        ("--list-models", Some(next)) if flag_value(next) => {
            result.list_models = Some(ListModels::Search(next.to_owned()));
            return true;
        }
        ("--list-models", _) => result.list_models = Some(ListModels::All),
        _ => return take_positional(result, arg, value),
    }
    false
}

/// Consume one positional token, file argument or unrecognized flag.
fn take_positional(result: &mut Args, arg: &str, value: Option<&str>) -> bool {
    if let Some(name) = arg.strip_prefix("--") {
        return take_unknown(result, name, value);
    }
    if let Some(name) = arg.strip_prefix('@') {
        result.file_args.push(name.to_owned());
    } else if arg.starts_with('-') {
        result.diagnostics.push(Diagnostic {
            r#type: DiagnosticType::Error,
            message: format!("Unknown option: {arg}"),
        });
    } else {
        result.messages.push_back(arg.to_owned());
    }
    false
}

/// Retain one unrecognized long flag, reporting whether it consumed its value.
fn take_unknown(result: &mut Args, name: &str, value: Option<&str>) -> bool {
    if let Some((name, text)) = name.split_once('=') {
        result
            .unknown_flags
            .insert(name.to_owned(), FlagValue::String(text.to_owned()));
        return false;
    }
    let Some(next) = value.filter(|next| flag_value(next)) else {
        result
            .unknown_flags
            .insert(name.to_owned(), FlagValue::Boolean(true));
        return false;
    };
    result
        .unknown_flags
        .insert(name.to_owned(), FlagValue::String(next.to_owned()));
    true
}
