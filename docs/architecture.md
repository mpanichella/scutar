# Arquitectura del engine

> Este documento detalla cómo funciona internamente el engine. Para
> contexto/decisiones de producto ver [`Agent.md`](../Agent.md). Para
> ejemplos de uso ver [`README.md`](../README.md).

## Crates del workspace Rust

```
engine/crates/
├── scutar-core/        # tipos compartidos, sin dependencias pesadas
│   ├── backend.rs       # trait StorageBackend + ObjectMeta + capabilities
│   ├── error.rs         # Error / Result
│   ├── repo_layout.rs   # constantes de layout (config.json, snapshots/, etc.)
│   └── spec.rs          # BackupSpec, ConnectionSpec, EncryptionSpec, ...
│
├── scutar-backends/    # impls del trait — uno por nube
│   ├── factory.rs       # ConnectionSpec -> Arc<dyn StorageBackend>
│   ├── local/           # filesystem local (tests + air-gapped)
│   ├── s3/              # aws-sdk-s3
│   ├── azure/           # azure_storage_blobs
│   ├── gcs/             # google-cloud-storage
│   └── sftp/            # russh + russh-sftp
│
├── scutar-engine/      # lógica de backup/sync/restore (independiente del backend)
│   ├── walker.rs        # walkdir + globset → lista de FileEntry
│   ├── chunker.rs       # FastCDC + BLAKE3 → Vec<Chunk>
│   ├── packer.rs        # acumula chunks en pack files, los sube
│   ├── manifest.rs      # estructuras serde: RepoConfig, SnapshotManifest, ...
│   ├── encryption.rs    # AES-256-GCM + Argon2id (Sealer)
│   ├── mirror.rs        # modo mirror end-to-end
│   ├── snapshot.rs      # modo snapshot end-to-end
│   ├── retention.rs     # keepLast/Daily/Weekly/Monthly/Yearly
│   ├── restore.rs       # leer manifest → reconstruir filesystem
│   └── report.rs        # RunReport (lo que el operator publica como status)
│
└── scutar-cli/         # binario `scutar-runner`
    └── main.rs          # parsea --spec / --restore / --target, dispatches
```

## Layout del repositorio en el bucket

```
<destination-prefix>/
├── config.json                      # RepoConfig (versión, params, encryption header)
├── index/
│   └── <blake3>.json                # PackIndex sealed (chunk_id → pack_id, offset, length)
├── data/
│   └── <blake3>.pack                # pack file sealed (concatenación de chunks)
├── snapshots/
│   └── 2026-04-08T02:00:01Z-<short>.json  # SnapshotManifest sealed (file tree + chunk refs)
└── .scutar-mirror-state.json        # solo en modo mirror: cache de hashes
```

* `<blake3>` siempre lowercase hex (BLAKE3 de los bytes que se suben).
* "Sealed" = pasado por `Sealer::seal()`. Si el repo es no encriptado,
  `Sealer::None` es pass-through y los archivos están en plaintext. Si el
  repo está encriptado, son `nonce(12) || ciphertext || tag(16)` con
  AES-256-GCM bajo el data key del repo.

## Pipeline modo `snapshot`

1. **Init repo**: si `config.json` no existe, crearlo. Si `encryption.enabled`,
   generar data key random + derivar KEK con Argon2id desde el password,
   wrap del data key con AES-GCM, persistir el header en `config.json`.
2. **Cargar dedup table**: descargar y desencriptar todos los `index/*.json`
   en una `BTreeMap<chunk_id, PackIndexEntry>` en memoria.
3. **Walker**: `walkdir` desde `source.path` aplicando include/exclude globs.
4. **Por archivo**: leer en memoria, FastCDC chunking (avg 1 MiB), BLAKE3
   por chunk. Para cada chunk:
   - Si está en la dedup table → solo registrar referencia en el manifest.
   - Si no → agregarlo al `Packer`. Cuando el pack llega a `pack_target_size`
     (16 MiB por default), `flush()`: sellar y subir como `data/<id>.pack`,
     emitir entries de `PackIndex`.
5. **Flush final del pack**, persistir el `index/<id>.json` con todos los
   chunks nuevos.
6. **Manifest**: armar `SnapshotManifest`, calcular su BLAKE3 como `id`,
   sellar, persistir como `snapshots/<timestamp>-<short-id>.json`.
7. **Retention**: si está configurada, borrar manifests viejos según
   `keepLast/Daily/Weekly/Monthly/Yearly`. **Nota**: no borra packs huérfanos
   (eso es un futuro `prune` job aparte porque es caro).

## Pipeline modo `mirror`

1. **Cargar estado previo**: descargar `.scutar-mirror-state.json` si existe.
2. **Walker**: igual que snapshot.
3. **Por archivo**: si `(size, mtime)` matchean el cache, comparar el hash
   cacheado. Si no, re-hashear (BLAKE3) y comparar contra el remoto. Subir
   solo si cambió (o es nuevo).
4. **Borrar huérfanos**: cualquier path en el state previo que no se vio
   localmente se borra del backend.
5. **Persistir nuevo state**.

No hay encriptación, ni dedup, ni manifest. Es un sync 1:1 estilo `rclone sync`.

## Encriptación (`Sealer`)

```
plaintext  ─┐
            ├─► AES-256-GCM(data_key, random_nonce) ─► nonce(12) || ciphertext || tag(16)
random nonce┘
```

* **Data key (DK)**: 32 random bytes, generados una sola vez al crear el repo.
* **Key encryption key (KEK)**: derivado del password del usuario via
  Argon2id (m=64MiB, t=3, p=1).
* **Wrap del DK**: `AES-256-GCM(KEK, wrap_nonce, DK)`. Persistido en
  `config.json` junto con el salt y los parámetros de Argon2id.
* **Sealing de packs y manifests**: cada uno usa el mismo DK pero un nonce
  random fresco. La salida es `nonce || ct || tag`.

Para restaurar: el engine lee `config.json`, deriva el KEK del password,
unwrappea el DK, y cualquier `Sealer::open()` posterior es transparente.

## Contrato operador ↔ engine

El operador escribe un `BackupSpec` YAML como ConfigMap (`/etc/scutar/spec.yaml`)
en el Pod del engine. El engine lo deserializa con `serde_yaml`, build
deserializa, ejecuta, y sale con código 0/1.

```yaml
name: postgres-daily
mode: snapshot
source:
  path: /data
  exclude: ["*.tmp"]
destination:
  type: s3
  bucket: my-backups
  region: us-east-1
  prefix: postgres/
encryption:
  enabled: true
  password_file: /etc/scutar/password/password
retention:
  keep_daily: 7
  keep_weekly: 4
credentials_dir: /etc/scutar/creds
labels:
  scutar.io/backup: postgres-daily
```

Las credenciales de la nube viven en `/etc/scutar/creds/` (mounteado del
Secret referenciado por `ScutarConnection.credentialsSecretRef`). Los SDKs
oficiales las descubren por convención (`AWS_ACCESS_KEY_ID`,
`AZURE_STORAGE_KEY`, `GOOGLE_APPLICATION_CREDENTIALS`, etc.).

## Output

El engine emite **una línea JSON** al stdout al terminar (además de los
logs estructurados en `stderr` vía `tracing`):

```json
{"mode":"snapshot","bytes_read":12345678,"bytes_written":4321000,"files_processed":542,"files_skipped":0,"files_deleted":0,"snapshot_id":"a1b2c3d4..."}
```

El operador parsea esa línea de los logs del Pod para crear el
`ScutarSnapshot` CRD correspondiente.
