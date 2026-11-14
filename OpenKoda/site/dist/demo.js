// The demo cannot access the parent: the iframe has an opaque sandbox origin.
for (const root of document.querySelectorAll('[data-koda-demo]')) {
  const frame = root.querySelector('iframe');
  const status = root.querySelector('.kd-status');
  let restarting = false;
  frame.addEventListener('load', () => {
    root.dataset.initialized = 'true';
    if (restarting) status.textContent = 'Exemplos restaurados.';
    restarting = false;
  });
  root.querySelector('[data-demo-reset]').addEventListener('click', () => {
    restarting = true;
    status.textContent = 'Reiniciando demonstração…';
    frame.src = '/demo-app/';
  });
}
