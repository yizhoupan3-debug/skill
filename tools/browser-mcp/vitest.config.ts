import { defineConfig } from 'vitest/config';

/**
 * Creates the Vitest configuration for the package.
 * @returns Vitest configuration.
 */
export default defineConfig({
  test: {
    include: ['tests/**/*.test.ts'],
    exclude: ['dist/**', 'node_modules/**'],
  },
});
