import { Configuration, DefaultConfig } from '@/api/generated/runtime'

// ルートガードがAPIを呼ぶ前に接続先を設定する。
export const configureApi = (basePath = import.meta.env.VITE_API_BASE_URL): void => {
  DefaultConfig.config = new Configuration({
    basePath: (basePath || '/api').replace(/\/$/, ''),
    credentials: 'include'
  })
}
