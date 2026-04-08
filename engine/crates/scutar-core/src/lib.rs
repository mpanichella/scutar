//! Scutar core: shared types, errors and the `StorageBackend` trait that all
//! cloud backends implement. This crate is intentionally dependency-light: it
//! defines the contract that the rest of the engine builds on top of.
//!
//! See `Agent.md` at the repository root for the full architecture context.

pub mod backend;
pub mod error;
pub mod repo_layout;
pub mod spec;

pub use backend::{BackendCapabilities, ObjectMeta, StorageBackend};
pub use error::{Error, Result};
pub use spec::{
    BackupMode, BackupSpec, ConnectionSpec, EncryptionSpec, Retention, SourceSpec,
};
