<template>
  <div class="connection-helper mt-3">
    <div class="helper-title d-flex align-items-center gap-2">
      <span aria-hidden="true">🔐</span>
      Conectar a {{ currentLabel }}
    </div>
    <p class="helper-desc mb-0">
      Completá los datos y creá el Secret directamente en el cluster (recomendado). También podés descargar el YAML para aplicarlo a mano.
    </p>

    <div v-if="connectionTargets.length > 1" class="mb-3">
      <label class="form-label form-section-title">Generar Secret para</label>
      <select v-model="selectedTarget" class="form-select form-select-sm" style="max-width: 280px;">
        <option v-for="t in connectionTargets" :key="t.key" :value="t.key">
          {{ t.label }}
        </option>
      </select>
    </div>

    <div class="mb-3">
      <label class="form-label form-section-title">Nombre del Secret</label>
      <input
        v-model="secretName"
        type="text"
        class="form-control form-control-sm"
        placeholder="ej. leviathan-s3-creds"
        style="max-width: 280px;"
      />
    </div>

    <!-- S3 -->
    <template v-if="selectedCloud === 's3'">
      <div class="row g-2 mb-2">
        <div class="col-md-6">
          <label class="form-label small">Access Key ID</label>
          <input v-model="s3.accessKeyId" type="text" class="form-control form-control-sm" placeholder="AKIA..." />
        </div>
        <div class="col-md-6">
          <label class="form-label small">Secret Access Key</label>
          <input v-model="s3.secretAccessKey" type="password" class="form-control form-control-sm" placeholder="••••••••" autocomplete="off" />
        </div>
        <div class="col-md-6">
          <label class="form-label small">Región</label>
          <input v-model="s3.region" type="text" class="form-control form-control-sm" placeholder="us-east-1" />
        </div>
      </div>
      <p class="small text-muted mb-0">
        Creá un usuario IAM con permisos S3 (GetObject, PutObject, ListBucket). En AWS Console: IAM → Users → Create user → Attach policy (AmazonS3FullAccess o política custom).
      </p>
    </template>

    <!-- Azure -->
    <template v-if="selectedCloud === 'azure'">
      <div class="row g-2 mb-2">
        <div class="col-md-6">
          <label class="form-label small">Nombre de la cuenta de almacenamiento</label>
          <input v-model="azure.account" type="text" class="form-control form-control-sm" placeholder="micuenta" />
        </div>
        <div class="col-md-6">
          <label class="form-label small">Clave de acceso (key)</label>
          <input v-model="azure.key" type="password" class="form-control form-control-sm" placeholder="••••••••" autocomplete="off" />
        </div>
      </div>
      <p class="small text-muted mb-0">
        En Azure Portal: Storage account → Access keys → Key1 o Key2. Usá el nombre de la cuenta y una de las claves.
      </p>
    </template>

    <!-- GCS -->
    <template v-if="selectedCloud === 'gcs'">
      <div class="mb-2">
        <label class="form-label small">JSON de cuenta de servicio (pegá el contenido completo)</label>
        <textarea
          v-model="gcs.serviceAccountJson"
          class="form-control form-control-sm font-monospace"
          rows="6"
          placeholder='{"type": "service_account", "project_id": "...", ...}'
        />
      </div>
      <p class="small text-muted mb-0">
        En GCP: IAM & Admin → Service accounts → Create key (JSON). Descargá el archivo y pegá su contenido acá.
      </p>
    </template>

    <!-- SFTP -->
    <template v-if="selectedCloud === 'sftp'">
      <div class="row g-2 mb-2">
        <div class="col-md-6">
          <label class="form-label small">Host</label>
          <input v-model="sftp.host" type="text" class="form-control form-control-sm" placeholder="sftp.ejemplo.com" />
        </div>
        <div class="col-md-3">
          <label class="form-label small">Puerto</label>
          <input v-model.number="sftp.port" type="number" class="form-control form-control-sm" placeholder="22" />
        </div>
        <div class="col-md-3">
          <label class="form-label small">Usuario</label>
          <input v-model="sftp.user" type="text" class="form-control form-control-sm" placeholder="usuario" />
        </div>
        <div class="col-md-6">
          <label class="form-label small">Contraseña</label>
          <input v-model="sftp.pass" type="password" class="form-control form-control-sm" placeholder="••••••••" autocomplete="off" />
        </div>
      </div>
      <p class="small text-muted mb-0">
        Si usás clave SSH en lugar de contraseña, después de generar el Secret podés editar el YAML y reemplazar <code>pass</code> por <code>key_file</code> con la ruta al archivo de clave en el pod.
      </p>
    </template>

    <div class="mt-3 d-flex flex-wrap gap-2 align-items-center">
      <button
        type="button"
        class="btn btn-primary btn-sm"
        :disabled="!canGenerate || creating"
        @click="createSecretInCluster"
      >
        {{ creating ? 'Creando…' : 'Crear Secret en el cluster' }}
      </button>
      <button type="button" class="btn btn-outline-secondary btn-sm" :disabled="!canGenerate" @click="generateYaml">
        Descargar YAML
      </button>
      <span v-if="!canGenerate" class="small text-muted">Completá los campos obligatorios.</span>
    </div>
    <p v-if="createError" class="small text-danger mt-2 mb-0">{{ createError }}</p>

    <div v-if="generatedYaml" class="mt-3">
      <label class="form-label form-section-title">YAML (alternativa manual)</label>
      <div class="d-flex gap-2 mb-2">
        <button type="button" class="btn btn-outline-primary btn-sm" @click="copyYaml">Copiar</button>
        <button type="button" class="btn btn-outline-secondary btn-sm" @click="downloadYaml">Descargar archivo</button>
        <button type="button" class="btn btn-outline-success btn-sm" @click="useSecretNameAndClose">Usar este nombre y cerrar</button>
      </div>
      <pre class="bg-dark text-light rounded p-3 small mb-0" style="max-height: 180px; overflow: auto;"><code>{{ generatedYaml }}</code></pre>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'

const CLOUD_LABELS: Record<string, string> = {
  s3: 'Amazon S3',
  azure: 'Azure Blob Storage',
  gcs: 'Google Cloud Storage',
  sftp: 'SFTP',
}

const props = withDefaults(
  defineProps<{
    sourceType: string
    destinationType: string
    namespace?: string
    currentSecretName?: string
    /** SFTP del origen (paso 2) para pre-llenar host, user, contraseña */
    initialSftpSource?: { host: string; port: number; user: string; path: string; password: string } | null
    /** SFTP del destino (paso 3) para pre-llenar host, user, contraseña */
    initialSftpDestination?: { host: string; port: number; user: string; path: string; password: string } | null
  }>(),
  { namespace: 'default', currentSecretName: '', initialSftpSource: null, initialSftpDestination: null }
)

const emit = defineEmits<{
  (e: 'use-secret', name: string): void
  (e: 'close'): void
}>()

const selectedTarget = ref('')
const secretName = ref(props.currentSecretName || 'leviathan-cloud-creds')
const s3 = ref({ accessKeyId: '', secretAccessKey: '', region: 'us-east-1' })
const azure = ref({ account: '', key: '' })
const gcs = ref({ serviceAccountJson: '' })
const sftp = ref({ host: '', port: 22, user: '', pass: '' })
const generatedYaml = ref('')
const creating = ref(false)
const createError = ref('')

const cloudTypes = ['s3', 'gcs', 'azure', 'sftp'] as const

/** Tipos de nube únicos (origen y destino): la credencial es la misma, no hace falta elegir "para qué"). */
const connectionTargets = computed(() => {
  const clouds = new Set<string>()
  if (cloudTypes.includes(props.sourceType as any)) clouds.add(props.sourceType)
  if (cloudTypes.includes(props.destinationType as any)) clouds.add(props.destinationType)
  return Array.from(clouds).map((cloud) => ({
    key: cloud,
    cloud,
    label: CLOUD_LABELS[cloud] || cloud,
  }))
})

const selectedCloud = computed(() => {
  const t = connectionTargets.value.find((x) => x.key === selectedTarget.value)
  return t?.cloud ?? connectionTargets.value[0]?.cloud ?? props.destinationType ?? props.sourceType ?? 's3'
})

const currentLabel = computed(() => CLOUD_LABELS[selectedCloud.value] || selectedCloud.value)

watch(
  () => connectionTargets.value,
  (targets) => {
    if (targets.length && !selectedTarget.value) selectedTarget.value = targets[0].key
    if (targets.length === 1) selectedTarget.value = targets[0].key
  },
  { immediate: true }
)

watch(
  () => props.currentSecretName,
  (v) => {
    if (v && !generatedYaml.value) secretName.value = v
  }
)

const initialSftp = computed(() => props.initialSftpDestination ?? props.initialSftpSource)

watch(
  () => [initialSftp.value, selectedCloud.value] as const,
  ([init, cloud]) => {
    if (cloud === 'sftp' && init && (init.host || init.user || init.password)) {
      sftp.value = {
        host: init.host || sftp.value.host,
        port: init.port || 22,
        user: init.user || sftp.value.user,
        pass: init.password || sftp.value.pass,
      }
    }
  },
  { immediate: true }
)

const canGenerate = computed(() => {
  const name = secretName.value.trim()
  if (!name) return false
  switch (selectedCloud.value) {
    case 's3':
      return !!(s3.value.accessKeyId && s3.value.secretAccessKey)
    case 'azure':
      return !!(azure.value.account && azure.value.key)
    case 'gcs':
      try {
        if (!gcs.value.serviceAccountJson.trim()) return false
        JSON.parse(gcs.value.serviceAccountJson)
        return true
      } catch {
        return false
      }
    case 'sftp':
      return !!(sftp.value.host && sftp.value.user && sftp.value.pass)
    default:
      return false
  }
})

function buildRcloneConfigContent(): string {
  const cloud = selectedCloud.value
  if (cloud === 's3') {
    return `[s3]
type = s3
provider = AWS
env_auth = false
access_key_id = ${s3.value.accessKeyId}
secret_access_key = ${s3.value.secretAccessKey}
region = ${s3.value.region || 'us-east-1'}
`
  }
  if (cloud === 'azure') {
    return `[azure]
type = azureblob
account = ${azure.value.account}
key = ${azure.value.key}
`
  }
  if (cloud === 'gcs') {
    const json = gcs.value.serviceAccountJson.trim().replace(/\n/g, ' ')
    return `[gcs]
type = google cloud storage
service_account_credentials = ${json}
`
  }
  if (cloud === 'sftp') {
    const pass = sftp.value.pass.replace(/\\/g, '\\\\').replace(/"/g, '\\"')
    return `[sftp]
type = sftp
host = ${sftp.value.host}
user = ${sftp.value.user}
port = ${sftp.value.port || 22}
pass = ${pass}
`
  }
  return ''
}

function buildRcloneConfig(): string {
  const name = secretName.value.trim()
  const ns = props.namespace || 'default'
  const content = buildRcloneConfigContent()
  const indent = '    '
  const escaped = content.trim().replace(/\n/g, `\n${indent}`)
  return `apiVersion: v1
kind: Secret
metadata:
  name: ${name}
  namespace: ${ns}
type: Opaque
stringData:
  config: |
${indent}${escaped}
`
}

async function createSecretInCluster() {
  if (!canGenerate.value) return
  creating.value = true
  createError.value = ''
  try {
    const res = await fetch('/api/secrets', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: secretName.value.trim(),
        namespace: props.namespace || 'default',
        config: buildRcloneConfigContent(),
        credentialType: selectedCloud.value,
      }),
    })
    const data = await res.json().catch(() => ({}))
    if (!res.ok) {
      createError.value = data.error || res.statusText || 'Error al crear el Secret'
      return
    }
    useSecretNameAndClose()
  } catch (e) {
    createError.value = e instanceof Error ? e.message : 'Error de conexión. ¿Está corriendo la API?'
  } finally {
    creating.value = false
  }
}

function generateYaml() {
  if (!canGenerate.value) return
  generatedYaml.value = buildRcloneConfig()
}

function copyYaml() {
  if (!generatedYaml.value) return
  navigator.clipboard.writeText(generatedYaml.value).then(() => {
    alert('Copiado al portapapeles.')
  })
}

function downloadYaml() {
  if (!generatedYaml.value) return
  const blob = new Blob([generatedYaml.value], { type: 'application/yaml' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `leviathan-secret-${secretName.value.trim() || 'credentials'}.yaml`
  a.click()
  URL.revokeObjectURL(a.href)
}

function useSecretNameAndClose() {
  const name = secretName.value.trim()
  if (name) {
    emit('use-secret', name)
    emit('close')
  }
}
</script>
