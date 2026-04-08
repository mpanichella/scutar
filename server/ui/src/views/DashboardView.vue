<template>
  <div>
    <div class="page-header">
      <h1>Dashboard</h1>
      <p class="lead">Bienvenido a Leviathan. Gestioná backups y recuperación ante desastres desde un solo lugar.</p>
    </div>
    <div class="row g-4">
      <div class="col-md-6">
        <div class="app-card h-100">
          <div class="card-body d-flex flex-column">
            <div class="d-flex align-items-center gap-2 mb-3">
              <span class="fs-4" aria-hidden="true">📦</span>
              <h5 class="card-title mb-0 text-primary">Backups</h5>
            </div>
            <p class="card-text small text-muted flex-grow-1">
              Creá y gestioná backups: sincronización, copia exacta o incremental. Definí origen, destino (S3, Azure, GCS, SFTP, local), credenciales y programación. Leviathan ejecuta lo que corresponda.
            </p>
            <router-link to="/backups" class="btn btn-primary btn-sm align-self-start">Ver y gestionar backups</router-link>
          </div>
        </div>
      </div>
      <div class="col-md-6">
        <div class="app-card h-100">
          <div class="card-body d-flex flex-column">
            <div class="d-flex align-items-center gap-2 mb-3">
              <span class="fs-4" aria-hidden="true">🔄</span>
              <h5 class="card-title mb-0">DR</h5>
            </div>
            <p class="card-text small text-muted flex-grow-1">
              Automatización de recuperación ante desastres.
            </p>
            <router-link to="/dr" class="btn btn-outline-secondary btn-sm align-self-start">Ir a DR</router-link>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// Dashboard – Leviathan
</script>
