//! Build the setup example, run it and stage the components and compiled code it produced.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::base::Supplied;
use super::compiled_code::{self, CompiledCode};
use super::inputs::Inputs;
use super::plan::component_inputs;
use super::{
    COMPILED, Component, MANIFEST, copy, digest, environment_file, example_program,
    executable_name, preparation_agent, preparation_command, read_base, remove, remove_directory,
    run_example,
};
use crate::example_files::validate_variable;
use crate::workspace::Workspace;

/// The components and the setup example a preparation staged.
#[derive(Default)]
pub(super) struct Examples {
    /// Each staged component, by the variable that exports it.
    pub(super) components: BTreeMap<String, Component>,
    /// How many components the base supplied.
    pub(super) reused: usize,
    /// How many components the setup example built.
    pub(super) built: usize,
    /// Digest of the staged setup example.
    pub(super) example: Option<String>,
    /// The code the example compiled, when the engine keeps it in its cache.
    pub(super) compiled: Option<CompiledCode>,
}

/// Build and run the setup example with the components a base supplies, then stage the
/// components, the example and the code it compiled in `output`, whose manifest goes first.
pub(super) fn stage(
    checkout: &Path,
    workspace: &Workspace,
    shared: &Inputs,
    output: &Path,
    target: &Path,
) -> io::Result<Examples> {
    let cargo = workspace.cargo();
    // The base may be the output directory itself, so it is read before that manifest goes.
    let base = read_base(checkout, shared);
    remove(&output.join(MANIFEST))?;
    remove(&environment_file(target))?;
    remove_directory(&preparation_agent(target))?;
    let example = example_program(cargo, checkout, workspace, target, "dev")?;
    let cache_directory =
        compiled_code::directory(&mut preparation_command(checkout, &example, target))?;
    let (supplied, seeded) = match &base {
        Some(base) => (
            base.supplied(checkout)?,
            base.seed(&cache_directory, &example)?,
        ),
        None => (Vec::new(), None),
    };
    run_example(output, workspace, &example, target, &supplied)?;
    let components = stage_components(checkout, output, target, shared, &supplied)?;
    let reused = supplied.len();
    let built = components.len().checked_sub(reused).ok_or_else(|| {
        io::Error::other(format!(
            "base supplied {reused} components but preparation staged only {}",
            components.len()
        ))
    })?;
    let (staged, compiled) = stage_example(output, &example, &cache_directory, seeded.as_deref())?;
    Ok(Examples {
        components,
        reused,
        built,
        example: Some(digest(&std::fs::read(&staged)?)),
        compiled,
    })
}

/// Stage each component the setup example exported in `output`, with the example files it was
/// built from: those the example reported in Cargo's dependency file, or those the base recorded
/// for a component it supplied. The file also exports what nextest hands the tests beside the
/// components, so only the entries in the component namespace stage.
pub(super) fn stage_components(
    checkout: &Path,
    output: &Path,
    target: &Path,
    shared: &Inputs,
    supplied: &[Supplied],
) -> io::Result<BTreeMap<String, Component>> {
    let shared = shared.files(checkout)?;
    let mut components = BTreeMap::new();
    for line in std::fs::read_to_string(environment_file(target))?.lines() {
        let (variable, source) = line
            .split_once('=')
            .ok_or_else(|| io::Error::other(format!("invalid component entry: {line}")))?;
        if validate_variable(variable).is_err() {
            continue;
        }
        let staged = output.join(format!("{variable}.wasm"));
        if Path::new(source) != staged {
            copy(Path::new(source), &staged)?;
        }
        let inputs = match supplied.iter().find(|base| base.variable == variable) {
            Some(base) => base.inputs.clone(),
            None => component_inputs(checkout, Path::new(source), &shared)?,
        };
        components.insert(
            variable.to_owned(),
            Component {
                file: digest(&std::fs::read(&staged)?),
                inputs,
            },
        );
    }
    Ok(components)
}

/// Stage the compiled code the example produced and the example itself in `output`.
fn stage_example(
    output: &Path,
    example: &Path,
    cache_directory: &Path,
    seeded: Option<&Path>,
) -> io::Result<(PathBuf, Option<CompiledCode>)> {
    remove_directory(&output.join(COMPILED))?;
    let compiled = CompiledCode::collect(cache_directory, example, &output.join(COMPILED), seeded)?;
    let staged = output.join(executable_name());
    copy(example, &staged)?;
    Ok((staged, compiled))
}
