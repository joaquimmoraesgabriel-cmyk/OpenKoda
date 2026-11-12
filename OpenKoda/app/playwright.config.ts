import { defineConfig } from '@playwright/test';

// Coverage for the desktop source edition. The 0.1.19 product and its complete multiengine
// acceptance suite can be reproduced together at Git commit 01c1960.
const historicalSpecs = [
  'accessibility-layout', 'ai-local-closure', 'ai-request-ledger', 'assessment-comparison',
  'combined-consumers', 'combined', 'http-validation', 'investigation',
  'kali-removal', 'kali-scan', 'network', 'operation-proposal',
  'project-chat', 'project-desk', 'project-launch', 'project-navigation',
  'project-picker', 'remediation-json', 'remediation-reconciliation', 'scope',
  'single-create', 'source-analysis', 'source-capture', 'workflow-backends',
  'workflow', 'workspace',
].map(name => '**/' + name + '.spec.ts');

const mixedSpecs = [
  'ai-budget', 'data-policy', 'foundation', 'native-contract',
  'panel-scroll', 'retention',
].map(name => '**/' + name + '.spec.ts');

export default defineConfig({
  projects: [
    { name: 'Edição de código', testIgnore: [...historicalSpecs, ...mixedSpecs] },
    { name: 'Recursos preservados', testMatch: mixedSpecs, grep: /@source-edition/ },
  ],
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: process.env.CI ? 2 : 3,
  timeout: 30_000,
  expect: { timeout: 7_000 },
  outputDir: 'test-results',
  reporter: [['list'], ['html', { open: 'never' }], ['json', { outputFile: 'test-results/results.json' }]],
  use: {
    baseURL: 'http://127.0.0.1:1420',
    channel: 'msedge',
    viewport: { width: 1440, height: 900 },
    locale: 'pt-BR',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    actionTimeout: 7_000,
  },
  webServer: {
    command: 'npm run dev',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
