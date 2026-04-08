# Scutar

**Scutar** es una plataforma de backup y sincronización para Kubernetes,
declarativa vía CRDs (estilo KEDA), con un **engine propio escrito en Rust**
que reemplaza la dependencia de herramientas externas como restic o rclone.

> 📖 El **contexto, las decisiones de arquitectura y el roadmap** viven en
> [`Agent.md`](Agent.md). Este `README.md` documenta **cómo está organizado**
> el repo y **cómo correrlo / probarlo**.

---

## ¿Qué hace?

Dos modos de operación, configurables por CRD:

| Modo | Para qué sirve | Características |
|---|---|---|
| **`snapshot`** | Backup tradicional con historia | Deduplicación content-addressable (FastCDC + BLAKE3), encriptación opcional AES-256-GCM, políticas de retención, restorable vía `ScutarRestoreRequest`. |
| **`mirror`** | Replicación 1:1 contra una nube | Sin historia, sin dedup, equivalente a `rclone sync`. Útil para mantener un mirror exacto. |

Backends de almacenamiento soportados:

- **Amazon S3** (y compatibles: MinIO, Wasabi, DigitalOcean Spaces, etc.)
- **Azure Blob Storage**
- **Google Cloud Storage**
- **SFTP**

Sumar una nube nueva = implementar un trait, registrar en el factory.
Ver [`docs/adding-a-backend.md`](docs/adding-a-backend.md) (a escribir).

---

## Estructura del repositorio

```
scutar/
├── Agent.md                  # contexto + decisiones de arquitectura (LEER PRIMERO)
├── README.md                 # este archivo
├── package.json              # workspaces npm (operator + server)
│
├── operator/                 # Operador K8s en Node.js + TypeScript
│   ├── src/                  # observa CRDs, valida, crea Jobs/CronJobs
│   ├── examples/             # YAMLs de ejemplo de los CRDs
│   └── deployment/helm/      # Helm chart
│
├── engine/                   # 🦀 Engine en Rust (workspace)
│   ├── Cargo.toml
│   ├── Dockerfile            # multi-stage, multi-arch (amd64/arm64)
│   └── crates/
│       ├── scutar-core/      # tipos, errores, trait StorageBackend, BackupSpec
│       ├── scutar-backends/  # impls por nube (s3, azure, gcs, sftp)
│       ├── scutar-engine/    # snapshot, mirror, restore
│       └── scutar-cli/       # binario `scutar-runner`
│
├── server/                   # API REST + UI (Express + Vue 3)
│   ├── backend/
│   └── ui/
│
├── examples/                 # 📋 manifests YAML listos para `kubectl apply`
│   ├── walkthrough/          # paso a paso end-to-end (orden numérico)
│   ├── connections/          # un template por backend (S3, Azure, GCS, SFTP, Local)
│   ├── backups/              # un template por escenario (snapshot, mirror, ...)
│   └── restore/              # restauración de snapshots
│
└── docs/
    ├── architecture.md       # detalles arquitectónicos del engine
    └── adding-a-backend.md   # cómo sumar una nube nueva
```

---

## Requisitos

- **Rust** 1.78+ (para compilar el engine)
- **Node.js** 20+ (para operator + server)
- **Docker** con `buildx` (para imágenes multi-arch)
- **Kubernetes** 1.21+ (para deployar el operator)
- **Helm** 3 (para el chart del operator)

---

## Cómo correr / probar

### Engine (Rust)

```bash
cd engine

# Compilar el workspace completo
cargo build

# Ejecutar tests (incluye tests de integración con MinIO/Azurite/etc — Fase 2)
cargo test

# Compilar release del binario del runner
cargo build --release --bin scutar-runner

# Buildear la imagen Docker multi-arch
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t scutar/scutar-runner:dev \
  --load .
```

El binario `scutar-runner` espera un `BackupSpec` YAML mounteado por el
operador. Para probarlo a mano:

```bash
cat > /tmp/spec.yaml <<'YAML'
name: test-backup
mode: snapshot
source:
  path: /tmp/source-data
destination:
  type: s3
  bucket: my-test-bucket
  region: us-east-1
  prefix: backups/test/
encryption:
  enabled: false
  password_file: ""
labels: {}
YAML

./engine/target/release/scutar-runner --spec /tmp/spec.yaml
```

### Operator (Node.js)

```bash
cd operator
npm install
npm run build
npm start                                # corre en foreground, observa CRDs

# Deploy en cluster
helm install scutar-operator ./deployment/helm -n scutar --create-namespace
```

### Server (API + UI)

```bash
# Desde la raíz del repo
npm install
npm run start:server       # build de UI + backend, todo en :3000
```

| Comando | Qué hace |
|---|---|
| `npm run start:operator` | Levanta sólo el operator |
| `npm run start:api`      | Levanta sólo el backend (API en `:3000`) |
| `npm run dev:ui`         | UI con hot-reload (`:5173`, proxy `/api → :3000`) |
| `npm run start:server`   | Build de UI + backend, todo servido en `:3000` |

---

## Ejemplos completos

Mirá [`examples/`](examples/) — tiene templates YAML productivos para cada
backend, cada modo, y un walkthrough end-to-end paso-a-paso para tener tu
primer backup corriendo en el cluster sin tener que escribir nada desde cero.
La sección de abajo es solo un resumen rápido.

## Ejemplo rápido

### 1. Crear una conexión

```yaml
apiVersion: scutar.io/v1alpha1
kind: ScutarConnection
metadata:
  name: prod-s3
  namespace: scutar
spec:
  type: s3
  s3:
    bucket: my-backups
    region: us-east-1
  credentialsSecretRef:
    name: prod-s3-creds
```

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: prod-s3-creds
  namespace: scutar
stringData:
  AWS_ACCESS_KEY_ID: AKIA...
  AWS_SECRET_ACCESS_KEY: ...
```

### 2. Definir un backup con historia

```yaml
apiVersion: scutar.io/v1alpha1
kind: ScutarBackup
metadata:
  name: postgres-daily
  namespace: scutar
spec:
  mode: snapshot
  schedule: "0 2 * * *"
  source:
    pvcName: postgres-data
    path: /data
    exclude: ["*.tmp", "*.log"]
  destinationConnectionRef:
    name: prod-s3
  destinationPath: postgres/
  encryption:
    enabled: true
    passwordSecretRef:
      name: postgres-backup-password
      key: password
  retention:
    keepDaily: 7
    keepWeekly: 4
    keepMonthly: 12
```

### 3. O un mirror sin historia

```yaml
apiVersion: scutar.io/v1alpha1
kind: ScutarBackup
metadata:
  name: assets-mirror
  namespace: scutar
spec:
  mode: mirror
  schedule: "*/15 * * * *"
  source:
    pvcName: cdn-assets
    path: /data
  destinationConnectionRef:
    name: prod-s3
  destinationPath: assets-mirror/
```

### 4. Restaurar un snapshot

```yaml
apiVersion: scutar.io/v1alpha1
kind: ScutarRestoreRequest
metadata:
  name: restore-postgres
  namespace: scutar
spec:
  snapshotRef:
    name: postgres-daily-20260408-020001
  targetPvcName: postgres-data-restore
  targetPath: /data
```

---

## Estado del proyecto

🚧 **Reescritura en curso.** Ver el roadmap detallado en
[`Agent.md` §6](Agent.md#6-roadmap). Estado actual:

- ✅ **Fase 0** — reestructuración del repo, esqueleto del workspace Rust,
  Dockerfile multi-arch, contratos definidos en `scutar-core`.
- 🚧 **Fase 1** — operador genera `BackupSpec` desde los CRDs nuevos.
- ⏳ **Fase 2** — implementación real de los 4 backends.
- ⏳ **Fase 3** — engine: `mirror` y `snapshot` end-to-end.
- ⏳ **Fase 4** — restore.
- ⏳ **Fase 5** — encriptación.

El runner Node.js anterior y los fuentes vendoreados de rclone/restic fueron
**eliminados**. Ver `Agent.md §2` para los motivos.

---

## Licencia

Apache-2.0
