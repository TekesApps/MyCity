//! Fixed process role heap. Only adapter implements the allocator safety boundary.
//! Requested Rust heap and physically reserved arena are different measurements.
#[allow(unsafe_code)]
mod adapter;
pub use adapter::{Activation, Allocator, Snapshot};
