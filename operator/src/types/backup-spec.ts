// `BackupSpec` — the contract between the operator (TypeScript) and the engine
// (Rust). The operator builds an instance of this, serializes it to YAML, and
// mounts it as a ConfigMap into the engine Pod at /etc/scutar/spec.yaml.
//
// IMPORTANT: every field here must match `engine/crates/scutar-core/src/spec.rs`
// exactly. Mismatches will surface as serde deserialization errors at engine
// startup, which is by design — we want loud, early failures.

export type BackupMode = "snapshot" | "mirror";

export type ConnectionSpec =
  | { type: "local"; path: string }
  | {
      type: "s3";
      bucket: string;
      region: string;
      prefix?: string;
      endpoint?: string;
      force_path_style?: boolean;
    }
  | { type: "azure"; account: string; container: string; prefix?: string }
  | { type: "gcs"; bucket: string; prefix?: string }
  | { type: "sftp"; host: string; port?: number; user: string; path: string };

export interface SourceSpec {
  path: string;
  include?: string[];
  exclude?: string[];
}

export interface EncryptionSpec {
  enabled: boolean;
  password_file: string;
}

export interface Retention {
  keep_last?: number;
  keep_daily?: number;
  keep_weekly?: number;
  keep_monthly?: number;
  keep_yearly?: number;
}

export interface BackupSpec {
  name: string;
  mode: BackupMode;
  source: SourceSpec;
  destination: ConnectionSpec;
  encryption?: EncryptionSpec;
  retention?: Retention;
  credentials_dir?: string;
  labels?: Record<string, string>;
}
