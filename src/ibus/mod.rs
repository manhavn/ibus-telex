//! The D-Bus side of the engine.

use std::future::Future;

pub mod engine;
pub mod keys;
pub mod object;

pub use engine::{run, Shared};

/// Drive a future to completion on the current thread.
///
/// zbus is built on `async-io`, so its own executor is all we need - and it
/// means the engine has no runtime dependency beyond that.
pub fn block_on<F: Future>(future: F) -> F::Output {
    zbus::block_on(future)
}
