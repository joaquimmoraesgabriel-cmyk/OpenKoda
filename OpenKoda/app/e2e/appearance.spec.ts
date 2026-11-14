import { test, expect } from './fixtures';
import { appearanceStorageKey } from '../src/appearance';
import { emptySnapshot, previewStorageKey } from '../src/preview';
import type { Page } from '@playwright/test';

async function appearance(page: Page) {
  await page.getByRole('button', {name:'Configurações', exact:true}).click();
  await page.getByRole('button', {name:'Aparência', exact:true}).click();
}

test('Geral compartilha o perfil, a busca encontra categorias sem acentos e fontes se aplicam em outras páginas', async({page})=>{
  await page.goto('/');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByLabel('Nome de exibição').fill('Usuária de teste');
  await page.getByRole('button',{name:'Salvar nome'}).click();
  await page.getByLabel('Atalho de envio').selectOption('enter');
  const search=page.getByRole('searchbox',{name:'Pesquisar configurações'});
  await search.fill('aparencia');
  await expect(page.getByRole('button',{name:'Geral',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Aparência',exact:true}).click();
  await page.getByLabel('Fonte de interface',{exact:true}).fill('Arial');
  await page.getByLabel('Tamanho do texto',{exact:true}).selectOption('18');
  await page.getByLabel('Tamanho do código',{exact:true}).selectOption('15');
  await page.getByRole('combobox',{name:'Navegação principal',exact:true}).selectOption('icons');
  await page.getByLabel('Avaliações e achados',{exact:true}).selectOption('line');
  await page.getByRole('radio',{name:'Névoa',exact:true}).check();
  await page.getByRole('button',{name:'Salvar alterações'}).click();
  await expect(page.locator('html')).toHaveAttribute('data-wallpaper','mist');
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Usuária de teste'})).toBeVisible();
  expect(await page.getByRole('heading',{name:'Perfil',exact:true}).evaluate(el=>getComputedStyle(el).fontFamily)).toContain('Arial');
  await expect(page.locator('html')).toHaveAttribute('data-sidebar-style','icons');
  await page.reload();
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await expect(page.getByLabel('Atalho de envio')).toHaveValue('enter');
  await expect(page.locator('html')).toHaveCSS('font-size','18px');
  expect(await page.locator('html').evaluate(el=>getComputedStyle(el).getPropertyValue('--code-font-size').trim())).toBe('15px');
  await expect(page.locator('html')).toHaveAttribute('data-list-style','line');
});

test('aparência é aplicada no dispositivo, mantém o formato anterior dos dados e sobrevive à reabertura', async ({page}) => {
  await page.goto('/');
  const open = () => appearance(page);
  await open();
  await expect(page.getByRole('heading', {name:'Aparência', exact:true})).toBeVisible();
  const scale = page.getByLabel('Tamanho da interface', {exact:true});
  const compact = page.getByRole('radio', {name:'Compacta'});
  const dark = page.getByRole('radio', {name:'Escuro'});
  const save = page.getByRole('button', {name:'Salvar alterações'});
  await dark.check();
  await scale.selectOption('125');
  await compact.check();
  await page.getByLabel('Movimento', {exact:true}).selectOption('reduced');
  await page.getByRole('button', {name:'Descartar', exact:true}).click();
  await expect(save).toBeDisabled();
  await expect(scale).toHaveValue('100');
  await expect(compact).not.toBeChecked();
  await expect(dark).not.toBeChecked();
  await dark.check();
  await scale.selectOption('125');
  await compact.check();
  await page.getByLabel('Movimento', {exact:true}).selectOption('reduced');
  await save.click();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(page.locator('html')).toHaveAttribute('data-motion', 'reduced');
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).zoom)).toBe('1.25');
  const oldFormat = await page.evaluate(() => JSON.parse(localStorage.getItem('openkoda.preview.v1')!).settings);
  expect(oldFormat.theme).toBe('dark');
  expect(oldFormat).not.toHaveProperty('interfaceScale');
  expect(oldFormat).not.toHaveProperty('density');
  expect(oldFormat).not.toHaveProperty('motion');
  await page.reload();
  await open();
  await expect(scale).toHaveValue('125');
  await expect(dark).toBeChecked();
  await expect(compact).toBeChecked();
  await expect(save).toBeDisabled();
  await page.setViewportSize({width:390, height:800});
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
  const profile = page.getByRole('button', {name:'Perfil', exact:true});
  await expect(profile).toBeInViewport({ratio:0.9});
});

test('falha ao salvar tema preserva as escolhas anteriores e mantém o rascunho para tentar de novo', async ({page}) => {
  await page.addInitScript(() => {
    const write = Storage.prototype.setItem;
    Storage.prototype.setItem = function(key, value) {
      if (key === 'openkoda.preview.v1') throw new Error('Disco de teste indisponível');
      write.call(this, key, value);
    };
  });
  await page.goto('/');
  await appearance(page);
  await page.getByRole('radio', {name:'Escuro'}).check();
  await page.getByRole('radio', {name:'Compacta'}).check();
  await page.getByRole('button', {name:'Salvar alterações'}).click();
  await expect(page.locator('.settings-actions [role=alert]')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('html')).toHaveAttribute('data-density', 'comfortable');
  expect(await page.evaluate(key => localStorage.getItem(key), appearanceStorageKey)).toBeNull();
  await expect(page.getByRole('radio', {name:'Compacta'})).toBeChecked();
  await expect(page.getByRole('button', {name:'Salvar alterações'})).toBeEnabled();
});

test('armazenamento visual indisponível conserva tema, oferece descarte e permite salvar tema sozinho', async ({page}) => {
  await page.addInitScript(key => {
    const write = Storage.prototype.setItem;
    Storage.prototype.setItem = function(k, value) {
      if (k === key) throw new Error('Quota de teste');
      write.call(this, k, value);
    };
  }, appearanceStorageKey);
  await page.goto('/');
  await appearance(page);
  await page.getByRole('radio', {name:'Escuro'}).check();
  await page.getByRole('radio', {name:'Compacta'}).check();
  await page.getByRole('button', {name:'Salvar alterações'}).click();
  await expect(page.getByRole('alert')).toContainText('não permitiu salvar');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.getByRole('button', {name:'Descartar',exact:true}).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.getByRole('radio', {name:'Escuro'}).check();
  await page.getByRole('button', {name:'Salvar alterações'}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});


test('as quatro paletas chegam aos painéis e acompanham a mudança do sistema', async ({ page }, testInfo) => {
  await page.emulateMedia({colorScheme: 'light'});
  await page.goto('/');
  await appearance(page);
  const root = page.locator('html');
  const surface = page.locator('.settings-content');
  const save = page.getByRole('button', {name: 'Salvar alterações'});
  const choose = async (name: string) => {
    await page.getByRole('radio', {name, exact: true}).check();
    if (await save.isEnabled()) await save.click();
  };

  await choose('Sistema');
  const palettes = [
    ['OpenKoda', 'rgb(250, 251, 246)', 'rgb(25, 43, 34)'],
    ['Neutro', 'rgb(255, 255, 255)', 'rgb(36, 36, 38)'],
    ['Ardósia', 'rgb(249, 251, 253)', 'rgb(32, 41, 50)'],
    ['Areia', 'rgb(255, 253, 248)', 'rgb(42, 39, 35)'],
  ] as const;
  for (const [name, light, dark] of palettes) {
    await choose(name);
    for (const [scheme, background] of [['light',light], ['dark',dark]] as const) {
      await page.emulateMedia({colorScheme: scheme});
      await expect(surface).toHaveCSS('background-color', background);
      const ratios = await page.locator('.settings-content .appearance-row small').first().evaluate(node => {
        const lum = (css: string) => {
          const [r,g,b] = css.match(/[\d.]+/g)!.slice(0,3).map(Number).map(n => {
            const channel = n / 255;
            return channel <= .04045 ? channel / 12.92 : ((channel+.055) / 1.055) ** 2.4;
          });
          return r * .2126 + g * .7152 + b * .0722;
        };
        const contrast = (fg: string, bg: string) => {
          const a = lum(fg), b = lum(bg);
          return (Math.max(a,b)+.05) / (Math.min(a,b)+.05);
        };
        const sheet = getComputedStyle(node.closest('.settings-content')!);
        const button = getComputedStyle(document.querySelector('.rail-button[aria-current]')!);
        return [contrast(getComputedStyle(node).color,sheet.backgroundColor), contrast(button.color,button.backgroundColor)];
      });
      expect(Math.min(...ratios), name + ' em ' + scheme).toBeGreaterThanOrEqual(4.5);
    }
  }
  await choose('OpenKoda');
  await choose('Claro');
  await expect(surface).toHaveCSS('background-color', palettes[0][1]);
  await choose('Escuro');
  await page.emulateMedia({colorScheme: 'light'});
  await expect(surface).toHaveCSS('background-color', palettes[0][2]);
  await page.reload();
  await appearance(page);
  await expect(root).toHaveAttribute('data-theme','dark');
  await expect(page.getByRole('radio', {name:'OpenKoda'})).toBeChecked();
  await page.locator('.settings-content').evaluate(el => el.scrollTop = 0);
  await page.screenshot({path:testInfo.outputPath('aparencia-escura.png'), animations:'disabled'});
  await page.setViewportSize({width:390,height:800});
  await expect(page.getByRole('heading',{name:'Aparência',exact:true})).toBeInViewport();
  const overflows = await surface.evaluate(el => el.scrollWidth > el.clientWidth + 1);
  expect(overflows).toBe(false);
  await page.screenshot({path:testInfo.outputPath('aparencia-390.png'), animations:'disabled'});
});

test('papel de parede, imagem local, movimento, escala e navegação afetam a janela', async ({page}) => {
  await page.goto('/');
  await appearance(page);
  const save = page.getByRole('button', {name: 'Salvar alterações'});
  const panel = page.locator('.settings-layout');
  const nav = page.getByRole('navigation', {name:'Navegação principal'});
  const settingsLabel = nav.locator('.rail-settings .rail-label');
  await expect(settingsLabel).toBeVisible();
  for (const [name,key] of [['Névoa','mist'], ['Dunas','dunes']]) {
    await page.getByRole('radio', {name,exact:true}).check();
    await save.click();
    await expect(page.locator('html')).toHaveAttribute('data-wallpaper',key);
    expect(await panel.evaluate(el=>getComputedStyle(el).backgroundImage)).toContain('radial-gradient');
  }
  await page.getByLabel('Animação do fundo').selectOption('dynamic');
  const visibility = page.getByLabel('Visibilidade do fundo');
  await visibility.focus();
  await visibility.press('End');
  await save.click();
  await expect(panel).toHaveCSS('animation-name','wallpaper-drift');
  await page.emulateMedia({reducedMotion:'reduce'});
  await expect(panel).toHaveCSS('animation-name','none');
  await page.emulateMedia({reducedMotion:'no-preference'});
  expect(await panel.evaluate(el=>getComputedStyle(el).getPropertyValue('--wallpaper-strength'))).toBe('20%');
  await page.getByLabel('Movimento', {exact:true}).selectOption('reduced');
  await save.click();
  await expect(panel).toHaveCSS('animation-name','none');
  await page.getByLabel('Movimento', {exact:true}).selectOption('system');
  await page.getByLabel('Animação do fundo').selectOption('static');
  await save.click();
  await expect(panel).toHaveCSS('animation-name','none');

  const image = await page.evaluate(() => {
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 16;
    canvas.getContext('2d')!.fillRect(0,0,16,16);
    return canvas.toDataURL().split(',')[1];
  });
  await page.getByLabel('Arquivo de papel de parede').setInputFiles({name:'amostra.png',mimeType:'image/png',buffer:Buffer.from(image,'base64')});
  await expect(page.getByRole('radio',{name:'Minha imagem'})).toBeChecked();
  await save.click();
  await page.reload();
  await appearance(page);
  expect(await panel.evaluate(el=>getComputedStyle(el).backgroundImage)).toContain('data:image/webp;base64,');
  await page.getByRole('button',{name:'Remover imagem'}).click();
  await save.click();
  await expect(panel).toHaveCSS('background-image','none');

  const scale = page.getByLabel('Tamanho da interface', {exact:true});
  for (const value of ['110','125','100']) {
    await scale.selectOption(value);
    await save.click();
    expect(await page.locator('html').evaluate(el=>Number(getComputedStyle(el).zoom))).toBe(Number(value)/100);
  }
  const sidebar = page.getByRole('combobox',{name:'Navegação principal',exact:true});
  await sidebar.selectOption('icons');
  await save.click();
  await expect(settingsLabel).toBeHidden();
  await sidebar.selectOption('auto');
  await save.click();
  await expect(settingsLabel).toBeVisible();
  await page.getByLabel('Fonte de interface',{exact:true}).fill('Tahoma');
  await page.getByLabel('Fonte de código',{exact:true}).fill('Courier New');
  await page.getByLabel('Tamanho do código',{exact:true}).selectOption('19');
  await save.click();
  await expect(page.getByRole('heading',{name:'Aparência',exact:true})).toHaveCSS('font-family', /Tahoma/);
  await expect(page.getByLabel('Amostra da fonte de código')).toHaveCSS('font-family', /Courier New/);
  await expect(page.getByLabel('Amostra da fonte de código')).toHaveCSS('font-size','19px');
  await page.getByRole('button',{name:'Restaurar fontes'}).click();
  await save.click();
  await expect(page.getByRole('heading',{name:'Aparência',exact:true})).toHaveCSS('font-family', /Georgia/);
});


test('densidade e estilo de lista preservam o achado e a conversa ao reabrir', async ({page}) => {
  const state = emptySnapshot();
  const at = '2026-11-13T12:00:00Z';
  state.projects = [{id:'calc',kind:'code',name:'Calculadora',target:'D:/fixture/calc',description:'Projeto de exemplo',createdAt:at}];
  state.assessments = [{id:'a',projectId:'calc',kind:'code',title:'Revisão da fórmula',target:'D:/fixture/calc',
    status:'completed',objective:'',summary:'',createdAt:at,updatedAt:at,events:[],coverage:[],evidence:[]}];
  state.findings = [{id:'f',assessmentId:'a',title:'Entrada da fórmula',severity:'high',confidence:'Revisão estática',
    status:'candidate',description:'Examine a expressão.',recommendation:'Validar a entrada.',createdAt:at,evidenceIds:[]}];
  await page.addInitScript(({key,state})=> {
    if (!localStorage.getItem(key)) localStorage.setItem(key,JSON.stringify(state));
  }, {key:previewStorageKey,state});
  await page.goto('/');
  await appearance(page);
  const row = page.locator('.appearance-row').last();
  const save = page.getByRole('button', {name:'Salvar alterações'});
  const comfortable = await row.evaluate(el=>getComputedStyle(el).paddingBlockStart);
  await page.getByRole('radio',{name:'Compacta'}).check();
  await page.getByLabel('Avaliações e achados',{exact:true}).selectOption('line');
  await save.click();
  const compact = await row.evaluate(el=>getComputedStyle(el).paddingBlockStart);
  expect(parseFloat(compact)).toBeLessThan(parseFloat(comfortable));
  await page.reload();
  const nav = page.getByRole('navigation',{name:'Navegação principal'});
  await nav.getByRole('button',{name:'Achados'}).click();
  const finding=page.getByRole('button',{name:/Entrada da fórmula/});
  await expect(finding).toBeVisible();
  await expect(finding.locator('.item-copy > span')).toBeHidden();
  await appearance(page);
  await page.getByRole('radio',{name:'Confortável'}).check();
  await page.getByLabel('Avaliações e achados',{exact:true}).selectOption('preview');
  await save.click();
  await nav.getByRole('button',{name:'Achados'}).click();
  await expect(finding.locator('.item-copy > span')).toHaveText('Revisão da fórmula');
  await expect(finding.locator('.item-copy > span')).toBeVisible();
  await nav.getByRole('button',{name:'Início'}).click();
  await expect(page.getByRole('button',{name:'Revisão da fórmula',exact:true})).toBeVisible();
});
