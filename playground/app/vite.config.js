import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import svgr from "vite-plugin-svgr";
import { createMonacoEditorPlugin } from '../lib/vite.plugin.js'

export default defineConfig({
  base: '',
  resolve: {
    dedupe: ['react', 'react-dom'],
  },
  server: {
    fs: {
      allow: ['.', '../lib']
    }
  },
  plugins: [
    svgr({
      svgrOptions: { exportType: 'default', ref: true, svgo: false, titleProp: true },
      include: '**/*.svg',
    }),
    react(),
    createMonacoEditorPlugin(),
  ],
})
