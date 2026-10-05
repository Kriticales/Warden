import tailwindcss from '@tailwindcss/vite';
import { tanstackRouter } from '@tanstack/router-plugin/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Porta fixa: o tauri.conf.json aponta o devUrl para ela.
const DEV_PORT = 1420;

export default defineConfig({
  // O plugin do roteador vem antes do React (rotas por arquivo; routeTree.gen.ts gerado).
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    // Tailwind CSS 4, configurado por CSS em src/styles/app.css (HANDOFF §1).
    tailwindcss(),
  ],
  clearScreen: false,
  server: {
    port: DEV_PORT,
    strictPort: true,
    host: '127.0.0.1',
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_'],
  build: {
    target: 'es2023',
    sourcemap: true,
  },
});
