import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solid()],
  server: {
    port: 3000,
  },
  resolve: {
    conditions: ['development', 'browser'],
  },
  build: {
    target: 'esnext',
    polyfillDynamicImport: false,
  }
});
