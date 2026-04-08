//! Filesystem walker shared by mirror and snapshot modes.
//!
//! Walks a source path top-down, applies include/exclude globs (relative to
//! the source root, POSIX separators), and yields each regular file with its
//! relative path and metadata. Symlinks are *not* followed by default — they
//! are reported as their own type and skipped, since following them across
//! arbitrary mounts is a footgun. A future flag can re-enable that.

use globset::{Glob, GlobSet, GlobSetBuilder};
use scutar_core::{Error, Result, SourceSpec};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Relative path from the source root, using POSIX separators.
    pub rel_path: String,
    /// Absolute path on disk.
    pub abs_path: PathBuf,
    pub size: u64,
    pub mtime: Option<i64>,
}

pub struct Walker {
    root: PathBuf,
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl Walker {
    pub fn from_source(source: &SourceSpec) -> Result<Self> {
        Ok(Self {
            root: source.path.clone(),
            include: build_globset(&source.include)?,
            exclude: build_globset(&source.exclude)?,
        })
    }

    /// Eagerly walk the source tree and return all matching files. We collect
    /// into a Vec rather than streaming because both mirror and snapshot need
    /// the full list (mirror to diff, snapshot to compute totals before
    /// uploading).
    pub fn collect(&self) -> Result<Vec<FileEntry>> {
        if !self.root.exists() {
            return Err(Error::Config(format!(
                "source path does not exist: {}",
                self.root.display()
            )));
        }
        let mut out = Vec::new();
        for entry in WalkDir::new(&self.root).follow_links(false) {
            let entry = entry.map_err(|e| Error::Other(format!("walkdir: {e}")))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs = entry.path().to_path_buf();
            let rel = match abs.strip_prefix(&self.root) {
                Ok(r) => r.to_path_buf(),
                Err(_) => continue,
            };
            let rel_str = rel.to_string_lossy().replace('\\', "/");

            if let Some(inc) = &self.include {
                if !inc.is_match(&rel_str) {
                    continue;
                }
            }
            if let Some(exc) = &self.exclude {
                if exc.is_match(&rel_str) {
                    continue;
                }
            }

            let meta = entry.metadata().map_err(|e| Error::Other(format!("metadata: {e}")))?;
            out.push(FileEntry {
                rel_path: rel_str,
                abs_path: abs,
                size: meta.len(),
                mtime: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64),
            });
        }
        // Sorted output makes diffs deterministic and tests easier.
        out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        Ok(out)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn build_globset(patterns: &[String]) -> Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        let glob = Glob::new(p).map_err(|e| Error::Config(format!("invalid glob '{p}': {e}")))?;
        builder.add(glob);
    }
    Ok(Some(
        builder.build().map_err(|e| Error::Config(format!("globset: {e}")))?,
    ))
}
