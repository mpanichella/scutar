use serde::{Deserialize, Serialize};

/// Report of a finished backup run, surfaced back to the operator (which will
/// translate it into a `ScutarSnapshot` CRD status update).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunReport {
    pub mode: String,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub files_processed: u64,
    pub files_skipped: u64,
    pub files_deleted: u64,
    pub snapshot_id: Option<String>,
}
