import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

// The browser asset is an ES module without imposing a package type on the site.
const orbitSource=await readFile(new URL('../dist/orbit-field.js',import.meta.url),'utf8');
const orbitUrl='data:text/javascript;base64,'+Buffer.from(orbitSource).toString('base64');
const {orbitVisibility}=await import(orbitUrl);
const source = (await readFile(new URL('../dist/matter-field.js', import.meta.url), 'utf8')).replace('./orbit-field.js',orbitUrl);
const {createMatterField, evolveMatterField, stepMatterField, resetMatterField, paintMatterField, renderMatterSvg, mountMatterFields, matterLimits, matterPauseKey} = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));

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
        if(scene==='constellation')assert.ok(field.compact ? field.y[i] > height * 0.36 : field.x[i] > width * 0.32);
        else if(orbitVisibility(field,field.x[i],field.y[i])===0)assert.equal(field.coverage[i],0,'the central reading area stays empty');
      }
      assert.ok(Math.max(...field.x) > width * 0.95, 'the field reaches the right section edge');
    }
  }
  const aperture = createMatterField('aperture', 1440, 800);
  const constellation = createMatterField('constellation', 1440, 800);
  assert.notDeepEqual(aperture.x, constellation.x);
  const narrow = createMatterField('aperture', 390, 800);
  assert.ok(Math.min(...narrow.x)<0&&Math.max(...narrow.x)>390,'portrait orbit reaches both side edges');
  assert.ok(createMatterField('aperture', 1440, 800, {coarse: true}).count <= matterLimits.compact);
  assert.throws(() => createMatterField('noise', 800, 600), TypeError);
  assert.throws(() => createMatterField('aperture', 0, 600), RangeError);
  assert.throws(() => createMatterField('aperture', Infinity, 600), RangeError);
});

test('local orbital gravity attracts clockwise and dissolves, then restores the field even after a stationary pointer', () => {
  const field = createMatterField('aperture', 1440, 800);
  const buffers = {x: field.x, y: field.y, heat: field.heat};
  const pointer = {active: true, x: field.x[150], y: field.y[150], radius: 100};
  for (let i = 0; i < 60; i++) stepMatterField(field, pointer, 1 / 60);
  assert.ok(field.heat[150] > 0.5);
  assert.ok([...field.heat].some(value => value === 0));
  assert.ok([...field.heat].every(value => value >= 0 && value <= 1));
  assert.ok([...field.dx, ...field.dy].some(value => Math.abs(value)>1));
  assert.ok(field.dissolve[150]>.5);
  for(let i=0;i<360;i++)stepMatterField(field,pointer,1/60);
  assert.ok([...field.dx,...field.dy,...field.heat,...field.dissolve].every(value=>value===0),'idle pointer releases the live field');
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

test('the living volume evolves at 0, 3 and 8 seconds without input, with bounded buffers and no drift', () => {
  for (const scene of ['aperture', 'constellation']) {
    for (const [width, height] of [[1440, 800], [390, 844]]) {
      const field = createMatterField(scene, width, height), anchors = [...field.x];
      const buffers = [field.flowX, field.flowY, field.coverage];
      const at = () => [...field.flowX, ...field.flowY, ...field.coverage];
      const initial = at();
      for (let frame = 0; frame < 180; frame++) evolveMatterField(field, 1 / 60);
      const three = at();
      for (let frame = 0; frame < 300; frame++) evolveMatterField(field, 1 / 60);
      const eight = at();
      assert.notDeepEqual(initial, three); assert.notDeepEqual(three, eight);
      assert.ok([...field.flowX, ...field.flowY].some(value => Math.abs(value) > 4));
      for (let i = 0; i < field.count; i++) {
        if(scene==='constellation'){
          assert.ok(Math.hypot(field.flowX[i], field.flowY[i]) <= matterLimits.ambientDisplacement);
          assert.ok(field.coverage[i] >= 0.79 && field.coverage[i] <= 1.21);
        }else{
          assert.ok(Math.hypot(field.flowX[i],field.flowY[i])<Math.hypot(width,height)*2.5);
          assert.ok(field.coverage[i]>=0&&field.coverage[i]<=1);
          const x=field.x[i]+field.flowX[i],y=field.y[i]+field.flowY[i];
          if(orbitVisibility(field,x,y)===0)assert.equal(field.coverage[i],0);
        }
      }
      assert.deepEqual([...field.x], anchors);
      assert.equal(field.flowX, buffers[0]); assert.equal(field.flowY, buffers[1]); assert.equal(field.coverage, buffers[2]);
      const frozen = at(); resetMatterField(field); assert.deepEqual(at(), frozen, 'resetting input never resets the living base');
    }
  }
});

test('leaving a stationary pointer restores the current living volume, not its initial frame', () => {
  const field = createMatterField('constellation', 1440, 800);
  for (let tick = 0; tick < 120; tick++) {
    evolveMatterField(field, 1 / 60);
    stepMatterField(field, {active: true, x: field.x[200], y: field.y[200]}, 1 / 60);
  }
  assert.ok(Math.abs(field.dx[200]) + Math.abs(field.dy[200]) > 1);
  const interactingTime = field.time;
  for (let tick = 0; tick < 300; tick++) { evolveMatterField(field, 1 / 60); stepMatterField(field, {active: false}, 1 / 60); }
  assert.ok(field.time > interactingTime + 4.9);
  assert.ok([...field.flowX].some(value => Math.abs(value) > 4));
  assert.ok([...field.dx, ...field.dy, ...field.heat].every(value => value === 0));
  const same = createMatterField('constellation', 1440, 800); same.time = field.time; evolveMatterField(same, 0);
  assert.deepEqual(field.flowX, same.flowX); assert.deepEqual(field.flowY, same.flowY);
});

test('autonomous time is frame-rate independent and a long suspension is clamped', () => {
  const at60 = createMatterField('aperture', 1440, 800), at120 = createMatterField('aperture', 1440, 800);
  for (let i = 0; i < 180; i++) evolveMatterField(at60, 1 / 60);
  for (let i = 0; i < 360; i++) evolveMatterField(at120, 1 / 120);
  for (let i = 0; i < at60.count; i++) assert.ok(Math.abs(at60.flowX[i] - at120.flowX[i]) < 0.0001);
  const previous = at60.time; evolveMatterField(at60, 30);
  assert.ok(at60.time - previous <= matterLimits.maxDelta + 0.000001);
});

test('the ellipse turns clockwise, spirals inward and fades before birth-cycle wrapping',()=>{
 const field=createMatterField('aperture',1440,800),i=0;
 const coordinate=()=>{const x=(field.x[i]+field.flowX[i]-field.width*.5)/field.radiusX,y=(field.y[i]+field.flowY[i]-field.height*.5)/field.radiusY;return {angle:Math.atan2(y,x),radius:Math.hypot(x,y)};};
 const start=coordinate();assert.equal(field.coverage[i],0);
 for(let n=0;n<180;n++)evolveMatterField(field,1/60);
 const end=coordinate(),turn=(end.angle-start.angle+Math.PI*2)%(Math.PI*2);
 assert.ok(turn>.6&&turn<.75,'clockwise motion is perceptible in three seconds');
 assert.ok(end.radius<start.radius,'orbital material moves inward');
 assert.ok(field.coverage[i]>0,'birth fades in');
 field.time=(1-.00001)/.021;evolveMatterField(field,0);assert.ok(field.coverage[i]<.00001);
 field.time=(1+.00001)/.021;evolveMatterField(field,0);assert.ok(field.coverage[i]<.00001);
});

test('orbital impulses stay local and bounded while rapid input cannot occupy the safe middle',()=>{
 const field=createMatterField('aperture',1024,640);
 for(let frame=0;frame<120;frame++){
  evolveMatterField(field,1/60);
  stepMatterField(field,{active:true,x:frame%2?80:940,y:100+frame*3},1/60);
  for(let i=0;i<field.count;i++)assert.ok(Math.hypot(field.dx[i],field.dy[i])<=field.maxDisplacement+.0001);
 }
 const context={clearRect(){},fillRect(x,y,size){if(this.globalAlpha>0)assert.ok(orbitVisibility(field,x+size/2,y+size/2)>0);}};
 paintMatterField(context,field,{ink:'#222222',accent:'#666666'});
 assert.ok([...field.dx].some(value=>value===0),'distant squares never receive a global drag');
});

function context() {
  return {clears: 0, squares: 0, dots: 0, transform: null, marks: [],
    clearRect() { this.clears++; this.marks = []; }, fillRect(...values) { this.squares++; if (this.marks.length < 12) this.marks.push(values); }, beginPath() {}, arc(...values) { this.dots++; if (this.marks.length < 12) this.marks.push(values); }, fill() {},
    setTransform(...args) { this.transform = args; }};
}

test('painting draws original marks without adding an overlay circle', () => {
  for (const scene of ['aperture', 'constellation']) {
    const field = createMatterField(scene, 1024, 640), ctx = context();
    stepMatterField(field, {active: true, x: field.x[100], y: field.y[100]}, 0.03);
    paintMatterField(ctx, field, {ink: '#141414', accent: '#7042ed'});
    assert.equal(ctx.clears, 1); assert.equal(ctx.globalAlpha, 1);
    assert.equal(scene === 'aperture' ? ctx.dots : ctx.squares, 0);
    assert.ok((scene === 'aperture' ? ctx.squares : ctx.dots) > 0);
  }
});

test('culling preserves every visible mark, alpha and paint order across evolving frames', () => {
  const palette = {ink: '#141414', accent: '#7042ed'};
  for (const scene of ['aperture', 'constellation']) {
    for (const [width, height] of [[1440, 800], [390, 844]]) {
      const field = createMatterField(scene, width, height);
      for (const time of [0, 3, 8]) {
        field.time = time; evolveMatterField(field, 0);
        stepMatterField(field, {active: true, x: width * .9, y: height * .5}, 1 / 60);
        const expected = []; let originalCalls = 0;
        // Reference the original two-pass painter, retaining only marks whose
        // bounds intersect the canvas. Compare double-precision values exactly.
        for (const accent of [false, true]) for (let i = 0; i < field.count; i++) {
          const strength = Math.max(field.tone[i], field.heat[i]);
          if (accent && strength < .008) continue;
          originalCalls++;
          const x = field.x[i] + field.flowX[i] + field.dx[i], y = field.y[i] + field.flowY[i] + field.dy[i];
          const size = field.size[i] * Math.sqrt(field.coverage[i]);
          const dots = scene === 'constellation', extent = dots ? size : size / 2;
          const visibility = dots ? 1 : orbitVisibility(field, x, y) * (1 - field.dissolve[i]);
          const base = Math.min(1, field.alpha[i] * field.coverage[i]);
          const alpha = accent ? base * strength * visibility : base * visibility;
          if (alpha <= 0 || size <= 0 || x + extent < 0 || x - extent > width || y + extent < 0 || y - extent > height) continue;
          expected.push({color: accent ? palette.accent : palette.ink, alpha,
            geometry: dots ? [x, y, size, 0, Math.PI * 2] : [x - size / 2, y - size / 2, size, size]});
        }
        const actual = [], ctx = {clearRect() {}, beginPath() {}, fill() {},
          fillRect(...geometry) { actual.push({color: this.fillStyle, alpha: this.globalAlpha, geometry}); },
          arc(...geometry) { actual.push({color: this.fillStyle, alpha: this.globalAlpha, geometry}); }};
        paintMatterField(ctx, field, palette);
        assert.deepEqual(actual, expected);
        assert.ok(actual.length < originalCalls, 'offscreen draws are removed without dropping visible marks');
      }
    }
  }
});

test('culling retains partially visible circles and squares at each canvas edge', () => {
  for (const scene of ['aperture', 'constellation']) {
    const field = createMatterField(scene, 100, 100);
    field.count = 8;
    field.x.set([-1, 101, 50, 50, -3, 103, 50, 50]);
    field.y.set([50, 50, -1, 101, 50, 50, -3, 103]);
    field.size.fill(scene === 'constellation' ? 2 : 4);
    field.alpha.fill(1); field.coverage.fill(1); field.tone.fill(0); field.heat.fill(0);
    field.flowX.fill(0); field.flowY.fill(0); field.dx.fill(0); field.dy.fill(0);
    if (scene === 'aperture') { field.safeX = field.safeY = 1; field.dissolve.fill(0); }
    const ctx = context(); paintMatterField(ctx, field, {ink: '#111111', accent: '#222222'});
    assert.equal(scene === 'constellation' ? ctx.dots : ctx.squares, 4);
    field.alpha.fill(0);
    const empty = context(); paintMatterField(empty, field, {ink: '#111111', accent: '#222222'});
    assert.equal(empty.squares + empty.dots, 0, 'reused buffers do not replay stale marks');
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

function browserFixture({reduced = false, coarse = false, noContext = false, paused = false, controls=true} = {}) {
  const win = new Events(), doc = new Events(), motions = new Map(), callbacks = new Map();
  const stored = new Map([[matterPauseKey, String(paused)]]);
  win.localStorage = {getItem: key => stored.get(key), setItem: (key, value) => stored.set(key, value)};
  let id = 0, time = 0;
  win.devicePixelRatio = 4; win.navigator = {deviceMemory: 8}; win.performance = {now: () => time};
  win.matchMedia = query => { if (!motions.has(query)) motions.set(query, Object.assign(new Events(), {matches: query.includes('reduced') ? reduced : coarse})); return motions.get(query); };
  win.requestAnimationFrame = callback => { callbacks.set(++id, callback); return id; };
  win.cancelAnimationFrame = handle => callbacks.delete(handle);
  win.getComputedStyle = () => ({getPropertyValue: name => name === '--matter-ink' ? '#18171b' : '#7042ed'});
  const observers = [];
  win.IntersectionObserver = class { constructor(callback) { this.kind = 'intersection'; this.callback = callback; this.targets = []; observers.push(this); } observe(node) { this.targets.push(node); } disconnect() { this.targets.length = 0; } };
  win.ResizeObserver = class { constructor(callback) { this.callback = callback; this.targets = []; observers.push(this); } observe(node) { this.targets.push(node); } disconnect() { this.targets.length = 0; } };
  win.MutationObserver = class { constructor(callback) { this.callback = callback; this.targets = []; observers.push(this); } observe(node) { this.targets.push(node); } disconnect() { this.targets.length = 0; } };
  doc.defaultView = win; doc.hidden = false; doc.documentElement = {};
  const sections = ['aperture', 'constellation'].map(scene => {
    const section = Object.assign(new Events(), {dataset: {}});
    const button = Object.assign(new Events(), {hidden: true, attributes: {}, setAttribute(name, value) { this.attributes[name] = value; }});
    section.button = button; section.querySelector = () => controls?button:null;
    const canvas = Object.assign(new Events(), {dataset: {matterField: scene}, width: 0, height: 0,
      getBoundingClientRect: () => ({width: 1024, height: 640, left: 0, top: 0}),
      closest: () => section, setAttribute() {}, getContext: () => noContext ? null : canvas.context, context: context()});
    section.canvas = canvas; return section;
  });
  doc.querySelectorAll = selector => selector==='[data-matter-pause]'?(controls?sections.map(section=>section.button):[]):sections.map(section => section.canvas);
  const flush = () => { time += 1000 / 60; const pending = [...callbacks.values()]; callbacks.clear(); for (const callback of pending) callback(time); };
  return {win, doc, sections, observers, motions, callbacks, flush, stored};
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
  assert.equal(env.callbacks.size, 1, 'one autonomous coordinator runs while a scene is visible');
  const firstFrame = env.sections[0].canvas.context.marks;
  for (let i = 0; i < 10; i++) env.flush();
  assert.notDeepEqual(env.sections[0].canvas.context.marks, firstFrame);
  env.sections[1].emit('pointermove', {pointerType: 'mouse', clientX: 900, clientY: 390});
  assert.equal(env.callbacks.size, 1); env.flush(); assert.equal(env.callbacks.size, 1);
  env.doc.hidden = true; env.doc.emit('visibilitychange'); assert.equal(env.callbacks.size, 0);
  env.doc.hidden = false; env.doc.emit('visibilitychange'); assert.equal(env.callbacks.size, 1); env.flush();
  env.observers[0].callback(env.sections.map(target => ({target, isIntersecting: false})));
  env.sections[1].emit('pointermove', {pointerType: 'mouse', clientX: 900, clientY: 390});
  assert.equal(env.callbacks.size, 0);
  cleanup(); cleanup();
  for (const section of env.sections) { assert.equal(section.count(), 0); assert.equal(section.canvas.count(), 0); assert.equal(section.button.count(), 0); assert.equal(section.dataset.matterReady, undefined); }
  for (const observer of env.observers) assert.equal(observer.targets.length, 0);
  for (const motion of env.motions.values()) assert.equal(motion.count(), 0);
  assert.equal(env.win.count(), 0); assert.equal(env.doc.count(), 0);
});

test('public scenes without pause controls ignore a legacy saved pause and keep automatic reduced motion',()=>{
 const env=browserFixture({paused:true,controls:false}),cleanup=mountMatterFields(env.doc);
 env.observers[0].callback(env.sections.map(target=>({target,isIntersecting:true})));env.flush();
 assert.equal(env.callbacks.size,1);
 env.win.emit('storage',{key:matterPauseKey,newValue:'true'});env.flush();assert.equal(env.callbacks.size,1);
 const reduce=env.motions.get('(prefers-reduced-motion: reduce)');reduce.matches=true;reduce.emit('change');env.flush();assert.equal(env.callbacks.size,0);
 cleanup();
});

test('reduced motion freezes the field while touch keeps autonomous motion without hijacking gestures', () => {
  for (const mode of [{reduced: true}, {coarse: true}]) {
    const env = browserFixture(mode), cleanup = mountMatterFields(env.doc);
    env.observers[0].callback(env.sections.map(target => ({target, isIntersecting: true}))); env.flush();
    env.sections[0].emit('pointermove', {pointerType: 'mouse', clientX: 800, clientY: 400});
    env.sections[0].emit('pointermove', {pointerType: 'touch', clientX: 800, clientY: 400});
    assert.equal(env.callbacks.size, mode.reduced ? 0 : 1); assert.equal(env.sections[0].dataset.matterReady, 'true');
    assert.equal(env.sections[0].listeners.has('touchmove'), false);
    assert.equal(env.sections[0].listeners.has('wheel'), false);
    cleanup();
  }
});

test('manual pause freezes the current frame, syncs controls and persists across mounting', () => {
  const env = browserFixture(), cleanup = mountMatterFields(env.doc);
  env.observers[0].callback(env.sections.map(target => ({target, isIntersecting: true})));
  for (let i = 0; i < 30; i++) env.flush();
  const current = env.sections[0].canvas.context.marks;
  env.sections[0].button.emit('click'); env.flush();
  assert.equal(env.callbacks.size, 0); assert.deepEqual(env.sections[0].canvas.context.marks, current);
  assert.equal(env.stored.get(matterPauseKey), 'true');
  for (const section of env.sections) { assert.equal(section.button.attributes['aria-pressed'], 'true'); assert.equal(section.button.textContent, 'Retomar animação'); }
  const reduce = env.motions.get('(prefers-reduced-motion: reduce)');
  reduce.matches = true; reduce.emit('change'); env.flush();
  assert.equal(env.sections[0].button.disabled, true);
  reduce.matches = false; reduce.emit('change'); env.flush();
  assert.equal(env.callbacks.size, 0, 'system preference does not override the manual pause');
  cleanup();
  const remount = mountMatterFields(env.doc);
  const intersection = env.observers.findLast(observer => observer.kind === 'intersection' && observer.targets.includes(env.sections[0]));
  intersection.callback(env.sections.map(target => ({target, isIntersecting: true}))); env.flush();
  assert.equal(env.callbacks.size, 0, 'remount restores persisted pause');
  env.sections[1].button.emit('click'); env.flush();
  assert.equal(env.callbacks.size, 1); assert.equal(env.stored.get(matterPauseKey), 'false');
  remount();
});

test('cross-tab pause and blocked preference storage do not break the artwork', () => {
  const env = browserFixture(); env.win.localStorage = {getItem() { throw new Error('blocked'); }, setItem() { throw new Error('blocked'); }};
  const cleanup = mountMatterFields(env.doc);
  env.observers[0].callback(env.sections.map(target => ({target, isIntersecting: true}))); env.flush();
  env.win.emit('storage', {key: matterPauseKey, newValue: 'true'}); env.flush(); assert.equal(env.callbacks.size, 0);
  env.sections[0].button.emit('click'); env.flush(); assert.equal(env.callbacks.size, 1);
  cleanup();
});

test('ten mounts and cleanups never accumulate frames, listeners or observers', () => {
  const env = browserFixture();
  for (let cycle = 0; cycle < 10; cycle++) {
    const cleanup = mountMatterFields(env.doc);
    const observer = env.observers.findLast(item => item.kind === 'intersection' && item.targets.includes(env.sections[0]));
    observer.callback(env.sections.map(target => ({target, isIntersecting: true}))); env.flush();
    cleanup();
    assert.equal(env.callbacks.size, 0); assert.equal(env.win.count(), 0); assert.equal(env.doc.count(), 0);
    for (const section of env.sections) assert.equal(section.count() + section.canvas.count() + section.button.count(), 0);
    assert.ok(env.observers.every(item => item.targets.length === 0));
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
