import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));
// The desktop entrypoint and its Tauri transport are never changed by this build.
export default defineConfig({
  root: here('./demo'),
  base: '/demo-app/',
  plugins: [react(), {
    name: 'demo-transport-only',
    enforce: 'pre',
    resolveId(source, importer) {
      if (importer && /\/src\//.test(importer.replaceAll('\\', '/')) && /^\.{1,2}\/api(?:\.ts)?$/.test(source)) return here('./demo/api.ts');
    },
  }],
  build: { outDir: here('../site/dist/demo-app'), emptyOutDir: true, sourcemap: false },
  server: { host: '127.0.0.1', port: 1426, strictPort: true },
  preview: { host: '127.0.0.1', port: 1427, strictPort: true },
});
