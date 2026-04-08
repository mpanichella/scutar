// CRD type definitions for `scutar.io/v1alpha1`. These mirror the YAML
// schemas declared in operator/deployment/helm/templates/crd-*.yaml.
//
// IMPORTANT: changes to these types must be reflected in both:
//   - operator/deployment/helm/templates/crd-*.yaml  (the cluster contract)
//   - server/backend/src/scutar-crd.ts                (UI/API view of the same)
//   - engine/crates/scutar-core/src/spec.rs           (engine view of the spec)

export const SCUTAR_GROUP = "scutar.io";
export const SCUTAR_VERSION = "v1alpha1";

export const SCUTAR_PLURAL_BACKUPS = "scutarbackups";
export const SCUTAR_PLURAL_CONNECTIONS = "scutarconnections";
export const SCUTAR_PLURAL_SNAPSHOTS = "scutarsnapshots";
export const SCUTAR_PLURAL_RESTORES = "scutarrestorerequests";

export const SCUTAR_KIND_BACKUP = "ScutarBackup";
export const SCUTAR_KIND_CONNECTION = "ScutarConnection";
export const SCUTAR_KIND_SNAPSHOT = "ScutarSnapshot";
export const SCUTAR_KIND_RESTORE = "ScutarRestoreRequest";

export type BackupMode = "snapshot" | "mirror";
export type ConnectionType = "local" | "s3" | "azure" | "gcs" | "sftp";

export interface ObjectMeta {
  name: string;
  namespace?: string;
  uid?: string;
  resourceVersion?: string;
  annotations?: Record<string, string>;
  labels?: Record<string, string>;
}

// ---- ScutarConnection -------------------------------------------------------

export interface S3ConnectionDetails {
  bucket: string;
  region: string;
  /** Optional S3-compatible endpoint (MinIO, Wasabi, ...). */
  endpoint?: string;
  /** Force path-style addressing (required by MinIO). */
  forcePathStyle?: boolean;
}

export interface AzureConnectionDetails {
  account: string;
  container: string;
}

export interface GcsConnectionDetails {
  bucket: string;
}

export interface SftpConnectionDetails {
  host: string;
  port?: number;
  user: string;
}

export interface LocalConnectionDetails {
  path: string;
}

export interface ScutarConnectionSpec {
  type: ConnectionType;
  s3?: S3ConnectionDetails;
  azure?: AzureConnectionDetails;
  gcs?: GcsConnectionDetails;
  sftp?: SftpConnectionDetails;
  local?: LocalConnectionDetails;
  /** Reference to a Secret holding credentials for this backend. */
  credentialsSecretRef?: { name: string };
}

export interface ScutarConnection {
  apiVersion: string;
  kind: typeof SCUTAR_KIND_CONNECTION;
  metadata: ObjectMeta;
  spec: ScutarConnectionSpec;
}

// ---- ScutarBackup -----------------------------------------------------------

export interface SourceSpec {
  /** PVC name to mount as the source volume. */
  pvcName: string;
  /** Path inside the mounted PVC. Defaults to "/data". */
  path?: string;
  /** Optional include globs (relative to source.path). */
  include?: string[];
  /** Optional exclude globs. */
  exclude?: string[];
}

export interface EncryptionSpec {
  enabled: boolean;
  passwordSecretRef?: { name: string; key?: string };
}

export interface RetentionSpec {
  keepLast?: number;
  keepDaily?: number;
  keepWeekly?: number;
  keepMonthly?: number;
  keepYearly?: number;
}

export interface JobTemplateOverrides {
  backoffLimit?: number;
  activeDeadlineSeconds?: number;
  ttlSecondsAfterFinished?: number;
  resources?: {
    limits?: Record<string, string>;
    requests?: Record<string, string>;
  };
}

export interface ScutarBackupSpec {
  /** Backup mode — see Agent.md §3.3. */
  mode: BackupMode;
  /** Cron schedule. If absent, the backup runs once on creation. */
  schedule?: string;
  source: SourceSpec;
  /** Reference to a `ScutarConnection` in the same namespace. */
  destinationConnectionRef: { name: string };
  /** Optional path/prefix inside the destination connection. */
  destinationPath?: string;
  encryption?: EncryptionSpec;
  retention?: RetentionSpec;
  jobTemplate?: JobTemplateOverrides;
  /** Suspend the schedule without deleting the resource. */
  suspend?: boolean;
}

export interface ScutarBackup {
  apiVersion: string;
  kind: typeof SCUTAR_KIND_BACKUP;
  metadata: ObjectMeta;
  spec: ScutarBackupSpec;
  status?: ScutarBackupStatus;
}

export interface ScutarBackupStatus {
  condition?: "Pending" | "Ready" | "Running" | "Failed" | "Invalid";
  message?: string;
  lastRun?: string;
  nextRun?: string;
  observedGeneration?: number;
}

// ---- ScutarSnapshot ---------------------------------------------------------

export interface ScutarSnapshotStatus {
  backupRef: string;
  snapshotId: string; // BLAKE3 hex
  createdAt: string;
  bytesRead?: number;
  bytesWritten?: number;
  filesProcessed?: number;
}

export interface ScutarSnapshot {
  apiVersion: string;
  kind: typeof SCUTAR_KIND_SNAPSHOT;
  metadata: ObjectMeta;
  status: ScutarSnapshotStatus;
}

// ---- ScutarRestoreRequest ---------------------------------------------------

export interface ScutarRestoreRequestSpec {
  snapshotRef: { name: string };
  targetPvcName: string;
  targetPath?: string;
}

export interface ScutarRestoreRequest {
  apiVersion: string;
  kind: typeof SCUTAR_KIND_RESTORE;
  metadata: ObjectMeta;
  spec: ScutarRestoreRequestSpec;
  status?: {
    condition?: "Pending" | "Running" | "Completed" | "Failed";
    message?: string;
  };
}
