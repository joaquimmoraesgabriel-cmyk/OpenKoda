// Original OpenGlitch compositions. Shapes are sampled once onto an ordered
// lattice; pointer input changes those same marks instead of regenerating noise.
const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
const TAU = Math.PI * 2;
const clamp = (value, low, high) => Math.min(high, Math.max(low, value));
const smooth = (low, high, value) => {
  const t = clamp((value - low) / (high - low), 0, 1);
  return t * t * (3 - 2 * t);
};
const finite = (value, fallback) => Number.isFinite(value) ? value : fallback;

export const matterLimits = Object.freeze({desktop: 2400, compact: 950, maxDpr: 1.75, maxPixels: 6000000, maxDimension: 8192, maxDelta: 0.05});

/** Deterministic composition in CSS pixels. Called on layout changes, not frames. */
export function createMatterField(scene, width, height, options = {}) {
  if (!['aperture', 'constellation'].includes(scene)) throw new TypeError('Unknown matter composition.');
  if (!(width > 0 && height > 0 && Number.isFinite(width) && Number.isFinite(height))) throw new RangeError('A positive finite field size is required.');
  const compact = width < 700 || height > width * 1.25;
  const limit = Math.max(160, Math.floor((compact || options.coarse ? matterLimits.compact : matterLimits.desktop) * clamp(finite(options.density, 1), 0.25, 1)));
  const spacing = Math.max(compact ? 9 : 10, Math.sqrt(width * height / (limit * 3)));
  const samples = [];
  const clusters = compact
    ? [[0.2, 0.7, 0.42, 0.2], [0.76, 0.89, 0.45, 0.24], [0.9, 0.52, 0.29, 0.2]]
    : [[0.63, 0.23, 0.19, 0.32], [0.88, 0.61, 0.25, 0.35], [0.59, 1.01, 0.23, 0.29]];
  let row = 0;
  for (let y = -spacing; y <= height + spacing; y += spacing, row++) {
    let column = 0;
    for (let x = -spacing; x <= width + spacing; x += spacing, column++) {
      // The quiet region belongs to the composition, not an opaque text overlay.
      const quiet = compact ? smooth(0.36, 0.62, y / height) : smooth(0.32, 0.57, x / width);
      if (!quiet) continue;
      let density, tone;
      if (scene === 'aperture') {
        const nx = (x / width - (compact ? 0.8 : 0.79)) / (compact ? 0.87 : 0.43);
        const ny = (y / height - (compact ? 0.76 : 0.53)) / (compact ? 0.42 : 0.76);
        const theta = Math.atan2(ny, nx), radius = Math.hypot(nx, ny);
        const folded = radius + Math.sin(theta * 3 + 0.6) * 0.085 + Math.cos(theta * 6) * 0.024;
        const opening = smooth(0.25, 0.78, Math.acos(Math.cos(theta - Math.PI)));
        const band = 1 - smooth(0.12, 0.33, Math.abs(folded - 0.92));
        const ribs = 0.66 + 0.34 * Math.pow(0.5 + 0.5 * Math.sin(folded * 35 - theta * 2), 2);
        density = band * opening * ribs * quiet;
        tone = smooth(0.82, 1.16, folded) * 0.14;
      } else {
        let core = 0, closest = 0;
        for (let k = 0; k < clusters.length; k++) {
          const [cx, cy, rx, ry] = clusters[k];
          const radius = Math.hypot((x / width - cx) / rx, (y / height - cy) / ry);
          const value = 1 - smooth(0.38, 1.12, radius);
          if (value > core) { core = value; closest = k; }
        }
        const seam = compact
          ? Math.abs(x / width - (0.42 + 0.22 * Math.sin(y / height * 6)))
          : Math.abs(y / height - (0.48 + 0.3 * Math.sin(x / width * 7)));
        const bridge = (1 - smooth(0.018, 0.065, seam)) * 0.3;
        density = Math.max(core, bridge) * quiet;
        tone = closest === 1 ? core * 0.2 : 0;
      }
      const threshold = (BAYER[((row & 3) << 2) + (column & 3)] + 0.5) / 16;
      if (density <= threshold * 0.94 || density < 0.055) continue;
      samples.push([x, y, spacing * (scene === 'aperture' ? 0.32 + 0.39 * density : 0.105 + 0.16 * density), 0.16 + 0.67 * density, tone]);
    }
  }
  const count = Math.min(limit, samples.length);
  const field = {scene, width, height, count, compact, maxDisplacement: Math.min(26, spacing * 1.7),
    x: new Float32Array(count), y: new Float32Array(count), size: new Float32Array(count), alpha: new Float32Array(count), tone: new Float32Array(count),
    heat: new Float32Array(count), dx: new Float32Array(count), dy: new Float32Array(count), vx: new Float32Array(count), vy: new Float32Array(count)};
  for (let i = 0; i < count; i++) {
    const sample = samples[Math.floor(i * samples.length / count)];
    field.x[i] = sample[0]; field.y[i] = sample[1]; field.size[i] = sample[2]; field.alpha[i] = sample[3]; field.tone[i] = sample[4];
  }
  return field;
}

export function resetMatterField(field) {
  for (const key of ['heat', 'dx', 'dy', 'vx', 'vy']) field[key].fill(0);
}

/** Build-time static counterpart; numeric geometry and hex colors only. */
export function renderMatterSvg({width, height, kind, ink = '#1a1920', accent = '#6f42e8', ...options}) {
  const field = createMatterField(kind, width, height, options);
  const color = (value, fallback) => /^#(?:[\da-f]{3}|[\da-f]{6}|[\da-f]{8})$/i.test(value) ? value : fallback;
  const n = value => Number(value.toFixed(3));
  const marks = accentPass => {
    const output = [];
    for (let i = 0; i < field.count; i++) {
      const alpha = field.alpha[i] * (accentPass ? field.tone[i] : 1);
      if (accentPass && field.tone[i] < 0.008) continue;
      output.push(kind === 'constellation'
        ? `<circle cx="${n(field.x[i])}" cy="${n(field.y[i])}" r="${n(field.size[i])}" opacity="${n(alpha)}"/>`
        : `<rect x="${n(field.x[i] - field.size[i] / 2)}" y="${n(field.y[i] - field.size[i] / 2)}" width="${n(field.size[i])}" height="${n(field.size[i])}" opacity="${n(alpha)}"/>`);
    }
    return output.join('');
  };
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${n(width)} ${n(height)}" aria-hidden="true" focusable="false"><g fill="${color(ink, '#1a1920')}">${marks(false)}</g><g fill="${color(accent, '#6f42e8')}">${marks(true)}</g></svg>`;
}

/** Updates existing buffers. Returns whether a transient still needs another frame. */
export function stepMatterField(field, pointer, elapsed) {
  const dt = clamp(finite(elapsed, 0), 0, matterLimits.maxDelta);
  if (!dt) return false;
  const active = !!pointer?.active && Number.isFinite(pointer.x) && Number.isFinite(pointer.y);
  const radius = clamp(finite(pointer?.radius, Math.min(field.width, field.height) * 0.23), 60, 220);
  const omega = 16, decay = Math.exp(-omega * dt);
  const heatEase = 1 - Math.exp(-(active ? 13 : 7) * dt);
  let unsettled = false;
  for (let i = 0; i < field.count; i++) {
    let influence = 0, targetX = 0, targetY = 0;
    if (active) {
      const px = field.x[i] - pointer.x, py = field.y[i] - pointer.y;
      const distance = Math.hypot(px, py);
      if (distance < radius) {
        influence = 1 - smooth(0, radius, distance);
        if (field.scene === 'constellation') {
          const magnitude = field.maxDisplacement * influence;
          const angle = distance < 0.001 ? (i % 16) / 16 * TAU : Math.atan2(py, px);
          targetX = Math.cos(angle) * magnitude; targetY = Math.sin(angle) * magnitude;
        }
      }
    }
    const oldHeat = field.heat[i];
    let nextHeat = oldHeat + (influence - oldHeat) * heatEase;
    if (nextHeat < 0.001 && !influence) nextHeat = 0;
    field.heat[i] = nextHeat;
    if (Math.abs(nextHeat - influence) > 0.002) unsettled = true;
    if (field.scene !== 'constellation') continue;
    // Exact critically damped spring integration for this frame's fixed target.
    const offsetX = field.dx[i] - targetX, offsetY = field.dy[i] - targetY;
    const springX = (field.vx[i] + omega * offsetX) * dt;
    const springY = (field.vy[i] + omega * offsetY) * dt;
    let dx = targetX + (offsetX + springX) * decay, dy = targetY + (offsetY + springY) * decay;
    let vx = (field.vx[i] - omega * springX) * decay, vy = (field.vy[i] - omega * springY) * decay;
    const displacement = Math.hypot(dx, dy);
    if (displacement > field.maxDisplacement) { const scale = field.maxDisplacement / displacement; dx *= scale; dy *= scale; vx *= scale; vy *= scale; }
    if (!influence && Math.abs(dx) + Math.abs(dy) + Math.abs(vx) + Math.abs(vy) < 0.015) dx = dy = vx = vy = 0;
    field.dx[i] = dx; field.dy[i] = dy; field.vx[i] = vx; field.vy[i] = vy;
    if (Math.abs(dx - targetX) + Math.abs(dy - targetY) > 0.015 || Math.abs(vx) + Math.abs(vy) > 0.035) unsettled = true;
  }
  return unsettled;
}

export function paintMatterField(context, field, palette) {
  context.clearRect(0, 0, field.width, field.height);
  const dots = field.scene === 'constellation';
  const mark = (x, y, size) => {
    if (dots) { context.beginPath(); context.arc(x, y, size, 0, TAU); context.fill(); }
    else context.fillRect(x - size / 2, y - size / 2, size, size);
  };
  // Two color passes keep color-state changes independent of the element count.
  context.fillStyle = palette.ink;
  for (let i = 0; i < field.count; i++) {
    context.globalAlpha = field.alpha[i];
    mark(field.x[i] + field.dx[i], field.y[i] + field.dy[i], field.size[i]);
  }
  context.fillStyle = palette.accent;
  for (let i = 0; i < field.count; i++) {
    const strength = Math.max(field.tone[i], field.heat[i]);
    if (strength < 0.008) continue;
    context.globalAlpha = field.alpha[i] * strength;
    mark(field.x[i] + field.dx[i], field.y[i] + field.dy[i], field.size[i]);
  }
  context.globalAlpha = 1;
}

const mounted = new WeakSet();

/** One coordinator per mounted root. Returns an idempotent resource cleanup. */
export function mountMatterFields(root = document) {
  const doc = root.ownerDocument || root;
  const win = doc.defaultView;
  if (!win?.requestAnimationFrame) return () => {};
  const reduce = win.matchMedia('(prefers-reduced-motion: reduce)');
  const coarse = win.matchMedia('(pointer: coarse)');
  const items = [];
  let frame = 0, previous = 0, disposed = false;
  const staticMode = () => reduce.matches || coarse.matches;
  function schedule() {
    if (!disposed && !doc.hidden && !frame && items.some(item => item.visible && (item.dirty || item.animating || item.resize))) frame = win.requestAnimationFrame(draw);
  }
  function fail(item) {
    item.failed = true; item.dirty = item.animating = item.resize = false;
    delete item.canvas.dataset.matterReady; delete item.section.dataset.matterReady;
  }
  function draw(now) {
    frame = 0;
    const dt = previous ? Math.min((now - previous) / 1000, matterLimits.maxDelta) : 1 / 60;
    previous = now;
    for (const item of items) {
      if (item.failed || !item.visible || !(item.dirty || item.animating || item.resize)) continue;
      try {
        if (item.resize) {
          const rect = item.canvas.getBoundingClientRect();
          if (rect.width < 2 || rect.height < 2) { item.resize = false; continue; }
          const dpr = Math.min(Math.max(0.5, finite(win.devicePixelRatio, 1)), matterLimits.maxDpr,
            Math.sqrt(matterLimits.maxPixels / (rect.width * rect.height)), matterLimits.maxDimension / Math.max(rect.width, rect.height));
          item.canvas.width = Math.round(rect.width * dpr); item.canvas.height = Math.round(rect.height * dpr);
          item.context.setTransform(dpr, 0, 0, dpr, 0, 0);
          item.field = createMatterField(item.scene, rect.width, rect.height, {coarse: coarse.matches, density: item.density});
          const css = win.getComputedStyle(item.section);
          item.palette = {ink: css.getPropertyValue('--matter-ink').trim() || '#1a1920', accent: css.getPropertyValue('--matter-accent').trim() || '#6f42e8'};
          item.resize = false;
        }
        if (!item.field) continue;
        item.animating = !staticMode() && stepMatterField(item.field, item.pointer, dt);
        const start = win.performance.now();
        paintMatterField(item.context, item.field, item.palette);
        item.slow = win.performance.now() - start > 12 ? item.slow + 1 : Math.max(0, item.slow - 1);
        if (item.slow >= 10 && item.density > 0.4) { item.density *= 0.72; item.resize = true; item.slow = 0; }
        item.dirty = false;
        item.canvas.dataset.matterReady = 'true'; item.section.dataset.matterReady = 'true';
      } catch { fail(item); }
    }
    schedule();
  }
  const intersection = win.IntersectionObserver ? new win.IntersectionObserver(entries => {
    for (const entry of entries) {
      const item = items.find(value => value.section === entry.target);
      if (!item) continue;
      item.visible = entry.isIntersecting;
      if (!item.visible) { item.pointer.active = false; item.animating = false; if (item.field) resetMatterField(item.field); }
      else item.dirty = true;
    }
    previous = 0; schedule();
  }, {threshold: 0}) : null;
  const resize = win.ResizeObserver ? new win.ResizeObserver(entries => {
    for (const entry of entries) { const item = items.find(value => value.canvas === entry.target); if (item) item.resize = true; }
    schedule();
  }) : null;
  const theme = win.MutationObserver ? new win.MutationObserver(() => layout()) : null;
  const themeAttributes = {attributes: true, attributeFilter: ['data-appearance', 'data-theme', 'class', 'style']};
  if (doc.documentElement) theme?.observe(doc.documentElement, themeAttributes);
  for (const canvas of root.querySelectorAll('canvas[data-matter-field]')) {
    const section = canvas.closest('[data-matter-section]');
    const scene = canvas.dataset.matterField;
    if (!section || mounted.has(canvas) || !['aperture', 'constellation'].includes(scene)) continue;
    let context;
    try { context = canvas.getContext('2d', {alpha: true}); } catch { continue; }
    if (!context) continue;
    canvas.setAttribute('aria-hidden', 'true');
    const item = {canvas, section, scene, context, field: null, dirty: true, animating: false, resize: true, visible: !intersection, failed: false,
      pointer: {active: false, x: 0, y: 0}, density: win.navigator.deviceMemory && win.navigator.deviceMemory <= 4 ? 0.75 : 1, slow: 0};
    const move = event => {
      if (staticMode() || item.failed || !item.visible || event.pointerType === 'touch') return;
      const rect = canvas.getBoundingClientRect();
      item.pointer.x = event.clientX - rect.left; item.pointer.y = event.clientY - rect.top; item.pointer.active = true; item.dirty = true; schedule();
    };
    const leave = () => { item.pointer.active = false; item.dirty = true; schedule(); };
    section.addEventListener('pointermove', move, {passive: true}); section.addEventListener('pointerleave', leave, {passive: true});
    const lost = () => fail(item);
    const restored = () => { item.failed = false; item.resize = item.dirty = true; schedule(); };
    canvas.addEventListener('contextlost', lost); canvas.addEventListener('contextrestored', restored);
    item.remove = () => {
      section.removeEventListener('pointermove', move); section.removeEventListener('pointerleave', leave);
      canvas.removeEventListener('contextlost', lost); canvas.removeEventListener('contextrestored', restored);
    };
    items.push(item); mounted.add(canvas); intersection?.observe(section); resize?.observe(canvas);
    theme?.observe(section, themeAttributes);
  }
  function layout() { for (const item of items) item.resize = true; schedule(); }
  function mode() {
    for (const item of items) { item.pointer.active = false; if (item.field) resetMatterField(item.field); item.animating = false; item.resize = item.dirty = true; }
    previous = 0; schedule();
  }
  function visibility() {
    if (doc.hidden && frame) { win.cancelAnimationFrame(frame); frame = 0; }
    if (doc.hidden) for (const item of items) { item.pointer.active = false; if (item.field) resetMatterField(item.field); item.animating = false; item.dirty = true; }
    previous = 0; if (!doc.hidden) schedule();
  }
  function cleanup() {
    if (disposed) return; disposed = true;
    if (frame) win.cancelAnimationFrame(frame);
    intersection?.disconnect(); resize?.disconnect(); theme?.disconnect();
    for (const item of items) { item.remove(); mounted.delete(item.canvas); delete item.canvas.dataset.matterReady; delete item.section.dataset.matterReady; }
    reduce.removeEventListener('change', mode); coarse.removeEventListener('change', mode);
    doc.removeEventListener('visibilitychange', visibility); win.removeEventListener('resize', layout); win.removeEventListener('pagehide', cleanup);
  }
  reduce.addEventListener('change', mode); coarse.addEventListener('change', mode);
  doc.addEventListener('visibilitychange', visibility); win.addEventListener('resize', layout, {passive: true}); win.addEventListener('pagehide', cleanup, {once: true});
  schedule();
  return cleanup;
}

if (typeof document !== 'undefined') {
  let cleanup;
  const initialize = () => { cleanup?.(); cleanup = mountMatterFields(document); };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initialize, {once: true});
  else initialize();
  // pagehide releases resources; a BFCache return reconstructs the same artwork.
  window.addEventListener('pageshow', event => { if (event.persisted) initialize(); });
}
