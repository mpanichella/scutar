//! Retention policy applied after a successful snapshot run.
//!
//! Implements a simplified `keepLast / keepDaily / keepWeekly / keepMonthly /
//! keepYearly` policy à la restic. Snapshots that don't satisfy *any* keep
//! rule are deleted from the bucket. Note that pack files are NOT pruned by
//! this function — orphaned chunks are reclaimed by a separate `prune`
//! operation (future work) which is expensive and should run on a slower
//! schedule than retention.
//!
//! Returns the number of snapshots removed.

use futures::StreamExt;
use scutar_core::{Error, Result, Retention, StorageBackend};
use std::collections::BTreeSet;
use std::sync::Arc;
use time::{format_description::well_known::Rfc3339, Date, OffsetDateTime, Weekday};

use crate::encryption::Sealer;
use crate::manifest::SnapshotManifest;

pub async fn apply(
    backend: &Arc<dyn StorageBackend>,
    sealer: &Sealer,
    policy: &Retention,
) -> Result<u64> {
    // List all snapshot manifests under `snapshots/`.
    let mut keys = Vec::new();
    {
        let mut stream = backend.list("snapshots/");
        while let Some(item) = stream.next().await {
            keys.push(item?.key);
        }
    }
    if keys.is_empty() {
        return Ok(0);
    }

    // Load each manifest just enough to read its `created_at`. We could parse
    // the timestamp from the key but loading the manifest is more robust if
    // we ever change the key format.
    let mut snapshots: Vec<(String, OffsetDateTime)> = Vec::new();
    for key in keys {
        let raw = backend.get(&key).await?;
        let opened = sealer.open(raw)?;
        let m: SnapshotManifest = serde_json::from_slice(&opened)
            .map_err(|e| Error::Serde(format!("manifest parse: {e}")))?;
        let dt = OffsetDateTime::parse(&m.created_at, &Rfc3339)
            .map_err(|e| Error::Other(format!("snapshot timestamp parse: {e}")))?;
        snapshots.push((key, dt));
    }

    // Newest first.
    snapshots.sort_by(|a, b| b.1.cmp(&a.1));

    let mut keep: BTreeSet<usize> = BTreeSet::new();

    if let Some(n) = policy.keep_last {
        for i in 0..(n as usize).min(snapshots.len()) {
            keep.insert(i);
        }
    }

    keep_buckets(&snapshots, policy.keep_daily, bucket_day, &mut keep);
    keep_buckets(&snapshots, policy.keep_weekly, bucket_week, &mut keep);
    keep_buckets(&snapshots, policy.keep_monthly, bucket_month, &mut keep);
    keep_buckets(&snapshots, policy.keep_yearly, bucket_year, &mut keep);

    let mut removed = 0u64;
    for (i, (key, _)) in snapshots.iter().enumerate() {
        if keep.contains(&i) {
            continue;
        }
        backend.delete(key).await?;
        removed += 1;
    }
    Ok(removed)
}

fn keep_buckets(
    snapshots: &[(String, OffsetDateTime)],
    keep_n: Option<u32>,
    bucket: fn(&OffsetDateTime) -> String,
    keep: &mut BTreeSet<usize>,
) {
    let Some(n) = keep_n else { return };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (i, (_, dt)) in snapshots.iter().enumerate() {
        let key = bucket(dt);
        if seen.insert(key) {
            keep.insert(i);
        }
        if seen.len() as u32 >= n {
            break;
        }
    }
}

fn bucket_day(dt: &OffsetDateTime) -> String {
    format!("{}-{:02}-{:02}", dt.year(), dt.month() as u8, dt.day())
}

fn bucket_week(dt: &OffsetDateTime) -> String {
    // ISO week number bucket. `Date::iso_week` would be ideal but isn't on
    // OffsetDateTime directly; compute via the date.
    let d: Date = dt.date();
    let week = iso_week(d);
    format!("{}-W{:02}", d.year(), week)
}

fn bucket_month(dt: &OffsetDateTime) -> String {
    format!("{}-{:02}", dt.year(), dt.month() as u8)
}

fn bucket_year(dt: &OffsetDateTime) -> String {
    format!("{}", dt.year())
}

fn iso_week(d: Date) -> u8 {
    // Cheap ISO week approximation: (ordinal + days-from-monday) / 7.
    // Good enough for retention bucketing — we just need a stable per-week id.
    let ordinal = d.ordinal() as i32;
    let weekday_offset = match d.weekday() {
        Weekday::Monday => 0,
        Weekday::Tuesday => 1,
        Weekday::Wednesday => 2,
        Weekday::Thursday => 3,
        Weekday::Friday => 4,
        Weekday::Saturday => 5,
        Weekday::Sunday => 6,
    };
    let monday_ordinal = ordinal - weekday_offset;
    ((monday_ordinal + 6) / 7).max(1) as u8
}

