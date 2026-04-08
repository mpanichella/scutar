<template>
  <div class="storage-config-form">
    <!-- Local -->
    <template v-if="type === 'local'">
      <div class="row g-3">
        <div class="col-12">
          <label class="form-label fw-semibold">Ruta en el pod</label>
          <input
            :value="(fields as LocalFields).path"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="/data o /var/backup"
            @input="updateLocal('path', ($event.target as HTMLInputElement).value)"
          />
          <div class="form-text">Ruta absoluta en el sistema de archivos del contenedor.</div>
        </div>
      </div>
    </template>

    <!-- S3 -->
    <template v-else-if="type === 's3'">
      <div class="row g-3">
        <div class="col-md-6">
          <label class="form-label fw-semibold">Bucket</label>
          <input
            :value="(fields as S3Fields).bucket"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="mi-bucket"
            @input="updateS3('bucket', ($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="col-md-6">
          <label class="form-label fw-semibold">Región</label>
          <input
            :value="(fields as S3Fields).region"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="us-east-1"
            @input="updateS3('region', ($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="col-12">
          <label class="form-label fw-semibold">Prefijo (ruta dentro del bucket)</label>
          <input
            :value="(fields as S3Fields).prefix"
            type="text"
            class="form-control font-monospace"
            placeholder="backups/app/ (opcional)"
            @input="updateS3('prefix', ($event.target as HTMLInputElement).value)"
          />
          <div class="form-text">Dejá vacío para usar la raíz del bucket.</div>
        </div>
      </div>
    </template>

    <!-- GCS -->
    <template v-else-if="type === 'gcs'">
      <div class="row g-3">
        <div class="col-12">
          <label class="form-label fw-semibold">Bucket</label>
          <input
            :value="(fields as GCSFields).bucket"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="mi-bucket-gcs"
            @input="updateGCS('bucket', ($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="col-12">
          <label class="form-label fw-semibold">Prefijo (ruta dentro del bucket)</label>
          <input
            :value="(fields as GCSFields).prefix"
            type="text"
            class="form-control font-monospace"
            placeholder="backups/ (opcional)"
            @input="updateGCS('prefix', ($event.target as HTMLInputElement).value)"
          />
        </div>
      </div>
    </template>

    <!-- Azure -->
    <template v-else-if="type === 'azure'">
      <div class="row g-3">
        <div class="col-12">
          <label class="form-label fw-semibold">Container</label>
          <input
            :value="(fields as AzureFields).container"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="nombre-del-container"
            @input="updateAzure('container', ($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="col-12">
          <label class="form-label fw-semibold">Prefijo (ruta dentro del container)</label>
          <input
            :value="(fields as AzureFields).prefix"
            type="text"
            class="form-control font-monospace"
            placeholder="backups/ (opcional)"
            @input="updateAzure('prefix', ($event.target as HTMLInputElement).value)"
          />
          <div class="form-text">La cuenta de almacenamiento y la clave se configuran en el Secret de credenciales.</div>
        </div>
      </div>
    </template>

    <!-- SFTP -->
    <template v-else-if="type === 'sftp'">
      <div class="row g-3">
        <div class="col-md-8">
          <label class="form-label fw-semibold">Host</label>
          <input
            :value="(fields as SFTPFields).host"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="sftp.ejemplo.com"
            @input="updateSFTP('host', ($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="col-md-4">
          <label class="form-label fw-semibold">Puerto</label>
          <input
            :value="(fields as SFTPFields).port"
            type="number"
            class="form-control form-control-lg"
            placeholder="22"
            min="1"
            max="65535"
            @input="updateSFTP('port', parseInt(($event.target as HTMLInputElement).value, 10) || 22)"
          />
        </div>
        <div class="col-12">
          <label class="form-label fw-semibold">Usuario</label>
          <input
            :value="(fields as SFTPFields).user"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="usuario"
            @input="updateSFTP('user', ($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="col-12">
          <label class="form-label fw-semibold">Ruta remota</label>
          <input
            :value="(fields as SFTPFields).path"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="/home/usuario/backups"
            @input="updateSFTP('path', ($event.target as HTMLInputElement).value)"
          />
          <div class="form-text">Ruta en el servidor SFTP. Host, usuario, puerto y contraseña (o clave SSH) se configuran en el paso Credenciales → "Configurar conexión a la nube"; acá solo la ruta remota. Si cargás un backup existente, completá de nuevo host/usuario si no los ves.</div>
        </div>
      </div>
    </template>

    <!-- Custom URI -->
    <template v-else-if="type === 'custom'">
      <div class="row g-3">
        <div class="col-12">
          <label class="form-label fw-semibold">URI completa</label>
          <input
            :value="(fields as CustomFields).uri"
            type="text"
            class="form-control form-control-lg font-monospace"
            placeholder="protocolo:parametros/ruta"
            @input="updateCustom('uri', ($event.target as HTMLInputElement).value)"
          />
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import {
  buildUri,
  parseUri,
  getDefaultFields,
  type StorageType,
  type LocalFields,
  type S3Fields,
  type GCSFields,
  type AzureFields,
  type SFTPFields,
  type CustomFields,
} from '../utils/storage-uri'

const props = withDefaults(
  defineProps<{
    modelValue: string
    type: StorageType
  }>(),
  { modelValue: '', type: 'local' }
)

const emit = defineEmits<{
  (e: 'update:modelValue', uri: string): void
  (e: 'sftp-credentials', data: { host: string; port: number; user: string; path: string; password: string }): void
}>()

const fields = ref<LocalFields | S3Fields | GCSFields | AzureFields | SFTPFields | CustomFields>(
  getDefaultFields(props.type)
)
const sftpPassword = ref('')

function syncFromUri(uri: string, forceType?: StorageType) {
  const parsed = parseUri(uri)
  const useType = forceType ?? parsed.type
  if (useType === parsed.type) {
    fields.value = parsed.fields
  } else {
    fields.value = getDefaultFields(useType)
  }
}

function emitUri() {
  const uri = buildUri(props.type, fields.value)
  emit('update:modelValue', uri as string)
}

function updateLocal<K extends keyof LocalFields>(key: K, value: LocalFields[K]) {
  const f = fields.value as LocalFields
  f[key] = value
  emitUri()
}
function updateS3<K extends keyof S3Fields>(key: K, value: S3Fields[K]) {
  const f = fields.value as S3Fields
  f[key] = value
  emitUri()
}
function updateGCS<K extends keyof GCSFields>(key: K, value: GCSFields[K]) {
  const f = fields.value as GCSFields
  f[key] = value
  emitUri()
}
function updateAzure<K extends keyof AzureFields>(key: K, value: AzureFields[K]) {
  const f = fields.value as AzureFields
  f[key] = value
  emitUri()
}
function updateSFTP<K extends keyof SFTPFields>(key: K, value: SFTPFields[K]) {
  const f = fields.value as SFTPFields
  f[key] = value
  emitUri()
  emitSftpCredentials()
}

function onSftpPasswordInput(ev: Event) {
  sftpPassword.value = (ev.target as HTMLInputElement).value
  emitSftpCredentials()
}

function emitSftpCredentials() {
  if (props.type !== 'sftp') return
  const f = fields.value as SFTPFields
  emit('sftp-credentials', {
    host: f.host,
    port: f.port || 22,
    user: f.user,
    path: f.path || '/',
    password: sftpPassword.value,
  })
}
function updateCustom<K extends keyof CustomFields>(key: K, value: CustomFields[K]) {
  const f = fields.value as CustomFields
  f[key] = value
  emitUri()
}

watch(
  () => [props.modelValue, props.type] as const,
  ([uri, type]) => {
    syncFromUri(uri || '', type)
    if (type === 'sftp') emitSftpCredentials()
  },
  { immediate: true }
)

watch(
  () => props.type,
  (newType, oldType) => {
    if (newType !== oldType) {
      if (newType !== 'sftp') sftpPassword.value = ''
      const parsed = parseUri(props.modelValue)
      if (parsed.type === newType) {
        fields.value = parsed.fields
      } else {
        fields.value = getDefaultFields(newType)
        emitUri()
      }
      if (newType === 'sftp') emitSftpCredentials()
    }
  }
)
</script>
