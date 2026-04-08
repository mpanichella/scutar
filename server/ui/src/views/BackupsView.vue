<template>
  <div>
    <div class="page-header d-flex flex-column flex-md-row justify-content-md-between align-items-start gap-3">
      <div>
        <h1>Backups</h1>
        <p class="lead">
          Configurá y gestioná backups: Sync, Copia exacta o Incremental. Definí origen, destino, credenciales y programación.
        </p>
      </div>
      <router-link to="/backups/new" class="btn btn-primary flex-shrink-0">Crear backup</router-link>
    </div>

    <div class="app-card">
      <div class="card-header">Listado de backups</div>
      <div class="card-body">
        <div v-if="loading" class="text-muted py-4">Cargando…</div>
        <template v-else>
          <div v-if="backups.length === 0" class="empty-state">
            <div class="empty-state-icon">📦</div>
            <p class="mb-2">No hay backups configurados</p>
            <p class="small mb-3">
              Creá uno con el botón "Crear backup". Asegurate de tener el API de Leviathan corriendo (<code>npm run start:api</code>).
            </p>
            <router-link to="/backups/new" class="btn btn-primary btn-sm">Crear primer backup</router-link>
          </div>
          <div v-else class="table-responsive">
            <table class="table table-hover align-middle mb-0">
              <thead>
                <tr>
                  <th>Nombre</th>
                  <th>Tipo</th>
                  <th>Origen</th>
                  <th>Destino</th>
                  <th>Programación</th>
                  <th>Estado</th>
                  <th style="width: 140px;"></th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="job in backups" :key="job.metadata?.name">
                  <td><strong>{{ job.metadata?.name }}</strong></td>
                  <td><span class="badge bg-primary">{{ typeLabel(job.spec?.backupType) }}</span></td>
                  <td><code class="small text-break">{{ job.spec?.source }}</code></td>
                  <td><code class="small text-break">{{ job.spec?.destination }}</code></td>
                  <td>{{ job.spec?.schedule || 'Una vez' }}</td>
                  <td><span class="badge" :class="statusClass(job.status?.condition)">{{ job.status?.condition || '—' }}</span></td>
                  <td>
                    <button
                      type="button"
                      class="btn btn-sm btn-success me-1"
                      :disabled="runningNow === job.metadata?.name"
                      @click="runNow(job.metadata?.name!, job.metadata?.namespace)"
                    >
                      {{ runningNow === job.metadata?.name ? '…' : 'Ejecutar ahora' }}
                    </button>
                    <router-link
                      :to="{ path: `/backups/edit/${job.metadata?.name}`, query: { namespace: job.metadata?.namespace || 'default' } }"
                      class="btn btn-sm btn-outline-primary me-1"
                    >
                      Editar
                    </router-link>
                    <button
                      type="button"
                      class="btn btn-sm btn-outline-danger"
                      @click="confirmDelete(job.metadata?.name!, job.metadata?.namespace)"
                    >
                      Eliminar
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>
      </div>
    </div>

    <div class="app-card mt-4">
      <div class="card-header">Origen y destino</div>
      <div class="card-body">
        <p class="small text-muted mb-2">
          Tanto el <strong>origen</strong> como el <strong>destino</strong> pueden ser local o remoto: Azure Blob → S3, SFTP → local, local → GCS, etc.
        </p>
        <ul class="small mb-0">
          <li><strong>Local</strong> – ruta en el pod (ej. /data)</li>
          <li><strong>Amazon S3</strong> – bucket y path</li>
          <li><strong>Google Cloud Storage</strong> – bucket y path</li>
          <li><strong>Azure Blob Storage</strong> – container y path</li>
          <li><strong>SFTP</strong> – servidor y ruta remota</li>
          <li><strong>URI personalizada</strong> – cualquier URI soportada</li>
        </ul>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { BACKUP_TYPE_LABELS } from '../types/backup'
import type { LeviathanBackup } from '../types/backup'

const router = useRouter()
const loading = ref(true)
const backups = ref<LeviathanBackup[]>([])
const namespace = ref('default')
const runningNow = ref<string | null>(null)

function typeLabel(t?: string): string {
  return (t && BACKUP_TYPE_LABELS[t as keyof typeof BACKUP_TYPE_LABELS]) || t || '—'
}

function statusClass(condition?: string): string {
  if (!condition) return 'bg-secondary'
  if (condition === 'Ready' || condition === 'Running') return 'bg-success'
  if (condition === 'Failed' || condition === 'Invalid') return 'bg-danger'
  return 'bg-secondary'
}

async function load() {
  loading.value = true
  try {
    const res = await fetch(`/api/backups?namespace=${encodeURIComponent(namespace.value)}`)
    if (res.ok) {
      const data = await res.json()
      backups.value = data.items || []
    } else {
      backups.value = []
    }
  } catch {
    backups.value = []
  } finally {
    loading.value = false
  }
}

function confirmDelete(name: string, ns?: string) {
  if (!confirm(`¿Eliminar el backup "${name}"?`)) return
  deleteBackup(name, ns)
}

async function deleteBackup(name: string, ns?: string) {
  try {
    const res = await fetch(
      `/api/backups/${encodeURIComponent(name)}?namespace=${encodeURIComponent(ns || namespace.value)}`,
      { method: 'DELETE' }
    )
    if (res.ok) await load()
    else alert('Error al eliminar')
  } catch (e) {
    alert('Error: ' + (e instanceof Error ? e.message : String(e)))
  }
}

async function runNow(name: string, ns?: string) {
  runningNow.value = name
  try {
    const res = await fetch(
      `/api/backups/${encodeURIComponent(name)}/run?namespace=${encodeURIComponent(ns || namespace.value)}`,
      { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' }
    )
    if (res.ok) {
      await load()
    } else {
      const data = await res.json().catch(() => ({}))
      alert(data.error || 'Error al ejecutar')
    }
  } catch (e) {
    alert('Error: ' + (e instanceof Error ? e.message : String(e)))
  } finally {
    runningNow.value = null
  }
}

onMounted(load)
</script>
