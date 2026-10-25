import sharedConfig from '../.prettierrc.json' with { type: 'json' }

export default {
  ...sharedConfig,
  plugins: ['prettier-plugin-tailwindcss'],
  tailwindStylesheet: './src/assets/main.css'
}
