//! Cancellation at the point where a native directory has entered the table.
use super::*;
use futures_util::future::{Either, select};
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtxBuilder, filesystem::WasiFilesystemCtxView};

/// Dropping an operation releases its directory before the next root call uses the table.
#[test]
fn cancelled_placement_releases_temporary_descriptor() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut ctx = WasiCtxBuilder::new().build();
            let mut table = ResourceTable::new();
            let root = crate::wasi::root::open().unwrap();
            let mut view = View {
                fs: WasiFilesystemCtxView {
                    ctx: ctx.filesystem(),
                    table: &mut table,
                },
                root: &root,
            };
            let fd = view.directories().unwrap().remove(0).0;
            for _ in 0..64 {
                let (sent, received) = tokio::sync::oneshot::channel();
                let mut operation = Box::pin(async {
                    let placed = view
                        .place(Resource::new_borrow(fd.rep()), ".".into(), false)
                        .await
                        .unwrap();
                    sent.send(()).unwrap();
                    std::future::pending::<()>().await;
                    drop(placed);
                });
                match select(received, &mut operation).await {
                    Either::Left((ready, _)) => ready.unwrap(),
                    Either::Right(_) => panic!("operation must remain pending"),
                }
                drop(operation);
                let next_root = view.directories().unwrap().remove(0).0;
                HostDescriptor::drop(&mut view, next_root).unwrap();
                assert_eq!(view.fs.table.len(), 1);
            }
        });
}
