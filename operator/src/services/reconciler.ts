// The reconcile loop. For each ScutarBackup event:
//   1. Resolve the referenced ScutarConnection.
//   2. Validate both CRs (early failure).
//   3. Build the engine BackupSpec.
//   4. Apply ConfigMap (spec.yaml) + Job/CronJob to the cluster.
//   5. Update the ScutarBackup status.

import {
  CR_NAME_ANNOTATION,
  JOB_NAME,
  RUN_NOW_ANNOTATION,
} from "../consts";
import { applyConfigMap, applyCronJob, applyJob, createAdHocJob } from "../k8s/dispatcher";
import {
  getBackup,
  getConnection,
  patchBackupStatus,
  removeRunNowAnnotation,
} from "../k8s/client";
import type { ScutarBackup } from "../types/crd";
import { buildManifests } from "./job-builder";
import { buildBackupSpec } from "./spec-builder";
import { ValidationError, validateBackup, validateConnection } from "./validation";

export async function reconcileBackup(cr: ScutarBackup): Promise<void> {
  const namespace = cr.metadata.namespace || "scutar";
  const name = cr.metadata.name;
  const log = (msg: string) => console.log(`[scutar] ${namespace}/${name}: ${msg}`);
  const errlog = (msg: string) => console.error(`[scutar] ${namespace}/${name}: ${msg}`);

  try {
    validateBackup(cr);

    const connection = await getConnection(
      cr.spec.destinationConnectionRef.name,
      namespace,
    );
    if (!connection) {
      throw new ValidationError(
        `referenced ScutarConnection '${cr.spec.destinationConnectionRef.name}' not found in namespace '${namespace}'`,
      );
    }
    validateConnection(connection);

    const spec = buildBackupSpec(cr, connection);
    const manifests = buildManifests(cr, connection, spec);

    await applyConfigMap(manifests.configMap, namespace);
    log(`ConfigMap with BackupSpec applied`);

    if (manifests.cronJob) {
      await applyCronJob(manifests.cronJob, namespace);
      log(`CronJob applied (schedule: ${cr.spec.schedule})`);

      // Honor run-now annotation: spawn an immediate Job derived from the
      // CronJob's pod template, then strip the annotation.
      const runNow = cr.metadata.annotations?.[RUN_NOW_ANNOTATION];
      if (runNow) {
        const adHocName = `${JOB_NAME(name)}-run-${runNow}`;
        await createAdHocJob(JOB_NAME(name), adHocName, namespace);
        await removeRunNowAnnotation(cr);
        log(`run-now Job ${adHocName} created`);
      }

      await patchBackupStatus(cr, {
        condition: "Ready",
        message: "CronJob applied",
        observedGeneration: cr.metadata as unknown as number,
      });
    } else if (manifests.job) {
      await applyJob(manifests.job, namespace);
      log(`one-off Job applied`);
      await patchBackupStatus(cr, {
        condition: "Running",
        message: "Job dispatched",
      });
    }
  } catch (err) {
    if (err instanceof ValidationError) {
      errlog(`invalid: ${err.message}`);
      await patchBackupStatus(cr, {
        condition: "Invalid",
        message: err.message,
      }).catch(() => {});
      return;
    }
    errlog(`reconcile failed: ${err instanceof Error ? err.message : String(err)}`);
    await patchBackupStatus(cr, {
      condition: "Failed",
      message: err instanceof Error ? err.message : String(err),
    }).catch(() => {});
  }
}

/** Re-export for the caller; not used internally. */
export const _internals = { CR_NAME_ANNOTATION };
