<template>
  <div class="flex min-h-screen bg-gray-100">
    <!-- Sidebar: solo Tailwind, fixed a la izquierda -->
    <aside
      class="fixed left-0 top-0 z-50 flex h-full flex-col overflow-y-auto bg-slate-800 text-white transition-[width] duration-200"
      :class="sidebarCollapsed ? 'w-[4.5rem]' : 'w-64'"
    >
      <div class="flex h-14 shrink-0 items-center justify-between border-b border-white/10 px-4">
        <router-link to="/" class="flex min-w-0 items-center gap-2 overflow-hidden">
          <img src="/LOGO_ISO.png" alt="Scutar" class="h-8 w-auto object-contain" />
        </router-link>
        <button
          type="button"
          class="rounded p-1.5 text-white/70 hover:bg-white/10 hover:text-white"
          aria-label="Contraer menú"
          @click="sidebarCollapsed = !sidebarCollapsed"
        >
          <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7" /></svg>
        </button>
      </div>
      <router-link
        to="/backups/new"
        class="mx-3 mt-4 flex items-center gap-3 rounded-lg bg-indigo-600 px-3 py-2.5 text-sm font-medium text-white hover:bg-indigo-700"
      >
        <span class="shrink-0 text-lg">+</span>
        <span class="truncate" :class="{ 'sr-only': sidebarCollapsed }">Nuevo backup</span>
      </router-link>
      <nav class="mt-4 flex-1 space-y-0.5 px-3 pb-4">
        <p class="px-3 py-1 text-xs font-semibold uppercase tracking-wider text-slate-400" :class="{ 'sr-only': sidebarCollapsed }">Menú</p>
        <ul class="space-y-0.5">
          <li>
            <router-link
              to="/"
              class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm text-slate-200 hover:bg-white/10 hover:text-white"
              active-class="!bg-indigo-600/30 !text-white"
              exact-active-class="!bg-indigo-600/30 !text-white"
            >
              <span class="shrink-0">⌂</span>
              <span class="truncate" :class="{ 'sr-only': sidebarCollapsed }">Dashboard</span>
            </router-link>
          </li>
          <li>
            <router-link
              to="/backups"
              class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm text-slate-200 hover:bg-white/10 hover:text-white"
              active-class="!bg-indigo-600/30 !text-white"
            >
              <span class="shrink-0">📦</span>
              <span class="truncate" :class="{ 'sr-only': sidebarCollapsed }">Backups</span>
            </router-link>
          </li>
          <li>
            <router-link
              to="/scheduled"
              class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm text-slate-200 hover:bg-white/10 hover:text-white"
              active-class="!bg-indigo-600/30 !text-white"
            >
              <span class="shrink-0">🕐</span>
              <span class="truncate" :class="{ 'sr-only': sidebarCollapsed }">Programados</span>
            </router-link>
          </li>
          <li>
            <router-link
              to="/credentials"
              class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm text-slate-200 hover:bg-white/10 hover:text-white"
              active-class="!bg-indigo-600/30 !text-white"
            >
              <span class="shrink-0">🔐</span>
              <span class="truncate" :class="{ 'sr-only': sidebarCollapsed }">Credenciales</span>
            </router-link>
          </li>
          <li>
            <router-link
              to="/connections"
              class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm text-slate-200 hover:bg-white/10 hover:text-white"
              active-class="!bg-indigo-600/30 !text-white"
            >
              <span class="shrink-0">🔌</span>
              <span class="truncate" :class="{ 'sr-only': sidebarCollapsed }">Conexiones</span>
            </router-link>
          </li>
          <li>
            <router-link
              to="/dr"
              class="flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm text-slate-200 hover:bg-white/10 hover:text-white"
              active-class="!bg-indigo-600/30 !text-white"
            >
              <span class="shrink-0">🔄</span>
              <span class="truncate" :class="{ 'sr-only': sidebarCollapsed }">DR</span>
            </router-link>
          </li>
        </ul>
      </nav>
    </aside>
    <!-- Contenido principal: margen = ancho del sidebar -->
    <main
      class="flex min-h-screen flex-1 flex-col transition-[margin] duration-200"
      :class="sidebarCollapsed ? 'ml-[4.5rem]' : 'ml-64'"
    >
      <header class="shrink-0 border-b border-gray-200 bg-white px-6 py-4">
        <nav aria-label="breadcrumb">
          <ol class="flex flex-wrap items-center gap-1 text-sm text-gray-500">
            <li v-for="(crumb, i) in breadcrumbs" :key="i" class="flex items-center gap-1">
              <template v-if="i > 0"><span class="text-gray-300">/</span></template>
              <router-link v-if="i < breadcrumbs.length - 1" :to="crumb.to" class="text-indigo-600 hover:underline">{{ crumb.label }}</router-link>
              <span v-else class="text-gray-700">{{ crumb.label }}</span>
            </li>
          </ol>
        </nav>
      </header>
      <div class="flex-1 p-6">
        <router-view />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()
const sidebarCollapsed = ref(false)

const breadcrumbs = computed(() => {
  const path = route.path
  const list: { label: string; to?: string }[] = [{ label: 'Scutar', to: '/' }]
  if (path === '/') {
    list.push({ label: 'Dashboard' })
    return list
  }
  if (path.startsWith('/backups/new')) {
    list.push({ label: 'Backups', to: '/backups' }, { label: 'Nuevo backup' })
    return list
  }
  if (path.startsWith('/backups/edit')) {
    const name = route.params.name as string
    list.push({ label: 'Backups', to: '/backups' }, { label: name ? `Editar: ${name}` : 'Editar' })
    return list
  }
  if (path === '/backups') list.push({ label: 'Backups' })
  else if (path === '/scheduled') list.push({ label: 'Programados' })
  else if (path === '/credentials') list.push({ label: 'Credenciales' })
  else if (path === '/connections') list.push({ label: 'Conexiones' })
  else if (path === '/dr') list.push({ label: 'DR' })
  else list.push({ label: (route.meta?.title as string) || path })
  return list
})
</script>
