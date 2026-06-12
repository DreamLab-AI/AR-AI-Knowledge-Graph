//! Process-wide tokio runtime for the gdext crate.
//!
//! The Godot scene tree is single-threaded; all WebSocket I/O runs on this
//! shared multi-thread runtime and hands results back to the main thread via
//! the thread-safe inbound queues in `transport`. Built lazily on first use so
//! a client that never networks pays nothing.

use std::sync::OnceLock;

use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("visionclaw-xr-net")
            .build()
            .expect("failed to build tokio runtime for visionclaw-xr-gdext")
    })
}
