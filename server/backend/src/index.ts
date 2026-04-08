import "dotenv/config";
import fs from "fs";
import path from "path";
import cors from "cors";
import express from "express";
import {
  listBackups,
  getBackup,
  createBackup,
  updateBackup,
  deleteBackup,
  runBackupNow,
  listConnections,
  getConnection,
  createConnection,
  updateConnection,
  deleteConnection,
} from "./k8s";
import {
  listCredentialSecrets,
  getCredentialSecret,
  createCredentialSecret,
  updateCredentialSecret,
  deleteCredentialSecret,
} from "./secrets";
import type { ScutarBackupSpec, ScutarConnectionSpec } from "./scutar-crd";

/** Extract a readable message from Kubernetes/client errors for API responses. */
function errorMessage(e: unknown): string {
  if (e && typeof e === "object") {
    const err = e as Record<string, unknown>;
    const body = (err.body ?? (err as { response?: { body?: unknown } }).response?.body) as
      | { message?: string; reason?: string }
      | string
      | undefined;
    const statusCode = err.statusCode as number | undefined;
    const msg = typeof err.message === "string" ? err.message : "";

    if (body && typeof body === "object" && body.message) return body.message;
    if (body && typeof body === "object" && body.reason) return body.reason;
    if (body && typeof body === "string") return body;
    if (statusCode === 404)
      return "Resource or CRD not found. Is the Scutar operator (CRD scutarbackups.scutar.io) installed?";
    if (statusCode === 403) return "Forbidden: check cluster RBAC and kubeconfig.";

    if (msg === "HTTP request failed" || msg.includes("HttpError")) {
      const cause = (e as Error & { cause?: Error }).cause;
      const causeStr = cause instanceof Error ? cause.message : cause ? String(cause) : "";
      const hint =
        causeStr || statusCode
          ? ` (${[causeStr, statusCode ? `HTTP ${statusCode}` : ""].filter(Boolean).join("; ")})`
          : ". Check KUBECONFIG and that the cluster is reachable (e.g. kubectl cluster-info).";
      return msg + hint;
    }
    if (err instanceof Error) return err.message;
  }
  if (e instanceof Error) return e.message;
  return String(e);
}

const app = express();
app.use(cors());
app.use(express.json());

const PORT = Number(process.env.PORT) || 3000;

app.get("/api/backups", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    const items = await listBackups(namespace);
    res.json({ items });
  } catch (e) {
    console.error("listBackups", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.get("/api/backups/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    const backup = await getBackup(req.params.name, namespace);
    if (!backup) return res.status(404).json({ error: "Not found" });
    res.json(backup);
  } catch (e) {
    console.error("getBackup", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

function validateBackupSpec(spec: ScutarBackupSpec): string | null {
  if (!spec) return "Missing spec";
  if (spec.mode !== "snapshot" && spec.mode !== "mirror") {
    return "spec.mode must be 'snapshot' or 'mirror'";
  }
  if (!spec.source?.pvcName) return "spec.source.pvcName is required";
  if (!spec.destinationConnectionRef?.name) return "spec.destinationConnectionRef.name is required";
  if (spec.encryption?.enabled) {
    if (spec.mode !== "snapshot") return "encryption is only supported in mode 'snapshot'";
    if (!spec.encryption.passwordSecretRef?.name) {
      return "encryption.enabled requires encryption.passwordSecretRef.name";
    }
  }
  if (spec.retention && spec.mode !== "snapshot") {
    return "retention is only honored in mode 'snapshot'";
  }
  return null;
}

app.post("/api/backups", async (req, res) => {
  try {
    const { name, namespace, spec } = req.body as {
      name: string;
      namespace?: string;
      spec: ScutarBackupSpec;
    };
    if (!name || !spec) {
      return res.status(400).json({ error: "Missing required fields: name, spec" });
    }
    const err = validateBackupSpec(spec);
    if (err) return res.status(400).json({ error: err });
    const created = await createBackup(name, spec, namespace);
    res.status(201).json(created);
  } catch (e) {
    console.error("createBackup", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.put("/api/backups/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || req.body?.namespace;
    const spec = req.body?.spec as ScutarBackupSpec;
    if (!spec) return res.status(400).json({ error: "Missing spec" });
    const err = validateBackupSpec(spec);
    if (err) return res.status(400).json({ error: err });
    const updated = await updateBackup(req.params.name, spec, namespace);
    res.json(updated);
  } catch (e) {
    console.error("updateBackup", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.delete("/api/backups/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    await deleteBackup(req.params.name, namespace);
    res.status(204).send();
  } catch (e) {
    console.error("deleteBackup", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.post("/api/backups/:name/run", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || req.body?.namespace || undefined;
    const updated = await runBackupNow(req.params.name, namespace);
    res.json(updated);
  } catch (e) {
    console.error("runBackupNow", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

// --- Credential Secrets (label scutar.io/credentials=true) ---

app.get("/api/secrets", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    const items = await listCredentialSecrets(namespace);
    res.json({ items });
  } catch (e) {
    console.error("listCredentialSecrets", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.get("/api/secrets/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    const secret = await getCredentialSecret(req.params.name, namespace);
    if (!secret) return res.status(404).json({ error: "Not found" });
    res.json(secret);
  } catch (e) {
    console.error("getCredentialSecret", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.post("/api/secrets", async (req, res) => {
  try {
    const { name, namespace, config, credentialType } = req.body as {
      name: string;
      namespace?: string;
      config: string;
      credentialType?: string;
    };
    if (!name || config === undefined) {
      return res.status(400).json({ error: "Missing required fields: name, config" });
    }
    const created = await createCredentialSecret(name, config, namespace, credentialType);
    res.status(201).json({ name: created.metadata?.name, namespace: created.metadata?.namespace });
  } catch (e) {
    console.error("createCredentialSecret", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.put("/api/secrets/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || req.body?.namespace;
    const { config } = req.body as { config: string };
    if (config === undefined) return res.status(400).json({ error: "Missing: config" });
    await updateCredentialSecret(req.params.name, config, namespace);
    res.json({ name: req.params.name, namespace: namespace || "scutar" });
  } catch (e) {
    console.error("updateCredentialSecret", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.delete("/api/secrets/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    await deleteCredentialSecret(req.params.name, namespace);
    res.status(204).send();
  } catch (e) {
    console.error("deleteCredentialSecret", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

// --- ScutarConnection CRD ---

app.get("/api/connections", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    const items = await listConnections(namespace);
    res.json({ items });
  } catch (e) {
    console.error("listConnections", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.get("/api/connections/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    const conn = await getConnection(req.params.name, namespace);
    if (!conn) return res.status(404).json({ error: "Not found" });
    res.json(conn);
  } catch (e) {
    console.error("getConnection", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.post("/api/connections", async (req, res) => {
  try {
    const { name, namespace, spec } = req.body as {
      name: string;
      namespace?: string;
      spec: ScutarConnectionSpec;
    };
    if (!name || !spec?.type) {
      return res.status(400).json({ error: "Missing required fields: name, spec.type" });
    }
    const created = await createConnection(name, spec, namespace);
    res.status(201).json(created);
  } catch (e) {
    console.error("createConnection", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.put("/api/connections/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || req.body?.namespace;
    const spec = req.body?.spec as ScutarConnectionSpec;
    if (!spec?.type) return res.status(400).json({ error: "Missing spec.type" });
    const updated = await updateConnection(req.params.name, spec, namespace);
    res.json(updated);
  } catch (e) {
    console.error("updateConnection", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

app.delete("/api/connections/:name", async (req, res) => {
  try {
    const namespace = (req.query.namespace as string) || undefined;
    await deleteConnection(req.params.name, namespace);
    res.status(204).send();
  } catch (e) {
    console.error("deleteConnection", e);
    res.status(500).json({ error: errorMessage(e) });
  }
});

// UI compilada en backend/public; el mismo servidor sirve API y sitio estático
const publicDir = path.join(__dirname, "..", "public");
if (fs.existsSync(publicDir)) {
  app.use(express.static(publicDir));
  app.get("*", (req, res, next) => {
    if (req.path.startsWith("/api")) return next();
    res.sendFile(path.join(publicDir, "index.html"));
  });
}

app.listen(PORT, () => {
  console.log(`Scutar server (API + UI) on http://localhost:${PORT}`);
});
