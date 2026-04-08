// Applies the manifests built by `job-builder` to the cluster, with
// idempotent create-or-replace semantics.

import {
  V1ConfigMap,
  V1CronJob,
  V1Job,
} from "@kubernetes/client-node";

import { batchApi, coreApi, isNotFound } from "./client";

export async function applyConfigMap(cm: object, namespace: string): Promise<void> {
  const body = cm as V1ConfigMap;
  const name = body.metadata!.name!;
  try {
    await coreApi.replaceNamespacedConfigMap(name, namespace, body);
  } catch (e) {
    if (!isNotFound(e)) throw e;
    await coreApi.createNamespacedConfigMap(namespace, body);
  }
}

export async function applyJob(job: object, namespace: string): Promise<void> {
  const body = job as V1Job;
  const name = body.metadata!.name!;
  // Jobs are immutable once created (their pod template is sealed). If one
  // already exists with the same name we leave it alone — that's the desired
  // behavior for one-off backups.
  try {
    await batchApi.readNamespacedJob(name, namespace);
    return; // already exists
  } catch (e) {
    if (!isNotFound(e)) throw e;
  }
  await batchApi.createNamespacedJob(namespace, body);
}

export async function applyCronJob(cron: object, namespace: string): Promise<void> {
  const body = cron as V1CronJob;
  const name = body.metadata!.name!;
  try {
    const existing = await batchApi.readNamespacedCronJob(name, namespace);
    body.metadata!.resourceVersion = existing.body.metadata!.resourceVersion;
    await batchApi.replaceNamespacedCronJob(name, namespace, body);
  } catch (e) {
    if (!isNotFound(e)) throw e;
    await batchApi.createNamespacedCronJob(namespace, body);
  }
}

/**
 * Create a one-off Job from an existing CronJob's pod template, with a unique
 * suffix. Used for "run now" annotations to trigger an immediate execution
 * of a scheduled backup.
 */
export async function createAdHocJob(
  podSpecCronJobName: string,
  jobName: string,
  namespace: string,
): Promise<void> {
  const cronRes = await batchApi.readNamespacedCronJob(podSpecCronJobName, namespace);
  const cronJob = cronRes.body;
  const job: V1Job = {
    apiVersion: "batch/v1",
    kind: "Job",
    metadata: {
      name: jobName,
      namespace,
      labels: cronJob.metadata?.labels,
      ownerReferences: cronJob.metadata?.ownerReferences,
    },
    spec: cronJob.spec!.jobTemplate.spec,
  };
  await applyJob(job, namespace);
}
