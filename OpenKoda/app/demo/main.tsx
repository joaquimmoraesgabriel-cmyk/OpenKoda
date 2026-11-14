import './demo.css';

// Install before importing App: some real desktop modules read preferences at load.
function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return { get length() { return values.size; }, clear: () => values.clear(),
    getItem: key => values.get(String(key)) ?? null, key: index => [...values.keys()][index] ?? null,
    removeItem: key => { values.delete(String(key)); }, setItem: (key, value) => { values.set(String(key), String(value)); } };
}
for (const key of ['localStorage', 'sessionStorage']) Object.defineProperty(window, key, { value: memoryStorage(), configurable: false });
const notice = document.getElementById('demo-notice')!;
let noticeTimer: ReturnType<typeof setTimeout>;
export function announce(message: string) {
  notice.textContent = message;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => { notice.textContent = 'Demonstração · dados fictícios · nenhuma chamada de IA'; }, 6500);
}
// No real uploads or external navigation from the embedded sample.
document.addEventListener('click', event => {
  const target = event.target as HTMLElement;
  const file = target.closest('input[type=file]') as HTMLInputElement | null;
  if (file) {
    event.preventDefault();
    const data = new DataTransfer();
    if(file.accept.includes('image')) {
      const sample=document.createElement('canvas');sample.width=240;sample.height=160;
      const brush=sample.getContext('2d')!;
      brush.fillStyle='#e8eddf';brush.fillRect(0,0,240,160);
      brush.fillStyle='#cee0ad';brush.fillRect(0,90,240,70);
      brush.fillStyle='#94b98a';brush.beginPath();brush.arc(200,160,100,0,Math.PI*2);brush.fill();
      const png=Uint8Array.from(atob(sample.toDataURL('image/png').split(',')[1]),character=>character.charCodeAt(0));
      data.items.add(new File([png],'imagem-exemplo.png',{type:'image/png'}));
    } else data.items.add(new File(['// Exemplo local da demonstração\nexport const project = "Loja Aurora";'], 'exemplo.ts', { type: 'text/plain' }));
    file.files = data.files;
    file.dispatchEvent(new Event('change', { bubbles: true }));
    announce('Arquivo fictício selecionado. Nenhum arquivo do seu dispositivo foi lido.');
  }
  const link = target.closest('a[href]');
  if (link && !link.getAttribute('href')?.startsWith('#')) {
    event.preventDefault(); announce('Este link abre um serviço no aplicativo instalado. A demonstração continua nesta janela.');
  }
}, true);
// Fail closed even if a future desktop component adds its own network call.
window.fetch = async () => { throw new Error('A demonstração não faz requisições de rede.'); };
window.open = () => { announce('A demonstração não abre serviços externos.'); return null; };
void import('./mount');
