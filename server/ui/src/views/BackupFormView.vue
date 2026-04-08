<template>
  <div>
    <div class="page-header mb-4">
      <h1>{{ isEdit ? 'Editar backup' : 'Nuevo backup' }}</h1>
      <p class="lead">
        Configurá tipo, origen, destino, credenciales y programación. Leviathan generará el recurso que el operator convertirá en un CronJob o Job.
      </p>
    </div>
    <BackupForm
      v-if="!loading"
      :initial="initial"
      :existing-name="isEdit ? (route.params.name as string) : undefined"
    />
    <div v-else class="text-muted py-4">Cargando…</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import BackupForm from '../components/BackupForm.vue'
import type { BackupFormState } from '../types/backup'

const route = useRoute()
const loading = ref(false)
const initial = ref<Partial<BackupFormState>>({})

const isEdit = computed(() => !!route.params.name)

onMounted(async () => {
  if (!isEdit.value) return
  const name = route.params.name as string
  const ns = (route.query.namespace as string) || 'default'
  loading.value = true
  try {
    const res = await fetch(`/api/backups/${encodeURIComponent(name)}?namespace=${encodeURIComponent(ns)}`)
    if (res.ok) {
      const data = await res.json()
      initial.value = {
        name: data.metadata?.name,
        namespace: data.metadata?.namespace || ns,
        spec: data.spec,
      }
    }
  } finally {
    loading.value = false
  }
})
</script>
