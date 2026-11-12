import { expect,test } from './fixtures';

test('o alvo escolhido na entrada abre o formulário correspondente pelo teclado',async({page})=>{
  await page.goto('/');
  const folder=page.getByRole('radio',{name:/Pasta local/});
  await expect(folder).toBeChecked();
  await folder.focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('radio',{name:/Web \/ API/})).toBeChecked();
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect(page.getByLabel('URL do site ou da API')).toBeVisible();
  await expect(page.getByRole('button',{name:'Análise Web / API'})).toHaveAttribute('aria-pressed','true');
});

test('árvore, painel e navegação compacta mantêm o foco e o espaço do chat',async({page})=>{
  await page.goto('/');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await page.getByLabel('Pasta do projeto',{exact:true}).fill('D:/Laboratorio/Portal');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  const tree=page.getByRole('complementary',{name:'Projetos e conversas'});
  await page.getByRole('button',{name:'Recolher lista de projetos'}).click();
  await expect(tree).toBeHidden();
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeVisible();
  await page.getByRole('button',{name:'Mostrar lista de projetos'}).click();
  await expect(tree).toBeVisible();
  const overview=page.getByRole('button',{name:'Painel do projeto'});
  await overview.click();
  await expect(page.getByRole('button',{name:'Fechar painel'})).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(overview).toBeFocused();
  await page.setViewportSize({width:600,height:760});
  const mobile=page.getByRole('button',{name:'Projetos',exact:true});
  await mobile.click();
  await expect(tree.getByRole('button',{name:'Novo projeto',exact:true})).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(tree).toBeHidden();
  await expect(mobile).toBeFocused();
});
