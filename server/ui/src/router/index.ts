import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    { path: '/', name: 'Dashboard', component: () => import('../views/DashboardView.vue'), meta: { title: 'Dashboard' } },
    { path: '/backups', name: 'Backups', component: () => import('../views/BackupsView.vue'), meta: { title: 'Backups' } },
    { path: '/backups/new', name: 'NewBackup', component: () => import('../views/BackupFormView.vue'), meta: { title: 'Nuevo backup' } },
    { path: '/backups/edit/:name', name: 'EditBackup', component: () => import('../views/BackupFormView.vue'), meta: { title: 'Editar backup' } },
    { path: '/scheduled', name: 'Scheduled', component: () => import('../views/ScheduledView.vue'), meta: { title: 'Programados' } },
    { path: '/credentials', name: 'Credentials', component: () => import('../views/CredentialsView.vue'), meta: { title: 'Credenciales' } },
    { path: '/dr', name: 'DR', component: () => import('../views/DRView.vue'), meta: { title: 'DR' } },
  ],
})

router.afterEach((to) => {
  document.title = to.meta.title ? `${to.meta.title} – Leviathan` : 'Leviathan'
})

export default router
