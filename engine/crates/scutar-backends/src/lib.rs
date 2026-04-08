//! Backend implementations of `scutar_core::StorageBackend`.
//!
//! Each submodule is responsible for a single cloud and is the *only* place
//! in the codebase that imports its respective SDK. The `factory` function
//! takes a `ConnectionSpec` and returns a boxed trait object — the rest of
//! the engine never knows which backend it's talking to.
//!
//! Adding a new backend:
//!   1. Create `src/<name>/mod.rs` implementing `StorageBackend`.
//!   2. Add a `pub mod <name>;` here.
//!   3. Add a match arm in `factory.rs`.
//!   4. Done. The engine layer needs zero changes.

pub mod azure;
pub mod gcs;
pub mod local;
pub mod s3;
pub mod sftp;

mod factory;
pub use factory::build_backend;
