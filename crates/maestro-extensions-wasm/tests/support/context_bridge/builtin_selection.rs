//! Forbidden effects make eager or invalid-name factory execution observable.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    BashOperations, BashToolOptions, DiagnosticErrorInfo, EditOperations, EditToolOptions,
    FindOperations, FindToolOptions, GrepOperations, GrepToolOptions, LsOperations, LsToolOptions,
    OperationFuture, ReadOperations, ReadToolOptions, ToolsOptions, WriteOperations,
    WriteToolOptions,
};
use std::{cell::RefCell, rc::Rc};

/// A positive transcript records every forbidden observation, including immediately ready calls.
pub(super) type Effects = Rc<RefCell<Vec<String>>>;

/// Records before returning an error, so even an unpolled callback is detected.
fn reject<T: 'static>(effects: &Effects, name: &str) -> OperationFuture<'static, T> {
    effects.borrow_mut().push(name.into());
    Box::pin(async { Err(DiagnosticErrorInfo::error("unexpected factory effect")) })
}

/// Keeps each callback's captured transcript alive independently of construction.
macro_rules! forbidden {
    ($effects:ident, |$($arg:pat_param),*| $name:literal) => {{
        let effects = $effects.clone();
        Rc::new(move |$($arg),*| reject(&effects, $name))
    }};
}

/// Every family has a supplied effect, so selection cannot conceal eager native work.
pub(super) fn options(effects: &Effects) -> ToolsOptions {
    ToolsOptions {
        read: read(effects),
        bash: bash(effects),
        edit: EditToolOptions {
            operations: Some(EditOperations {
                access: forbidden!(effects, |_| "edit.access"),
                read_file: forbidden!(effects, |_| "edit.read_file"),
                write_file: forbidden!(effects, |_, _| "edit.write_file"),
            }),
        },
        write: WriteToolOptions {
            operations: Some(WriteOperations {
                mkdir: forbidden!(effects, |_| "write.mkdir"),
                write_file: forbidden!(effects, |_, _| "write.write_file"),
            }),
        },
        grep: GrepToolOptions {
            operations: Some(GrepOperations {
                is_directory: forbidden!(effects, |_| "grep.is_directory"),
                read_file: forbidden!(effects, |_| "grep.read_file"),
            }),
        },
        find: FindToolOptions {
            operations: Some(FindOperations {
                exists: forbidden!(effects, |_| "find.exists"),
                glob: forbidden!(effects, |_, _, _| "find.glob"),
            }),
        },
        ls: LsToolOptions {
            operations: Some(LsOperations {
                exists: forbidden!(effects, |_| "ls.exists"),
                stat: forbidden!(effects, |_| "ls.stat"),
                readdir: forbidden!(effects, |_| "ls.readdir"),
            }),
        },
    }
}

/// Shell execution and synchronous interception use the same closed transcript.
fn bash(effects: &Effects) -> BashToolOptions {
    let observed = effects.clone();
    let operations = Some(BashOperations {
        exec: Rc::new(move |_, _, _| {
            observed.borrow_mut().push("bash.exec".into());
            Box::pin(async { Err("unexpected factory execution".into()) })
        }),
    });
    let effects = effects.clone();
    BashToolOptions {
        operations,
        spawn_hook: Some(Rc::new(move |_, _| {
            effects.borrow_mut().push("bash.hook".into());
            Err("unexpected factory interception".into())
        })),
        ..BashToolOptions::default()
    }
}

/// Supplies every read observation, including the independent MIME callback.
fn read(effects: &Effects) -> ReadToolOptions {
    ReadToolOptions {
        auto_resize_images: None,
        operations: Some(ReadOperations {
            access: forbidden!(effects, |_| "read.access"),
            read_file: forbidden!(effects, |_| "read.read_file"),
            detect_image_mime_type: Some(forbidden!(effects, |_| "read.mime")),
        }),
    }
}
