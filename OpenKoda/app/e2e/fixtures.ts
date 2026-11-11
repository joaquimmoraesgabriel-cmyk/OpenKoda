import { test as base, expect } from '@playwright/test';
import { installDataFixture } from './data-fixture';

/** Each test gets its own browser context and storage, never a shared seed user. */
export const test = base.extend<{ diagnostics: void }>({
  diagnostics: [async ({ page }, use, testInfo) => {
    await installDataFixture(page);
    const consoleMessages: string[] = [];
    const pageErrors: string[] = [];
    const requests: string[] = [];
    page.on('console', message => consoleMessages.push(`${message.type()}: ${message.text()}`));
    page.on('pageerror', error => pageErrors.push(error.message));
    page.on('requestfailed', request => requests.push(`${request.method()} ${request.url()} ${request.failure()?.errorText}`));
    page.on('response', response => {
      if (response.status() >= 400) requests.push(`${response.status()} ${response.url()}`);
    });
    await use();
    const keyWarnings=consoleMessages.filter(message=>/Encountered two children with the same key|Each child in a list should have a unique/.test(message));
    if (testInfo.status !== testInfo.expectedStatus || pageErrors.length || keyWarnings.length) {
      await testInfo.attach('browser-diagnostics', {
        body: JSON.stringify({ consoleMessages, pageErrors, requests }, null, 2),
        contentType: 'application/json',
      });
    }
    expect(pageErrors, 'No unhandled runtime exceptions').toEqual([]);
    expect(keyWarnings, 'React component identity remains unambiguous').toEqual([]);
  }, { auto: true }],
});

export { expect };
