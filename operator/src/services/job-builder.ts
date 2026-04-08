// Builds the Kubernetes Job / CronJob / ConfigMap manifests that the operator
// applies in the cluster. The Pod template here is the runtime contract: it
// must mount what the engine expects (see consts.ts and engine README).

import * as yaml from "js-yaml";
import {
  CR_NAMESPACE_ANNOTATION,
  CR_NAME_ANNOTATION,
  CREDENTIALS_MOUNT_DIR,
  CREDS_VOLUME,
  JOB_NAME,
  OWNER_LABEL,
  PASSWORD_MOUNT_DIR,
  PASSWORD_VOLUME,
  SCUTAR_ENGINE_IMAGE,
  SCUTAR_ENGINE_IMAGE_PULL_POLICY,
  SCUTAR_ENGINE_SERVICE_ACCOUNT,
  SOURCE_MOUNT_DIR,
  SOURCE_VOLUME,
  SPEC_CONFIGMAP_NAME,
  SPEC_MOUNT_DIR,
  SPEC_MOUNT_PATH,
  SPEC_VOLUME,
} from "../consts";
import type { BackupSpec } from "../types/backup-spec";
import type { ScutarBackup, ScutarConnection } from "../types/crd";

export interface DispatchManifests {
  configMap: object;
  job?: object;
  cronJob?: object;
}

export function buildManifests(
  backup: ScutarBackup,
  connection: ScutarConnection,
  spec: BackupSpec,
): DispatchManifests {
  const namespace = backup.metadata.namespace || "scutar";
  const name = backup.metadata.name;
  const ownerRefs = ownerReferences(backup);

  const configMap = buildConfigMap(name, namespace, spec, ownerRefs, backup);
  const podSpec = buildPodSpec(backup, connection);

  if (backup.spec.schedule) {
    return {
      configMap,
      cronJob: buildCronJob(name, namespace, backup, podSpec, ownerRefs),
    };
  }

  return {
    configMap,
    job: buildJob(JOB_NAME(name), namespace, backup, podSpec, ownerRefs),
  };
}

function ownerReferences(backup: ScutarBackup) {
  if (!backup.metadata.uid) return [];
  return [
    {
      apiVersion: "scutar.io/v1alpha1",
      kind: "ScutarBackup",
      name: backup.metadata.name,
      uid: backup.metadata.uid,
      controller: true,
      blockOwnerDeletion: true,
    },
  ];
}

function buildConfigMap(
  name: string,
  namespace: string,
  spec: BackupSpec,
  ownerRefs: ReturnType<typeof ownerReferences>,
  backup: ScutarBackup,
) {
  return {
    apiVersion: "v1",
    kind: "ConfigMap",
    metadata: {
      name: SPEC_CONFIGMAP_NAME(name),
      namespace,
      labels: { [OWNER_LABEL]: name },
      annotations: {
        [CR_NAME_ANNOTATION]: backup.metadata.name,
        [CR_NAMESPACE_ANNOTATION]: namespace,
      },
      ownerReferences: ownerRefs,
    },
    data: {
      "spec.yaml": yaml.dump(spec, { lineWidth: 120, noRefs: true }),
    },
  };
}

function buildPodSpec(backup: ScutarBackup, connection: ScutarConnection) {
  const volumes: Record<string, unknown>[] = [
    {
      name: SPEC_VOLUME,
      configMap: { name: SPEC_CONFIGMAP_NAME(backup.metadata.name) },
    },
    {
      name: SOURCE_VOLUME,
      persistentVolumeClaim: { claimName: backup.spec.source.pvcName },
    },
  ];

  if (connection.spec.credentialsSecretRef?.name) {
    volumes.push({
      name: CREDS_VOLUME,
      secret: { secretName: connection.spec.credentialsSecretRef.name },
    });
  }

  if (backup.spec.encryption?.enabled && backup.spec.encryption.passwordSecretRef?.name) {
    volumes.push({
      name: PASSWORD_VOLUME,
      secret: { secretName: backup.spec.encryption.passwordSecretRef.name },
    });
  }

  const volumeMounts: Record<string, unknown>[] = [
    {
      name: SPEC_VOLUME,
      mountPath: SPEC_MOUNT_DIR,
      readOnly: true,
    },
    {
      name: SOURCE_VOLUME,
      mountPath: SOURCE_MOUNT_DIR,
    },
  ];

  if (connection.spec.credentialsSecretRef?.name) {
    volumeMounts.push({
      name: CREDS_VOLUME,
      mountPath: CREDENTIALS_MOUNT_DIR,
      readOnly: true,
    });
  }

  if (backup.spec.encryption?.enabled) {
    volumeMounts.push({
      name: PASSWORD_VOLUME,
      mountPath: PASSWORD_MOUNT_DIR,
      readOnly: true,
    });
  }

  const env = buildEnv(connection);

  return {
    serviceAccountName: SCUTAR_ENGINE_SERVICE_ACCOUNT,
    restartPolicy: "Never",
    containers: [
      {
        name: "scutar-runner",
        image: SCUTAR_ENGINE_IMAGE,
        imagePullPolicy: SCUTAR_ENGINE_IMAGE_PULL_POLICY,
        args: ["--spec", SPEC_MOUNT_PATH],
        env,
        volumeMounts,
        resources: backup.spec.jobTemplate?.resources,
      },
    ],
    volumes,
  };
}

/** Build env vars so the cloud SDKs auto-discover credentials from the
 *  mounted Secret. The engine itself never reads these — they exist purely
 *  for SDK convention (AWS_*, AZURE_*, GOOGLE_APPLICATION_CREDENTIALS, ...).
 */
function buildEnv(connection: ScutarConnection): Record<string, unknown>[] {
  const env: Record<string, unknown>[] = [];

  if (!connection.spec.credentialsSecretRef?.name) return env;

  // We expose the entire Secret as files under CREDENTIALS_MOUNT_DIR (above);
  // here we additionally export the standard SDK env vars when the Secret has
  // the conventional keys. The Secret author chooses; we just wire the names.
  const secretName = connection.spec.credentialsSecretRef.name;
  const exportKey = (envName: string, key: string, optional = true) => {
    env.push({
      name: envName,
      valueFrom: {
        secretKeyRef: { name: secretName, key, optional },
      },
    });
  };

  switch (connection.spec.type) {
    case "s3":
      exportKey("AWS_ACCESS_KEY_ID", "AWS_ACCESS_KEY_ID");
      exportKey("AWS_SECRET_ACCESS_KEY", "AWS_SECRET_ACCESS_KEY");
      exportKey("AWS_SESSION_TOKEN", "AWS_SESSION_TOKEN");
      break;
    case "azure":
      exportKey("AZURE_STORAGE_ACCOUNT", "AZURE_STORAGE_ACCOUNT");
      exportKey("AZURE_STORAGE_KEY", "AZURE_STORAGE_KEY");
      exportKey("AZURE_STORAGE_SAS_TOKEN", "AZURE_STORAGE_SAS_TOKEN");
      break;
    case "gcs":
      // GCS service account JSON is mounted as a file under CREDS_VOLUME.
      env.push({
        name: "GOOGLE_APPLICATION_CREDENTIALS",
        value: `${CREDENTIALS_MOUNT_DIR}/service-account.json`,
      });
      break;
    case "sftp":
      // SFTP private key + optional passphrase live as files under CREDS_VOLUME.
      // Engine reads them directly via credentials_dir in the BackupSpec.
      break;
  }

  return env;
}

function buildJob(
  name: string,
  namespace: string,
  backup: ScutarBackup,
  podSpec: object,
  ownerRefs: ReturnType<typeof ownerReferences>,
) {
  return {
    apiVersion: "batch/v1",
    kind: "Job",
    metadata: {
      name,
      namespace,
      labels: { [OWNER_LABEL]: backup.metadata.name },
      ownerReferences: ownerRefs,
    },
    spec: {
      backoffLimit: backup.spec.jobTemplate?.backoffLimit ?? 0,
      activeDeadlineSeconds: backup.spec.jobTemplate?.activeDeadlineSeconds,
      ttlSecondsAfterFinished: backup.spec.jobTemplate?.ttlSecondsAfterFinished ?? 86_400,
      template: {
        metadata: {
          labels: { [OWNER_LABEL]: backup.metadata.name },
        },
        spec: podSpec,
      },
    },
  };
}

function buildCronJob(
  name: string,
  namespace: string,
  backup: ScutarBackup,
  podSpec: object,
  ownerRefs: ReturnType<typeof ownerReferences>,
) {
  return {
    apiVersion: "batch/v1",
    kind: "CronJob",
    metadata: {
      name: JOB_NAME(name),
      namespace,
      labels: { [OWNER_LABEL]: name },
      ownerReferences: ownerRefs,
    },
    spec: {
      schedule: backup.spec.schedule,
      suspend: backup.spec.suspend ?? false,
      concurrencyPolicy: "Forbid",
      successfulJobsHistoryLimit: 3,
      failedJobsHistoryLimit: 3,
      jobTemplate: {
        spec: {
          backoffLimit: backup.spec.jobTemplate?.backoffLimit ?? 0,
          activeDeadlineSeconds: backup.spec.jobTemplate?.activeDeadlineSeconds,
          ttlSecondsAfterFinished:
            backup.spec.jobTemplate?.ttlSecondsAfterFinished ?? 86_400,
          template: {
            metadata: {
              labels: { [OWNER_LABEL]: name },
            },
            spec: podSpec,
          },
        },
      },
    },
  };
}
