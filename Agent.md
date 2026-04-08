# Agent.md — Contexto del proyecto Scutar

> Documento **vivo**. Cada vez que se toma una decisión de arquitectura o
> producto, se actualiza este archivo. Es la fuente de verdad de **por qué**
> el proyecto está como está. El `README.md` documenta el **qué** y el **cómo**.

---

## 1. Visión

**Scutar** es una plataforma de backup, sincronización y restore para
Kubernetes, declarativa vía CRDs (estilo KEDA), con un engine propio escrito
en Rust que reemplaza la necesidad de depender de herramientas externas como
restic o rclone.

Tres pilares:

| Pilar | Descripción |
|---|---|
| **Backups con historia** (modo `snapshot`) | Snapshots deduplicados, content-addressable, opcionalmente encriptados, con políticas de retención. Restorables. |
| **Mirror / Sync** (modo `mirror`) | Réplica 1:1 de un volumen contra un blob storage. Sin historia, sin dedup. Útil para replicación. |
| **Multi-cloud** | Azure Blob, Amazon S3 (y compatibles), Google Cloud Storage, SFTP. Arquitectura abierta para sumar nuevas nubes sin tocar el core. |

---

## 2. Estado anterior y por qué se reescribe

La versión inicial (pre-2026-04-08) usaba un **runner Node.js** que hacía
`spawn` de los binarios `rclone` y `restic` como procesos hijos. Funcionaba
para casos básicos pero acumuló los siguientes problemas reales:

1. **Dos modelos mentales distintos** — rclone usa archivos de configuración
   (`rclone.conf`), restic usa variables de entorno. El runner tenía que
   inferir uno desde el otro (parsing de `rclone.conf` para deducir
   `RESTIC_REPOSITORY`), lógica frágil y propensa a fallar por mismatch de
   nombres entre el `ScutarConnection` y la sección de `rclone.conf`.

2. **Procesos hijos** — si un proceso hijo fallaba, el origen del error era
   difícil de rastrear: stack traces ajenos, formatos de log distintos,
   `maxBuffer` de 1 MiB de Node `spawnSync` matando `restic backup --json`
   en transferencias grandes (los datos llegaban a S3 pero el Job marcaba
   failure), exit codes mezclados, etc.

3. **Configuración fragmentada** — cada cliente tenía que entender ambas
   herramientas para configurar bien sus backups. Demasiada superficie.

4. **Multi-arch frágil** — binarios precompilados de rclone/restic bajados en
   build time, errores silenciosos de "exec format error" cuando la imagen no
   estaba buildeada para la arquitectura del nodo.

5. **Validación tardía** — el operador no validaba que los Secrets tuvieran
   las keys correctas; los Jobs fallaban en runtime con errores opacos.

**Decisión 2026-04-08**: reingeniería completa. El engine se reescribe en
**Rust**, embebe toda la lógica de sync/backup/restore en proceso, sin
spawning de binarios externos. Los fuentes de rclone y restic
(`rclone-master/`, `restic-master/`) que estaban vendoreados en el repo se
**eliminan**: sirvieron únicamente como referencia de diseño (estrategias de
chunking, dedup, sync diff) durante la reescritura.

---

## 3. Arquitectura

```
┌────────────────────────────────────────────────────────────────────┐
│                         Kubernetes cluster                         │
│                                                                    │
│  ┌────────────────┐         ┌─────────────────────────────────┐   │
│  │  Operator      │ watch   │  CRDs (scutar.io/v1alpha1)      │   │
│  │  (Node.js TS)  │◄───────►│   - ScutarConnection            │   │
│  │                │         │   - ScutarBackup                │   │
│  │  - valida CRs  │         │   - ScutarSnapshot              │   │
│  │  - genera Spec │         │   - ScutarRestoreRequest        │   │
│  │  - crea Jobs   │         └─────────────────────────────────┘   │
│  └───────┬────────┘                                                │
│          │ crea Job + ConfigMap (BackupSpec) + Secret (creds)     │
│          ▼                                                         │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │  Pod: scutar-runner (Rust)                                 │   │
│  │  ┌────────────────────────────────────────────────────┐    │   │
│  │  │  scutar-cli (binary)                               │    │   │
│  │  │  └─► scutar-engine                                 │    │   │
│  │  │      ├─► snapshot mode (FastCDC + BLAKE3 + AES-GCM)│    │   │
│  │  │      └─► mirror mode  (diff + upload + delete)     │    │   │
│  │  │          └─► scutar-backends (dyn StorageBackend)  │    │   │
│  │  │              ├─ s3   (aws-sdk-s3)                  │    │   │
│  │  │              ├─ azure(azure_storage_blobs)         │    │   │
│  │  │              ├─ gcs  (google-cloud-storage)        │    │   │
│  │  │              └─ sftp (russh + russh-sftp)          │    │   │
│  │  └────────────────────────────────────────────────────┘    │   │
│  │  Mounts:                                                   │   │
│  │   - /etc/scutar/spec.yaml  (ConfigMap, BackupSpec)         │   │
│  │   - /etc/scutar/creds/...  (Secret, credenciales nube)     │   │
│  │   - /data                  (PVC source/destino)            │   │
│  └────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
```

### 3.1 Layout del repositorio

```
scutar/
├── Agent.md                  # este archivo
├── README.md                 # cómo correr / cómo probar
├── package.json              # workspaces npm (operator + server)
│
├── operator/                 # operador Kubernetes (Node.js + TypeScript)
│   ├── src/                  # observa CRDs, genera BackupSpec, crea Jobs
│   ├── examples/             # YAMLs de ejemplo de los CRDs
│   └── deployment/helm/      # Helm chart
│
├── engine/                   # Rust workspace — el corazón
│   ├── Cargo.toml            # workspace root
│   ├── Dockerfile            # multi-stage, multi-arch
│   ├── crates/
│   │   ├── scutar-core/      # tipos, errores, trait StorageBackend, BackupSpec
│   │   ├── scutar-backends/  # impls por nube (s3, azure, gcs, sftp)
│   │   ├── scutar-engine/    # lógica snapshot/mirror/restore
│   │   └── scutar-cli/       # binario `scutar-runner`
│   └── tests/                # tests de integración (MinIO/Azurite/sshd)
│
├── server/                   # API REST + UI (Express + Vue 3)
│   ├── backend/
│   └── ui/
│
├── examples/                 # manifests YAML productivos para kubectl apply
│   ├── walkthrough/          # paso a paso end-to-end (orden numérico)
│   ├── connections/          # un template por backend
│   ├── backups/              # un template por escenario
│   └── restore/
│
└── docs/
    ├── architecture.md       # detalle arquitectónico
    └── adding-a-backend.md   # guía para sumar una nube nueva
```

### 3.2 Trait `StorageBackend`

El trait vive en `engine/crates/scutar-core/src/backend.rs`. Toda nube nueva
implementa este contrato y se registra en
`engine/crates/scutar-backends/src/factory.rs`. **El engine nunca conoce qué
backend está usando**: opera sobre `Arc<dyn StorageBackend>`.

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> BackendCapabilities;
    async fn put(&self, key: &str, data: Bytes) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Bytes>;
    async fn get_range(&self, key: &str, offset: u64, length: u64) -> Result<Bytes>;
    async fn exists(&self, key: &str) -> Result<bool>;
    fn list<'a>(&'a self, prefix: &'a str) -> BoxStream<'a, Result<ObjectMeta>>;
    async fn delete(&self, key: &str) -> Result<()>;
}
```

`BackendCapabilities` permite al engine elegir la estrategia óptima
(multipart vs single PUT, rename atómico, tamaño máximo de objeto). Sumar
una nube = nuevo módulo + impl del trait + arm en el factory. **Cero**
modificación del engine layer.

### 3.3 Modos de operación

| Modo | Caso de uso | Tiene historia | Dedup | Encriptable | Restorable vía CRD |
|---|---|---|---|---|---|
| `snapshot` | Backup tradicional con retention | Sí | Sí (FastCDC + BLAKE3) | Sí (AES-256-GCM) | Sí (`ScutarSnapshot` + `ScutarRestoreRequest`) |
| `mirror` | Replicación 1:1 (sin versionado) | No | No | No | N/A (la "restore" es leer del bucket) |

El "incremental" del proyecto anterior **no** es un modo separado: en
`snapshot`, el primer run sube todo, los siguientes son incrementales por
naturaleza gracias al dedup content-addressable. El usuario sólo elige entre
`snapshot` y `mirror`.

### 3.4 Contrato operador ↔ engine

Estilo **KEDA**: todo configurable vía CRD, credenciales en `Secret`. El
operador:

1. Lee y valida un `ScutarBackup` + el `ScutarConnection` referenciado.
2. Construye un `BackupSpec` (struct definido en `scutar-core::spec`) a
   partir de los CRDs.
3. Crea un `ConfigMap` con el `BackupSpec` serializado a YAML.
4. Crea un `Job` (o `CronJob`) con el Pod del runner, montando:
   - `ConfigMap` en `/etc/scutar/spec.yaml`
   - `Secret` con credenciales en `/etc/scutar/creds/`
   - El PVC fuente/destino en `/data` (según corresponda)
5. El runner lee `spec.yaml`, deserializa con `serde`, ejecuta y reporta.

**Sin variables de entorno mágicas, sin inferencias.** Si un campo falta,
el operador lo rechaza ANTES de crear el Job (validación temprana).

### 3.5 Stack de dependencias del engine

Política de "no terceros" (decisión 2026-04-08): **no dependemos de procesos
externos**. Toda la lógica de sync/backup/dedup/encriptación está implementada
en proceso. Las únicas dependencias externas son librerías Rust que actúan
como bloques de construcción de bajo nivel:

- **SDKs oficiales de cada proveedor de nube** (`aws-sdk-s3`,
  `azure_storage_blobs`, `google-cloud-storage`) — son del proveedor mismo,
  reescribirlos sería reinventar SigV4 / firmas Azure / OAuth GCS.
- **`russh` + `russh-sftp`** para SFTP — implementación pura Rust de SSH.
- **`tokio`** — runtime async.
- **`serde` / `serde_yaml` / `serde_json`** — serialización tipada.
- **`blake3`** — hashing criptográfico (chunk IDs).
- **`fastcdc`** — chunking content-defined (algoritmo público, evita reinventar).
- **`aes-gcm` + `argon2`** (RustCrypto) — encriptación + KDF.

**Lo que NO usamos**: rclone, restic, kopia, ningún binario externo, ninguna
abstracción de terceros tipo `object_store` que esconda lógica nuestra.

---

## 4. CRDs

Group `scutar.io/v1alpha1`. Definidos en `operator/crds/`.

### 4.1 `ScutarConnection`

Define un destino de almacenamiento. Sin lógica, sólo coordenadas + ref a
secret de credenciales.

```yaml
apiVersion: scutar.io/v1alpha1
kind: ScutarConnection
metadata:
  name: prod-s3
spec:
  type: s3                              # s3 | azure | gcs | sftp
  s3:
    bucket: my-backups
    region: us-east-1
    endpoint: ""                        # opcional, S3-compatibles
  credentialsSecretRef:
    name: prod-s3-creds                 # Secret con AWS_ACCESS_KEY_ID / SECRET
```

### 4.2 `ScutarBackup`

Define una unidad de backup. **Reemplaza** el `backupType: sync|full|incremental`
del esquema anterior por `mode: snapshot|mirror`.

```yaml
apiVersion: scutar.io/v1alpha1
kind: ScutarBackup
metadata:
  name: postgres-daily
spec:
  mode: snapshot                        # snapshot | mirror
  schedule: "0 2 * * *"                 # opcional; vacío = one-shot
  source:
    pvcName: postgres-data
    path: /data                         # path dentro del PVC montado
    include: []
    exclude: ["*.tmp"]
  destinationConnectionRef:
    name: prod-s3
  destinationPath: "postgres/"          # prefijo dentro del bucket
  encryption:                           # sólo aplica en mode: snapshot
    enabled: true
    passwordSecretRef:
      name: postgres-backup-password
      key: password
  retention:                            # sólo aplica en mode: snapshot
    keepDaily: 7
    keepWeekly: 4
    keepMonthly: 12
```

### 4.3 `ScutarSnapshot`

**Generado por el engine** al terminar un run en modo `snapshot`. Es la
entidad que un `ScutarRestoreRequest` referencia para restaurar.

```yaml
apiVersion: scutar.io/v1alpha1
kind: ScutarSnapshot
metadata:
  name: postgres-daily-20260408-020001
status:
  backupRef: postgres-daily
  snapshotId: "blake3:abcd1234..."
  createdAt: "2026-04-08T02:00:01Z"
  bytesRead: 12345678
  bytesWritten: 4321000
  filesProcessed: 542
```

### 4.4 `ScutarRestoreRequest`

Pide restaurar un snapshot a un PVC.

```yaml
apiVersion: scutar.io/v1alpha1
kind: ScutarRestoreRequest
metadata:
  name: restore-postgres-20260408
spec:
  snapshotRef:
    name: postgres-daily-20260408-020001
  targetPvcName: postgres-data-restore
  targetPath: /data
```

---

## 5. Decisiones tomadas (changelog de arquitectura)

### 2026-04-08 (segunda iteración — fases 1–7)

- **Backend `Local` agregado** al engine. No estaba en el plan original pero
  resultó muy útil para tests de integración (no requiere nube) y para casos
  air-gapped donde el destino es un PVC NFS o similar. Vive en
  `engine/crates/scutar-backends/src/local/`.
- **`BackupSpec` extendido**: agregados `credentials_dir` (para que cada backend
  encuentre las credenciales mounteadas por el operador en un path conocido)
  y `SourceSpec` con include/exclude de globs. Mantiene compatibilidad
  forward serializando todo opcional.
- **`Sealer` enum** (`None | Gcm`) en `engine/crates/scutar-engine/src/encryption.rs`.
  Toda lectura/escritura del repo de snapshots pasa por `sealer.seal()` /
  `sealer.open()`, así el modo encriptado y no-encriptado comparten 100% del
  pipeline. Argon2id con m=64MiB, t=3, p=1; AES-256-GCM con nonce random por
  mensaje (formato `nonce||ct||tag`).
- **Snapshot manifest format v1** en `manifest.rs`. Pack files agrupan chunks
  hasta `pack_target_size` (16 MiB default), pack id = BLAKE3 del pack
  *encriptado* (cuando aplica). Index files mapean `chunk_id -> (pack_id,
  offset, length)`.
- **Retención simplificada**: implementada como
  `keepLast/Daily/Weekly/Monthly/Yearly`. Sólo borra manifests, **no** los
  packs huérfanos — eso requiere un `prune` job posterior, que es caro y
  debería correr en una schedule más lenta. Decisión consciente para no
  bloquear el path crítico de backups.
- **Operador reescrito** con estructura modular:
  `types/{crd,backup-spec}.ts`, `services/{validation,spec-builder,
  job-builder,reconciler}.ts`, `k8s/{client,dispatcher}.ts`, `index.ts`. La
  validación es **temprana**: si un CR es inválido, se rechaza ANTES de
  crear el Job y se marca `status.condition: Invalid`. Cero inferencias en
  runtime.
- **Job/CronJob mounting**: el operador monta:
  * `ConfigMap` con `BackupSpec` YAML en `/etc/scutar/spec.yaml`
  * `Secret` de credenciales del `ScutarConnection` en `/etc/scutar/creds/`
    (también exporta `AWS_*`/`AZURE_*`/`GOOGLE_APPLICATION_CREDENTIALS` como
    env vars para que los SDKs autodescubran)
  * `Secret` con password de encriptación en `/etc/scutar/password/`
  * PVC source en `/data`
- **CLI `scutar-runner`**: dos modos en el mismo binario:
  - `--spec <spec.yaml>` → backup (snapshot o mirror según el spec)
  - `--spec <spec.yaml> --restore <id> --target <dir>` → restore
  Output: línea JSON al stdout con el `RunReport` para que el operator
  pueda parsearla del log del Pod.
- **Helm chart**: CRDs en `operator/deployment/helm/crds/` (cuatro: Connection,
  Backup, Snapshot, RestoreRequest), templates `deployment.yaml`,
  `serviceaccount.yaml`, `rbac.yaml`. Valores default en `values.yaml`.
- **Server backend** (Express): adaptado el validador para el nuevo schema
  (`mode`, `source.pvcName`, `destinationConnectionRef.name`, `encryption`,
  `retention`).
- **Server UI** (Vue): los tipos en `server/ui/src/types/backup.ts` están
  reescritos al nuevo schema con shims `@deprecated` (`LeviathanBackup`,
  `BackupType`) para que el código existente compile. **Las vistas
  (BackupForm, BackupsView, ScheduledView) NO fueron rewriteadas** porque
  son ~2400 LOC de UX y son trabajo de iteración aparte. Hoy compilan pero
  mostrarán campos incorrectos hasta que la fase 9 las cubra. Ver §8.
- **`docs/architecture.md` y `docs/adding-a-backend.md`** escritos.
- **`engines/scutar-runner/`** (Node.js + sh) eliminado completamente,
  reemplazado por `engine/` (Rust workspace).

### 2026-04-08 (primera iteración — fase 0)

- **Reescritura del engine en Rust.** Reemplaza el runner Node.js que hacía
  shell-out a rclone y restic. Justificación: ver §2.
- **Sin procesos hijos.** Toda la lógica de backup/sync corre en proceso.
- **Dependencias permitidas**: SDKs oficiales de cada nube + crates de
  bajo nivel (criptografía, chunking, async runtime). NO rclone, NO restic,
  NO `object_store` ni abstracciones de terceros que escondan lógica nuestra.
- **`object_store` (Apache Arrow) descartado**, propuesto y rechazado por el
  usuario porque "es una abstracción de terceros que no controlamos".
- **Modos**: `snapshot` y `mirror`. Se elimina la trinidad confusa
  `sync|full|incremental`. Incremental no es un modo: es una propiedad
  emergente del dedup en `snapshot`.
- **Encriptación opcional, configurable por CRD** (`spec.encryption.enabled`).
  Cuando está activa: AES-256-GCM con KEK derivada por Argon2id desde un
  password mounteado por Secret. El runner nunca recibe el password en env vars.
- **Dedup con FastCDC + BLAKE3 desde el día 1** en modo snapshot.
- **Contrato operador↔engine vía ConfigMap YAML** mounteado en
  `/etc/scutar/spec.yaml`. Cero env vars mágicas. Validación temprana en el
  operador.
- **`rclone-master/` y `restic-master/` eliminados del repo.** Eran árboles
  upstream vendoreados sin buildear, servían sólo como referencia de diseño.
- **`scutar-server` (UI + API REST) se mantiene** en su stack actual
  (Express + Vue 3). Sólo se ajusta para reflejar los nuevos campos de los
  CRDs (`mode` en lugar de `backupType`). Renombrado a `server/`.
- **Operador sigue en Node.js + TypeScript.** Su rol es interpretar CRDs y
  disparar Jobs; ese trabajo lo hace bien con `@kubernetes/client-node`.
  Renombrado a `operator/`.
- **Reemplazo directo, sin convivencia.** El proyecto no está en producción;
  no hay flag para elegir entre runner viejo y runner nuevo.

---

## 6. Roadmap

| Fase | Contenido | Estado |
|---|---|---|
| **0** | Reestructurar repo, Agent.md, README, esqueleto Rust workspace, Dockerfile | ✅ |
| **1** | CRDs nuevos (`mode: snapshot\|mirror`), generación de `BackupSpec` desde el operador, montaje de ConfigMap en el Pod | ✅ |
| **2** | Implementación real de los backends (Local, S3, Azure, GCS, SFTP) | ✅ |
| **3** | Engine: `mirror` end-to-end y `snapshot` (FastCDC + BLAKE3 + pack files + manifest + retention) | ✅ |
| **4** | Restore: leer manifest y reconstruir el filesystem | ✅ |
| **5** | Encriptación AES-256-GCM + KDF Argon2id (integrada en snapshot/restore) | ✅ |
| **6** | Adaptar `server/` backend a los nuevos CRDs (UI con shims, ver §8) | ⚠️ parcial |
| **7** | Helm chart con CRDs + docs `architecture.md` y `adding-a-backend.md` | ✅ |
| **8** | Tests de integración (MinIO/Azurite/fake-gcs/sshd) y validación con `cargo check`/`cargo test` | pendiente |
| **9** | Reescritura de las vistas Vue (BackupForm, ScheduledView, BackupsView) para el nuevo schema | pendiente |
| **10** | `prune` job: reclamar packs huérfanos tras retention | pendiente |
| **11** | Paralelizar chunking y uploads (perf) | pendiente |

---

## 7. Estado conocido y deuda técnica

Snapshot del estado al cierre de la segunda iteración (fases 1-7 completas):

### Compilación
- **El workspace Rust nunca fue validado con `cargo check` en este entorno**
  porque no hay Rust instalado. Las firmas de los crates de nube
  (`aws-sdk-s3`, `azure_storage_blobs`, `google-cloud-storage`, `russh`,
  `russh-sftp`) cambian entre versiones; es muy probable que el primer
  `cargo check` arroje errores de API surface en uno o más backends. Cada
  error está aislado a un solo módulo (`scutar-backends/src/<nube>/mod.rs`)
  y debería resolverse leyendo la doc de la crate en su versión exacta.
  El **engine layer** (mirror, snapshot, chunker, packer, encryption,
  retention, restore) sólo usa crates muy estables (`tokio`, `serde`,
  `blake3`, `fastcdc`, `aes-gcm`, `argon2`) y debería compilar limpio.

### Tests
- **No hay tests de integración escritos todavía** (fase 8 pendiente). El
  approach previsto:
  - Local backend → tests puros con tempdir
  - S3 → testcontainers con MinIO
  - Azure → testcontainers con Azurite
  - GCS → testcontainers con `fsouza/fake-gcs-server`
  - SFTP → testcontainers con `linuxserver/openssh-server` o `atmoz/sftp`
- Una vez que la matriz de tests pase localmente, el engine entero puede
  considerarse listo para una primera prueba productiva en cluster.

### UI
- `server/ui/src/types/backup.ts` está actualizado al nuevo schema y exporta
  shims `@deprecated` para los nombres viejos.
- Los componentes `BackupForm.vue`, `BackupsView.vue`, `ScheduledView.vue`
  todavía referencian `spec.backupType`, `spec.source` (string), `spec.
  destination`, `spec.credentialsSecret`. Compilan gracias a los shims pero
  no representan los campos nuevos (`mode`, `source.pvcName`, `encryption`,
  `retention`). **Trabajo pendiente (fase 9)**: rediseñar el wizard de la UI
  para exponer:
  - selector `mode: snapshot | mirror`
  - dropdown de `ScutarConnection` (no se escribe URI a mano)
  - selector de PVC y subpath
  - sección de encriptación (toggle + secret picker)
  - sección de retention (5 inputs)

### Engine
- **No hay paralelismo todavía**: el snapshot procesa archivos secuencialmente
  y los uploads van uno por uno. Para volúmenes grandes esto va a doler. La
  pieza siguiente es paralelizar el chunking + uploads con un work-stealing
  pool de tokio (fase 11).
- **No hay `prune`**: la retención borra manifests pero no los packs
  huérfanos. Hace falta un job aparte que liste packs, los cruce contra
  todos los manifests y elimine los no referenciados. Decisión consciente
  porque es un job caro que debe correr en una schedule más lenta.
- **SFTP host key pinning**: el handler actual acepta cualquier server key.
  Marcado con `TODO(security)` en `sftp/mod.rs`. Antes de productivo:
  pinearle un known_hosts mounteado del Secret.
- **Read-into-memory**: el snapshot lee cada archivo entero en memoria antes
  de chunkearlo. Está bien para archivos chicos/medianos pero va a romper
  con archivos de cientos de GiB. Solución: streamear con `tokio::fs::File`
  + `BufReader` y empujar al chunker en buffers de N MiB. Follow-up.

### Operador
- El reconciler crea ConfigMap+Job/CronJob bien, pero **no maneja
  `ScutarRestoreRequest` todavía**. La fase 4 implementa la lógica del
  engine; falta conectar el operador para que cuando vea un
  `ScutarRestoreRequest`, monte el PVC destino, genere un `BackupSpec`
  derivado del Snapshot referenciado y dispare un Job con
  `--restore <id> --target /data`. Es código casi mecánico siguiendo
  `reconciler.ts` como template.
- El operador tampoco crea `ScutarSnapshot` CRDs leyendo el output del Pod
  todavía. Hoy el engine emite el JSON al stdout pero el operador no lo
  consume. Follow-up trivial: un watcher de Pods owned + parser de la
  última línea del log.

### Memoria del agente
- Cuando arranquemos cualquiera de estos pendientes, recordar que el
  contexto de cómo está armado vive en este Agent.md. Antes de tocar
  cualquier crate Rust, releer §5 (changelog) para entender por qué algo
  está donde está.

## 8. Cómo trabajar con el agente en este repo

- Antes de tocar arquitectura, **leer este archivo** y, si la decisión cambia
  algo registrado acá, **actualizarlo** en la misma PR.
- Las decisiones se registran en §5 con fecha. Nunca borrar entries históricas;
  si una decisión se revierte, agregar una nueva entry explicando el cambio
  y por qué.
- Convención de los crates Rust: nada de `unwrap()` fuera de tests; usar
  `Result<T, scutar_core::Error>` o `anyhow::Result<T>` en el binario.
- Tests de integración de backends usan contenedores efímeros (MinIO, Azurite,
  fake-gcs-server, openssh-server) — ver `engine/tests/` (a crear en fase 8).
