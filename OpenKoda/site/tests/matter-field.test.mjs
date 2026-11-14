import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

// The browser asset is an ES module without imposing a package type on the site.
const source = await readFile(new URL('../dist/matter-field.js', import.meta.url), 'utf8');
const {createMatterField, stepMatterField, resetMatterField, paintMatterField, renderMatterSvg, mountMatterFields, matterLimits} = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));

test('the two compositions are deterministic, distinct, bounded and recomposed for portrait', () => {
  for (const scene of ['aperture', 'constellation']) {
    for (const [width, height] of [[1440, 800], [1024, 680], [768, 1000], [390, 800], [800, 390]]) {
      const field = createMatterField(scene, width, height);
      const duplicate = createMatterField(scene, width, height);
      assert.deepEqual(field, duplicate);
      assert.ok(field.count > 50);
      assert.ok(field.count <= (field.compact ? matterLimits.compact : matterLimits.desktop));
      for (let i = 0; i < field.count; i++) {
        assert.ok(Number.isFinite(field.x[i] + field.y[i] + field.size[i]));
        assert.ok(field.size[i] > 0 && field.size[i] < 40);
        assert.ok(field.alpha[i] >= 0 && field.alpha[i] <= 1);
        assert.ok(field.compact ? field.y[i] > height * 0.36 : field.x[i] > width * 0.32);
      }
      assert.ok(Math.max(...field.x) > width * 0.95, 'the field reaches the right section edge');
    }
  }
  const aperture = createMatterField('aperture', 1440, 800);
  const constellation = createMatterField('constellation', 1440, 800);
  assert.notDeepEqual(aperture.x, constellation.x);
  const narrow = createMatterField('aperture', 390, 800);
  assert.ok(Math.min(...narrow.y) > 800 * 0.36, 'portrait has its own upper reading region');
  assert.ok(createMatterField('aperture', 1440, 800, {coarse: true}).count <= matterLimits.compact);
  assert.throws(() => createMatterField('noise', 800, 600), TypeError);
  assert.throws(() => createMatterField('aperture', 0, 600), RangeError);
  assert.throws(() => createMatterField('aperture', Infinity, 600), RangeError);
});

test('color belongs to the same square modules and decays to rest without allocating buffers', () => {
  const field = createMatterField('aperture', 1440, 800);
  const buffers = {x: field.x, y: field.y, heat: field.heat};
  const pointer = {active: true, x: field.x[150], y: field.y[150], radius: 100};
  for (let i = 0; i < 60; i++) stepMatterField(field, pointer, 1 / 60);
  assert.ok(field.heat[150] > 0.99);
  assert.ok([...field.heat].some(value => value === 0));
  assert.ok([...field.heat].every(value => value >= 0 && value <= 1));
  assert.ok([...field.dx, ...field.dy].every(value => value === 0));
  let moving = true;
  for (let i = 0; i < 240; i++) moving = stepMatterField(field, {active: false}, 1 / 60);
  assert.equal(moving, false);
  assert.ok([...field.heat].every(value => value === 0));
  assert.equal(field.x, buffers.x); assert.equal(field.y, buffers.y); assert.equal(field.heat, buffers.heat);
});

test('elastic points remain bounded during rapid reversals, then return to the exact lattice', () => {
  const field = createMatterField('constellation', 1440, 800);
  const restX = [...field.x], restY = [...field.y];
  for (let frame = 0; frame < 180; frame++) {
    const i = 300 + frame % 200;
    stepMatterField(field, {active: true, x: field.x[i] + (frame % 2 ? 8 : -8), y: field.y[i]}, frame % 3 ? 1 / 60 : 1 / 15);
    for (let j = 0; j < field.count; j++) assert.ok(Math.hypot(field.dx[j], field.dy[j]) <= field.maxDisplacement + 0.0001);
  }
  assert.ok([...field.dx, ...field.dy].some(value => Math.abs(value) > 1));
  for (let frame = 0; frame < 300; frame++) stepMatterField(field, {active: false}, 1 / 60);
  for (const key of ['dx', 'dy', 'vx', 'vy', 'heat']) assert.ok([...field[key]].every(value => value === 0), key);
  assert.deepEqual([...field.x], restX); assert.deepEqual([...field.y], restY);
});

test('a paused tab cannot introduce a large delta jump, and reset clears only interaction state', () => {
  const a = createMatterField('constellation', 1024, 640), b = createMatterField('constellation', 1024, 640);
  const pointer = {active: true, x: a.x[100], y: a.y[100]};
  stepMatterField(a, pointer, 30); stepMatterField(b, pointer, matterLimits.maxDelta);
  assert.deepEqual(a, b);
  const rest = [...a.x]; resetMatterField(a);
  assert.deepEqual([...a.x], rest);
  for (const key of ['dx', 'dy', 'vx', 'vy', 'heat']) assert.ok([...a[key]].every(value => value === 0));
  assert.equal(stepMatterField(a, pointer, NaN), false);
});

function context() {
  return {clears: 0, squares: 0, dots: 0, transform: null,
    clearRect() { this.clears++; }, fillRect() { this.squares++; }, beginPath() {}, arc() { this.dots++; }, fill() {},
    setTransform(...args) { this.transform = args; }};
}

test('painting draws original marks without adding an overlay circle', () => {
  for (const scene of ['aperture', 'constellation']) {
    const field = createMatterField(scene, 1024, 640), ctx = context();
    stepMatterField(field, {active: true, x: field.x[100], y: field.y[100]}, 0.03);
    paintMatterField(ctx, field, {ink: '#141414', accent: '#7042ed'});
    assert.equal(ctx.clears, 1); assert.equal(ctx.globalAlpha, 1);
    assert.equal(scene === 'aperture' ? ctx.dots : ctx.squares, 0);
    assert.ok((scene === 'aperture' ? ctx.squares : ctx.dots) >= field.count);
  }
});

test('static SVG uses the same finite composition and cannot interpolate markup through colors', () => {
  for (const kind of ['aperture', 'constellation']) {
    const options = {kind, width: 1440, height: 800, ink: '#121212', accent: '#7540e9'};
    const svg = renderMatterSvg(options), field = createMatterField(kind, options.width, options.height);
    assert.equal(svg, renderMatterSvg(options));
    assert.match(svg, /viewBox="0 0 1440 800"/);
    assert.match(svg, /aria-hidden="true" focusable="false"/);
    assert.ok((svg.match(kind === 'aperture' ? /<rect /g : /<circle /g) || []).length >= field.count);
    assert.doesNotMatch(svg, /NaN|Infinity|undefined/);
    assert.doesNotMatch(renderMatterSvg({...options, ink: '"><script>alert(1)</script>'}), /script|alert/);
  }
});

class Events {
  listeners = new Map();
  addEventListener(name, handler) { if (!this.listeners.has(name)) this.listeners.set(name, new Set()); this.listeners.get(name).add(handler); }
  removeEventListener(name, handler) { this.listeners.get(name)?.delete(handler); }
  emit(name, event = {}) { for (const handler of this.listeners.get(name) || []) handler(event); }
  count() { return [...this.listeners.values()].reduce((sum, handlers) => sum + handlers.size, 0); }
}

function browserFixture({reduced = false, coarse = false, noContext = false} = {}) {
  const win = new Events(), doc = new Events(), motions = new Map(), callbacks = new Map();
  let id = 0, time = 0;
  win.devicePixelRatio = 4; win.navigator = {deviceMemory: 8}; win.performance = {now: () => time};
  win.matchMedia = query => { if (!motions.has(query)) motions.set(query, Object.assign(new Events(), {matches: query.includes('reduced') ? reduced : coarse})); return motions.get(query); };
  win.requestAnimationFrame = callback => { callbacks.set(++id, callback); return id; };
  win.cancelAnimationFrame = handle => callbacks.delete(handle);
  win.getComputedStyle = () => ({getPropertyValue: name => name === '--matter-ink' ? '#18171b' : '#7042ed'});
  const observers = [];
  win.IntersectionObserver = class { constructor(callback) { this.callback = callback; this.targets = []; observers.push(this); } observe(node) { this.targets.push(node); } disconnect() { this.targets.length = 0; } };
  win.ResizeObserver = class { constructor(callback) { this.callback = callback; this.targets = []; observers.push(this); } observe(node) { this.targets.push(node); } disconnect() { this.targets.length = 0; } };
  win.MutationObserver = class { constructor(callback) { this.callback = callback; this.targets = []; observers.push(this); } observe(node) { this.targets.push(node); } disconnect() { this.targets.length = 0; } };
  doc.defaultView = win; doc.hidden = false; doc.documentElement = {};
  const sections = ['aperture', 'constellation'].map(scene => {
    const section = Object.assign(new Events(), {dataset: {}});
    const canvas = Object.assign(new Events(), {dataset: {matterField: scene}, width: 0, height: 0,
      getBoundingClientRect: () => ({width: 1024, height: 640, left: 0, top: 0}),
      closest: () => section, setAttribute() {}, getContext: () => noContext ? null : canvas.context, context: context()});
    section.canvas = canvas; return section;
  });
  doc.querySelectorAll = () => sections.map(section => section.canvas);
  const flush = () => { time += 1000 / 60; const pending = [...callbacks.values()]; callbacks.clear(); for (const callback of pending) callback(time); };
  return {win, doc, sections, observers, motions, callbacks, flush};
}

test('one coordinated frame serves visible sections; hidden/offscreen work pauses and cleanup removes resources', () => {
  const env = browserFixture(), cleanup = mountMatterFields(env.doc);
  assert.equal(env.callbacks.size, 0, 'wait for intersection before rendering');
  env.observers[0].callback(env.sections.map(target => ({target, isIntersecting: true})));
  assert.equal(env.callbacks.size, 1);
  assert.equal(env.sections[0].dataset.matterReady, undefined);
  env.flush();
  for (const section of env.sections) {
    assert.equal(section.dataset.matterReady, 'true');
    assert.equal(section.canvas.width, Math.round(1024 * matterLimits.maxDpr));
  }
  assert.equal(env.callbacks.size, 0, 'no ambient battery loop');
  env.sections[1].emit('pointermove', {pointerType: 'mouse', clientX: 900, clientY: 390});
  assert.equal(env.callbacks.size, 1); env.flush(); assert.equal(env.callbacks.size, 1);
  env.doc.hidden = true; env.doc.emit('visibilitychange'); assert.equal(env.callbacks.size, 0);
  env.doc.hidden = false; env.doc.emit('visibilitychange'); assert.equal(env.callbacks.size, 1); env.flush();
  env.observers[0].callback(env.sections.map(target => ({target, isIntersecting: false})));
  env.sections[1].emit('pointermove', {pointerType: 'mouse', clientX: 900, clientY: 390});
  assert.equal(env.callbacks.size, 0);
  cleanup(); cleanup();
  for (const section of env.sections) { assert.equal(section.count(), 0); assert.equal(section.canvas.count(), 0); assert.equal(section.dataset.matterReady, undefined); }
  for (const observer of env.observers) assert.equal(observer.targets.length, 0);
  for (const motion of env.motions.values()) assert.equal(motion.count(), 0);
  assert.equal(env.win.count(), 0); assert.equal(env.doc.count(), 0);
});

test('reduced motion and touch keep a complete static field without hijacking gestures', () => {
  for (const mode of [{reduced: true}, {coarse: true}]) {
    const env = browserFixture(mode), cleanup = mountMatterFields(env.doc);
    env.observers[0].callback(env.sections.map(target => ({target, isIntersecting: true}))); env.flush();
    env.sections[0].emit('pointermove', {pointerType: 'mouse', clientX: 800, clientY: 400});
    env.sections[0].emit('pointermove', {pointerType: 'touch', clientX: 800, clientY: 400});
    assert.equal(env.callbacks.size, 0); assert.equal(env.sections[0].dataset.matterReady, 'true');
    assert.equal(env.sections[0].listeners.has('touchmove'), false);
    assert.equal(env.sections[0].listeners.has('wheel'), false);
    cleanup();
  }
});

test('missing or failed canvas leaves the static fallback visible', () => {
  const unavailable = browserFixture({noContext: true}), cleanupUnavailable = mountMatterFields(unavailable.doc);
  assert.equal(unavailable.callbacks.size, 0);
  assert.equal(unavailable.sections[0].dataset.matterReady, undefined); cleanupUnavailable();
  const broken = browserFixture(); broken.sections[0].canvas.context.clearRect = () => { throw new Error('Renderer unavailable'); };
  const cleanupBroken = mountMatterFields(broken.doc);
  broken.observers[0].callback(broken.sections.map(target => ({target, isIntersecting: true}))); broken.flush();
  assert.equal(broken.sections[0].dataset.matterReady, undefined);
  assert.equal(broken.sections[1].dataset.matterReady, 'true'); cleanupBroken();
});

test('theme, motion, layout and context changes work without reloading', () => {
  const env = browserFixture(), cleanup = mountMatterFields(env.doc);
  env.observers[0].callback(env.sections.map(target => ({target, isIntersecting: true}))); env.flush();
  env.win.getComputedStyle = () => ({getPropertyValue: name => name === '--matter-ink' ? '#f2f1f0' : '#9876ff'});
  env.observers[2].callback([]); env.flush();
  assert.equal(env.sections[0].canvas.context.fillStyle, '#9876ff');
  const preference = env.motions.get('(prefers-reduced-motion: reduce)');
  preference.matches = true; preference.emit('change'); env.flush();
  assert.equal(env.callbacks.size, 0);
  const canvas = env.sections[1].canvas;
  canvas.getBoundingClientRect = () => ({width: 4000, height: 2500, left: 0, top: 0});
  env.observers[1].callback([{target: canvas}]); env.flush();
  assert.ok(canvas.width * canvas.height <= matterLimits.maxPixels + 5000);
  canvas.emit('contextlost'); assert.equal(canvas.dataset.matterReady, undefined);
  canvas.emit('contextrestored'); env.flush(); assert.equal(canvas.dataset.matterReady, 'true');
  env.win.emit('pagehide'); assert.equal(env.callbacks.size, 0); assert.equal(canvas.count(), 0);
  cleanup();
});
