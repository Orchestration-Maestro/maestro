//! A native child and its byte reader end with their capabilities.
use super::ended_endpoint;
use crate::local_resources::{
    Capability,
    native_resources::{Child, Reader},
};
use maestro_extensions::native_io::{ChildProcess, ExitStatus, IoFuture, Readable, Writable};
use std::rc::Rc;

/// A reader that is always at its end.
struct Drained;

impl Readable for Drained {
    fn read(&self) -> IoFuture<'_, Option<Vec<u8>>> {
        Box::pin(async { Ok(None) })
    }

    fn close(&self) -> Result<(), String> {
        Ok(())
    }
}

/// A child without descriptors.
struct Detached;

impl ChildProcess for Detached {
    fn pid(&self) -> Option<u32> {
        None
    }

    fn stdin(&self) -> Option<Rc<dyn Writable>> {
        None
    }

    fn stdout(&self) -> Option<Rc<dyn Readable>> {
        None
    }

    fn stderr(&self) -> Option<Rc<dyn Readable>> {
        None
    }

    fn wait(&self) -> IoFuture<'_, ExitStatus> {
        Box::pin(async {
            Ok(ExitStatus {
                code: Some(0),
                signal: None,
            })
        })
    }

    fn kill(&self, _signal: Option<String>) -> Result<bool, String> {
        Ok(false)
    }
}

#[test]
fn released_child_rejects_every_kind_of_operation_with_its_ended_text() {
    let errors = ended_endpoint(Capability::NativeChild(Rc::new(Detached)), Child, |child| {
        [
            child.pid().unwrap_err(),
            child.stdin().err().unwrap(),
            child.output(false).err().unwrap(),
            child.output(true).err().unwrap(),
            child.kill(None).unwrap_err(),
        ]
    });
    assert_eq!(errors, ["native child resource ended"; 5]);
}

#[test]
fn released_reader_rejects_close_with_its_ended_text() {
    let errors = ended_endpoint(
        Capability::NativeReader(Rc::new(Drained)),
        Reader,
        |reader| [reader.close().unwrap_err()],
    );
    assert_eq!(errors, ["native byte resource ended"]);
}
