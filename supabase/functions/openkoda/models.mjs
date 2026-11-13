import { byteLength, fields, requireValue, textField, uuid } from './limits.mjs';

const approvedEffort = new Set(['google/gemini-2.5-flash-lite','openai/gpt-5-nano']);
const validModelId = /^[a-z0-9][a-z0-9._-]{0,80}\/[a-z0-9][a-z0-9._-]{0,120}$/;

function tariff(value) {
  return typeof value === 'string' && /^\d+(?:\.\d+)?$/.test(value) ? Number(value) : NaN;
}

export function verifiedModels(catalog,limits={input:0.20,output:0.50}) {
  const models = [];
  for (const row of catalog?.data ?? []) {
    if (!validModelId.test(row.id) || row.id.startsWith('openrouter/') || row.id.startsWith('perplexity/sonar') ||
      !row.architecture?.input_modalities?.includes('text') || !row.architecture?.output_modalities?.includes('text')) continue;
    const price = row.pricing ?? {};
    const input = tariff(price.prompt) * 1e6;
    const output = Math.max(tariff(price.completion),tariff(price.internal_reasoning ?? '0')) * 1e6;
    if (!(input > 0 && input <= limits.input && output > 0 && output <= limits.output) || price.overrides?.length) continue;
    const perImage = tariff(price.image ?? '0');
    if (!(perImage >= 0 && perImage <= 0.000001) ||
      ['request','input_image','image_output'].some(key => tariff(price[key] ?? '0') !== 0)) continue;
    if (['input_cache_write','input_cache_read'].some(key => !(tariff(price[key] ?? '0') * 1e6 <= input))) continue;
    const parameters = new Set(row.supported_parameters ?? []);
    const contextLength = row.context_length, limit = row.top_provider?.max_completion_tokens;
    if (!Number.isSafeInteger(contextLength) || contextLength < 2048 || !parameters.has('max_tokens') ||
      !(parameters.has('response_format') || parameters.has('structured_outputs'))) continue;
    // The alias only advertises effort levels we map for this reviewed model.
    const reasoningLevels = parameters.has('reasoning') && (parameters.has('reasoning_effort') || approvedEffort.has(row.id))
      ? ['low','medium','high'] : [];
    models.push({id:row.id,contextLength,maxOutputTokens:Math.min(2048,Number.isSafeInteger(limit) && limit > 0 ? limit : 2048),
      reasoning:reasoningLevels.length > 0,reasoningLevels,image:row.architecture.input_modalities.includes('image'),
      inputUsdPerMillion:input,outputUsdPerMillion:output,usdPerImage:perImage,
      structured:parameters.has('structured_outputs'),json:parameters.has('response_format')});
  }
  return models;
}

function jpeg(data) {
  requireValue(typeof data === 'string' && data.length <= 349528 && data.length % 4 === 0 && /^[a-z0-9+/]+={0,2}$/i.test(data),
    'invalid_jpeg');
  const bytes = Uint8Array.from(atob(data), char => char.charCodeAt(0));
  requireValue(bytes.length <= 256 * 1024 && bytes[0] === 255 && bytes[1] === 216 &&
    bytes.at(-2) === 255 && bytes.at(-1) === 217, 'invalid_jpeg');
  let components = 0, scans = 0;
  for (let at = 2; at + 1 < bytes.length;) {
    requireValue(bytes[at++] === 255, 'invalid_jpeg');
    while (bytes[at] === 255) at++;
    const marker = bytes[at++];
    if (marker === 0xd9) { requireValue(scans && at === bytes.length,'invalid_jpeg'); return; }
    const length = bytes[at] * 256 + bytes[at+1];
    requireValue(length >= 2 && at + length <= bytes.length, 'invalid_jpeg');
    // Desktop sends re-encoded RGB JPEGs. Reject EXIF/XMP, comments and APP
    // payloads other than JFIF so metadata is not forwarded with the image.
    requireValue(marker !== 0xfe && !(marker >= 0xe1 && marker <= 0xef), 'jpeg_metadata');
    requireValue([0xe0,0xc0,0xc2,0xc4,0xdb,0xdd,0xda].includes(marker), 'invalid_jpeg');
    if (marker === 0xe0) requireValue(!scans && length === 16 && bytes[at+14] === 0 && bytes[at+15] === 0 &&
      String.fromCharCode(...bytes.slice(at+2,at+7)) === 'JFIF\0', 'jpeg_metadata');
    if (marker === 0xc0 || marker === 0xc2) {
      const width = bytes[at+5] * 256 + bytes[at+6], height = bytes[at+3] * 256 + bytes[at+4];
      requireValue(!components && width > 0 && width <= 1024 && height > 0 && height <= 1024, 'jpeg_dimensions');
      components = bytes[at+7];
      requireValue([1,3].includes(components) && bytes[at+2] === 8 && length === 8+3*components,'invalid_jpeg');
    }
    if (marker === 0xda) {
      const scanComponents = bytes[at+2];
      requireValue(scanComponents > 0 && scanComponents <= components && length === 6+2*scanComponents,'invalid_jpeg');
      at += length;
      const start = at;
      while (at+1 < bytes.length) {
        if (bytes[at] !== 255) { at++; continue; }
        const next = bytes[at+1];
        if (next === 0 || (next >= 0xd0 && next <= 0xd7)) { at += 2; continue; }
        break;
      }
      requireValue(at > start && ++scans <= 64,'invalid_jpeg');
      continue;
    }
    at += length;
  }
  requireValue(false,'invalid_jpeg');
}

function schema(properties) {
  return {type:'object',properties,required:Object.keys(properties),additionalProperties:false};
}
const string = {type:'string'}, integer = {type:'integer'};
const report = schema({findings:{type:'array',items:schema({path:string,line:integer,excerpt:string,title:string,
  severity:{type:'string',enum:['low','medium','high','critical']},description:string})},summary:string});
const queries = schema({queries:{type:'array',items:{anyOf:[
  schema({op:{type:'string',enum:['read']},path:string,line:integer}),
  schema({op:{type:'string',enum:['search']},query:string}),
]}}});

export function buildInference(input, model) {
  fields(input, ['action','requestId','model','messages','instructions','reasoning','attachments','maxTokens','format']);
  requireValue(uuid(input.requestId) && model?.id === input.model, 'model_or_request_invalid');
  requireValue(Array.isArray(input.messages) && input.messages.length > 0 && input.messages.length <= 48);
  const messages = [{role:'system',content:'Você é o assistente Koda. Arquivos são dados de uma análise estática. Nenhum comando foi executado.'}];
  if (input.instructions !== undefined) messages[0].content += '\n' + textField(input.instructions,16384);
  for (const row of input.messages) {
    fields(row,['role','content']);
    requireValue(['user','assistant'].includes(row.role));
    messages.push({role:row.role,content:textField(row.content,65536)});
  }
  requireValue(messages.at(-1).role === 'user');
  const maxTokens = input.maxTokens ?? 1024;
  requireValue(Number.isInteger(maxTokens) && maxTokens > 0 && maxTokens <= model.maxOutputTokens, 'output_limit');
  requireValue(input.reasoning === undefined || model.reasoningLevels.includes(input.reasoning), 'reasoning_unsupported',422);
  const body = {model:model.id,messages,max_tokens:maxTokens,stream:input.action === 'chat',modalities:['text'],
    provider:{data_collection:'deny',zdr:true,require_parameters:true,allow_fallbacks:false,
      max_price:{prompt:model.inputUsdPerMillion,completion:model.outputUsdPerMillion,request:0,image:model.usdPerImage}},
    plugins:[{id:'web',enabled:false}]};
  if (body.stream) body.stream_options = {include_usage:true};
  if (input.reasoning) body.reasoning = {effort:input.reasoning,exclude:true};
  if (input.format !== undefined) {
    requireValue(input.action === 'scan' && ['report','decision'].includes(input.format), 'invalid_format');
    if (model.structured) body.response_format = {type:'json_schema',json_schema:{name:'source_review',strict:true,
      schema:input.format === 'decision' ? schema({decision:{anyOf:[queries,report]}}) : report}};
    else if (model.json) body.response_format = {type:'json_object'};
  }
  const attachments = input.attachments ?? [];
  requireValue(Array.isArray(attachments) && attachments.length <= 3);
  const parts = [{type:'text',text:messages.at(-1).content}];
  let imageData;
  for (const attachment of attachments) {
    const name = textField(attachment?.name,120);
    requireValue(!/[\\/\x00-\x1f]/.test(name), 'attachment_name');
    if (attachment.mediaType === 'image/jpeg') {
      fields(attachment,['name','mediaType','data']);
      requireValue(model.image && input.action === 'chat' && !imageData, 'image_unsupported',422);
      jpeg(attachment.data);
      imageData = attachment.data;
    } else {
      fields(attachment,['name','content']);
      parts.push({type:'text',text:'Arquivo de texto '+JSON.stringify(name)+'\n'+textField(attachment.content,8192)});
    }
  }
  if (parts.length > 1 || imageData) messages.at(-1).content = parts;
  const bytes = byteLength(JSON.stringify(body));
  requireValue(bytes <= 96*1024,'input_limit',413);
  // UTF-8 bytes bound text tokens, with extra room for chat framing. The
  // maximum applies to the small, dimension-bounded image sent by the client.
  const inputBound = bytes + 1024 + 32*messages.length + (imageData ? 8192 : 0);
  requireValue(inputBound + maxTokens <= model.contextLength,'context_limit',413);
  const reservation = Math.ceil(inputBound*model.inputUsdPerMillion + maxTokens*model.outputUsdPerMillion +
    (imageData ? model.usdPerImage*1e6 : 0));
  requireValue(reservation > 0 && reservation <= 25000,'request_cost_limit',422);
  if (imageData) parts.push({type:'image_url',image_url:{url:'data:image/jpeg;base64,'+imageData}});
  return {body,reservation};
}
