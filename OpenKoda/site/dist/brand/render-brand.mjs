import { chromium } from '../../../app/node_modules/playwright/index.mjs';
import { fileURLToPath } from 'node:url';
const output = (name) => fileURLToPath(new URL(name, import.meta.url));
const browser = await chromium.launch({ channel: 'msedge', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 1000 }, deviceScaleFactor: 1 });
  await page.goto(new URL('preview.html', import.meta.url).href);
  await page.locator('img').evaluateAll((images) => Promise.all(images.map((image) => image.decode())));
  await page.screenshot({ path: output('preview.png'), fullPage: true });
  for (const size of [16, 24, 32]) {
    const name = size === 32 ? 'favicon.svg' : `favicon-${size}.svg`;
    await page.goto(new URL(name, import.meta.url).href);
    await page.setViewportSize({ width: size, height: size });
    await page.screenshot({ path: output(`favicon-${size}.png`) });
  }
  console.log('Rendered preview.png and exact-size favicon PNGs (16/24/32).');
} finally { await browser.close(); }
