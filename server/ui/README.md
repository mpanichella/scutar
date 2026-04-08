# Leviathan – UI

UI liviana para Leviathan: **Vue 3** + **Vite** + **Bootstrap 5**.

## Por qué Vue en lugar de Angular

- **Más liviano**: bundle más chico y menos boilerplate que Angular.
- **Rápido de iterar**: Vite + Vue es muy ágil para desarrollo.
- **Bootstrap**: se usa Bootstrap 5 (CSS + JS) sin necesidad de ng-bootstrap; mismo look que podrías tener en Angular.

Si preferís **Angular**, se puede añadir un segundo frontend en `ui-angular/` y compartir el mismo backend/API.

## Comandos

```bash
npm install
npm run dev      # http://localhost:5173
npm run build    # genera dist/
npm run preview  # previsualizar build
```

## Estructura

- `src/App.vue` – shell con navbar y router-view.
- `src/views/` – Dashboard, Backups, DR (placeholders para conectar al API).
- `src/router/` – rutas.
- Bootstrap se importa en `main.ts`.

## Próximos pasos

- Conectar con el API de Leviathan (listar LeviathanBackup, estado, DR).
- Añadir observabilidad (gráficos, últimos runs, salud).
