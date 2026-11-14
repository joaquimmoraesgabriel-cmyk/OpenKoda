import {test,expect} from '@playwright/test';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const origin='https://real-demo.example.invalid';
let site:{fetch:(request:Request,env:object)=>Promise<Response>};
test.beforeAll(async()=>{
  execFileSync(process.execPath,[fileURLToPath(new URL('../../site/build.mjs',import.meta.url))]);
  const source=await readFile(new URL('../../site/dist/server/index.js',import.meta.url),'utf8');
  site=(await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'))).default;
});

test.beforeEach(async({page,context})=>{
  await context.route('**/*',async route=>{
    const url=new URL(route.request().url());
    if(url.origin!==origin)throw new Error('Unexpected demo network request: '+url.origin);
    const response=await site.fetch(new Request(url),{});
    await route.fulfill({status:response.status,headers:Object.fromEntries(response.headers),body:Buffer.from(await response.arrayBuffer())});
  });
  await page.goto(origin+'/demo-app/');
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeVisible();
});
test('aplicativo real conversa, interrompe e reinicia sem requisição remota',async({page},info)=>{
  const remote:string[]=[],errors:string[]=[];
  page.on('request',request=>{if(!new URL(request.url()).pathname.startsWith('/demo-app/'))remote.push(request.url());});
  page.on('pageerror',error=>errors.push(error.message));
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await input.fill('Minha pergunta local');await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  await expect(page.getByText('Minha pergunta local',{exact:true})).toBeVisible();
  await expect(page.getByText(/Esta é uma resposta pré-definida da demonstração/)).toBeVisible();
  await input.fill('Segunda pergunta');await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  await page.getByRole('button',{name:'Cancelar trabalho',exact:true}).click();
  await expect(page.getByText(/Demonstração interrompida/)).toBeVisible();
  await page.screenshot({path:info.outputPath('real-app-desktop.png')});
  await page.reload();await expect(page.getByText('Minha pergunta local',{exact:true})).toHaveCount(0);
  expect(remote).toEqual([]);expect(errors).toEqual([]);
});
test('configurações, aparência, conta e histórico reutilizam telas reais',async({page},info)=>{
  const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Aparência',exact:true}).click();
  await expect(page.getByRole('region',{name:'Conteúdo de Aparência'})).toBeVisible();
  await page.screenshot({path:info.outputPath('real-app-appearance.png')});
  await page.getByRole('button',{name:'Histórico de uso',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Uso da assinatura'})).toBeVisible();
  await page.getByRole('button',{name:'Análises',exact:true}).click();
  await expect(page.getByText('Números fictícios para explorar o histórico. A demonstração não consome créditos.')).toBeVisible();
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await expect(page.getByText('visitante@example.invalid · PLUS')).toBeVisible();
  expect(errors).toEqual([]);
});
test('cria projeto local com análise fictícia e conversa nova',async({page})=>{
  await page.getByRole('button',{name:'Novo projeto',exact:true}).click();
  await page.getByRole('button',{name:'Escolher pasta',exact:true}).click();
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  const dialog=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await dialog.getByRole('button',{name:'Aurora · exemplo',exact:true}).first().click();
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeVisible();
  await expect(page.getByText('Relatório fictício pronto para revisão.').first()).toBeVisible();
});
test('a 390px navega e preserva isolamento das preferências',async({page},info)=>{
  await page.setViewportSize({width:390,height:740});
  await page.getByRole('button',{name:'Projetos',exact:true}).click();
  await expect(page.getByRole('complementary',{name:'Projetos e conversas'})).toBeVisible();
  await page.keyboard.press('Escape');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
  await page.screenshot({path:info.outputPath('real-app-mobile.png')});
  await page.evaluate(()=>localStorage.setItem('demo-only','true'));await page.reload();
  expect(await page.evaluate(()=>localStorage.getItem('demo-only'))).toBeNull();
});

for(const width of [320,334,350])test(`compositor a ${width}px envia sem deslocar ou cortar a conversa`,async({page},info)=>{
  await page.setViewportSize({width,height:740});
  const conversation=page.locator('.desk-conversation');
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await input.fill('Verificar os pedidos neste exemplo estreito');
  await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  const answer=page.getByText(/Esta é uma resposta pré-definida da demonstração/);
  await expect(answer).toBeVisible();
  await answer.scrollIntoViewIfNeeded();
  const sizes=await page.locator('.desk-conversation,.project-chat,.project-chat-bottom,.project-composer,.project-composer-tools,.reasoning-auto').evaluateAll(elements=>elements.map(element=>({name:element.className,width:element.clientWidth,scrollWidth:element.scrollWidth,left:element.scrollLeft})));
  for(const size of sizes){expect(size.scrollWidth,JSON.stringify(size)).toBeLessThanOrEqual(size.width+1);expect(size.left,JSON.stringify(size)).toBe(0);}
  const bounds=await conversation.boundingBox();expect(bounds).not.toBeNull();
  for(const control of [input,page.getByRole('button',{name:'Adicionar à mensagem'}),page.getByRole('button',{name:/Escolher modelo/}),page.locator('.reasoning-control input'),page.getByRole('button',{name:'Enviar mensagem',exact:true}),answer]){
    await expect(control).toBeVisible();const box=await control.boundingBox();expect(box).not.toBeNull();
    expect(box!.x).toBeGreaterThanOrEqual(bounds!.x-1);expect(box!.x+box!.width).toBeLessThanOrEqual(bounds!.x+bounds!.width+1);
  }
  await page.getByRole('button',{name:'Projetos',exact:true}).click();
  await page.getByRole('button',{name:'Relatório de segurança',exact:true}).click();
  await expect(page.getByText('Relatório de exemplo pronto para revisão.')).toBeVisible();
  await page.getByRole('button',{name:'Projetos',exact:true}).click();
  await page.getByRole('button',{name:'Entender o acesso aos pedidos',exact:true}).click();
  await expect(answer).toBeVisible();
  await expect(conversation).toHaveJSProperty('scrollLeft',0);
  await page.screenshot({path:info.outputPath('real-app-narrow-'+width+'.png')});
});
test('capturas atuais das telas reais para os guias',async({page},info)=>{
  await page.setViewportSize({width:1280,height:832});
  await page.locator('#root').screenshot({path:info.outputPath('app-conversa.png')});
  await page.getByRole('button',{name:'Relatório de segurança',exact:true}).click();
  await page.getByText('Relatório da análise',{exact:true}).click();
  await page.getByRole('region',{name:'Histórico da conversa'}).evaluate(el=>{el.scrollTop=0;});
  await page.locator('#root').screenshot({path:info.outputPath('app-relatorio.png')});
  await page.getByRole('button',{name:'Novo projeto',exact:true}).click();
  await page.getByRole('button',{name:'Escolher pasta',exact:true}).click();
  await page.locator('.launch-page').evaluate(el=>{for(let parent:HTMLElement|null=el as HTMLElement;parent;parent=parent.parentElement)parent.scrollTop=0;});
  await page.locator('#root').screenshot({path:info.outputPath('app-projeto.png')});
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Aparência',exact:true}).click();
  await page.locator('#root').screenshot({path:info.outputPath('app-aparencia.png')});
  await page.getByRole('button',{name:'Histórico de uso',exact:true}).click();
  await page.locator('#root').screenshot({path:info.outputPath('app-uso.png')});
  await page.getByRole('button',{name:'IA e modelos',exact:true}).click();
  await expect(page.getByText('Configurado',{exact:true}).first()).toBeVisible();
  await page.locator('#root').screenshot({path:info.outputPath('app-modelos.png')});
});
test('todas as categorias de configurações abrem sem erro de execução',async({page})=>{
  const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  for(const name of ['Geral','Aparência','Dados e privacidade','Armazenamento','Backup e recuperação','Permissões','IA e modelos','Histórico de uso','Diagnóstico e suporte','Relatar um erro','Atualizações']){
    await page.getByRole('button',{name,exact:true}).click();
    await expect(page.getByRole('region',{name:'Conteúdo de '+name})).toBeVisible();
    await expect(page.getByRole('alert')).toHaveCount(0);
  }
  await page.getByRole('button',{name:'Dados e privacidade',exact:true}).click();
  await page.getByLabel('Projeto para política de dados').selectOption('aurora');
  await page.locator('summary').filter({hasText:'Política de dados do projeto'}).click();
  await expect(page.getByText('Política corrente',{exact:true})).toBeVisible();
  expect(errors).toEqual([]);
});
test('anexo é fictício, modelo muda e preferências só duram na demonstração',async({page})=>{
  await page.getByRole('button',{name:'Adicionar à mensagem'}).click();
  await page.getByRole('menuitem',{name:'Anexar foto ou arquivo'}).click();
  await expect(page.getByText('exemplo.ts',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Remover anexo'}).click();
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  const dialog=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await dialog.getByRole('button',{name:'OpenRouter',exact:true}).click();
  await dialog.getByRole('button',{name:'Atlas · exemplo',exact:true}).click();
  await expect(page.getByRole('button',{name:/Escolher modelo/})).toContainText('Atlas');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Aparência',exact:true}).click();
  await page.getByRole('radio',{name:'Escuro',exact:true}).check();
  await page.getByRole('button',{name:'Salvar alterações',exact:true}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await page.reload();await expect(page.locator('html')).toHaveAttribute('data-theme','light');
});
test('GitHub usa cópia fictícia e chamado permanece na conta de demonstração',async({page})=>{
  await page.getByRole('button',{name:'Novo projeto',exact:true}).click();
  await page.getByRole('button',{name:'GitHub',exact:true}).click();
  await page.getByLabel('Link do repositório no GitHub').fill('https://github.com/real-company/private-code');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect(page.getByRole('button',{name:'repositorio-demonstracao',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Relatar um erro',exact:true}).click();
  await page.getByLabel('Assunto',{exact:true}).fill('Chamado local');
  await page.getByLabel('Descrição do chamado').fill('Este conteúdo deve existir apenas no exemplo.');
  await page.getByRole('button',{name:'Enviar chamado'}).click();
  await expect(page.getByText('Chamado local',{exact:true})).toBeVisible();
  await expect(page.locator('#demo-notice')).toContainText('Nenhuma mensagem foi enviada');
});
test('gerenciar modelos preserva seleção e padrão ao reabrir',async({page})=>{
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'IA e modelos',exact:true}).click();
  await page.getByRole('button',{name:/OpenRouter.*Configurado/}).click();
  await page.getByRole('button',{name:'Gerenciar modelos',exact:true}).click();
  let dialog=page.getByRole('dialog',{name:'Escolher modelos · OpenRouter'});
  await dialog.getByRole('checkbox',{name:'Aurora · exemplo',exact:true}).uncheck();
  await dialog.getByRole('checkbox',{name:'Orion · exemplo',exact:true}).uncheck();
  await dialog.getByRole('checkbox',{name:'Usar como padrão da instalação'}).check();
  await dialog.getByRole('combobox',{name:'Modelo padrão'}).selectOption('Atlas · exemplo');
  await dialog.getByRole('button',{name:'Salvar modelos',exact:true}).click();
  await expect(page.getByText('1 modelo selecionado',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Gerenciar modelos',exact:true}).click();
  dialog=page.getByRole('dialog',{name:'Escolher modelos · OpenRouter'});
  await expect(dialog.getByRole('checkbox',{name:'Atlas · exemplo',exact:true})).toBeChecked();
  await expect(dialog.getByRole('checkbox',{name:'Aurora · exemplo',exact:true})).not.toBeChecked();
  await expect(dialog.getByRole('combobox',{name:'Modelo padrão'})).toHaveValue('Atlas · exemplo');
  await dialog.getByRole('button',{name:'Cancelar',exact:true}).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('conversa nova nasce na primeira mensagem e pode ser renomeada',async({page})=>{
  await page.getByRole('button',{name:'Criar nova conversa em Loja Aurora'}).click();
  await expect(page.locator('.tree-chat')).toHaveCount(2);
  await expect(page.getByRole('region',{name:'Histórico da conversa'}).getByText('Onde devo começar a revisão de segurança desta loja?')).toHaveCount(0);
  await page.getByRole('textbox',{name:'Mensagem da conversa'}).fill('Revisar pedidos de teste');
  await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  await expect(page.getByText(/Esta é uma resposta pré-definida da demonstração/)).toBeVisible();
  await expect(page.locator('.tree-chat')).toHaveCount(3);
  await expect(page.getByRole('button',{name:'Revisar pedidos de teste',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Renomear conversa'}).click();
  await page.getByRole('textbox',{name:'Título da conversa'}).fill('Revisão demonstrativa');
  await page.getByRole('button',{name:'Salvar título'}).click();
  await page.getByRole('button',{name:'Entender o acesso aos pedidos',exact:true}).click();
  await page.getByRole('button',{name:'Revisão demonstrativa',exact:true}).click();
  await expect(page.getByText('Revisar pedidos de teste',{exact:true})).toBeVisible();
});

test('papel de parede recebe imagem fictícia sem abrir arquivos locais',async({page})=>{
  let fileChoosers=0;page.on('filechooser',()=>fileChoosers++);
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Aparência',exact:true}).click();
  await page.getByRole('button',{name:'Escolher imagem…',exact:true}).click();
  await expect(page.getByRole('radio',{name:'Minha imagem',exact:true})).toBeChecked();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.getByRole('button',{name:'Salvar alterações',exact:true}).click();
  expect(fileChoosers).toBe(0);
  await expect(page.locator('#demo-notice')).toContainText('Arquivo fictício selecionado');
});
