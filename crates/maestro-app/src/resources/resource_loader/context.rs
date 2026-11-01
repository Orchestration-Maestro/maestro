//! Context files, system prompt files and the warning written when one cannot be read.
use super::{ContextFile, DefaultEventBus, DefaultResourceLoader, ResourceKind, overrides::apply};
use maestro_models::system_prompt::CONTEXT_FILE_NAMES;
use maestro_path::join;
use maestro_resources::{
    paths::{CONFIG_DIR_NAME, resolve},
    read_text,
};
use std::{
    collections::HashSet,
    io::{self, IsTerminal},
    path::Path,
    rc::Rc,
};

/// Writes `text` to standard error, in yellow when standard error is a terminal.
fn warn(text: &str) {
    if io::stderr().is_terminal() {
        eprintln!("\u{1b}[33m{text}\u{1b}[39m");
    } else {
        eprintln!("{text}");
    }
}

/// The text of the file named by `input` when it exists and can be read, else `input`
/// itself; nothing for an empty input.
fn resolve_prompt_input(input: &str, description: &str) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    if !Path::new(input).exists() {
        return Some(input.to_owned());
    }
    Some(read_text(Path::new(input)).unwrap_or_else(|error| {
        warn(&format!(
            "Warning: Could not read {description} file {input}: {error}"
        ));
        input.to_owned()
    }))
}

/// The first readable context file of `dir`; a file that exists but cannot be read is
/// reported and the next name is tried.
fn load_context_file_from_dir(dir: &str) -> Option<ContextFile> {
    CONTEXT_FILE_NAMES.iter().find_map(|name| {
        let path = join(&[dir, name]);
        if !Path::new(&path).exists() {
            return None;
        }
        match read_text(Path::new(&path)) {
            Ok(content) => Some(ContextFile { path, content }),
            Err(error) => {
                warn(&format!("Warning: Could not read {path}: {error}"));
                None
            }
        }
    })
}

/// The global context file, then one file per directory from the filesystem root down
/// to `cwd`, each path once.
///
/// # Errors
/// Returns the working-directory error when a relative `cwd` needs it.
pub fn load_project_context_files(cwd: &str, agent_dir: &str) -> io::Result<Vec<ContextFile>> {
    let mut seen = HashSet::new();
    let mut files: Vec<ContextFile> = load_context_file_from_dir(agent_dir).into_iter().collect();
    seen.extend(files.iter().map(|file| file.path.clone()));
    let root = resolve(&["/"])?;
    let mut ancestors = Vec::new();
    let mut dir = cwd.to_owned();
    loop {
        if let Some(file) =
            load_context_file_from_dir(&dir).filter(|file| seen.insert(file.path.clone()))
        {
            ancestors.push(file);
        }
        if dir == root {
            break;
        }
        let parent = resolve(&[&dir, ".."])?;
        if parent == dir {
            break;
        }
        dir = parent;
    }
    files.extend(ancestors.into_iter().rev());
    Ok(files)
}

impl<E: DefaultEventBus> DefaultResourceLoader<E> {
    /// The project file `name` when it exists, else the global one when it exists.
    fn discover_prompt_file(&self, name: &str) -> Option<String> {
        [
            join(&[&self.cwd, CONFIG_DIR_NAME, name]),
            join(&[&self.agent_dir, name]),
        ]
        .into_iter()
        .find(|path| Path::new(path).exists())
    }

    /// Loads context files, the system prompt and the appended texts, each through its
    /// override, and stores them.
    pub(super) fn load_prompt_files(&self) -> io::Result<()> {
        let agents_files = if self.is_disabled(ResourceKind::ContextFiles) {
            Vec::new()
        } else {
            load_project_context_files(&self.cwd, &self.agent_dir)?
        };
        let agents_files = apply(self.options.agents_files_override.as_ref(), agents_files);
        self.state.borrow_mut().agents_files = Rc::from(agents_files);

        let options = &self.options;
        let discovered = options
            .system_prompt
            .is_none()
            .then(|| self.discover_prompt_file("SYSTEM.md"))
            .flatten();
        let system_prompt = options
            .system_prompt
            .as_deref()
            .or(discovered.as_deref())
            .and_then(|input| resolve_prompt_input(input, "system prompt"));
        let system_prompt = if options.system_prompt.is_none() && discovered.is_none() {
            self.state.borrow().base_system_prompt.clone()
        } else {
            system_prompt
        };
        let system_prompt = apply(options.system_prompt_override.as_ref(), system_prompt);
        self.state.borrow_mut().system_prompt = system_prompt.map(Rc::from);

        let discovered = options
            .append_system_prompt
            .is_none()
            .then(|| self.discover_prompt_file("APPEND_SYSTEM.md"))
            .flatten();
        let appended = options
            .append_system_prompt
            .as_deref()
            .unwrap_or(discovered.as_slice())
            .iter()
            .filter_map(|input| resolve_prompt_input(input, "append system prompt"))
            .collect();
        let appended = apply(options.append_system_prompt_override.as_ref(), appended);
        let mut combined = self.state.borrow().base_append_text.clone();
        combined.extend(appended);
        self.state.borrow_mut().append_system_prompt = Rc::from(combined);
        Ok(())
    }
}
