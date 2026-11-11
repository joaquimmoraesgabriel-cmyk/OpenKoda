import { test, expect } from './fixtures';

for (const outcome of ['ausente','preservada'] as const) test('retirada confirma a identidade e termina com a distribuição '+outcome, async ({page}) => {
  await page.addInitScript(() => {
    const id = crypto.randomUUID();
    const registration = {guid: crypto.randomUUID(), name: 'OpenKoda-Kali', basePath: 'C:\\fixture\\kali-distro', version: 2};
    const plan = {id: crypto.randomUUID(), digest: 'a'.repeat(64), expiresAt:'2099-01-01T00:00:00Z',
      payload:{environmentId:id, registration, spool:{rawCaptures:1,terminalCaptures:2,sessionRecords:3}, effects:['Os dados Linux podem ser apagados.']}};
    const w = window as unknown as {isTauri:boolean; __TAURI_INTERNALS__: unknown; externalKaliState: string; detachments:number};
    w.isTauri = true;
    w.externalKaliState = 'registration_conflict';
    w.detachments = 0;
    let observation = 'awaiting_windows';
    const record = () => ({environmentId:id, registration, observation,
      pathStatus: observation === 'registration_absent' ? 'remaining' : null});
    w.__TAURI_INTERNALS__ = {invoke: async (cmd:string, args:{request:{action:string;payload:Record<string,unknown>}}) => {
      if (cmd !== 'request') throw new Error('Unexpected IPC');
      const {action,payload} = args.request;
      switch(action) {
        case 'snapshot': return {schema:'openkoda.snapshot-view.v1',projection:{schema:'openkoda.projection.v1'},projects:[],assessments:[],findings:[],settings:{theme:'light',mode:'assisted'},plans:[],executions:[],validations:[]};
        case 'environment.status': return {status: w.detachments ? 'degraded' : 'ready',distro:registration.name,tools:[],message:'Kali da fixture'};
        case 'projectPurge.status': return {hasUnresolved:false,hasPartialHistory:false,journals:[]};
        case 'packages.catalog': return {tools:[],inventoryVerified:true};
        case 'data.session.select': return {};
        case 'environment.removal.plan': return plan;
        case 'environment.removal.detach':
          if(payload.planId!==plan.id || payload.digest!==plan.digest || payload.confirmName!==registration.name || payload.acceptLinuxDataLoss!==true) throw new Error('Invalid confirmation');
          w.detachments++;
          return {record:record(),confirmedNow:false};
        case 'environment.removal.status': return {record:w.detachments ? record() : null,confirmedNow:false};
        case 'environment.removal.inspect':
          observation=w.externalKaliState;
          return {record:record(),confirmedNow:observation==='still_registered'};
        case 'environment.removal.keep':
          if(payload.environmentId!==id || payload.keepDistribution!==true) throw new Error('Invalid release');
          observation='owner_released';
          return {record:record(),confirmedNow:false};
        default: throw new Error('Unexpected action '+action);
      }
    }};
  });
  await page.goto('/');
  await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Manutenção',exact:true}).click();
  const panel=page.getByRole('region',{name:'Remoção do Kali'});
  await expect(panel.getByRole('link',{name:'exportar os arquivos do Kali'})).toHaveAttribute('href','#kali-export');
  await panel.getByRole('button',{name:'Revisar retirada do Kali'}).click();
  const submit=panel.getByRole('button',{name:'Desativar este executor'});
  const name=panel.getByLabel('Digite o nome da distribuição a retirar');
  await name.fill('Ubuntu');
  await panel.getByRole('checkbox').check();
  await expect(submit).toBeDisabled();
  await name.fill('OpenKoda-Kali');
  await submit.click();
  const inspect=panel.getByRole('button',{name:'Conferir registro no Windows'});
  const command=panel.getByText('wsl.exe --unregister OpenKoda-Kali',{exact:true});
  await expect(inspect).toBeEnabled();
  await expect(command).toHaveCount(0);
  await inspect.click();
  await expect(panel).toContainText('O registro atual diverge da retirada.');
  await expect(command).toHaveCount(0);
  if(outcome==='preservada'){
    await panel.getByText('Preservar a distribuição',{exact:true}).click();
    const keep=panel.getByRole('button',{name:'Encerrar vínculo local'});
    await expect(keep).toBeDisabled();
    await panel.getByRole('checkbox',{name:/Quero concluir/}).check();
    await keep.click();
    await expect(panel).toContainText('Vínculo encerrado.');
    await expect(command).toHaveCount(0);
    return;
  }
  await page.evaluate(()=>{(window as unknown as {externalKaliState:string}).externalKaliState='still_registered';});
  await inspect.click();
  await expect(command).toBeVisible();
  await panel.getByRole('button',{name:'Consultar retirada registrada'}).click();
  await expect(command).toHaveCount(0);
  await page.evaluate(()=>{(window as unknown as {externalKaliState:string}).externalKaliState='registration_absent';});
  await inspect.click();
  await expect(panel).toContainText('O local original ainda existe.');
  expect(await page.evaluate(()=>(window as unknown as {detachments:number}).detachments)).toBe(1);
});
