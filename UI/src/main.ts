import { Configuration, DefaultConfig } from '@/api/generated/runtime'

import './assets/main.css'

import { createPinia } from 'pinia'
import { createApp } from 'vue'
import App from './App.vue'
import router from './router'

DefaultConfig.config = new Configuration({
  basePath: import.meta.env.VITE_API_BASE_URL || '/api/v1',
  credentials: 'include'
})

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(router)
app.mount('#app')
