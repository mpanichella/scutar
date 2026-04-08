// UI-side types for ScutarBackup. Mirror of `server/backend/src/scutar-crd.ts`
// and (transitively) `operator/src/types/crd.ts`. Update all three together.

export type BackupMode = 'snapshot' | 'mirror'

export const BACKUP_MODE_LABELS: Record<BackupMode, string> = {
  snapshot: 'Snapshot (con historia + dedup)',
  mirror: 'Mirror (sync 1:1)',
}

export interface ScutarSourceSpec {
  pvcName: string
  path?: string
  include?: string[]
  exclude?: string[]
}

export interface ScutarEncryptionSpec {
  enabled: boolean
  passwordSecretRef?: { name: string; key?: string }
}

export interface ScutarRetentionSpec {
  keepLast?: number
  keepDaily?: number
  keepWeekly?: number
  keepMonthly?: number
  keepYearly?: number
}

export interface ScutarBackupSpec {
  mode: BackupMode
  schedule?: string
  source: ScutarSourceSpec
  destinationConnectionRef: { name: string }
  destinationPath?: string
  encryption?: ScutarEncryptionSpec
  retention?: ScutarRetentionSpec
  suspend?: boolean
  jobTemplate?: {
    backoffLimit?: number
    activeDeadlineSeconds?: number
    ttlSecondsAfterFinished?: number
    resources?: { limits?: Record<string, string>; requests?: Record<string, string> }
  }
}

export interface ScutarBackup {
  apiVersion?: string
  kind?: string
  metadata?: { name: string; namespace?: string }
  spec: ScutarBackupSpec
  status?: {
    condition?: string
    message?: string
    lastRun?: string
    nextRun?: string
  }
}

export interface BackupFormState {
  name: string
  namespace: string
  spec: ScutarBackupSpec
}

// ---- Backwards-compat shims --------------------------------------------------
//
// Several Vue components still import the old `LeviathanBackup` / `BackupType`
// names. They are kept here as type aliases so the project compiles after the
// CRD reshape; the affected views are flagged with `// TODO(scutar-ui)` and
// will be rewritten in a follow-up that updates the form UX to expose the new
// fields (mode, source.pvcName, encryption, retention).

/** @deprecated use BackupMode */
export type BackupType = 'snapshot' | 'mirror' | 'sync' | 'full' | 'incremental'

/** @deprecated use BACKUP_MODE_LABELS */
export const BACKUP_TYPE_LABELS: Record<string, string> = {
  ...BACKUP_MODE_LABELS,
  sync: 'Mirror',
  full: 'Snapshot',
  incremental: 'Snapshot',
}

/** @deprecated use ScutarBackup */
export type LeviathanBackup = ScutarBackup
/** @deprecated use ScutarBackupSpec */
export type LeviathanBackupSpec = ScutarBackupSpec
