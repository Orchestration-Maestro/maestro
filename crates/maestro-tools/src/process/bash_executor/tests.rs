//! Native failures at the direct output boundary.
use super::*;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct FailingWriter {
    dropped: Arc<AtomicBool>,
    fail_write: bool,
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            Err(io::Error::from_raw_os_error(28))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(5))
    }
}
impl Drop for FailingWriter {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[test]
fn direct_spill_io_errors_are_returned() {
    if std::env::var_os("MAESTRO_DIRECT_MISSING_TEMP").is_some() {
        missing_temp_child();
        return;
    }
    let missing = std::env::temp_dir().join(format!(
        "maestro-missing-{}",
        crate::output::output_accumulator::random_id().unwrap()
    ));
    let child = maestro_test_process::TestProcess::current("direct_spill_io_errors_are_returned")
        .unwrap()
        .env("MAESTRO_DIRECT_MISSING_TEMP", "1")
        .temp_dir(&missing)
        .status()
        .unwrap();
    assert!(child.success());
    for fail_write in [true, false] {
        let dropped = Arc::new(AtomicBool::new(false));
        let mut output = Output {
            writer: Some(Box::new(FailingWriter {
                dropped: dropped.clone(),
                fail_write,
            })),
            ..Output::default()
        };
        let written = output.store::<dyn FnMut(&str)>("output".into(), &mut None);
        let closed = close_artifact(output.writer.take());
        assert!(dropped.load(Ordering::SeqCst));
        if fail_write {
            let BashError::Other(error) = written.unwrap_err() else {
                panic!("wrong error category")
            };
            assert_eq!(
                error.downcast_ref::<io::Error>().unwrap().raw_os_error(),
                Some(28)
            );
            assert_eq!(closed.unwrap_err().raw_os_error(), Some(5));
        } else {
            written.unwrap();
            assert_eq!(closed.unwrap_err().raw_os_error(), Some(5));
        }
    }
}

struct Producer;
impl BashOperations for Producer {
    fn exec<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        options: BashExecOptions<'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<super::super::bash::BashExecResult, BashError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            (options.on_data)(&vec![b'x'; DEFAULT_MAX_BYTES + 1]);
            (options.on_data)(b"later");
            Err(BashError::Timeout(7.0))
        })
    }
}

fn missing_temp_child() {
    let mut callbacks = 0;
    let mut callback = |_: &str| {
        callbacks += 1;
    };
    let error = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(execute_bash_with_operations(
            "command",
            "cwd",
            &Producer,
            BashExecutorOptions {
                on_chunk: Some(&mut callback),
                signal: None,
            },
        ))
        .unwrap_err();
    let BashError::Other(error) = error else {
        panic!("processing failure lost to operation error")
    };
    assert_eq!(
        error.downcast_ref::<io::Error>().unwrap().kind(),
        io::ErrorKind::NotFound
    );
    assert_eq!(callbacks, 0);
}
