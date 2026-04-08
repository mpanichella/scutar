// Early validation of incoming CRDs. The operator refuses to dispatch a Job
// when the spec is incomplete or contradictory — the engine should *never*
// have to guess. This is the explicit rejection of the previous design where
// the runner inferred missing fields at runtime.

import type { ScutarBackup, ScutarConnection } from "../types/crd";

export class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ValidationError";
  }
}

export function validateBackup(cr: ScutarBackup): void {
  const spec = cr.spec;
  if (!spec) throw new ValidationError("spec is required");

  if (spec.mode !== "snapshot" && spec.mode !== "mirror") {
    throw new ValidationError(`spec.mode must be 'snapshot' or 'mirror' (got '${spec.mode}')`);
  }

  if (!spec.source?.pvcName) {
    throw new ValidationError("spec.source.pvcName is required");
  }

  if (!spec.destinationConnectionRef?.name) {
    throw new ValidationError("spec.destinationConnectionRef.name is required");
  }

  if (spec.encryption?.enabled) {
    if (spec.mode !== "snapshot") {
      throw new ValidationError(
        "spec.encryption.enabled is only supported in mode 'snapshot'",
      );
    }
    if (!spec.encryption.passwordSecretRef?.name) {
      throw new ValidationError(
        "spec.encryption.enabled=true requires spec.encryption.passwordSecretRef.name",
      );
    }
  }

  if (spec.retention && spec.mode !== "snapshot") {
    throw new ValidationError("spec.retention is only honored in mode 'snapshot'");
  }

  if (spec.schedule) {
    // Cheap cron sanity check: 5 fields. Real validation happens server-side
    // when the CronJob is created.
    const fields = spec.schedule.trim().split(/\s+/);
    if (fields.length !== 5) {
      throw new ValidationError(
        `spec.schedule must be a 5-field cron expression (got '${spec.schedule}')`,
      );
    }
  }
}

export function validateConnection(cr: ScutarConnection): void {
  const spec = cr.spec;
  if (!spec) throw new ValidationError("spec is required");

  switch (spec.type) {
    case "s3":
      if (!spec.s3?.bucket || !spec.s3?.region) {
        throw new ValidationError("ScutarConnection type=s3 requires spec.s3.bucket and spec.s3.region");
      }
      break;
    case "azure":
      if (!spec.azure?.account || !spec.azure?.container) {
        throw new ValidationError("ScutarConnection type=azure requires spec.azure.account and spec.azure.container");
      }
      break;
    case "gcs":
      if (!spec.gcs?.bucket) {
        throw new ValidationError("ScutarConnection type=gcs requires spec.gcs.bucket");
      }
      break;
    case "sftp":
      if (!spec.sftp?.host || !spec.sftp?.user) {
        throw new ValidationError("ScutarConnection type=sftp requires spec.sftp.host and spec.sftp.user");
      }
      break;
    case "local":
      if (!spec.local?.path) {
        throw new ValidationError("ScutarConnection type=local requires spec.local.path");
      }
      break;
    default:
      throw new ValidationError(`unknown ScutarConnection type: '${(spec as { type: string }).type}'`);
  }

  if (spec.type !== "local" && !spec.credentialsSecretRef?.name) {
    throw new ValidationError(
      `ScutarConnection type=${spec.type} requires spec.credentialsSecretRef.name`,
    );
  }
}
