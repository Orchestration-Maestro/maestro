//! Controlled HTTP bytes for WAD discovery without live downloads.
use maestro_extensions::native_io::{
    ChildProcess, FsWatcher, IoFuture, NativeIo, Readable, Request, Response, SpawnOptions,
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

/// Requests observed before returning the supplied response.
pub struct Io {
    /// Requested destinations in admission order.
    pub requests: RefCell<Vec<String>>,
    /// Supplied response status and body.
    pub response: Rc<Reply>,
}
impl NativeIo for Io {
    fn watch(&self, _: String, _: Rc<dyn Fn()>) -> Result<Rc<dyn FsWatcher>, String> {
        Err("unexpected watch".into())
    }
    fn fetch(&self, request: Request) -> IoFuture<'_, Rc<dyn Response>> {
        assert!(request.method.is_none());
        assert!(request.headers.is_empty());
        assert!(request.body.is_none());
        self.requests.borrow_mut().push(request.url);
        let response: Rc<dyn Response> = self.response.clone();
        Box::pin(async move { Ok(response) })
    }
    fn spawn(
        &self,
        _: String,
        _: Vec<String>,
        _: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>> {
        Box::pin(async { Err("unexpected spawn".into()) })
    }
    fn kill(&self, _: i32, _: Option<String>) -> Result<(), String> {
        Err("unexpected kill".into())
    }
}
/// One response with an independently retained consuming reader.
pub struct Reply {
    /// Supplied status, including failures.
    pub status: u16,
    /// Raw chunks, optionally ending with a read failure.
    pub reader: Rc<Reader>,
}
impl Response for Reply {
    fn json(&self) -> IoFuture<'_, serde_json::Value> {
        Box::pin(async { Err("unexpected JSON read".into()) })
    }
    fn text(&self) -> IoFuture<'_, String> {
        Box::pin(async { Err("unexpected text read".into()) })
    }
    fn status(&self) -> u16 {
        self.status
    }
    fn status_text(&self) -> String {
        String::new()
    }
    fn headers(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    fn body(&self) -> Option<Rc<dyn Readable>> {
        Some(self.reader.clone())
    }
}
/// Sequential response bytes and completion.
pub struct Reader {
    /// Remaining chunks or the supplied failure.
    pub chunks: RefCell<VecDeque<Result<Vec<u8>, String>>>,
}
impl Readable for Reader {
    fn read(&self) -> IoFuture<'_, Option<Vec<u8>>> {
        let next = self.chunks.borrow_mut().pop_front().transpose();
        Box::pin(async move { next })
    }
    fn close(&self) -> Result<(), String> {
        self.chunks.borrow_mut().clear();
        Ok(())
    }
}
