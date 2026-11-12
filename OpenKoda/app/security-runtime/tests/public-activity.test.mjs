import assert from 'node:assert/strict';
import test from 'node:test';
import {publicSessionUpdate,publicToolUpdate,progressMessage} from '../public-activity.mjs';
test('publishes public commentary, never reasoning or an analysis-channel message',()=>{
  const session=(type,payload)=>({event:{type,payload}});
  for(const type of ['agent_reasoning','agent_reasoning_raw_content','agent_reasoning_delta'])
    assert.equal(publicSessionUpdate(session('event_msg',{type,message:'private',text:'private'})),null);
  for(const channel of ['analysis','summary',undefined])assert.equal(publicSessionUpdate(session('response_item',{
    type:'message',role:'assistant',channel,content:[{type:'output_text',text:'private'}]})),null);
  const event=publicSessionUpdate(session('response_item',{type:'message',role:'assistant',channel:'commentary',
    content:[{type:'output_text',text:'Revisando entradas.'},{type:'reasoning_text',text:'private'}]}));
  assert.equal(event.message,'Revisando entradas.');assert.equal(event.stage,'message');
  const safe=publicSessionUpdate(session('event_msg',{type:'agent_message',message:'key=fixture-secret\n'+ 'x'.repeat(900)}),'fixture-secret');
  assert.ok(!safe.message.includes('fixture-secret'));assert.ok(safe.message.length<=480);
});
test('shows real operation status and bounded relative paths without command contents',()=>{
  const event=publicToolUpdate({kind:'command',status:'running',description:'echo secret-value',paths:['src/main.rs','../private','C:/private','/etc/secret','file']});
  assert.equal(event.message,'Consultando o código · src/main.rs, file');assert.ok(!JSON.stringify(event).includes('secret-value'));
  assert.equal(publicToolUpdate({kind:'reasoning',status:'completed',description:'private'}),null);
});
test('progress counts preserve the SDK unit and reject inconsistent counts',()=>{
  assert.match(progressMessage({filesCompleted:2,filesTotal:8},'Revisando.'),/2 de 8 arquivos/);
  assert.match(progressMessage({filesCompleted:2,filesTotal:8,reviewItems:{completed:3,total:12}},'Revisando.'),/3 de 12 itens/);
  assert.equal(progressMessage({filesCompleted:9,filesTotal:8},'Revisando.'),'Revisando.');
});
