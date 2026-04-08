use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level spec consumed by the engine. The operator generates this from
/// `ScutarBackup` + `ScutarConnection` CRDs and mounts it as a ConfigMap into
/// the engine Pod. The engine reads it once at startup — no env-var inference,
/// no runtime guessing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSpec {
    /// Logical name of this backup run (matches `ScutarBackup.metadata.name`).
    pub name: String,

    /// Backup mode. See `BackupMode` for semantics.
    pub mode: BackupMode,

    /// Source: where to read data from (typically a mounted PVC path).
    pub source: SourceSpec,

    /// Destination: which storage backend + path to write to.
    pub destination: ConnectionSpec,

    /// Optional encryption settings (only honored in `Snapshot` mode).
    #[serde(default)]
    pub encryption: Option<EncryptionSpec>,

    /// Snapshot retention policy (only honored in `Snapshot` mode).
    #[serde(default)]
    pub retention: Option<Retention>,

    /// Filesystem path where the operator mounted the Secret containing
    /// credentials for the destination backend. Each backend reads what it
    /// needs from well-known files inside this directory (see backend docs).
    /// If absent, backends fall back to environment variables / SDK defaults.
    #[serde(default)]
    pub credentials_dir: Option<PathBuf>,

    /// Free-form labels propagated to the snapshot manifest / status.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Backup mode — replaces the previous `sync | full | incremental` triplet.
///
/// * `Snapshot` — content-addressable, deduplicated, encrypted (optional),
///   keeps history. The first run is a full snapshot; subsequent runs are
///   incremental for free thanks to dedup. Restorable.
/// * `Mirror` — 1:1 mirror of the source against the destination, no history,
///   no dedup. Equivalent to what rclone sync does. Useful for replication.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BackupMode {
    Snapshot,
    Mirror,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpec {
    /// Local filesystem path inside the engine Pod.
    pub path: PathBuf,
    /// Optional include globs (relative to source.path). Empty = include all.
    #[serde(default)]
    pub include: Vec<String>,
    /// Optional exclude globs (applied after include).
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Connection to a remote storage backend. Mirrors the `ScutarConnection` CRD
/// but lives here so the engine has zero K8s coupling.
///
/// `prefix` (or `path` for SFTP) is the *root* the engine writes under;
/// snapshot/mirror layouts live underneath it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConnectionSpec {
    /// Local filesystem destination — useful for tests and air-gapped envs.
    Local {
        path: PathBuf,
    },
    S3 {
        bucket: String,
        region: String,
        #[serde(default)]
        prefix: Option<String>,
        /// Optional endpoint for S3-compatible providers (MinIO, Wasabi, ...).
        #[serde(default)]
        endpoint: Option<String>,
        /// Force path-style addressing (required by MinIO and most local S3
        /// emulators).
        #[serde(default)]
        force_path_style: bool,
    },
    Azure {
        account: String,
        container: String,
        #[serde(default)]
        prefix: Option<String>,
    },
    Gcs {
        bucket: String,
        #[serde(default)]
        prefix: Option<String>,
    },
    Sftp {
        host: String,
        #[serde(default)]
        port: Option<u16>,
        user: String,
        /// Absolute path on the remote host where the engine writes.
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionSpec {
    pub enabled: bool,
    /// Path to a file mounted from a Kubernetes Secret containing the password.
    /// The engine reads it once at startup and zeroizes it from memory after
    /// deriving the KEK with Argon2id.
    pub password_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Retention {
    #[serde(default)]
    pub keep_last: Option<u32>,
    #[serde(default)]
    pub keep_daily: Option<u32>,
    #[serde(default)]
    pub keep_weekly: Option<u32>,
    #[serde(default)]
    pub keep_monthly: Option<u32>,
    #[serde(default)]
    pub keep_yearly: Option<u32>,
}

impl ConnectionSpec {
    /// Stable backend name for logging and error messages.
    pub fn backend_name(&self) -> &'static str {
        match self {
            ConnectionSpec::Local { .. } => "local",
            ConnectionSpec::S3 { .. } => "s3",
            ConnectionSpec::Azure { .. } => "azure",
            ConnectionSpec::Gcs { .. } => "gcs",
            ConnectionSpec::Sftp { .. } => "sftp",
        }
    }
}
