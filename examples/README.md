# Manifests de ejemplo para Scutar

Templates listos para copiar/pegar/editar y aplicar al cluster con `kubectl`.
Cada archivo es un YAML autocontenido — leelo, reemplazá los placeholders
`<...>`, y aplicalo.

> **Antes de empezar**: el operador Scutar tiene que estar instalado en el
> cluster (`helm install scutar-operator ./operator/deployment/helm
> -n scutar --create-namespace`). Eso instala los CRDs (`ScutarConnection`,
> `ScutarBackup`, `ScutarSnapshot`, `ScutarRestoreRequest`) y el deployment
> del operador. Sin operador, los `kubectl apply` van a fallar con
> "no matches for kind".
>
> **Convención de namespace**: todo Scutar (operator, CRDs, conexiones,
> backups, secrets) vive por default en el namespace `scutar`. Si querés
> usar otro, reemplazá `namespace: scutar` en cada manifest y los
> `kubectl -n scutar` por tu namespace.

## Estructura

```
examples/
├── README.md                  ← este archivo
├── walkthrough/               ← end-to-end paso a paso, en orden
│   ├── 01-namespace.yaml
│   ├── 02-secret-s3-creds.yaml
│   ├── 03-connection-s3.yaml
│   ├── 04-pvc-source.yaml
│   ├── 05-encryption-password.yaml
│   ├── 06-backup-snapshot.yaml
│   └── 07-restore.yaml
├── connections/               ← un template por backend
│   ├── s3-aws.yaml
│   ├── s3-minio.yaml
│   ├── azure-blob.yaml
│   ├── gcs.yaml
│   ├── sftp.yaml
│   └── local.yaml
├── backups/                   ← un template por escenario
│   ├── snapshot-daily-encrypted.yaml
│   ├── snapshot-oneoff.yaml
│   ├── snapshot-suspended.yaml
│   └── mirror-15min.yaml
└── restore/
    └── restore-snapshot.yaml
```

## Camino corto: walkthrough end-to-end

Si nunca usaste Scutar, andá directo al [`walkthrough/`](walkthrough/) y
aplicá los archivos en orden numérico:

```bash
cd examples/walkthrough

# 1. namespace
kubectl apply -f 01-namespace.yaml

# 2. EDITAR primero (poner credenciales reales) y aplicar
$EDITOR 02-secret-s3-creds.yaml
kubectl apply -f 02-secret-s3-creds.yaml

# 3. EDITAR (bucket + region) y aplicar
$EDITOR 03-connection-s3.yaml
kubectl apply -f 03-connection-s3.yaml

# 4. PVC origen + Job que lo siembra con datos de prueba
kubectl apply -f 04-pvc-source.yaml
kubectl -n scutar wait --for=condition=complete job/seed-app-data --timeout=60s

# 5. EDITAR (poner un password real) y aplicar
$EDITOR 05-encryption-password.yaml
kubectl apply -f 05-encryption-password.yaml

# 6. ScutarBackup → el operador genera ConfigMap+Job y dispara el engine
kubectl apply -f 06-backup-snapshot.yaml

# observar
kubectl -n scutar get scutarbackups
kubectl -n scutar get jobs -l scutar.io/owned-by=walkthrough-snapshot
kubectl -n scutar logs -l scutar.io/owned-by=walkthrough-snapshot --tail=200 -f

# 7. cuando termine, listar snapshots y restaurar (opcional)
kubectl -n scutar get scutarsnapshots
$EDITOR 07-restore.yaml      # poner el nombre del snapshot
kubectl apply -f 07-restore.yaml
```

## Camino largo: editar templates por nube

Cada subdirectorio (`connections/`, `backups/`, `restore/`) tiene templates
independientes. Workflow típico:

1. **Elegí un backend** y copiá el template correspondiente de
   `connections/`. Reemplazá los `<...>` con valores reales y aplicalo.

   ```bash
   cp examples/connections/azure-blob.yaml /tmp/my-azure.yaml
   $EDITOR /tmp/my-azure.yaml
   kubectl apply -f /tmp/my-azure.yaml
   ```

2. **Elegí un patrón de backup** de `backups/` y editalo:
   - `snapshot-daily-encrypted.yaml` — el caso "production-grade".
   - `snapshot-oneoff.yaml` — para snapshots manuales puntuales.
   - `mirror-15min.yaml` — réplica 1:1 sin historia (estilo `rclone sync`).
   - `snapshot-suspended.yaml` — para tener la definición pero pausada.

   Ajustá `source.pvcName`, `destinationConnectionRef.name` y
   `destinationPath`. Aplicar.

3. **Restore** cuando necesites: copiar
   `restore/restore-snapshot.yaml`, reemplazar el `snapshotRef.name` por uno
   real (sacalo de `kubectl get scutarsnapshots`), aplicar.

## Estados que vas a ver en `kubectl get scutarbackups`

| `STATUS` | Significado |
|---|---|
| `Pending` | el operador todavía no procesó el CR (transitorio, dura segundos) |
| `Ready` | CronJob aplicado, schedule activo |
| `Running` | one-off Job aplicado, en curso |
| `Invalid` | el CR no pasó validación; mirá `status.message` con `kubectl describe scutarbackup <name>` |
| `Failed` | el operador falló al crear los recursos en el cluster (no es lo mismo que un Job que falla) |

## Cosas que NO van en estos templates

- **Imágenes**: el operador usa la imagen configurada en el Helm chart
  (`engine.image.repository:tag`). Si querés overridearla por backup,
  todavía no hay un campo en el CR para eso — agregalo en `values.yaml`
  del operador y reinstalá.
- **ServiceAccount del engine**: viene del Helm chart
  (`scutar-runner` por default). El RBAC ya está cubierto allí.
- **NetworkPolicy / PSP / PodSecurityStandards**: depende del cluster.
  Los Pods del engine no necesitan privilegios especiales — corren como
  UID 65532 non-root y solo necesitan red saliente al backend.

## Variables de entorno que el operador wirea automáticamente

Cuando el operador crea el Pod del engine, exporta del Secret de
credenciales las env vars que cada SDK conoce por convención:

| Backend | Env vars exportadas | De qué key del Secret |
|---|---|---|
| s3 | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN` (opcional) | mismas keys del Secret |
| azure | `AZURE_STORAGE_ACCOUNT`, `AZURE_STORAGE_KEY`, `AZURE_STORAGE_SAS_TOKEN` (opcional) | mismas keys del Secret |
| gcs | `GOOGLE_APPLICATION_CREDENTIALS=/etc/scutar/creds/service-account.json` | el archivo `service-account.json` debe estar en el Secret |
| sftp | (ninguna — el engine lee la llave/password de `/etc/scutar/creds/`) | `id_ed25519` / `id_rsa` / `password` |
| local | (no usa credenciales) | — |

Por eso los Secrets de los templates usan exactamente esos nombres de keys.

## Para más contexto

- [Agent.md](../Agent.md) — arquitectura y decisiones de diseño.
- [docs/architecture.md](../docs/architecture.md) — detalle del engine
  (layout del repo, pipeline snapshot/mirror, encriptación).
- [docs/adding-a-backend.md](../docs/adding-a-backend.md) — cómo sumar una
  nube nueva.
