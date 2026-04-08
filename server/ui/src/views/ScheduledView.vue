<template>
  <div>
    <div class="page-header d-flex flex-column flex-md-row justify-content-md-between align-items-start gap-3">
      <div>
        <h1>Programados</h1>
        <p class="lead">
          Backups con programación (cron). Los que no tienen programación aparecen en Backups como "Una vez".
        </p>
      </div>
      <router-link to="/backups/new" class="btn btn-primary flex-shrink-0">Nuevo backup</router-link>
    </div>

    <div class="app-card">
      <div class="card-header">Tareas programadas</div>
      <div class="card-body">
        <div v-if="loading" class="text-muted py-4">Cargando…</div>
        <template v-else>
          <div v-if="scheduled.length === 0" class="empty-state">
            <div class="empty-state-icon">🕐</div>
            <p class="mb-2">No hay backups programados</p>
            <p class="small mb-3">
              Los backups con cron aparecen acá. Creá uno desde <router-link to="/backups">Backups</router-link> con programación distinta de "Una sola vez".
            </p>
            <router-link to="/backups/new" class="btn btn-primary btn-sm">Nuevo backup</router-link>
          </div>
          <div v-else class="table-responsive">
            <table class="table table-hover align-middle mb-0">
              <thead>
                <tr>
                  <th>Nombre</th>
                  <th>Tipo</th>
                  <th>Origen</th>
                  <th>Destino</th>
                  <th>Cron</th>
                  <th>Estado</th>
                  <th style="width: 140px;"></th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="job in scheduled" :key="job.metadata?.name">
                  <td><strong>{{ job.metadata?.name }}</strong></td>
                  <td><span class="badge bg-primary">{{ typeLabel(job.spec?.backupType) }}</span></td>
                  <td><code class="small text-break">{{ job.spec?.source }}</code></td>
                  <td><code class="small text-break">{{ job.spec?.destination }}</code></td>
                  <td><code class="small">{{ job.spec?.schedule }}</code></td>
                  <td><span class="badge" :class="statusClass(job.status?.condition)">{{ job.status?.condition || '—' }}</span></td>
                  <td>
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { BACKUP_TYPE_LABELS } from '../types/backup'
import type { LeviathanBackup } from '../types/backup'

const loading = ref(true)
const backups = ref<LeviathanBackup[]>([])
const namespace = ref('default')

const scheduled = computed(() =>
  backups.value.filter((b) => b.spec?.schedule && b.spec.schedule.trim() !== '')
)

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

onMounted(load)
</script>
