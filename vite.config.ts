import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: '127.0.0.1', port: 1421, strictPort: true,
    // Cargo and native acceptance tests create many files unrelated to the UI.
    // Watching those trees delays even the first CSS transform on Windows.
    watch: { ignored: ['**/src-tauri/target/**', '**/artifacts/**', '**/docs/evidence/**'] },
  },
  build: { target: 'es2022' },
  test: { environment: 'jsdom', include: ['src/**/*.test.{ts,tsx}'], setupFiles: ['./src/test-setup.ts'] },
});
