import { describe, expect, it } from 'vitest';
import { createPreviewTransport, previewStorageKey } from './preview';
import { Assessment, Project, Snapshot } from './types';

function memoryStorage() {
  const values=new Map<string,string>();
  return {getItem:(key:string)=>values.get(key)??null,setItem:(key:string,value:string)=>{values.set(key,value);}};
}
async function project(request:ReturnType<typeof createPreviewTransport>) {
  return request<Project>('project.create',{name:'Laboratório',kind:'web',target:'https://lab.example.test',description:'Fixture isolada.'});
}
async function assessment(request:ReturnType<typeof createPreviewTransport>,p:Project) {
  return request<Assessment>('assessment.create',{projectId:p.id,kind:p.kind,target:p.target,title:'Revisar laboratório',objective:'Conferir limites sem executar ferramentas.'});
}
describe('preview isolada',()=>{
  it('persiste o vínculo projeto e avaliação entre instâncias',async()=>{
    const storage=memoryStorage(),request=createPreviewTransport(storage),p=await project(request),a=await assessment(request,p);
    const reopened=await createPreviewTransport(storage)<Snapshot>('snapshot');
    expect(reopened.projects[0].id).toBe(p.id);
    expect(reopened.assessments[0]).toMatchObject({id:a.id,projectId:p.id,status:'draft',target:p.target});
  });
  it('recusa uma avaliação de outro alvo e preserva o snapshot',async()=>{
    const storage=memoryStorage(),request=createPreviewTransport(storage),p=await project(request);
    const before=storage.getItem(previewStorageKey);
    await expect(request('assessment.create',{projectId:p.id,kind:p.kind,target:'https://other.example.test',title:'Fora de escopo',objective:'Não executar'})).rejects.toThrow(/alvo/);
    expect(storage.getItem(previewStorageKey)).toBe(before);
  });
  it('não simula execução ou disponibilidade do Kali',async()=>{
    const request=createPreviewTransport(memoryStorage()),a=await assessment(request,await project(request));
    await expect(request('assessment.start',{id:a.id})).rejects.toThrow(/desktop/);
    expect((await request<Snapshot>('snapshot')).assessments[0].status).toBe('draft');
    expect(await request('environment.status')).toMatchObject({status:'unavailable',tools:[]});
  });
  it('cancela rascunho sem descartar seu histórico',async()=>{
    const request=createPreviewTransport(memoryStorage()),a=await assessment(request,await project(request));
    const canceled=await request<Assessment>('assessment.cancel',{id:a.id});
    expect(canceled.status).toBe('canceled');expect(canceled.events.length).toBe(2);
    await expect(request('assessment.cancel',{id:a.id})).rejects.toThrow(/encerrada/);
  });
  it('arquiva projeto preservando avaliações e impedindo novos rascunhos',async()=>{
    const request=createPreviewTransport(memoryStorage()),p=await project(request),a=await assessment(request,p);
    await request('project.archive',{id:p.id});
    expect((await request<Snapshot>('snapshot')).assessments[0].id).toBe(a.id);
    await expect(assessment(request,p)).rejects.toThrow(/ativo/);
  });
  it('não aceita credenciais no armazenamento da prévia',async()=>{
    const storage=memoryStorage(),request=createPreviewTransport(storage);
    await expect(request('settings.save',{key:'fixture-not-a-secret'})).rejects.toThrow(/não permitido/);
    await expect(request('ai.key.save',{key:'fixture-not-a-secret'})).rejects.toThrow(/não está disponível/);
    expect(storage.getItem(previewStorageKey)).toBeNull();
  });
  it('mantém os bytes corrompidos para recuperação',async()=>{
    const storage=memoryStorage();storage.setItem(previewStorageKey,'{broken');
    await expect(createPreviewTransport(storage)('snapshot')).rejects.toThrow(/preservados/);
    expect(storage.getItem(previewStorageKey)).toBe('{broken');
  });
  it('recusa objetos incompletos sem deixar a interface falhar nem regravar dados',async()=>{
    const storage=memoryStorage();const invalid=JSON.stringify({projects:[{}],assessments:[],findings:[],settings:{}});storage.setItem(previewStorageKey,invalid);
    await expect(createPreviewTransport(storage)('snapshot')).rejects.toThrow(/preservados/);
    expect(storage.getItem(previewStorageKey)).toBe(invalid);
  });
  it('não vaza alterações de um snapshot retornado para a próxima leitura',async()=>{
    const request=createPreviewTransport(memoryStorage());await project(request);
    const returned=await request<Snapshot>('snapshot');returned.projects[0].name='Alteração externa';
    expect((await request<Snapshot>('snapshot')).projects[0].name).toBe('Laboratório');
  });
});

 it('não aceita autorização de escopo versionado na prévia',async()=>{const request=createPreviewTransport(memoryStorage());const p=await request<Project>('project.create',{name:'Escopo local',kind:'code',target:'D:/Fixture',description:''});await expect(request('assessment.create',{projectId:p.id,kind:p.kind,target:p.target,title:'Escopo nativo',objective:'Revisão',scope:{methods:[],acknowledgeScope:true}})).rejects.toThrow(/desktop/);});
