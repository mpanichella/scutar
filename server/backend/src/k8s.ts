import { CustomObjectsApi, KubeConfig } from "@kubernetes/client-node";
import {
  SCUTAR_GROUP,
  SCUTAR_PLURAL,
  SCUTAR_PLURAL_CONNECTIONS,
  SCUTAR_VERSION,
  type ScutarBackupBody,
  type ScutarBackupSpec,
  type ScutarConnectionBody,
  type ScutarConnectionSpec,
} from "./scutar-crd";

const kc = new KubeConfig();
kc.loadFromDefault();
const customApi = kc.makeApiClient(CustomObjectsApi);

const defaultNamespace = process.env.SCUTAR_NAMESPACE || "scutar";

export async function listBackups(namespace?: string): Promise<ScutarBackupBody[]> {
  const ns = namespace || defaultNamespace;
  const res = await customApi.listNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL
  );
  const body = res.body as { items?: ScutarBackupBody[] };
  return body.items || [];
}

export async function getBackup(name: string, namespace?: string): Promise<ScutarBackupBody | null> {
  const ns = namespace || defaultNamespace;
  try {
    const res = await customApi.getNamespacedCustomObject(
      SCUTAR_GROUP,
      SCUTAR_VERSION,
      ns,
      SCUTAR_PLURAL,
      name
    );
    return res.body as ScutarBackupBody;
  } catch (e: unknown) {
    if (e && typeof e === "object" && "statusCode" in e && (e as { statusCode: number }).statusCode === 404) {
      return null;
    }
    throw e;
  }
}

export async function createBackup(
  name: string,
  spec: ScutarBackupSpec,
  namespace?: string
): Promise<ScutarBackupBody> {
  const ns = namespace || defaultNamespace;
  const body: ScutarBackupBody = {
    apiVersion: `${SCUTAR_GROUP}/${SCUTAR_VERSION}`,
    kind: "ScutarBackup",
    metadata: { name, namespace: ns },
    spec,
  };
  const res = await customApi.createNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL,
    body
  );
  return res.body as ScutarBackupBody;
}

export async function updateBackup(
  name: string,
  spec: ScutarBackupSpec,
  namespace?: string
): Promise<ScutarBackupBody> {
  const ns = namespace || defaultNamespace;
  const existing = await getBackup(name, ns);
  if (!existing) throw new Error(`Backup ${name} not found`);
  const body: ScutarBackupBody = {
    ...existing,
    spec,
  };
  const res = await customApi.replaceNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL,
    name,
    body
  );
  return res.body as ScutarBackupBody;
}

const RUN_NOW_ANNOTATION = "scutar.io/run-now";

/** Add run-now annotation so the operator creates a one-off Job. */
export async function runBackupNow(name: string, namespace?: string): Promise<ScutarBackupBody> {
  const ns = namespace || defaultNamespace;
  const existing = await getBackup(name, ns);
  if (!existing) throw new Error(`Backup ${name} not found`);
  const annotations = { ...(existing.metadata?.annotations ?? {}), [RUN_NOW_ANNOTATION]: String(Date.now()) };
  const body: ScutarBackupBody = {
    ...existing,
    metadata: { ...existing.metadata, name, namespace: ns, annotations },
  };
  const res = await customApi.replaceNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL,
    name,
    body
  );
  return res.body as ScutarBackupBody;
}

export async function deleteBackup(name: string, namespace?: string): Promise<void> {
  const ns = namespace || defaultNamespace;
  await customApi.deleteNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL,
    name
  );
}

// --- ScutarConnection CRD ---

export async function listConnections(namespace?: string): Promise<ScutarConnectionBody[]> {
  const ns = namespace || defaultNamespace;
  const res = await customApi.listNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL_CONNECTIONS
  );
  const body = res.body as { items?: ScutarConnectionBody[] };
  return body.items || [];
}

export async function getConnection(
  name: string,
  namespace?: string
): Promise<ScutarConnectionBody | null> {
  const ns = namespace || defaultNamespace;
  try {
    const res = await customApi.getNamespacedCustomObject(
      SCUTAR_GROUP,
      SCUTAR_VERSION,
      ns,
      SCUTAR_PLURAL_CONNECTIONS,
      name
    );
    return res.body as ScutarConnectionBody;
  } catch (e: unknown) {
    if (
      e &&
      typeof e === "object" &&
      "statusCode" in e &&
      (e as { statusCode: number }).statusCode === 404
    ) {
      return null;
    }
    throw e;
  }
}

export async function createConnection(
  name: string,
  spec: ScutarConnectionSpec,
  namespace?: string
): Promise<ScutarConnectionBody> {
  const ns = namespace || defaultNamespace;
  const body: ScutarConnectionBody = {
    apiVersion: `${SCUTAR_GROUP}/${SCUTAR_VERSION}`,
    kind: "ScutarConnection",
    metadata: { name, namespace: ns },
    spec,
  };
  const res = await customApi.createNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL_CONNECTIONS,
    body
  );
  return res.body as ScutarConnectionBody;
}

export async function updateConnection(
  name: string,
  spec: ScutarConnectionSpec,
  namespace?: string
): Promise<ScutarConnectionBody> {
  const ns = namespace || defaultNamespace;
  const existing = await getConnection(name, ns);
  if (!existing) throw new Error(`Connection ${name} not found`);
  const body: ScutarConnectionBody = {
    ...existing,
    spec,
  };
  const res = await customApi.replaceNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL_CONNECTIONS,
    name,
    body
  );
  return res.body as ScutarConnectionBody;
}

export async function deleteConnection(name: string, namespace?: string): Promise<void> {
  const ns = namespace || defaultNamespace;
  await customApi.deleteNamespacedCustomObject(
    SCUTAR_GROUP,
    SCUTAR_VERSION,
    ns,
    SCUTAR_PLURAL_CONNECTIONS,
    name
  );
}
