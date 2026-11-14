(() => {
  const key='koda.appearance';
  const root=document.documentElement;
  const control=document.getElementById('site-appearance');
  if (!control) return;
  function apply(value) {
    const mode=['light','dark'].includes(value) ? value : 'system';
    if(mode==='system') root.removeAttribute('data-appearance');
    else root.dataset.appearance=mode;
    control.value=mode;
  }
  function read() {try{return localStorage.getItem(key)}catch{return null}}
  apply(read());
  root.classList.add('appearance-ready');
  control.addEventListener('change',() => {
    apply(control.value);
    try {
      if(control.value==='system') localStorage.removeItem(key);
      else localStorage.setItem(key,control.value);
    } catch { /* The choice still works on the current page. */ }
  });
  window.addEventListener('storage',event => {
    if(event.key===key || event.key===null) apply(read());
  });
})();
