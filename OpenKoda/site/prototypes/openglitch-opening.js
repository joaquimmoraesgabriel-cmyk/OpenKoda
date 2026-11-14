// Isolated prototype. No production preference keys or application state.
const appearanceControls = document.querySelectorAll('[data-appearance-select]');
appearanceControls.forEach(control => {
  control.disabled = false;
  control.addEventListener('change', () => {
    const value = control.value;
    if (value === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = value;
    appearanceControls.forEach(other => { other.value = value; });
    document.dispatchEvent(new Event('prototype-appearance-change'));
  });
});

const mobileMenu = document.querySelector('.mobile-menu');
mobileMenu.addEventListener('keydown', event => {
  if (event.key === 'Escape' && mobileMenu.open) {
    mobileMenu.open = false;
    mobileMenu.querySelector('summary').focus();
  }
});

// The imported engine mounts itself. It has no continuous ambient animation:
// pointer changes settle, then its coordinator stops requesting frames.
// The independent controls above remain usable if the renderer cannot load.
import('../dist/matter-field.js').then(({renderMatterSvg}) => {
  let cleanup = mountFallback(renderMatterSvg);
  window.addEventListener('pagehide', () => cleanup());
  window.addEventListener('pageshow', event => {
    if (event.persisted) cleanup = mountFallback(renderMatterSvg);
  });
}).catch(() => {
  // Pre-generated desktop/mobile SVGs remain visible with no module or Canvas.
});

function mountFallback(renderMatterSvg) {
  const section = document.querySelector('[data-matter-section]');
  const canvas = section.querySelector('canvas');
  const fallback = section.querySelector('[data-matter-fallback-runtime]');
  const systemTheme = matchMedia('(prefers-color-scheme: dark)');
  const coarsePointer = matchMedia('(pointer: coarse)');
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
  let pending = 0, disposed = false;

  function clearFallback() {
    fallback.replaceChildren();
    fallback.hidden = true;
    delete section.dataset.fallbackReady;
  }

  function updateFallback() {
    pending = 0;
    if (disposed) return;
    if (section.dataset.matterReady === 'true' && !reducedMotion.matches) {
      clearFallback();
      return;
    }
    const {width, height} = canvas.getBoundingClientRect();
    if (!(width >= 2 && height >= 2)) return;
    try {
      const style = getComputedStyle(section);
      const source = renderMatterSvg({
        width, height, kind: 'aperture', coarse: coarsePointer.matches,
        density: navigator.deviceMemory && navigator.deviceMemory <= 4 ? 0.75 : 1,
        ink: style.getPropertyValue('--matter-ink').trim(),
        accent: style.getPropertyValue('--matter-accent').trim()
      });
      const svg = new DOMParser().parseFromString(source, 'image/svg+xml').documentElement;
      svg.setAttribute('preserveAspectRatio', 'none');
      // Equal-opacity squares do not overlap. Merging their exact rectangles
      // into subpaths preserves compositing and avoids one DOM node per mark.
      // The SVG is mounted only in reduced motion or renderer failure states.
      for (const group of svg.children) {
        const paths = new Map();
        for (const rect of group.children) {
          const opacity = rect.getAttribute('opacity');
          const x = rect.getAttribute('x'), y = rect.getAttribute('y');
          const w = rect.getAttribute('width'), h = rect.getAttribute('height');
          const d = `M${x} ${y}h${w}v${h}h-${w}Z`;
          paths.set(opacity, (paths.get(opacity) || '') + d);
        }
        const nodes = [...paths].map(([opacity, d]) => {
          const path = svg.ownerDocument.createElementNS(svg.namespaceURI, 'path');
          path.setAttribute('d', d);
          path.setAttribute('opacity', opacity);
          return path;
        });
        group.replaceChildren(...nodes);
      }
      fallback.replaceChildren(svg);
      fallback.hidden = false;
      section.dataset.fallbackReady = 'true';
    } catch {
      clearFallback();
      // Generated files remain visible; no blob/data URLs or CSP changes.
    }
  }

  function scheduleFallback() {
    if (disposed) return;
    clearTimeout(pending);
    pending = setTimeout(updateFallback, 80);
  }
  const resize = typeof ResizeObserver === 'function' ? new ResizeObserver(scheduleFallback) : null;
  const state = typeof MutationObserver === 'function' ? new MutationObserver(scheduleFallback) : null;
  resize?.observe(canvas);
  state?.observe(section, {attributes: true, attributeFilter: ['data-matter-ready']});
  window.addEventListener('resize', scheduleFallback, {passive: true});
  systemTheme.addEventListener('change', scheduleFallback);
  coarsePointer.addEventListener('change', scheduleFallback);
  reducedMotion.addEventListener('change', scheduleFallback);
  document.addEventListener('prototype-appearance-change', scheduleFallback);
  updateFallback();

  return () => {
    if (disposed) return;
    disposed = true;
    clearTimeout(pending);
    resize?.disconnect();
    state?.disconnect();
    window.removeEventListener('resize', scheduleFallback);
    systemTheme.removeEventListener('change', scheduleFallback);
    coarsePointer.removeEventListener('change', scheduleFallback);
    reducedMotion.removeEventListener('change', scheduleFallback);
    document.removeEventListener('prototype-appearance-change', scheduleFallback);
    clearFallback();
  };
}
