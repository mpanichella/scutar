import { CoreV1Api, KubeConfig } from "@kubernetes/client-node";
import type { V1Secret } from "@kubernetes/client-node";

const kc = new KubeConfig();
kc.loadFromDefault();
const coreApi = kc.makeApiClient(CoreV1Api);

const defaultNamespace = process.env.SCUTAR_NAMESPACE || "scutar";

/** Label to identify Scutar credential Secrets. */
export const SCUTAR_CREDENTIALS_LABEL = "scutar.io/credentials";
export const SCUTAR_CREDENTIALS_LABEL_VALUE = "true";

/** Secret type: only Secrets with this type are listed in the credentials combo. */
export const SCUTAR_CREDENTIALS_SECRET_TYPE = "scutar.io/credentials";
export const SCUTAR_MANAGED_BY_LABEL = "scutar.io/managed-by";
export const SCUTAR_MANAGED_BY_VALUE = "scutar-ui";

export interface ScutarSecretMeta {
  name: string;
  namespace: string;
  /** Optional: type hint from label scutar.io/credential-type (s3, azure, gcs, sftp) */
  credentialType?: string;
}

export async function listCredentialSecrets(namespace?: string): Promise<ScutarSecretMeta[]> {
  const ns = namespace || defaultNamespace;
  const res = await coreApi.listNamespacedSecret(
    ns,
    undefined, // pretty
    undefined, // allowWatchBookmarks
    undefined, // continue
    `type=${SCUTAR_CREDENTIALS_SECRET_TYPE}`,
    undefined
  );
  const items = (res.body?.items || []) as V1Secret[];
  return items.map((s) => ({
    name: s.metadata?.name ?? "",
    namespace: s.metadata?.namespace ?? ns,
    credentialType: s.metadata?.labels?.["scutar.io/credential-type"],
  }));
}

export async function getCredentialSecret(name: string, namespace?: string): Promise<{ config: string } | null> {
  const ns = namespace || defaultNamespace;
  try {
    const res = await coreApi.readNamespacedSecret(name, ns);
    const secret = res.body as V1Secret;
    const b64 = secret.data?.config;
    if (!b64) return null;
    const config = Buffer.from(b64, "base64").toString("utf-8");
    return { config };
  } catch (e: unknown) {
    if (e && typeof e === "object" && "statusCode" in e && (e as { statusCode: number }).statusCode === 404) {
      return null;
    }
    throw e;
  }
}

export async function createCredentialSecret(
  name: string,
  config: string,
  namespace?: string,
  credentialType?: string
): Promise<V1Secret> {
  const ns = namespace || defaultNamespace;
  const labels: Record<string, string> = {
    [SCUTAR_CREDENTIALS_LABEL]: SCUTAR_CREDENTIALS_LABEL_VALUE,
    [SCUTAR_MANAGED_BY_LABEL]: SCUTAR_MANAGED_BY_VALUE,
  };
  if (credentialType) labels["scutar.io/credential-type"] = credentialType;

  const body: V1Secret = {
    apiVersion: "v1",
    kind: "Secret",
    metadata: {
      name,
      namespace: ns,
      labels,
    },
    type: SCUTAR_CREDENTIALS_SECRET_TYPE,
    data: {
      config: Buffer.from(config, "utf-8").toString("base64"),
    },
  };

  const res = await coreApi.createNamespacedSecret(ns, body);
  return res.body as V1Secret;
}

export async function updateCredentialSecret(
  name: string,
  config: string,
  namespace?: string
): Promise<V1Secret> {
  const ns = namespace || defaultNamespace;
  const existing = await coreApi.readNamespacedSecret(name, ns);
  const secret = existing.body as V1Secret;

  const body: V1Secret = {
    ...secret,
    data: {
      ...secret.data,
      config: Buffer.from(config, "utf-8").toString("base64"),
    },
  };

  const res = await coreApi.replaceNamespacedSecret(name, ns, body);
  return res.body as V1Secret;
}

export async function deleteCredentialSecret(name: string, namespace?: string): Promise<void> {
  const ns = namespace || defaultNamespace;
  await coreApi.deleteNamespacedSecret(name, ns);
}
