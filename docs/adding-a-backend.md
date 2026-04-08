# Cómo agregar un backend de almacenamiento nuevo a Scutar

Esta guía describe los pasos exactos para sumar una nube nueva al engine.
La arquitectura está pensada para que esto sea **un solo módulo nuevo**, sin
tocar el engine ni la lógica de snapshot/mirror.

## Resumen del contrato

Todo lo que el engine necesita está definido por el trait
[`StorageBackend`](../engine/crates/scutar-core/src/backend.rs):

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

El engine consume `Arc<dyn StorageBackend>`. **Nunca conoce qué backend está
usando.** Si tu impl satisface el trait, snapshot, mirror, restore, retention
y encriptación funcionan automáticamente sobre ella.

## Pasos

### 1. Definir el `ConnectionSpec`

Editar [`engine/crates/scutar-core/src/spec.rs`](../engine/crates/scutar-core/src/spec.rs)
y agregar una variante al enum `ConnectionSpec`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConnectionSpec {
    // ...existing variants...
    Backblaze {
        bucket: String,
        region: String,
        prefix: Option<String>,
    },
}
```

Y agregarle una entrada en `backend_name()`:

```rust
ConnectionSpec::Backblaze { .. } => "backblaze",
```

### 2. Crear el módulo del backend

Crear `engine/crates/scutar-backends/src/backblaze/mod.rs`:

```rust
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{self, BoxStream};
use scutar_core::{
    BackendCapabilities, ConnectionSpec, Error, ObjectMeta, Result, StorageBackend,
};
use std::sync::Arc;

pub struct BackblazeBackend {
    // tu cliente aquí
}

#[async_trait]
impl StorageBackend for BackblazeBackend {
    fn name(&self) -> &str { "backblaze" }
    fn capabilities(&self) -> BackendCapabilities { /* ... */ }
    async fn put(&self, key: &str, data: Bytes) -> Result<()> { /* ... */ }
    async fn get(&self, key: &str) -> Result<Bytes> { /* ... */ }
    async fn get_range(&self, key: &str, offset: u64, length: u64) -> Result<Bytes> { /* ... */ }
    async fn exists(&self, key: &str) -> Result<bool> { /* ... */ }
    fn list<'a>(&'a self, prefix: &'a str) -> BoxStream<'a, Result<ObjectMeta>> { /* ... */ }
    async fn delete(&self, key: &str) -> Result<()> { /* ... */ }
}

pub async fn build(spec: &ConnectionSpec) -> Result<Arc<dyn StorageBackend>> {
    match spec {
        ConnectionSpec::Backblaze { .. } => {
            // construir el cliente
            Ok(Arc::new(BackblazeBackend { /* ... */ }))
        }
        _ => Err(Error::Config("expected Backblaze connection spec".into())),
    }
}
```

### 3. Registrar el módulo

En [`engine/crates/scutar-backends/src/lib.rs`](../engine/crates/scutar-backends/src/lib.rs):

```rust
pub mod backblaze;
```

### 4. Conectar al factory

En [`engine/crates/scutar-backends/src/factory.rs`](../engine/crates/scutar-backends/src/factory.rs):

```rust
pub async fn build_backend(spec: &ConnectionSpec) -> Result<Arc<dyn StorageBackend>> {
    match spec {
        // ...existing arms...
        ConnectionSpec::Backblaze { .. } => crate::backblaze::build(spec).await,
    }
}
```

### 5. Agregar el SDK al `Cargo.toml`

En [`engine/Cargo.toml`](../engine/Cargo.toml), bajo `[workspace.dependencies]`:

```toml
backblaze-sdk = "x.y.z"
```

Y en `engine/crates/scutar-backends/Cargo.toml`:

```toml
backblaze-sdk.workspace = true
```

### 6. Reflejar el tipo en el operador y la UI

Para que el usuario pueda crear `ScutarConnection` del tipo nuevo:

* `operator/src/types/crd.ts` — agregar `BackblazeConnectionDetails` y la
  variante en `ConnectionType`.
* `operator/src/services/spec-builder.ts` — caso `case "backblaze":` que
  arme el `ConnectionSpec` que va al engine.
* `operator/src/services/validation.ts` — validación temprana.
* `operator/src/services/job-builder.ts` — env vars del SDK (si aplica).
* `operator/deployment/helm/crds/scutarconnection.yaml` — agregar el tipo
  al `enum` y el sub-objeto.
* `server/backend/src/scutar-crd.ts` y `server/ui/src/types/backup.ts` —
  para que la UI pueda crear el recurso.

### 7. Tests de integración

Idealmente: un contenedor efímero que emule el servicio (MinIO para S3,
Azurite para Azure, fake-gcs-server para GCS, openssh-server para SFTP).
Crear el test en `engine/tests/<backend>_integration.rs` siguiendo el patrón
de los otros backends.

## Cosas a tener en cuenta

* **Idempotencia de `delete`**: borrar un objeto que no existe NO es un
  error. Devolver `Ok(())`.
* **`NotFound`**: cuando un objeto no existe, devolver `Error::NotFound(key)`,
  no `Error::Backend(...)`. La capa engine usa esto para distinguir.
* **Streams en `list`**: si el backend tiene paginación, manejarla
  internamente y emitir un único stream lineal — el engine no debe paginar.
* **`get_range`**: si el backend no soporta ranged reads, podés caer a
  `get` + `slice`. La capa engine usa `get_range` solo en restore sobre
  repos NO encriptados; si todos tus usuarios encriptan, no es crítico.
* **Capacidades**: declarar honestamente `supports_multipart` y
  `max_object_size`. La capa engine puede consultar esto para elegir
  estrategias.
* **Credenciales**: leer del directorio mounteado por el operador
  (`/etc/scutar/creds/`) o de las env vars que el SDK reconoce. El engine
  no debería ver passwords directamente, salvo el password de encriptación
  que viene por archivo separado.
