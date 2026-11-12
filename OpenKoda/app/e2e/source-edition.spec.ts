import { expect,test } from './fixtures';
import { emptySnapshot } from '../src/preview';

test('a edição de código abre a pasta local e exige modelo sem oferecer os antigos motores',async({page})=>{
  await page.goto('/?area=tecnica');
  await expect(page.getByRole('heading',{name:'Seu primeiro projeto'})).toBeVisible();
  await expect(page.getByRole('radio')).toHaveCount(1);
  const source=page.getByRole('radio',{name:/Pasta local/});
  await source.focus();await page.keyboard.press('ArrowRight');
  await expect(source).toBeChecked();
  const create=page.getByRole('button',{name:'Criar projeto',exact:true});
  await create.click();
  await page.getByLabel('Pasta do projeto',{exact:true}).fill('D:/teste/local');
  await page.getByRole('switch',{name:'Opções avançadas'}).check();
  await expect(page.getByRole('button',{name:/Análise Web|Análise de rede|Usar só a análise nativa/})).toHaveCount(0);
  await expect(page.getByLabel('Portas TCP')).toHaveCount(0);
  await expect(page.getByRole('heading',{name:'Ferramentas do Kali'})).toHaveCount(0);
  await expect(create).toBeDisabled();
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await expect(page.getByRole('button',{name:/área técnica/i})).toHaveCount(0);
});

test('a rail conserva projetos de código, foco, conversa e painel em largura compacta',async({page})=>{
  const s=emptySnapshot(),at='2026-11-12T08:00:00Z';
  s.projects=[
    {id:'local',kind:'code',name:'Portal local',target:'D:/Fixture',createdAt:at,description:''},
    {id:'legacy',kind:'web',name:'Site histórico',target:'https://fixture.invalid',createdAt:at,description:''},
  ];
  s.assessments=s.projects.map(p=>({id:'a-'+p.id,projectId:p.id,kind:p.kind,title:'Conversa de '+p.name,target:p.target,objective:'Conferir registros.',createdAt:at,updatedAt:at,status:'draft',coverage:[],evidence:[],events:[],summary:''}));
  await page.addInitScript(s=>{
    Object.assign(window,{isTauri:true,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request?:{action:string}})=>{
      switch(args?.request?.action){
        case 'snapshot':return s;
        case 'ai.history':return {messages:[]};
        case 'ai.chat.status':return {requestId:null,stopping:false};
        case 'security.scan.get':return {scan:null};
        case 'security.status':return {available:false,hasApiKey:false,reason:'Sem credencial na fixture.'};
        case 'ai.budget.history':return {rows:[]};
        default:return {};
      }
    }}});
  },s);
  await page.goto('/');
  const tree=page.getByRole('complementary',{name:'Projetos e conversas'});
  await expect(tree.getByRole('button',{name:'Portal local',exact:true})).toBeVisible();
  await expect(tree.getByText('Site histórico')).toHaveCount(0);
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});await input.fill('Pergunta de teste');
  const open=page.getByRole('button',{name:'Painel do projeto'});
  await open.click();
  const panel=page.getByRole('complementary',{name:'Projeto',exact:true});
  await expect(panel.getByRole('button',{name:'Fechar painel'})).toBeFocused();
  await panel.getByRole('button',{name:'Mostrar ao lado da conversa'}).click();
  await expect(input).toHaveValue('Pergunta de teste');
  await panel.getByRole('tab',{name:'Visão geral'}).focus();await page.keyboard.press('End');
  await expect(panel.getByRole('tab',{name:'Configurações'})).toBeFocused();
  await expect(panel.getByRole('heading',{name:'Arquivar projeto'})).toBeVisible();
  await expect(panel.getByRole('heading',{name:'Pasta do projeto'})).toBeVisible();
  await expect(panel.getByText('Alvos e escopo',{exact:true})).toHaveCount(0);
  await page.keyboard.press('Escape');await expect(open).toBeFocused();
  await page.setViewportSize({width:512,height:500});
  await page.getByRole('button',{name:'Projetos',exact:true}).click();
  await expect(tree.getByRole('button',{name:'Novo projeto'})).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(tree).toBeHidden();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});

test('o seletor de pasta conserva a seleção ao cancelar ou receber uma falha do Windows',async({page})=>{
  await page.addInitScript(state=>{
    let attempts=0;
    Object.assign(window,{isTauri:true,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      const {action,payload}=args.request;
      if(action==='snapshot')return state;
      if(action==='project.pickTarget'){
        if(payload.selection!=='folder')throw new Error('Modalidade inesperada.');
        attempts++;
        if(attempts===1)return 'D:/Fixture/Seleção local';
        if(attempts===2)return null;
        throw new Error('O seletor de pasta não abriu.');
      }
      return {};
    }}});
  },emptySnapshot());
  await page.goto('/');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  const folder=page.getByRole('button',{name:'Escolher pasta',exact:true});
  const path=page.getByLabel('Pasta do projeto',{exact:true});
  await folder.click();await expect(path).toHaveValue('D:/Fixture/Seleção local');
  await folder.click();await expect(path).toBeEditable();await expect(path).toHaveValue('D:/Fixture/Seleção local');
  await folder.click();await expect(page.getByRole('alert')).toContainText('O seletor de pasta não abriu.');
  await expect(path).toHaveValue('D:/Fixture/Seleção local');
  await expect(page.getByRole('button',{name:'Criar projeto',exact:true})).toBeDisabled();
});

test('a preferência da janela é confirmada pelo host e persiste ao reabrir as configurações',async({page})=>{
  await page.addInitScript(state=>{
    let minimize=false;
    Object.assign(window,{isTauri:true,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request:{action:string;payload:{minimize?:boolean}}})=>{
      const {action,payload}=args.request;
      if(action==='snapshot')return state;
      if(action==='window.behavior.set'){
        if(typeof payload.minimize!=='boolean')throw new Error('Faltou a escolha explícita.');
        minimize=payload.minimize;
      }
      if(action.startsWith('window.behavior.'))return {minimize};
      return {};
    }}});
  },emptySnapshot());
  await page.goto('/');
  const settings=page.getByRole('button',{name:'Configurações',exact:true});
  const behavior=page.getByRole('combobox',{name:'Ao fechar a janela'});
  await settings.click();await expect(behavior).toHaveValue('exit');
  await behavior.selectOption('minimize');
  await expect(page.getByText('Preferência salva.',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await settings.click();await expect(behavior).toHaveValue('minimize');
});
