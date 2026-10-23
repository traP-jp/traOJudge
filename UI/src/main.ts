import './assets/main.css'

import { VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import { createApp } from 'vue'

import { configureApi } from '@/api/config'
import { queryClient } from '@/api/queryClient'

import App from './App.vue'
import router, { watchSessionRouting } from './router'

const app = createApp(App)
const pinia = createPinia()

configureApi()
app.use(pinia)
app.use(VueQueryPlugin, { queryClient })
app.use(router)
watchSessionRouting()
app.mount('#app')
