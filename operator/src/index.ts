// Operator entrypoint. Watches `ScutarBackup` resources cluster-wide (or in a
// single namespace if SCUTAR_WATCH_NAMESPACE is set) and reconciles each
// change by building a BackupSpec ConfigMap and a Job/CronJob.

import * as dotenv from "dotenv";
dotenv.config();

import { SCUTAR_GROUP, SCUTAR_PLURAL_BACKUPS, SCUTAR_VERSION } from "./types/crd";
import { WATCH_NAMESPACE } from "./consts";
import { watchClient, listBackups } from "./k8s/client";
import { reconcileBackup } from "./services/reconciler";
import type { ScutarBackup } from "./types/crd";

async function bootstrap(): Promise<void> {
  console.log(`[scutar] starting operator (watching ${WATCH_NAMESPACE || "all namespaces"})`);

  // Initial sync: reconcile every existing ScutarBackup once at startup so we
  // converge on the desired state even after a restart.
  try {
    const items = await listBackups(WATCH_NAMESPACE || undefined);
    console.log(`[scutar] initial sync: ${items.length} ScutarBackup resource(s)`);
    for (const item of items) {
      await reconcileBackup(item);
    }
  } catch (e) {
    console.error("[scutar] initial sync failed:", e);
    if (isCrdNotFound(e)) {
      console.error("[scutar] CRD scutarbackups.scutar.io is not installed in the cluster.");
      console.error("[scutar] Install with: helm install scutar-operator ./deployment/helm -n scutar --create-namespace");
      process.exit(1);
    }
  }

  startWatch();
}

function startWatch(): void {
  const path = WATCH_NAMESPACE
    ? `/apis/${SCUTAR_GROUP}/${SCUTAR_VERSION}/namespaces/${WATCH_NAMESPACE}/${SCUTAR_PLURAL_BACKUPS}`
    : `/apis/${SCUTAR_GROUP}/${SCUTAR_VERSION}/${SCUTAR_PLURAL_BACKUPS}`;

  watchClient
    .watch(
      path,
      {},
      (type: string, obj: ScutarBackup) => {
        if (type === "ADDED" || type === "MODIFIED") {
          void reconcileBackup(obj);
        } else if (type === "DELETED") {
          const ns = obj.metadata?.namespace || "scutar";
          const name = obj.metadata?.name || "?";
          // ownerReferences cascade-delete the ConfigMap/Job/CronJob; nothing
          // else to do here besides logging.
          console.log(`[scutar] ${ns}/${name}: deleted`);
        }
      },
      (err: unknown) => {
        if (err) {
          console.error("[scutar] watch error:", err);
        }
        // Reconnect after a short delay.
        setTimeout(() => startWatch(), 5_000);
      },
    )
    .catch((err: unknown) => {
      console.error("[scutar] failed to start watch:", err);
      setTimeout(() => startWatch(), 5_000);
    });
}

function isCrdNotFound(e: unknown): boolean {
  if (!e || typeof e !== "object") return false;
  const obj = e as { statusCode?: number; body?: string };
  return obj.statusCode === 404 || (typeof obj.body === "string" && obj.body.includes("404"));
}

process.on("unhandledRejection", (reason: unknown) => {
  console.error("[scutar] unhandled rejection:", reason);
});

bootstrap().catch((err) => {
  console.error("[scutar] fatal:", err);
  process.exit(1);
});
