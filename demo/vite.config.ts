import path from 'node:path';
import { defineConfig } from 'vite';

export default defineConfig({
   resolve: {
      alias: {
         '@vault/sdk': path.resolve(__dirname, '../sdk/ts/src/index.ts'),
      },
   },
});
