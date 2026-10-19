//! Shell operations retain their execution family's original ownership.
#![cfg(test)]
use maestro_models::Local;
use maestro_tools::{BashError, BashExecOptions, BashExecResult, BashExecution, BashOperations};
use std::{cell::Cell, rc::Rc};

/// Local operations can capture a caller-owned non-thread-safe owner.
struct LocalOperation(Rc<Cell<usize>>);

impl BashOperations<Local> for LocalOperation {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: BashExecOptions<'a, <Local as BashExecution>::Data<'a>>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<BashExecResult, BashError>> + 'a>> {
        Box::pin(async move {
            self.0.set(self.0.get() + 1);
            (options.on_data)(command.as_bytes());
            (options.on_data)(cwd.as_bytes());
            Ok(BashExecResult {
                exit_code: Some(7.0),
            })
        })
    }
}

#[test]
fn local_operation_retains_local_state_and_borrows_the_output_sink() {
    let calls = Rc::new(Cell::new(0));
    let operations: Rc<<Local as BashExecution>::Operations> =
        Rc::new(LocalOperation(calls.clone()));
    let mut output = Vec::new();
    let mut sink = |bytes: &[u8]| output.extend_from_slice(bytes);
    let result = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(operations.exec(
            " raw \0",
            "/cwd",
            BashExecOptions {
                on_data: &mut sink,
                signal: None,
                timeout: None,
                env: None,
            },
        ))
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(output, b" raw \0/cwd");
    assert_eq!(result.exit_code, Some(7.0));
}
