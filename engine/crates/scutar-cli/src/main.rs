//! `scutar-runner` — the engine binary that runs inside the Pod created by
//! the operator. Modes:
//!
//!   * Backup (default): read a `BackupSpec` YAML, run snapshot or mirror.
//!     Usage: `scutar-runner --spec /etc/scutar/spec.yaml`
//!
//!   * Restore: read a spec (for the destination + encryption settings),
//!     pull a snapshot id, write to a target path.
//!     Usage: `scutar-runner --spec /etc/scutar/spec.yaml \
//!                            --restore <snapshot-id> \
//!                            --target /data`

use std::{path::PathBuf, process::ExitCode};

use scutar_core::BackupSpec;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    init_tracing();

    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("error: {msg}");
            print_usage();
            return ExitCode::from(2);
        }
    };

    match dispatch(args).await {
        Ok(report) => {
            // Print the report as a single JSON line so the operator's status
            // collector can pick it up from the Pod logs.
            match serde_json::to_string(&report) {
                Ok(line) => println!("{line}"),
                Err(_) => tracing::warn!("failed to serialize report"),
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            tracing::error!(error = %err, "scutar-runner failed");
            ExitCode::FAILURE
        }
    }
}

struct Args {
    spec_path: PathBuf,
    restore: Option<String>,
    target: Option<PathBuf>,
}

async fn dispatch(args: Args) -> anyhow::Result<scutar_engine::RunReport> {
    let raw = std::fs::read_to_string(&args.spec_path)?;
    let spec: BackupSpec = serde_yaml::from_str(&raw)?;
    if let Some(snap) = args.restore {
        let target = args
            .target
            .ok_or_else(|| anyhow::anyhow!("--restore requires --target <dir>"))?;
        tracing::info!(snapshot = %snap, target = %target.display(), "restore mode");
        let report = scutar_engine::restore::restore_from_spec(&spec, &snap, &target).await?;
        Ok(report)
    } else {
        tracing::info!(name = %spec.name, mode = ?spec.mode, "backup mode");
        let report = scutar_engine::run(&spec).await?;
        Ok(report)
    }
}

fn parse_args() -> Result<Args, String> {
    let mut iter = std::env::args().skip(1);
    let mut spec: Option<PathBuf> = None;
    let mut restore: Option<String> = None;
    let mut target: Option<PathBuf> = None;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--spec" => spec = iter.next().map(PathBuf::from),
            "--restore" => restore = iter.next(),
            "--target" => target = iter.next().map(PathBuf::from),
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Args {
        spec_path: spec.ok_or_else(|| "--spec is required".to_string())?,
        restore,
        target,
    })
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  scutar-runner --spec <spec.yaml>");
    eprintln!("  scutar-runner --spec <spec.yaml> --restore <snapshot-id> --target <dir>");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .json()
        .init();
}
