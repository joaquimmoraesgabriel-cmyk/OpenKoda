import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// Isolated IPC fixture: these tests exercise UI decisions, never disk deletion.
async function retentionFixture(page:Page,mode='success'){
  await page.addInitScript(({mode})=>{
    const now=new Date().toISOString();const windowFixture=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__retentionCalls:{action:string;payload:Record<string,unknown>}[]};
    windowFixture.isTauri=true;windowFixture.__retentionCalls=[];
    const settings={provider:'openai',endpoint:'https://fixture.example.test/v1',model:'fixture',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false};
    const snapshot={projects:[],assessments:[],findings:[],settings,plans:[],executions:[],validations:[]};
    type Artifact={category:string;id:string;label:string;bytes:number;fileCount:number;files:{name:string;bytes:number;sha256:string;identity:string}[];identity:string;capture?:{assessmentId:string;manifestDigest:string;purgedManifestDigest:string;generation:number;preservedSha256:string;chunkCount:number};kaliCapture?:{reference:{captureId:string;assessmentId:string;runId:string;environmentId:string;ownerProfileFingerprint:string;manifestDigest:string;preservedBytes:number;availability:string};inspectionDigest:string;previousReceiptId?:string;previousReceiptDigest?:string};kaliMetadataItem?:Record<string,unknown>;kaliMetadata?:Record<string,unknown>};
    let items:Artifact[]=[{category:'backup',id:'11111111-1111-4111-8111-111111111111',label:'Backup do laboratório',bytes:120,fileCount:2,files:[{name:'manifest.json',bytes:20,sha256:'',identity:'file-manifest'},{name:'openkoda.sqlite3',bytes:100,sha256:'',identity:'file-database'}],identity:'backup-directory'},{category:'report',id:'22222222-2222-4222-8222-222222222222',label:'Relatório do laboratório',bytes:50,fileCount:1,files:[{name:'report.md',bytes:50,sha256:'',identity:'file-report'}],identity:'report-directory'}];
    let plan:Record<string,unknown>|undefined;let blocked=mode==='active';let metadataReconciled=false;let planCount=0;
    if(mode==='capture')items.push({category:'capture',id:'44444444-4444-4444-8444-444444444444',label:'Captura da operação local',bytes:48000,fileCount:0,files:[],identity:'owned-core-database',capture:{assessmentId:'55555555-5555-4555-8555-555555555555',manifestDigest:'b'.repeat(64),purgedManifestDigest:'c'.repeat(64),generation:3,preservedSha256:'d'.repeat(64),chunkCount:2}});
    if(mode==='kali_capture'||mode==='kali_partial')items.push({category:'kali_capture',id:'66666666-6666-4666-8666-666666666666',label:'Captura do Kali do laboratório',bytes:1024,fileCount:0,files:[],identity:'owned-linux-spool',kaliCapture:{reference:{captureId:'66666666-6666-4666-8666-666666666666',assessmentId:'55555555-5555-4555-8555-555555555555',runId:'77777777-7777-4777-8777-777777777777',environmentId:'Kali gerido do laboratório',ownerProfileFingerprint:'e'.repeat(64),manifestDigest:'f'.repeat(64),preservedBytes:1024,availability:'owned'},inspectionDigest:'9'.repeat(64)}});
    if(mode==='kali_partial'){const capture=items.at(-1)!.kaliCapture!;capture.previousReceiptId='88888888-8888-4888-8888-888888888888';capture.previousReceiptDigest='7'.repeat(64);}
    if(mode.startsWith('kali_metadata'))items.push({category:'kali_capture_metadata',id:'66666666-6666-4666-8666-666666666666',label:'Metadados da captura do laboratório',bytes:0,fileCount:0,files:[],identity:'owned-linux-metadata',kaliMetadataItem:{reference:{captureId:'66666666-6666-4666-8666-666666666666',assessmentId:'55555555-5555-4555-8555-555555555555',runId:'77777777-7777-4777-8777-777777777777',environmentId:'Kali gerido do laboratório',ownerProfileFingerprint:'e'.repeat(64),manifestDigest:'f'.repeat(64),preservedBytes:64000,availability:'purged'},purgeControlRunId:'88888888-8888-4888-8888-888888888888',purgeReceiptId:'99999999-9999-4999-8999-999999999999',purgeProofSha256:null,state:'uninspected'}});
    const limits=mode.startsWith('kali_metadata')?['Remove somente os metadados terminais revisados. A prova permanece no histórico do produtor; não remove novamente bytes da captura.']:['Somente cópias de backup e relatórios geridos; projetos e evidências do banco permanecem.'];
    windowFixture.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');
      const {action,payload}=args.request;windowFixture.__retentionCalls.push({action,payload});
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='snapshot')return structuredClone(snapshot);
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture sem ambiente real.',tools:[],executorReady:false};
      if(action==='ai.status')return {hasApiKey:false,verified:false,checkedAt:null,model:settings.model,endpoint:settings.endpoint};
      if(action==='backup.list')return {backups:[]};
      if(action==='activation.status')return {currentProfile:{kind:'base'},activeProfile:{kind:'base'},previousProfile:null,pendingPlan:null,restartRequired:false,notice:'Fixture; perfil não alterado.'};
      if(action==='activation.list')return {plans:[]};
      if(action==='retention.inventory')return structuredClone({items,journals:plan?[plan]:[],hasUnresolved:plan?.status==='unknown',blocked,limits,projectPurgeSupported:false,automaticPurge:false,retentionDays:90});
      if(action==='retention.plan'){
        const requested=payload.items as {category:string;id:string}[];
        const selected:Artifact[]=items.filter(item=>requested.some(entry=>entry.id===item.id&&entry.category===item.category)).map(item=>({...item,files:item.files.map(file=>({...file,sha256:'a'.repeat(64)}))}));
        for(const item of selected)if(item.category==='kali_capture_metadata'){
          item.bytes=metadataReconciled?300:700;item.fileCount=metadataReconciled?1:2;
          item.kaliMetadata={item:{...item.kaliMetadataItem,purgeProofSha256:'1'.repeat(64),state:metadataReconciled?'remaining':'uninspected'},inspectionDigest:'2'.repeat(64),stateDigest:'3'.repeat(64),previousReceiptId:metadataReconciled?'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa':null,previousReceiptDigest:metadataReconciled?'4'.repeat(64):null,observation:{metadataBytes:item.bytes,slotPresent:true,entries:metadataReconciled?[{name:'manifest.json',bytes:300,sha256:'5'.repeat(64),identity:{device:'1',inode:'9007199254740993'}}]:[{name:'manifest.json',bytes:300,sha256:'5'.repeat(64),identity:{device:'1',inode:'9007199254740993'}},{name:'purge.json',bytes:400,sha256:'6'.repeat(64),identity:{device:'1',inode:'9007199254740994'}}]}};
          delete item.kaliMetadataItem;
          const observation=item.kaliMetadata.observation as Record<string,unknown>;
          observation.pendingProducerWrite=null;
          if(mode==='kali_metadata_pending'){
            item.bytes+=128;item.fileCount++;observation.metadataBytes=item.bytes;
            observation.pendingProducerWrite={identity:{device:'1',inode:'9007199254740995'},bytes:128,sha256:'8'.repeat(64),attemptId:'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',complete:false};
          }
        }
        planCount++;
        plan={id:mode.startsWith('kali_metadata')&&planCount>1?'33333333-3333-4333-8333-333333333334':'33333333-3333-4333-8333-333333333333',digest:'retention-fixture-digest',status:'prepared',createdAt:now,expiresAt:new Date(Date.now()+(mode==='expired'?-60000:600000)).toISOString(),items:selected,totalBytes:selected.reduce((total,item)=>total+item.bytes,0),totalFiles:selected.reduce((total,item)=>total+item.fileCount,0),confirmationPhrase:'EXCLUIR PERMANENTEMENTE',limits,progress:[],error:null};return structuredClone(plan);
      }
      if(action==='retention.apply'){
        if(payload.id!==plan?.id||payload.digest!==plan?.digest||payload.confirmation!=='EXCLUIR PERMANENTEMENTE')throw new Error('Confirmação do plano divergente.');
        if(mode==='stale'){plan!.status='rejected';plan!.error='Arquivo alterado após a revisão. Nenhuma exclusão iniciada.';throw new Error(String(plan!.error));}
        const selected=plan!.items as Artifact[];
        if((mode==='kali_metadata_unknown'||mode==='kali_metadata_reconciled')&&!metadataReconciled){plan!.status='unknown';plan!.error='Término da consolidação ainda não confirmado.';blocked=true;plan!.progress=[{category:selected[0].category,id:selected[0].id,name:'linux_capture_metadata',status:'deleting',sha256:'2'.repeat(64)}];}
        else if(mode==='unknown'){plan!.status='unknown';plan!.error='Não foi possível confirmar o efeito da tentativa.';blocked=true;plan!.progress=[{category:selected[0].category,id:selected[0].id,name:selected[0].files[0].name,status:'deleting',sha256:'a'.repeat(64)}];}
        else if(mode==='partial'){plan!.status='partial';plan!.error='Uma parte dos arquivos permanece.';plan!.progress=selected[0].files.map((file,index)=>({category:selected[0].category,id:selected[0].id,name:file.name,status:index===0?'deleted':'present',sha256:file.sha256}));}
        else{plan!.status='applied';plan!.progress=selected.filter(item=>item.category==='kali_capture_metadata').map(item=>({category:item.category,id:item.id,name:'linux_capture_metadata',status:'deleted',sha256:'2'.repeat(64)}));items=items.filter(item=>!selected.some(entry=>entry.id===item.id&&entry.category===item.category));}
        return structuredClone(plan);
      }
      if(action==='retention.reconcile'){
        if(payload.id!==plan?.id||plan?.status!=='unknown')throw new Error('Journal não corresponde à inspeção.');
        if(mode==='kali_metadata_reconciled'){
          metadataReconciled=true;plan!.status='applied';plan!.error=null;blocked=false;
          const item=(plan!.items as Artifact[])[0];plan!.progress=[{category:item.category,id:item.id,name:'linux_capture_metadata',status:'absent',sha256:'2'.repeat(64)}];
          items=items.filter(entry=>entry.category!==item.category||entry.id!==item.id);return structuredClone(plan);
        }
        if(mode==='kali_metadata_unknown'){
          metadataReconciled=true;plan!.status='partial';plan!.error='Metadados restantes confirmados sem nova exclusão.';blocked=false;
          const item=(plan!.items as Artifact[])[0];plan!.progress=[{category:item.category,id:item.id,name:'linux_capture_metadata',status:'present',sha256:'2'.repeat(64)}];
          const available=items.find(entry=>entry.category===item.category&&entry.id===item.id)!;available.kaliMetadataItem!.state='remaining';return structuredClone(plan);
        }
        plan!.status='partial';plan!.error='Inspeção confirmou um arquivo ausente e um presente. Nada foi excluído pela inspeção.';blocked=false;return structuredClone(plan);
      }
      throw new Error('Ação não prevista no contrato de retenção: '+action);
    }};
  },{mode});
  await page.goto('/');await page.getByRole('button',{name:'Configurações',exact:true}).click();await page.getByRole('button',{name:'Dados e privacidade',exact:true}).click();
  await expect(page.locator('.retention').getByText('Backup do laboratório',{exact:true})).toBeVisible();
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__retentionCalls:{action:string;payload:Record<string,unknown>}[]}).__retentionCalls.filter(call=>call.action===name),action);}
async function prepare(page:Page){
  const panel=page.locator('.retention');
  await panel.getByRole('checkbox',{name:/Backup do laboratório/}).check();
  await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('retention-fixture-digest');
}
async function confirm(page:Page){
  await page.getByRole('checkbox',{name:'Revisei os arquivos e aceito a exclusão permanente dos itens deste plano.'}).check();
  await page.getByLabel('Frase de confirmação',{exact:true}).fill('EXCLUIR PERMANENTEMENTE');
  await page.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true}).click();
}

test('metadados Kali exigem revisão separada sem apresentar tamanho desconhecido como zero',async({page})=>{
  await retentionFixture(page,'kali_metadata');const panel=page.locator('.retention');
  const selection=panel.getByRole('checkbox',{name:/Metadados da captura do laboratório/});
  await expect(selection).toBeVisible();
  const item=panel.getByText('Metadados da captura do laboratório',{exact:true}).locator('..');
  await expect(item).toContainText('Tamanho a conferir na preparação');
  await expect(item).not.toContainText('0 bytes');
  expect(await calls(page,'retention.apply')).toEqual([]);
  await selection.check();await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  expect((await calls(page,'retention.plan'))[0].payload).toEqual({items:[{category:'kali_capture_metadata',id:'66666666-6666-4666-8666-666666666666'}]});
  const review=panel.getByLabel('Plano de exclusão');
  await expect(review).toContainText('1 item(ns) · 2 arquivo(s) · 700 bytes');
  await expect(review).not.toContainText('64.000 bytes');
  await panel.getByText('Identidade dos metadados no Kali',{exact:true}).click();
  await expect(review).toContainText('Pode liberar 1 slot');
  await expect(review).toContainText('Não remove novamente os bytes');
  await expect(review).toContainText('99999999-9999-4999-8999-999999999999');
  await expect(review).toContainText('1'.repeat(64));
  await expect(review.getByRole('list',{name:'Metadados selecionados'})).toContainText('purge.json');
  await expect(review.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toBeDisabled();
  await panel.screenshot({path:test.info().outputPath('kali-metadata-review-desktop.png')});
  await page.setViewportSize({width:390,height:844});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1)).toBe(true);
  await panel.screenshot({path:test.info().outputPath('kali-metadata-review-narrow.png')});
  await page.setViewportSize({width:1440,height:900});
  await review.getByRole('button',{name:'Fechar revisão da exclusão',exact:true}).click();
  await expect(review).toHaveCount(0);expect(await calls(page,'retention.apply')).toEqual([]);
  await expect(panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true})).toBeFocused();
  await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  await confirm(page);await expect(review.getByRole('status')).toHaveText('Metadados consolidados; slot liberado.');
  await expect(selection).toHaveCount(0);await expect(panel.getByRole('checkbox',{name:/Backup do laboratório/})).toBeVisible();
  await panel.getByRole('button',{name:'Atualizar arquivos e histórico',exact:true}).click();
  expect(await calls(page,'retention.apply')).toHaveLength(1);
});

test('consolidação Kali desconhecida é inspecionada sem repetir e restante exige outro plano',async({page})=>{
  await retentionFixture(page,'kali_metadata_unknown');const panel=page.locator('.retention');
  await panel.getByRole('checkbox',{name:/Metadados da captura do laboratório/}).check();
  await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  await confirm(page);const review=panel.getByLabel('Plano de exclusão');
  await expect(review).toContainText('Resultado desconhecido');
  await expect(review.getByRole('status')).toContainText('ainda desconhecido');
  await expect(review).not.toContainText('slot liberado');
  await expect(panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true})).toBeDisabled();
  await panel.getByRole('button',{name:'Atualizar arquivos e histórico',exact:true}).click();
  expect(await calls(page,'retention.apply')).toHaveLength(1);
  await review.getByRole('button',{name:'Fechar revisão da exclusão',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Atualizar arquivos e histórico',exact:true})).toBeFocused();
  await panel.getByText('Histórico de exclusões',{exact:true}).click();
  await panel.getByRole('button',{name:'Consultar plano',exact:true}).click();
  await review.getByRole('button',{name:'Inspecionar resultado sem excluir',exact:true}).click();
  await expect(review.getByRole('status')).toHaveText('Metadados restantes — prepare outro plano.');
  await expect(review.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toHaveCount(0);
  expect(await calls(page,'retention.reconcile')).toHaveLength(1);expect(await calls(page,'retention.apply')).toHaveLength(1);
  await panel.getByRole('checkbox',{name:/Metadados da captura do laboratório/}).check();
  await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  await expect(review).toContainText('1 item(ns) · 1 arquivo(s) · 300 bytes');
  await panel.getByText('Identidade dos metadados no Kali',{exact:true}).click();
  await expect(review).toContainText('Tentativa anterior de consolidação');
  await expect(review).toContainText('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa');
  await expect(review).toContainText('4'.repeat(64));
  await expect(review.getByRole('checkbox',{name:'Revisei os arquivos e aceito a exclusão permanente dos itens deste plano.'})).not.toBeChecked();
  await expect(review.getByLabel('Frase de confirmação',{exact:true})).toHaveValue('');
  await confirm(page);await expect(review.getByRole('status')).toContainText('slot liberado');
  const applies=await calls(page,'retention.apply');expect(applies).toHaveLength(2);expect(applies[0].payload.id).not.toEqual(applies[1].payload.id);
});

test('metadados Kali mostram a escrita interrompida revisada antes do primeiro compromisso',async({page})=>{
  await retentionFixture(page,'kali_metadata_pending');const panel=page.locator('.retention');
  await panel.getByRole('checkbox',{name:/Metadados da captura do laboratório/}).check();
  await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  const review=panel.getByLabel('Plano de exclusão');await expect(review).toContainText('1 item(ns) · 3 arquivo(s) · 828 bytes');
  await panel.getByText('Identidade dos metadados no Kali',{exact:true}).click();
  const pending=review.getByRole('region',{name:'Escrita interrompida do registro do produtor'});
  await expect(pending).toContainText('77777777-7777-4777-8777-777777777777.json.next');
  await expect(pending).toContainText('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb');
  await expect(pending).toContainText('128 bytes');await expect(pending).toContainText('8'.repeat(64));
  await expect(pending).toContainText('Trecho incompleto reconhecido pelo núcleo');
  await expect(review).not.toContainText('Tentativa anterior de consolidação');
  await expect(review.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toBeDisabled();
  await review.screenshot({path:test.info().outputPath('kali-metadata-pending-write.png')});
  await review.getByRole('button',{name:'Fechar revisão da exclusão',exact:true}).click();
  expect(await calls(page,'retention.apply')).toEqual([]);expect(await calls(page,'retention.reconcile')).toEqual([]);
  await expect(panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true})).toBeFocused();
});

test('inspeção confirma consolidação Kali após resposta perdida sem novo apply',async({page})=>{
  await retentionFixture(page,'kali_metadata_reconciled');const panel=page.locator('.retention');
  const selection=panel.getByRole('checkbox',{name:/Metadados da captura do laboratório/});await selection.check();
  await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();await confirm(page);
  const review=panel.getByLabel('Plano de exclusão');await expect(review).toContainText('Resultado desconhecido');
  await review.getByRole('button',{name:'Inspecionar resultado sem excluir',exact:true}).click();
  await expect(review.getByRole('status')).toHaveText('Metadados consolidados; slot liberado.');
  await expect(selection).toHaveCount(0);
  await panel.getByRole('button',{name:'Atualizar arquivos e histórico',exact:true}).click();
  expect(await calls(page,'retention.apply')).toHaveLength(1);expect(await calls(page,'retention.reconcile')).toHaveLength(1);
  await expect(review.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toHaveCount(0);
});

test('retenção revisa captura por hash e preserva aviso do histórico após exclusão',async({page})=>{
  await retentionFixture(page,'capture');const panel=page.locator('.retention');
  const selection=panel.getByRole('checkbox',{name:/Captura da operação local/});
  await expect(selection).toBeVisible();expect(await calls(page,'retention.apply')).toEqual([]);
  await selection.check();await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  expect((await calls(page,'retention.plan'))[0].payload).toEqual({items:[{category:'capture',id:'44444444-4444-4444-8444-444444444444'}]});
  await panel.getByText('Identidade da captura',{exact:true}).click();
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('d'.repeat(64));
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('A origem e os hashes continuarão no histórico.');
  await expect(panel.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toBeDisabled();
  await confirm(page);await expect(panel.getByLabel('Plano de exclusão')).toContainText('Exclusão concluída');
  await expect(selection).toHaveCount(0);await expect(panel.getByRole('checkbox',{name:/Backup do laboratório/})).toBeVisible();
  await panel.getByRole('button',{name:'Atualizar arquivos e histórico',exact:true}).click();
  expect(await calls(page,'retention.apply')).toHaveLength(1);
});

test('retenção exige seleção, revisão e frase exata sem exclusão automática',async({page})=>{
  await retentionFixture(page);const panel=page.locator('.retention');
  await expect(panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true})).toBeDisabled();
  expect(await calls(page,'retention.plan')).toEqual([]);expect(await calls(page,'retention.apply')).toEqual([]);
  await prepare(page);
  expect((await calls(page,'retention.plan'))[0].payload).toEqual({items:[{category:'backup',id:'11111111-1111-4111-8111-111111111111'}]});
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('1 item(ns) · 2 arquivo(s) · 120 bytes');
  await panel.getByText('Arquivos de Backup do laboratório',{exact:true}).click();
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('openkoda.sqlite3');
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('a'.repeat(64));
  await page.getByRole('checkbox',{name:'Revisei os arquivos e aceito a exclusão permanente dos itens deste plano.'}).check();
  await page.getByLabel('Frase de confirmação',{exact:true}).fill('excluir permanentemente');
  await expect(panel.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toBeDisabled();
  await page.getByLabel('Frase de confirmação',{exact:true}).fill('EXCLUIR PERMANENTEMENTE');
  await panel.screenshot({path:test.info().outputPath('retencao-revisao.png')});
  await panel.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true}).click();
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Exclusão concluída');
  expect((await calls(page,'retention.apply'))[0].payload).toEqual({id:'33333333-3333-4333-8333-333333333333',digest:'retention-fixture-digest',confirmation:'EXCLUIR PERMANENTEMENTE'});
  await expect(panel.getByRole('checkbox',{name:/Backup do laboratório/})).toHaveCount(0);
  await expect(panel.getByRole('checkbox',{name:/Relatório do laboratório/})).toBeVisible();
  expect(await calls(page,'settings.save')).toEqual([]);
});

test('retenção descarta confirmação ao mudar a seleção e bloqueia plano expirado',async({page})=>{
  await retentionFixture(page,'expired');await prepare(page);const panel=page.locator('.retention');
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Expirado');
  await expect(page.getByLabel('Frase de confirmação',{exact:true})).toBeDisabled();
  await expect(panel.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toBeDisabled();
  await panel.getByRole('checkbox',{name:/Relatório do laboratório/}).check();
  await expect(panel.getByLabel('Plano de exclusão')).toHaveCount(0);expect(await calls(page,'retention.apply')).toEqual([]);
});

test('retenção recusa arquivo alterado e atualizar inventário não repete exclusão',async({page})=>{
  await retentionFixture(page,'stale');await prepare(page);await confirm(page);const panel=page.locator('.retention');
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Recusado');
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Arquivo alterado após a revisão');
  await panel.getByRole('button',{name:'Atualizar arquivos e histórico',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Atualizar arquivos e histórico',exact:true})).toBeEnabled();
  expect(await calls(page,'retention.apply')).toHaveLength(1);
  await expect(panel.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toHaveCount(0);
});

test('retenção parcial conserva resultado e não oferece repetir a exclusão',async({page})=>{
  await retentionFixture(page,'partial');await prepare(page);await confirm(page);const panel=page.locator('.retention');
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Resultado parcial');
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Nenhuma exclusão será repetida automaticamente.');
  await expect(panel.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toHaveCount(0);
  expect(await calls(page,'retention.apply')).toHaveLength(1);
});

test('retenção desconhecida bloqueia novo plano e reconcilia apenas após inspeção explícita',async({page})=>{
  await retentionFixture(page,'unknown');await prepare(page);await confirm(page);const panel=page.locator('.retention');
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Resultado desconhecido');
  await expect(panel.getByRole('checkbox',{name:/Backup do laboratório/})).toBeDisabled();
  await expect(panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true})).toBeDisabled();
  expect(await calls(page,'retention.reconcile')).toEqual([]);
  await panel.getByRole('button',{name:'Inspecionar resultado sem excluir',exact:true}).click();
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Resultado parcial');
  await expect(panel.getByLabel('Plano de exclusão')).toContainText('Nada foi excluído pela inspeção.');
  expect((await calls(page,'retention.reconcile'))[0].payload).toEqual({id:'33333333-3333-4333-8333-333333333333'});
  expect(await calls(page,'retention.apply')).toHaveLength(1);
});

test('retenção bloqueada por avaliação ativa informa o motivo sem presumir exclusão anterior',async({page})=>{
  await retentionFixture(page,'active');const panel=page.locator('.retention');
  await expect(panel).toContainText('Encerre ou reconcilie avaliações ativas');
  await expect(panel).not.toContainText('Há uma exclusão em andamento');
  await expect(panel.getByRole('checkbox',{name:/Backup do laboratório/})).toBeDisabled();
  expect(await calls(page,'retention.plan')).toEqual([]);expect(await calls(page,'retention.apply')).toEqual([]);
});


test('nova retenção de captura parcial mostra o recibo anterior e exige outra confirmação',async({page})=>{
  await retentionFixture(page,'kali_partial');const panel=page.locator('.retention');
  await panel.getByRole('checkbox',{name:/Captura do Kali do laboratório/}).check();
  await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  await panel.getByText('Identidade da captura no Kali',{exact:true}).click();
  const review=panel.getByLabel('Plano de exclusão');
  await expect(review).toContainText('Uma inspeção confirmou que restaram bytes da exclusão anterior.');
  await expect(review).toContainText('nova confirmação');
  await expect(review).toContainText('88888888-8888-4888-8888-888888888888');
  await expect(review).toContainText('7'.repeat(64));
  await expect(panel.getByRole('button',{name:'Excluir itens revisados permanentemente',exact:true})).toBeDisabled();
  expect(await calls(page,'retention.apply')).toHaveLength(0);
  await confirm(page);await expect(review).toContainText('Exclusão concluída');
  expect(await calls(page,'retention.apply')).toHaveLength(1);
});

test('retenção apresenta cópia no Kali separada e exige confirmação sem repetir exclusão',async({page})=>{
  await retentionFixture(page,'kali_capture');const panel=page.locator('.retention');
  await expect(panel).toContainText('A cópia local e a cópia no Kali são itens separados');
  await panel.getByRole('checkbox',{name:/Captura do Kali do laboratório/}).check();
  await panel.getByRole('button',{name:'Revisar exclusão selecionada',exact:true}).click();
  expect((await calls(page,'retention.plan'))[0].payload).toEqual({items:[{category:'kali_capture',id:'66666666-6666-4666-8666-666666666666'}]});
  await panel.getByText('Identidade da captura no Kali',{exact:true}).click();
  const review=panel.getByLabel('Plano de exclusão');
  await expect(review).toContainText('f'.repeat(64));await expect(review).toContainText('9'.repeat(64));
  await expect(review).toContainText('A cópia local exige seleção própria.');
  expect(await calls(page,'retention.apply')).toHaveLength(0);
  await confirm(page);await expect(review).toContainText('Exclusão concluída');
  await panel.getByRole('button',{name:'Atualizar arquivos e histórico',exact:true}).click();
  await expect(panel.getByRole('checkbox',{name:/Backup do laboratório/})).toBeVisible();
  expect(await calls(page,'retention.apply')).toHaveLength(1);
});
