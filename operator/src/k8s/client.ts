// Thin wrapper around @kubernetes/client-node. Provides typed accessors for
// the four Scutar CRDs and helpers to apply ConfigMaps/Jobs/CronJobs.

import {
  BatchV1Api,
  CoreV1Api,
  CustomObjectsApi,
  KubeConfig,
  Watch,
} from "@kubernetes/client-node";
import {
  SCUTAR_GROUP,
  SCUTAR_PLURAL_BACKUPS,
  SCUTAR_PLURAL_CONNECTIONS,
  SCUTAR_PLURAL_RESTORES,
  SCUTAR_PLURAL_SNAPSHOTS,
  SCUTAR_VERSION,
  type ScutarBackup,
  type ScutarConnection,
  type ScutarRestoreRequest,
  type ScutarSnapshot,
} from "../types/crd";

const kc = new KubeConfig();
kc.loadFromDefault();

export const customApi = kc.makeApiClient(CustomObjectsApi);
export const batchApi = kc.makeApiClient(BatchV1Api);
export const coreApi = kc.makeApiClient(CoreV1Api);
export const watchClient = new Watch(kc);

// ---- ScutarBackup -----------------------------------------------------------

export async function getBackup(
  name: string,
  namespace: string,
): Promise<ScutarBackup | null> {
  try {
    const res = await customApi.getNamespacedCustomObject(
      SCUTAR_GROUP,
      SCUTAR_VERSION,
      namespace,
      SCUTAR_PLURAL_BACKUPS,
      name,
    );
    return res.body as ScutarBackup;
  } catch (e) {
    if (isNotFound(e)) return null;
    throw e;
  }
}

export async function listBackups(namespace?: string): Promise<ScutarBackup[]> {
  if (namespace) {
    const res = await customApi.listNamespacedCustomObject(
      SCUTAR_GROUP,
      SCUTAR_VERSION,
      namespace,
      SCUTAR_PLURAL_BACKUPS,
    );
    return ((res.body as { items?: ScutarBackup[] }).items ?? []) as ScutarBackup[];
  }
  const res = await customApi.listClusterCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    SCUTAR_PLURAL_BACKUPS,
  );
  return ((res.body as { items?: ScutarBackup[] }).items ?? []) as ScutarBackup[];
}

export async function patchBackupStatus(
  cr: ScutarBackup,
  status: ScutarBackup["status"],
): Promise<void> {
  const ns = cr.metadata.namespace || "scutar";
  const patch = [{ op: "replace", path: "/status", value: status }];
  await customApi.patchNamespacedCustomObjectStatus(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL_BACKUPS,
    cr.metadata.name,
    patch,
    undefined,
    undefined,
    undefined,
    { headers: { "content-type": "application/json-patch+json" } },
  );
}

export async function removeRunNowAnnotation(cr: ScutarBackup): Promise<void> {
  const ns = cr.metadata.namespace || "scutar";
  const patch = [
    {
      op: "remove",
      path: "/metadata/annotations/scutar.io~1run-now",
    },
  ];
  try {
    await customApi.patchNamespacedCustomObject(
      SCUTAR_GROUP,
      SCUTAR_VERSION,
      ns,
      SCUTAR_PLURAL_BACKUPS,
      cr.metadata.name,
      patch,
      undefined,
      undefined,
      undefined,
      { headers: { "content-type": "application/json-patch+json" } },
    );
  } catch {
    // annotation may already be gone — ignore
  }
}

// ---- ScutarConnection -------------------------------------------------------

export async function getConnection(
  name: string,
  namespace: string,
): Promise<ScutarConnection | null> {
  try {
    const res = await customApi.getNamespacedCustomObject(
      SCUTAR_GROUP,
      SCUTAR_VERSION,
      namespace,
      SCUTAR_PLURAL_CONNECTIONS,
      name,
    );
    return res.body as ScutarConnection;
  } catch (e) {
    if (isNotFound(e)) return null;
    throw e;
  }
}

// ---- ScutarSnapshot ---------------------------------------------------------

export async function createSnapshot(snap: ScutarSnapshot): Promise<void> {
  const ns = snap.metadata.namespace || "scutar";
  await customApi.createNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL_SNAPSHOTS,
    snap,
  );
}

// ---- ScutarRestoreRequest ---------------------------------------------------

export async function listRestoreRequests(
  namespace?: string,
): Promise<ScutarRestoreRequest[]> {
  if (namespace) {
    const res = await customApi.listNamespacedCustomObject(
      SCUTAR_GROUP,
      SCUTAR_VERSION,
      namespace,
      SCUTAR_PLURAL_RESTORES,
    );
    return ((res.body as { items?: ScutarRestoreRequest[] }).items ?? []) as ScutarRestoreRequest[];
  }
  const res = await customApi.listClusterCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    SCUTAR_PLURAL_RESTORES,
  );
  return ((res.body as { items?: ScutarRestoreRequest[] }).items ?? []) as ScutarRestoreRequest[];
}

// ---- helpers ----------------------------------------------------------------

export function isNotFound(e: unknown): boolean {
  if (!e || typeof e !== "object") return false;
  const code = (e as { statusCode?: number; response?: { statusCode?: number } }).statusCode
    ?? (e as { response?: { statusCode?: number } }).response?.statusCode;
  return code === 404;
}
