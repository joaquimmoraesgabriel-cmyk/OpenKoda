import { expect, test } from './fixtures';

test('o primeiro projeto abre em conversa e os painéis compartilham o espaço com o chat',async({page})=>{
  await page.goto('/');
  await expect(page.getByRole('navigation',{name:'Navegação principal'}).getByRole('button',{name:'Avaliações'})).toHaveCount(0);
  await expect(page.getByRole('complementary',{name:'Projetos e conversas'})).toHaveCount(0);
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await page.getByLabel('Pasta do projeto',{exact:true}).fill('D:/Laboratorio/portal');
  await page.getByRole('switch',{name:'Contexto adicional',exact:true}).check();
  await page.getByLabel('Áreas de foco').fill('Autorização');
  await page.getByLabel('Descrição',{exact:true}).fill('Verificar o controle de acesso.');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  const tree=page.getByRole('complementary',{name:'Projetos e conversas'});
  await expect(tree).toBeVisible();
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeVisible();
  await page.getByRole('button',{name:'Visão geral do projeto'}).click();
  const panel=page.getByRole('complementary',{name:'Visão geral do projeto'});
  await expect(panel.getByRole('heading',{name:'Uso de IA'})).toBeVisible();
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeHidden();
  await panel.getByRole('button',{name:'Mostrar ao lado da conversa'}).click();
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeVisible();
  await panel.getByRole('button',{name:'Fechar painel'}).click();
  await page.getByRole('button',{name:'Configurações do projeto'}).click();
  await expect(page.getByRole('heading',{name:'Arquivar projeto'})).toBeVisible();
  await page.getByRole('button',{name:'Mostrar ao lado da conversa'}).click();
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeVisible();
  await tree.getByRole('button',{name:'Criar nova conversa em portal'}).click();
  await expect(tree.getByRole('button',{name:'Conversa 2',exact:true})).toHaveAttribute('aria-current','page');
  await page.reload();
  await expect(tree.getByRole('button',{name:'Conversa 2',exact:true})).toBeVisible();
});

test('a modalidade define o alvo e a largura reduzida conserva os controles',async({page})=>{
  await page.setViewportSize({width:520,height:680});
  await page.goto('/');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await page.getByRole('button',{name:'Análise Web / API'}).click();
  await expect(page.getByLabel('URL do site ou da API')).toBeVisible();
  await page.getByRole('button',{name:'Análise de rede'}).click();
  await expect(page.getByLabel('IP ou rede CIDR')).toBeVisible();
  await page.getByRole('switch',{name:'Opções avançadas'}).check();
  await expect(page.getByLabel('Portas TCP')).toBeVisible();
  await expect(page.getByText('Modelo de IA',{exact:true})).toBeVisible();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
});
