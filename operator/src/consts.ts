// Operator-wide constants. Anything that's tunable lives in env vars below.

/** Default image used for the engine Pod. */
export const SCUTAR_ENGINE_IMAGE =
  process.env.SCUTAR_ENGINE_IMAGE || "addlayer/scutar-runner:latest";

/** Optional pull policy override. */
export const SCUTAR_ENGINE_IMAGE_PULL_POLICY =
  process.env.SCUTAR_ENGINE_IMAGE_PULL_POLICY || "IfNotPresent";

/** Service account the engine Pod runs as (overridable per chart values). */
export const SCUTAR_ENGINE_SERVICE_ACCOUNT =
  process.env.SCUTAR_ENGINE_SERVICE_ACCOUNT || "default";

/** Namespace to watch when SCUTAR_WATCH_NAMESPACE is empty (watch all). */
export const WATCH_NAMESPACE = process.env.SCUTAR_WATCH_NAMESPACE || "";

/** Annotation set by API/UI to trigger an immediate run. */
export const RUN_NOW_ANNOTATION = "scutar.io/run-now";

/** Owner labels and annotations applied to managed Jobs/CronJobs/ConfigMaps. */
export const OWNER_LABEL = "scutar.io/owned-by";
export const CR_NAME_ANNOTATION = "scutar.io/cr-name";
export const CR_NAMESPACE_ANNOTATION = "scutar.io/cr-namespace";

/** Mount paths inside the engine Pod (must match engine expectations). */
export const SPEC_MOUNT_PATH = "/etc/scutar/spec.yaml";
export const SPEC_MOUNT_DIR = "/etc/scutar";
export const CREDENTIALS_MOUNT_DIR = "/etc/scutar/creds";
export const SOURCE_MOUNT_DIR = "/data";
export const PASSWORD_MOUNT_DIR = "/etc/scutar/password";

/** Volume names used in the Pod template. */
export const SPEC_VOLUME = "scutar-spec";
export const CREDS_VOLUME = "scutar-creds";
export const PASSWORD_VOLUME = "scutar-password";
export const SOURCE_VOLUME = "scutar-source";

/** ConfigMap name template for a backup run. */
export const SPEC_CONFIGMAP_NAME = (backupName: string) => `scutar-spec-${backupName}`;

/** Job/CronJob name template for a backup. */
export const JOB_NAME = (backupName: string) => `scutar-${backupName}`;

/** Snapshot name template emitted by the engine when a snapshot run completes. */
export const SNAPSHOT_NAME = (backupName: string, isoTimestamp: string) =>
  `${backupName}-${isoTimestamp.replace(/[:.]/g, "-")}`;
