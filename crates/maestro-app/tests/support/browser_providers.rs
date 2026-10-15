// Controlled runtime for browser provider witnesses.
/// Drive a controlled future with the native I/O runtime.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime().build().unwrap().block_on(future)
}

/// Prepare the common native I/O runtime for a controlled test.
pub fn runtime() -> tokio::runtime::Builder {
    let mut builder = tokio::runtime::Builder::new_current_thread();
    builder.enable_all();
    builder
}
