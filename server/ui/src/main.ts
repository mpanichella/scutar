import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import '@tabler/core/dist/css/tabler.min.css'
import 'bootstrap' /* modales y dropdowns (data-bs-toggle, data-bs-dismiss) */
import './styles/app.css'

const app = createApp(App)
app.use(router)
app.mount('#app')
