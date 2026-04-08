# Scutar Server

Servidor unificado: **API REST** (backups, credenciales) + **UI** (Vue 3) en un solo proceso.

- **`backend/`** – Express: rutas bajo `/api` y archivos estáticos desde `backend/public/`.
- **`ui/`** – Vue 3 + Vite: al hacer `npm run build` (desde la raíz: `npm run build:ui`), el output se genera en `backend/public/`.

## Cómo levantar todo en uno

Desde la **raíz del monorepo**:

```bash
npm run start:server
```

Eso compila la UI → `backend/public`, compila el backend y arranca el servidor en **http://localhost:3000**. La web y la API comparten el mismo origen, así que las llamadas a `/api/*` no pasan por otro puerto ni por proxy.

## Desarrollo

- **Solo backend:** `npm run start:api` (desde la raíz) o `npm run start` desde `backend/`.
- **Solo UI con hot-reload:** `npm run dev:ui` (desde la raíz); la UI corre en :5173 y hace proxy de `/api` a :3000 (hay que tener el backend levantado).
