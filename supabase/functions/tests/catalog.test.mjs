import assert from 'node:assert/strict';
import test from 'node:test';
import { verifiedModels, reasoningLevels, describeCatalog, buildInference } from '../openkoda/models.mjs';

const row = {id:'brand/example',canonical_slug:'brand/example-canonical',name:'Example',
  architecture:{input_modalities:['text'],output_modalities:['text']},
  supported_parameters:['max_tokens','reasoning'],pricing:{prompt:'0.0000001',completion:'0.0000004'},
  context_length:32768,top_provider:{max_completion_tokens:8192},endpoints:['/api/v1/chat/completions']};

test('effort selection follows nested capabilities, not model identity or a generic parameter',()=>{
  assert.deepEqual(reasoningLevels(row),[]);
  assert.deepEqual(reasoningLevels({...row,reasoning:{mandatory:true}}),[]);
  assert.deepEqual(reasoningLevels({...row,reasoning:{supported_efforts:['high','low']}}),['low','high']);
  assert.deepEqual(reasoningLevels({...row,reasoning:{supported_efforts:null}}),['minimal','low','medium','high','xhigh','max']);
  assert.deepEqual(reasoningLevels({...row,reasoning:{supported_efforts:[]}}),[]);
});

test('text chat stays available without structured analysis; modality and batch-only models are distinct',()=>{
  const catalog={data:[row,{...row,id:'brand/image',architecture:{input_modalities:['text'],output_modalities:['image']}},
    {...row,id:'brand/batch',endpoints:['/api/v1/batches']},
    {...row,id:'brand/expensive',pricing:{prompt:'0.00002',completion:'0.0001'}}]};
  const models=verifiedModels(catalog), view=describeCatalog(catalog,models);
  assert.equal(models.length,1);assert.equal(models[0].analysis,false);
  assert.deepEqual(view.entries.map(e=>e.availability),['eligible','unsupported','unsupported','policy']);
  const input={action:'chat',requestId:'14b2b3cc-2222-4444-8888-000000000001',model:row.id,messages:[{role:'user',content:'Olá'}],maxTokens:64};
  assert.equal(buildInference(input,models[0]).body.max_tokens,64);
  assert.throws(()=>buildInference({...input,action:'scan',format:'report'},models[0]),{message:'analysis_unsupported'});
});

test('canonical aliases cannot inflate the eligible model count',()=>{
  const catalog={data:[row,{...row,id:'brand/alias'},row]};
  const view=describeCatalog(catalog,verifiedModels(catalog));
  assert.equal(view.eligibleCount,1);assert.equal(view.entries.length,2);
});
