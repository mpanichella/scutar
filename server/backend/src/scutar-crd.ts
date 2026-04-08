// CRD types as seen by the API/UI. Mirrors `operator/src/types/crd.ts` —
// keep them in sync. The UI consumes these to render forms; the API uses
// them when proxying CRUD calls to the Kubernetes API.

export const SCUTAR_GROUP = "scutar.io";
export const SCUTAR_VERSION = "v1alpha1";

export const SCUTAR_PLURAL_BACKUPS = "scutarbackups";
export const SCUTAR_PLURAL_CONNECTIONS = "scutarconnections";
export const SCUTAR_PLURAL_SNAPSHOTS = "scutarsnapshots";
export const SCUTAR_PLURAL_RESTORES = "scutarrestorerequests";

// Backwards-compatible aliases used elsewhere in the backend.
export const SCUTAR_PLURAL = SCUTAR_PLURAL_BACKUPS;

export type BackupMode = "snapshot" | "mirror";

export type ConnectionType = "local" | "s3" | "azure" | "gcs" | "sftp";

export interface S3ConnectionDetails {
  bucket: string;
  region: string;
  endpoint?: string;
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
  credentialsSecretRef?: { name: string };
}

export interface ScutarConnectionBody {
  apiVersion: string;
  kind: string;
  metadata: { name: string; namespace?: string };
  spec: ScutarConnectionSpec;
}

export interface SourceSpec {
  pvcName: string;
  path?: string;
  include?: string[];
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
  mode: BackupMode;
  schedule?: string;
  source: SourceSpec;
  destinationConnectionRef: { name: string };
  destinationPath?: string;
  encryption?: EncryptionSpec;
  retention?: RetentionSpec;
  jobTemplate?: JobTemplateOverrides;
  suspend?: boolean;
}

export interface ScutarBackupBody {
  apiVersion: string;
  kind: string;
  metadata: { name: string; namespace?: string; annotations?: Record<string, string> };
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

export interface ScutarSnapshotStatus {
  backupRef: string;
  snapshotId: string;
  createdAt: string;
  bytesRead?: number;
  bytesWritten?: number;
  filesProcessed?: number;
}

export interface ScutarSnapshotBody {
  apiVersion: string;
  kind: string;
  metadata: { name: string; namespace?: string };
  status: ScutarSnapshotStatus;
}

export interface ScutarRestoreRequestSpec {
  snapshotRef: { name: string };
  targetPvcName: string;
  targetPath?: string;
}

export interface ScutarRestoreRequestBody {
  apiVersion: string;
  kind: string;
  metadata: { name: string; namespace?: string };
  spec: ScutarRestoreRequestSpec;
  status?: {
    condition?: "Pending" | "Running" | "Completed" | "Failed";
    message?: string;
  };
}
