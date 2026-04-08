// Translates a `ScutarBackup` + `ScutarConnection` pair into the engine-side
// `BackupSpec`. This is the only place in the operator that knows the shape
// of the engine contract.

import {
  CREDENTIALS_MOUNT_DIR,
  PASSWORD_MOUNT_DIR,
  SOURCE_MOUNT_DIR,
} from "../consts";
import type { BackupSpec, ConnectionSpec } from "../types/backup-spec";
import type { ScutarBackup, ScutarConnection } from "../types/crd";
import { ValidationError } from "./validation";

export function buildBackupSpec(
  backup: ScutarBackup,
  connection: ScutarConnection,
): BackupSpec {
  const sourcePath = backup.spec.source.path
    ? joinPaths(SOURCE_MOUNT_DIR, backup.spec.source.path)
    : SOURCE_MOUNT_DIR;

  const destination = buildConnectionSpec(backup, connection);

  const spec: BackupSpec = {
    name: backup.metadata.name,
    mode: backup.spec.mode,
    source: {
      path: sourcePath,
      include: backup.spec.source.include,
      exclude: backup.spec.source.exclude,
    },
    destination,
    credentials_dir: CREDENTIALS_MOUNT_DIR,
    labels: {
      "scutar.io/backup": backup.metadata.name,
      "scutar.io/namespace": backup.metadata.namespace || "scutar",
    },
  };

  if (backup.spec.encryption?.enabled) {
    const key = backup.spec.encryption.passwordSecretRef?.key || "password";
    spec.encryption = {
      enabled: true,
      password_file: `${PASSWORD_MOUNT_DIR}/${key}`,
    };
  }

  if (backup.spec.retention && backup.spec.mode === "snapshot") {
    spec.retention = {
      keep_last: backup.spec.retention.keepLast,
      keep_daily: backup.spec.retention.keepDaily,
      keep_weekly: backup.spec.retention.keepWeekly,
      keep_monthly: backup.spec.retention.keepMonthly,
      keep_yearly: backup.spec.retention.keepYearly,
    };
  }

  return spec;
}

function buildConnectionSpec(
  backup: ScutarBackup,
  conn: ScutarConnection,
): ConnectionSpec {
  const prefix = backup.spec.destinationPath || undefined;

  switch (conn.spec.type) {
    case "local":
      // For `local`, the operator must mount the path into the engine Pod
      // as a hostPath/PVC; the engine just sees a filesystem path.
      return { type: "local", path: conn.spec.local!.path };

    case "s3": {
      const s3 = conn.spec.s3!;
      return {
        type: "s3",
        bucket: s3.bucket,
        region: s3.region,
        prefix,
        endpoint: s3.endpoint,
        force_path_style: s3.forcePathStyle,
      };
    }

    case "azure": {
      const az = conn.spec.azure!;
      return {
        type: "azure",
        account: az.account,
        container: az.container,
        prefix,
      };
    }

    case "gcs":
      return {
        type: "gcs",
        bucket: conn.spec.gcs!.bucket,
        prefix,
      };

    case "sftp": {
      const sf = conn.spec.sftp!;
      return {
        type: "sftp",
        host: sf.host,
        port: sf.port,
        user: sf.user,
        path: backup.spec.destinationPath || "/",
      };
    }
  }

  throw new ValidationError(
    `unknown connection type: ${(conn.spec as { type: string }).type}`,
  );
}

function joinPaths(base: string, suffix: string): string {
  if (!suffix) return base;
  if (suffix.startsWith("/")) return suffix;
  return `${base.replace(/\/+$/, "")}/${suffix.replace(/^\/+/, "")}`;
}
