<template>
  <div>
    <div class="page-header d-flex flex-column flex-md-row justify-content-md-between align-items-start gap-3">
      <div>
        <h1>Credenciales</h1>
        <p class="lead">
          Secrets de Kubernetes para conectar a S3, Azure, GCS o SFTP. Creá las que necesites y luego elegilas al armar un backup.
        </p>
      </div>
      <div class="d-flex gap-2 flex-wrap">
        <select v-model="namespace" class="form-select" style="max-width: 180px;">
          <option value="default">default</option>
          <option v-for="ns in namespaces" :key="ns" :value="ns">{{ ns }}</option>
        </select>
        <div class="btn-group">
          <button type="button" class="btn btn-primary dropdown-toggle" data-bs-toggle="dropdown" aria-expanded="false">
            Nueva credencial
          </button>
          <ul class="dropdown-menu">
            <li><a class="dropdown-item" href="#" @click.prevent="openNewModal('s3')">Amazon S3</a></li>
            <li><a class="dropdown-item" href="#" @click.prevent="openNewModal('azure')">Azure Blob</a></li>
            <li><a class="dropdown-item" href="#" @click.prevent="openNewModal('gcs')">Google Cloud Storage</a></li>
            <li><a class="dropdown-item" href="#" @click.prevent="openNewModal('sftp')">SFTP</a></li>
          </ul>
        </div>
        <!-- Trigger oculto para que Bootstrap data-api abra el modal -->
        <button
          ref="modalTriggerRef"
          type="button"
          class="d-none"
          data-bs-toggle="modal"
          data-bs-target="#newCredentialModal"
          aria-hidden="true"
        ></button>
      </div>
    </div>

    <div class="app-card">
      <div class="card-header">Credenciales en este namespace</div>
      <div class="card-body">
        <div v-if="loading" class="text-muted py-4">Cargando…</div>
        <template v-else>
          <div v-if="items.length === 0" class="empty-state">
            <div class="empty-state-icon">🔐</div>
            <p class="mb-2">No hay credenciales</p>
            <p class="small mb-3">Creá una con el botón "Nueva credencial" (S3, Azure, GCS o SFTP) para usarla en tus backups.</p>
          </div>
          <div v-else class="table-responsive">
            <table class="table table-hover align-middle mb-0">
              <thead>
                <tr>
                  <th>Nombre</th>
                  <th>Tipo</th>
                  <th>Namespace</th>
                  <th style="width: 100px;"></th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="s in items" :key="s.namespace + '/' + s.name">
                  <td><strong>{{ s.name }}</strong></td>
                  <td><span class="badge bg-secondary">{{ s.credentialType || '—' }}</span></td>
                  <td><code class="small">{{ s.namespace }}</code></td>
                  <td>
                    <button
                      type="button"
                      class="btn btn-sm btn-outline-danger"
                      @click="confirmDelete(s.name, s.namespace)"
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

    <!-- Modal: Nueva credencial -->
    <div
      ref="modalRef"
      class="modal fade"
      id="newCredentialModal"
      tabindex="-1"
      aria-labelledby="newCredentialModalLabel"
      aria-hidden="true"
    >
      <div class="modal-dialog modal-lg modal-dialog-scrollable">
        <div class="modal-content">
          <div class="modal-header">
            <h5 class="modal-title" id="newCredentialModalLabel">Nueva credencial {{ credentialTypeLabel }}</h5>
            <button type="button" class="btn-close" data-bs-dismiss="modal" aria-label="Cerrar"></button>
          </div>
          <div class="modal-body">
            <CloudConnectionHelper
              v-if="newCredentialType"
              :source-type="newCredentialType"
              :destination-type="newCredentialType"
              :namespace="namespace"
              :current-secret-name="''"
              @close="closeModal"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, nextTick, onMounted } from 'vue'
import CloudConnectionHelper from '../components/CloudConnectionHelper.vue'

const CLOUD_LABELS: Record<string, string> = {
  s3: 'Amazon S3',
  azure: 'Azure Blob',
  gcs: 'Google Cloud Storage',
  sftp: 'SFTP',
}

const namespace = ref('default')
const namespaces = ref<string[]>(['default'])
const loading = ref(true)
const items = ref<{ name: string; namespace: string; credentialType?: string }[]>([])
const modalRef = ref<HTMLElement | null>(null)
const modalTriggerRef = ref<HTMLButtonElement | null>(null)
const newCredentialType = ref<string | null>(null)

const credentialTypeLabel = computed(() => (newCredentialType.value ? CLOUD_LABELS[newCredentialType.value] || newCredentialType.value : ''))

async function load() {
  loading.value = true
  try {
    const res = await fetch(`/api/secrets?namespace=${encodeURIComponent(namespace.value)}`)
    const data = await res.json().catch(() => ({ items: [] }))
    items.value = data.items || []
  } catch {
    items.value = []
  } finally {
    loading.value = false
  }
}

function openNewModal(type: string) {
  newCredentialType.value = type
  nextTick(() => {
    modalTriggerRef.value?.click()
  })
}

function closeModal() {
  const el = document.getElementById('newCredentialModal')
  if (el) {
    const Bootstrap = (window as unknown as { bootstrap?: { Modal: { getInstance: (el: HTMLElement) => { hide: () => void } | null } } }).bootstrap
    const instance = Bootstrap?.Modal?.getInstance?.(el)
    if (instance) instance.hide()
  }
  newCredentialType.value = null
  load()
}

function confirmDelete(name: string, ns: string) {
  if (!confirm(`¿Eliminar la credencial "${name}"?`)) return
  doDelete(name, ns)
}

async function doDelete(name: string, ns: string) {
  try {
    const res = await fetch(`/api/secrets/${encodeURIComponent(name)}?namespace=${encodeURIComponent(ns)}`, { method: 'DELETE' })
    if (res.ok) await load()
    else alert('Error al eliminar')
  } catch (e) {
    alert('Error: ' + (e instanceof Error ? e.message : String(e)))
  }
}

watch(namespace, load)

onMounted(load)
</script>
