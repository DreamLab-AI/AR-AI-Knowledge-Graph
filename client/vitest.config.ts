import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    // Force NODE_ENV=test regardless of the ambient shell env: some dev
    // containers export NODE_ENV=production globally, which makes React
    // resolve its production build — that build strips `React.act`, which
    // @testing-library/react's `render()` requires (react-dom/test-utils.act
    // delegates to it). Without this, any test using RTL's `render` fails
    // with "React.act is not a function".
    env: { NODE_ENV: 'test' },
    setupFiles: ['./src/setupTests.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules', 'dist', '**/*.integration.*'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.d.ts',
        'src/tests/**',
        'src/**/__tests__/**',
        'src/__mocks__/**',
        'src/index.tsx',
        'src/vite-env.d.ts',
        'src/setupTests.ts',
      ],
      thresholds: {
        global: {
          branches: 60,
          functions: 60,
          lines: 60,
          statements: 60,
        },
      },
    },
    testTimeout: 10000,
    // Mock Three.js and WebGL
    deps: {
      inline: ['three', '@react-three/fiber', '@react-three/drei', 'troika-three-text'],
    },
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
});
