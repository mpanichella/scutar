<template>
  <form class="backup-wizard needs-validation" :class="{ 'was-validated': submitted }" novalidate @submit.prevent="onSubmit">
    <!-- Stepper -->
    <nav class="wizard-stepper mb-4" aria-label="Pasos del backup">
      <ol class="stepper-list">
        <li
          v-for="(step, i) in STEPS"
          :key="step.id"
          class="stepper-item"
          :class="{ active: currentStep === step.id, done: currentStep > step.id }"
        >
          <button
            type="button"
            class="stepper-btn"
            :class="{ active: currentStep === step.id }"
            :aria-current="currentStep === step.id ? 'step' : undefined"
            @click="goToStep(step.id)"
          >
            <span class="stepper-num">{{ i + 1 }}</span>
            <span class="stepper-label d-none d-md-inline">{{ step.short }}</span>
          </button>
          <span v-if="i < STEPS.length - 1" class="stepper-line" aria-hidden="true"></span>
        </li>
      </ol>
    </nav>

    <!-- Step content -->
    <div class="wizard-panel app-card">
      <!-- Step 1: Identificación -->
      <div v-show="currentStep === 1" class="wizard-step">
        <div class="card-header">Identificación del backup</div>
        <div class="card-body">
          <p class="text-muted small mb-4">Nombre, namespace y tipo de backup (sync, copia exacta o incremental).</p>
          <div class="row g-3">
            <div class="col-md-6">
              <label class="form-label fw-semibold">Nombre</label>
              <input
                v-model="form.name"
                name="backup-name"
                type="text"
                class="form-control form-control-lg"
                required
                :disabled="!!existingName"
                placeholder="ej. daily-app-backup"
              />
              <div class="invalid-feedback">Requerido.</div>
            </div>
            <div class="col-md-6">
              <label class="form-label fw-semibold">Namespace</label>
              <input v-model="form.namespace" type="text" class="form-control form-control-lg" placeholder="default" />
            </div>
            <div class="col-12">
              <label class="form-label fw-semibold">Tipo de backup</label>
              <div class="row g-2">
                <div class="col-md-4">
                  <div class="form-check form-check-card">
                    <input v-model="form.spec.backupType" value="sync" type="radio" class="form-check-input" id="bt-sync" />
                    <label class="form-check-label w-100" for="bt-sync">
                      <strong>Sync</strong>
                      <span class="d-block small text-muted">Espejo origen ↔ destino</span>
                    </label>
                  </div>
                </div>
                <div class="col-md-4">
                  <div class="form-check form-check-card">
                    <input v-model="form.spec.backupType" value="full" type="radio" class="form-check-input" id="bt-full" />
                    <label class="form-check-label w-100" for="bt-full">
                      <strong>Copia exacta</strong>
                      <span class="d-block small text-muted">Sin historial</span>
                    </label>
                  </div>
                </div>
                <div class="col-md-4">
                  <div class="form-check form-check-card">
                    <input v-model="form.spec.backupType" value="incremental" type="radio" class="form-check-input" id="bt-inc" />
                    <label class="form-check-label w-100" for="bt-inc">
                      <strong>Incremental</strong>
                      <span class="d-block small text-muted">Con historial y deduplicación</span>
                    </label>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Step 2: Origen -->
      <div v-show="currentStep === 2" class="wizard-step">
        <div class="card-header">Origen</div>
        <div class="card-body">
          <p class="text-muted small mb-4">Dónde se leen los datos: carpeta local en el pod o almacenamiento remoto (S3, Azure, GCS, SFTP). Configurá todos los parámetros según el tipo elegido.</p>
          <div class="row g-3">
            <div class="col-12">
              <label class="form-label fw-semibold">Tipo de origen</label>
              <select v-model="sourceType" class="form-select form-select-lg">
                <option v-for="opt in STORAGE_OPTIONS" :key="opt.id" :value="opt.id">{{ opt.label }}</option>
              </select>
            </div>
            <div class="col-12">
              <StorageConfigForm
                v-model="form.spec.source"
                :type="sourceType as StorageType"
              />
              <div v-if="submitted && !form.spec.source" class="invalid-feedback d-block">Completá el origen.</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Step 3: Destino -->
      <div v-show="currentStep === 3" class="wizard-step">
        <div class="card-header">Destino</div>
        <div class="card-body">
          <p class="text-muted small mb-4">Dónde se escriben los datos: local o nube (S3, Azure, GCS, SFTP). Configurá host, bucket, rutas, etc. según el tipo.</p>
          <div class="row g-3">
            <div class="col-12">
              <label class="form-label fw-semibold">Tipo de destino</label>
              <select v-model="destinationType" class="form-select form-select-lg">
                <option v-for="opt in STORAGE_OPTIONS" :key="opt.id" :value="opt.id">{{ opt.label }}</option>
              </select>
            </div>
            <div class="col-12">
              <StorageConfigForm
                v-model="form.spec.destination"
                :type="destinationType as StorageType"
              />
              <div v-if="submitted && !form.spec.destination" class="invalid-feedback d-block">Completá el destino.</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Step 4: Credenciales -->
      <div v-show="currentStep === 4" class="wizard-step">
        <div class="card-header">Credenciales</div>
        <div class="card-body">
          <p class="text-muted small mb-4">
            Elegí el Secret de credenciales que va a usar este backup (mismo namespace: <strong>{{ form.namespace || 'default' }}</strong>). Las credenciales se crean y gestionan en <router-link to="/credentials">Credenciales</router-link>.
          </p>
          <div class="row g-3">
            <div class="col-12">
              <label class="form-label fw-semibold">Credencial</label>
              <div class="d-flex flex-wrap gap-2 align-items-center">
                <select
                  v-model="form.spec.credentialsSecret"
                  class="form-select form-select-lg"
                  required
                  style="max-width: 320px;"
                >
                  <option value="">— Seleccionar —</option>
                  <option v-for="s in credentialSecrets" :key="s.namespace + '/' + s.name" :value="s.name">
                    {{ s.name }}{{ s.credentialType ? ' (' + s.credentialType + ')' : '' }}
                  </option>
                </select>
                <button type="button" class="btn btn-outline-secondary btn-sm" @click="loadCredentialSecrets" :disabled="loadingCredentials">
                  {{ loadingCredentials ? '…' : 'Actualizar' }}
                </button>
              </div>
              <div v-if="!loadingCredentials && credentialSecrets.length === 0" class="alert alert-info mt-2 mb-0 py-2 small">
                No hay credenciales en este namespace. <router-link to="/credentials">Crear en Credenciales</router-link> (S3, Azure, GCS o SFTP) y volvé a elegir acá.
              </div>
              <div v-else class="form-text">
                <router-link to="/credentials">Gestionar credenciales</router-link>
              </div>
              <div class="invalid-feedback">Seleccioná una credencial.</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Step 5: Programación -->
      <div v-show="currentStep === 5" class="wizard-step">
        <div class="card-header">Programación</div>
        <div class="card-body">
          <p class="text-muted small mb-4">Ejecución única o recurrente (cron).</p>
          <div class="row g-3">
            <div class="col-12">
              <label class="form-label fw-semibold">Frecuencia</label>
              <select v-model="schedulePreset" class="form-select form-select-lg" style="max-width: 280px;">
                <option v-for="p in CRON_PRESETS" :key="p.value || 'custom'" :value="p.value">{{ p.label }}</option>
              </select>
              <input
                v-if="schedulePreset === ''"
                v-model="form.spec.schedule"
                type="text"
                class="form-control form-control-lg font-monospace mt-2"
                style="max-width: 280px;"
                placeholder="0 2 * * *"
              />
              <div class="form-text">Dejá "Una sola vez" para ejecutar solo una vez.</div>
            </div>
            <div class="col-12">
              <div class="form-check form-switch">
                <input v-model="form.spec.suspend" type="checkbox" class="form-check-input" id="suspend" />
                <label class="form-check-label" for="suspend">Suspender programación (no ejecutar hasta desactivar)</label>
              </div>
            </div>
            <div class="col-12">
              <button
                type="button"
                class="btn btn-link btn-sm p-0 text-secondary"
                data-bs-toggle="collapse"
                data-bs-target="#advancedBackup"
              >
                Opciones avanzadas
              </button>
              <div id="advancedBackup" class="collapse mt-2">
                <div class="row g-2">
                  <div class="col-md-4">
                    <label class="form-label small">Backoff limit</label>
                    <input v-model.number="jobTemplate.backoffLimit" type="number" class="form-control" min="0" />
                  </div>
                  <div class="col-md-4">
                    <label class="form-label small">TTL después de terminar (seg)</label>
                    <input v-model.number="jobTemplate.ttlSecondsAfterFinished" type="number" class="form-control" min="0" />
                  </div>
                  <div class="col-12">
                    <label class="form-label small">Argumentos extra (uno por línea)</label>
                    <textarea v-model="extraArgsText" class="form-control font-monospace" rows="2" placeholder="--transfers 4" />
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Step 6: Resumen -->
      <div v-show="currentStep === 6" class="wizard-step">
        <div class="card-header">Resumen</div>
        <div class="card-body">
          <p class="text-muted small mb-4">Revisá la configuración. Podés editar cualquier sección.</p>
          <div class="row g-3">
            <div class="col-md-6">
              <div class="summary-card">
                <div class="d-flex justify-content-between align-items-start">
                  <h6 class="text-muted text-uppercase small mb-2">Identificación</h6>
                  <button type="button" class="btn btn-link btn-sm p-0" @click="goToStep(1)">Editar</button>
                </div>
                <p class="mb-0"><strong>{{ form.name || '—' }}</strong></p>
                <p class="small text-muted mb-0">Namespace: {{ form.namespace || 'default' }}</p>
                <p class="small mb-0">Tipo: {{ backupTypeLabel(form.spec.backupType) }}</p>
              </div>
            </div>
            <div class="col-md-6">
              <div class="summary-card">
                <div class="d-flex justify-content-between align-items-start">
                  <h6 class="text-muted text-uppercase small mb-2">Origen</h6>
                  <button type="button" class="btn btn-link btn-sm p-0" @click="goToStep(2)">Editar</button>
                </div>
                <p class="small font-monospace mb-0 text-break">{{ form.spec.source || '—' }}</p>
              </div>
            </div>
            <div class="col-md-6">
              <div class="summary-card">
                <div class="d-flex justify-content-between align-items-start">
                  <h6 class="text-muted text-uppercase small mb-2">Destino</h6>
                  <button type="button" class="btn btn-link btn-sm p-0" @click="goToStep(3)">Editar</button>
                </div>
                <p class="small font-monospace mb-0 text-break">{{ form.spec.destination || '—' }}</p>
              </div>
            </div>
            <div class="col-md-6">
              <div class="summary-card">
                <div class="d-flex justify-content-between align-items-start">
                  <h6 class="text-muted text-uppercase small mb-2">Credenciales</h6>
                  <button type="button" class="btn btn-link btn-sm p-0" @click="goToStep(4)">Editar</button>
                </div>
                <p class="small font-monospace mb-0">{{ form.spec.credentialsSecret || '—' }}</p>
              </div>
            </div>
            <div class="col-12">
              <div class="summary-card">
                <div class="d-flex justify-content-between align-items-start">
                  <h6 class="text-muted text-uppercase small mb-2">Programación</h6>
                  <button type="button" class="btn btn-link btn-sm p-0" @click="goToStep(5)">Editar</button>
                </div>
                <p class="small mb-0">{{ form.spec.schedule ? form.spec.schedule : 'Una sola vez' }} <span v-if="form.spec.suspend" class="badge bg-warning text-dark">Suspendido</span></p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Wizard footer -->
    <div class="wizard-footer mt-4">
      <div class="wizard-footer-inner">
        <router-link to="/backups" class="btn btn-outline-secondary">Cancelar</router-link>
        <div class="wizard-footer-actions">
          <button v-if="currentStep > 1" type="button" class="btn btn-outline-primary" @click="prevStep">
            Anterior
          </button>
          <button v-if="currentStep < 6" type="button" class="btn btn-primary" @click="nextStep">
            Siguiente
          </button>
          <button v-if="currentStep === 6" type="submit" class="btn btn-primary" :disabled="saving">
            {{ saving ? 'Guardando…' : (existingName ? 'Actualizar backup' : 'Crear backup') }}
          </button>
        </div>
      </div>
      <div v-if="currentStep < 6" class="mt-2 text-end">
        <button type="button" class="btn btn-link btn-sm p-0 text-muted" @click="currentStep = 6">
          Ir al resumen →
        </button>
      </div>
    </div>
  </form>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { STORAGE_OPTIONS, CRON_PRESETS } from '../constants/destinations'
import type { BackupFormState, LeviathanBackupSpec } from '../types/backup'
import { BACKUP_TYPE_LABELS } from '../types/backup'
import type { BackupType } from '../types/backup'
import type { StorageType } from '../utils/storage-uri'
import StorageConfigForm from './StorageConfigForm.vue'

const STEPS = [
  { id: 1, short: 'Identificación' },
  { id: 2, short: 'Origen' },
  { id: 3, short: 'Destino' },
  { id: 4, short: 'Credenciales' },
  { id: 5, short: 'Programación' },
  { id: 6, short: 'Resumen' },
]

const props = defineProps<{
  initial?: Partial<BackupFormState> & { name?: string; namespace?: string; spec?: Partial<LeviathanBackupSpec> }
  existingName?: string
}>()

const router = useRouter()
const currentStep = ref(1)
const credentialSecrets = ref<{ name: string; namespace: string; credentialType?: string }[]>([])
const loadingCredentials = ref(false)
const submitted = ref(false)
const saving = ref(false)
const sourceType = ref('local')
const destinationType = ref('s3')
const schedulePreset = ref(
  props.initial?.spec?.schedule
    ? (CRON_PRESETS.find((p) => p.value === props.initial!.spec!.schedule)?.value ?? '')
    : '__none__'
)
const extraArgsText = ref('')

const form = ref<BackupFormState>({
  name: props.initial?.name ?? '',
  namespace: props.initial?.namespace ?? 'default',
  spec: {
    backupType: props.initial?.spec?.backupType ?? 'sync',
    source: props.initial?.spec?.source ?? '',
    destination: props.initial?.spec?.destination ?? '',
    credentialsSecret: props.initial?.spec?.credentialsSecret ?? '',
    schedule: props.initial?.spec?.schedule,
    suspend: props.initial?.spec?.suspend ?? false,
    extraArgs: props.initial?.spec?.extraArgs,
    jobTemplate: props.initial?.spec?.jobTemplate,
  },
})

const jobTemplate = ref({
  backoffLimit: props.initial?.spec?.jobTemplate?.backoffLimit ?? 3,
  ttlSecondsAfterFinished: props.initial?.spec?.jobTemplate?.ttlSecondsAfterFinished ?? 3600,
})

function backupTypeLabel(t: BackupType | undefined) {
  return t ? BACKUP_TYPE_LABELS[t] ?? t : '—'
}

function inferStorageType(uri: string): string {
  if (!uri) return 'local'
  const u = uri.toLowerCase()
  if (u.startsWith('s3:') || u.startsWith('s3://')) return 's3'
  if (u.startsWith('gcs:')) return 'gcs'
  if (u.startsWith('azure:')) return 'azure'
  if (u.startsWith('sftp') || u.includes('sftp://')) return 'sftp'
  if (u.startsWith('local:') || u.startsWith('/')) return 'local'
  return 'custom'
}

function goToStep(step: number) {
  currentStep.value = step
}

function nextStep() {
  if (currentStep.value < 6) currentStep.value++
}

function prevStep() {
  if (currentStep.value > 1) currentStep.value--
}

async function loadCredentialSecrets() {
  const ns = form.value.namespace || 'default'
  loadingCredentials.value = true
  try {
    const res = await fetch(`/api/secrets?namespace=${encodeURIComponent(ns)}`)
    const data = await res.json().catch(() => ({ items: [] }))
    credentialSecrets.value = data.items || []
  } catch {
    credentialSecrets.value = []
  } finally {
    loadingCredentials.value = false
  }
}

watch(currentStep, (step) => {
  if (step === 4) loadCredentialSecrets()
})
watch(
  () => form.value.namespace,
  () => {
    if (currentStep.value === 4) loadCredentialSecrets()
  }
)
watch(schedulePreset, (v) => {
  if (v === '__none__' || v === '') form.value.spec.schedule = undefined
  else form.value.spec.schedule = v
})
watch(
  () => form.value.spec.schedule,
  (v) => {
    const found = CRON_PRESETS.find((p) => p.value === v)
    if (found) schedulePreset.value = found.value
    else if (!v) schedulePreset.value = '__none__'
    else schedulePreset.value = ''
  }
)
watch(extraArgsText, (t) => {
  const args = t
    .split(/\n/)
    .map((s) => s.trim())
    .filter(Boolean)
  form.value.spec.extraArgs = args.length ? args : undefined
})
watch(
  () => jobTemplate.value.backoffLimit,
  (v) => {
    if (!form.value.spec.jobTemplate) form.value.spec.jobTemplate = {}
    form.value.spec.jobTemplate!.backoffLimit = v
  }
)
watch(
  () => jobTemplate.value.ttlSecondsAfterFinished,
  (v) => {
    if (!form.value.spec.jobTemplate) form.value.spec.jobTemplate = {}
    form.value.spec.jobTemplate!.ttlSecondsAfterFinished = v
  }
)
watch(
  () => props.initial,
  (v) => {
    if (v?.spec) {
      form.value.name = v.name ?? form.value.name
      form.value.namespace = v.namespace ?? form.value.namespace
      form.value.spec = { ...form.value.spec, ...v.spec }
      sourceType.value = inferStorageType(v.spec.source ?? '')
      destinationType.value = inferStorageType(v.spec.destination ?? '')
      if (v.spec.schedule) schedulePreset.value = CRON_PRESETS.find((p) => p.value === v.spec!.schedule)?.value ?? v.spec.schedule ?? ''
      else schedulePreset.value = '__none__'
      if (v.spec.jobTemplate) {
        jobTemplate.value.backoffLimit = v.spec.jobTemplate.backoffLimit ?? 3
        jobTemplate.value.ttlSecondsAfterFinished = v.spec.jobTemplate.ttlSecondsAfterFinished ?? 3600
      }
      if (v.spec.extraArgs?.length) extraArgsText.value = v.spec.extraArgs.join('\n')
    }
  },
  { deep: true }
)

onMounted(() => {
  if (props.initial?.spec) {
    form.value.name = props.initial.name ?? form.value.name
    form.value.namespace = props.initial.namespace ?? form.value.namespace
    form.value.spec = { ...form.value.spec, ...props.initial.spec }
    sourceType.value = inferStorageType(props.initial.spec.source ?? '')
    destinationType.value = inferStorageType(props.initial.spec.destination ?? '')
    if (props.initial.spec.schedule) schedulePreset.value = CRON_PRESETS.find((p) => p.value === props.initial!.spec!.schedule)?.value ?? props.initial.spec.schedule
    else schedulePreset.value = '__none__'
    if (props.initial.spec.jobTemplate) {
      jobTemplate.value.backoffLimit = props.initial.spec.jobTemplate.backoffLimit ?? 3
      jobTemplate.value.ttlSecondsAfterFinished = props.initial.spec.jobTemplate.ttlSecondsAfterFinished ?? 3600
    }
    if (props.initial.spec.extraArgs?.length) extraArgsText.value = props.initial.spec.extraArgs.join('\n')
  }
})

const currentSourceOption = computed(() => STORAGE_OPTIONS.find((o) => o.id === sourceType.value))
const currentSourceExample = computed(
  () => currentSourceOption.value?.exampleSource ?? currentSourceOption.value?.example ?? 'ej. /data o azure:container/path'
)
const currentSourceDesc = computed(
  () => currentSourceOption.value?.descriptionSource ?? currentSourceOption.value?.description ?? 'Origen: carpeta local o remoto.'
)
const currentDestinationOption = computed(() => STORAGE_OPTIONS.find((o) => o.id === destinationType.value))
const currentDestinationExample = computed(() => currentDestinationOption.value?.example ?? '')
const currentDestinationDesc = computed(() => currentDestinationOption.value?.description ?? '')

async function onSubmit() {
  submitted.value = true
  if (!form.value.name) {
    currentStep.value = 1
    return
  }
  if (!form.value.spec.source) {
    currentStep.value = 2
    return
  }
  if (!form.value.spec.destination) {
    currentStep.value = 3
    return
  }
  if (!form.value.spec.credentialsSecret) {
    currentStep.value = 4
    return
  }
  if (schedulePreset.value && schedulePreset.value !== '__none__') form.value.spec.schedule = schedulePreset.value
  else form.value.spec.schedule = undefined
  form.value.spec.jobTemplate = {
    ...form.value.spec.jobTemplate,
    backoffLimit: jobTemplate.value.backoffLimit,
    ttlSecondsAfterFinished: jobTemplate.value.ttlSecondsAfterFinished,
  }
  saving.value = true
  try {
    const ns = form.value.namespace || 'default'
    const url = props.existingName
      ? `/api/backups/${encodeURIComponent(props.existingName)}?namespace=${encodeURIComponent(ns)}`
      : '/api/backups'
    const method = props.existingName ? 'PUT' : 'POST'
    const body = props.existingName
      ? { spec: form.value.spec, namespace: ns }
      : { name: form.value.name, namespace: ns, spec: form.value.spec }
    const res = await fetch(url, {
      method,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    })
    if (!res.ok) {
      const err = await res.json().catch(() => ({}))
      throw new Error(err.error || res.statusText)
    }
    router.push('/backups')
  } catch (e) {
    alert('Error: ' + (e instanceof Error ? e.message : String(e)))
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.backup-wizard {
  width: 100%;
  max-width: 720px;
  margin: 0 auto;
  min-width: 0;
}

.wizard-footer {
  width: 100%;
  min-width: 0;
}

.wizard-footer-inner {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.wizard-footer-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.wizard-stepper {
  background: var(--lev-card-bg, #fff);
  border: 1px solid var(--lev-border, #e2e8f0);
  border-radius: var(--lev-radius, 8px);
  padding: 0.75rem 1rem;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

.stepper-list {
  display: flex;
  align-items: center;
  justify-content: space-between;
  list-style: none;
  margin: 0;
  padding: 0;
  gap: 0;
}

.stepper-item {
  display: flex;
  align-items: center;
  flex: 0 0 auto;
}

.stepper-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.6rem;
  border: none;
  background: transparent;
  border-radius: 6px;
  font-size: 0.875rem;
  color: var(--lev-text-muted, #64748b);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.stepper-btn:hover {
  background: rgba(13, 110, 253, 0.08);
  color: var(--lev-primary, #0d6efd);
}

.stepper-btn.active {
  background: var(--lev-primary, #0d6efd);
  color: #fff;
  font-weight: 600;
}

.stepper-num {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.75rem;
  height: 1.75rem;
  border-radius: 50%;
  background: #e2e8f0;
  color: #64748b;
  font-size: 0.8rem;
  font-weight: 600;
}

.stepper-btn.active .stepper-num {
  background: rgba(255, 255, 255, 0.3);
  color: #fff;
}

.stepper-item.done .stepper-num {
  background: var(--lev-primary, #0d6efd);
  color: #fff;
}

.stepper-line {
  flex: 1;
  min-width: 1.5rem;
  height: 2px;
  background: #e2e8f0;
  margin: 0 0.25rem;
}

.stepper-item.done .stepper-line {
  background: var(--lev-primary, #0d6efd);
}

.wizard-panel {
  min-height: 280px;
}

.wizard-step .card-body {
  padding: 1.5rem 1.75rem;
}

.form-check-card {
  border: 1px solid var(--lev-border, #e2e8f0);
  border-radius: 8px;
  padding: 0.75rem 1rem;
  height: 100%;
}

.form-check-card .form-check-input {
  margin-top: 0.25rem;
}

.form-check-card:has(.form-check-input:checked) {
  border-color: var(--lev-primary, #0d6efd);
  background: rgba(13, 110, 253, 0.04);
}

.summary-card {
  border: 1px solid var(--lev-border, #e2e8f0);
  border-radius: 8px;
  padding: 1rem 1.25rem;
  height: 100%;
}

.summary-card h6 {
  letter-spacing: 0.04em;
}
</style>
