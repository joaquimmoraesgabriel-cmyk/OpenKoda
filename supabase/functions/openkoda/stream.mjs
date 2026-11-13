import { ApiError, requireValue } from './limits.mjs';

export const generationId = value => typeof value === 'string' && /^gen-[a-zA-Z0-9-]{1,124}$/.test(value);

export function metering(usage, id) {
  requireValue(generationId(id), 'generation_unconfirmed',502);
  requireValue(usage && Number.isSafeInteger(usage.prompt_tokens) && usage.prompt_tokens >= 0 &&
    Number.isSafeInteger(usage.completion_tokens) && usage.completion_tokens >= 0 &&
    typeof usage.cost === 'number' && Number.isFinite(usage.cost) && usage.cost >= 0 && Number.isSafeInteger(Math.ceil(usage.cost*1e6)),
    'usage_unconfirmed',502);
  return {id,inputTokens:usage.prompt_tokens,outputTokens:usage.completion_tokens,costUsd:usage.cost};
}

// OpenRouter sends Chat Completions frames terminated by a blank line. Only
// public text is forwarded; final usage is emitted after the durable settlement.
export function chatStream(upstream, requestId, model, controller, settle, recordFailure, cleanup) {
  const encoder = new TextEncoder();
  let downstreamClosed = false;
  return new ReadableStream({
    async start(output) {
      const reader = upstream.body.getReader();
      const decoder = new TextDecoder('utf-8',{fatal:true});
      let buffer = '', total = 0, id = upstream.headers.get('x-generation-id'), usage, finish, done = false;
      const emit = data => {
        if (!downstreamClosed) output.enqueue(encoder.encode('data: '+data+'\n\n'));
      };
      const process = frame => {
        const lines = frame.split(/\r?\n/).filter(line => line.startsWith('data:'));
        if (!lines.length) return;
        const data = lines.map(line => line.slice(5).trimStart()).join('\n');
        if (data === '[DONE]') { done = true; return; }
        requireValue(!done,'invalid_stream',502);
        const chunk = JSON.parse(data);
        requireValue(!chunk.error && (!chunk.model || chunk.model === model), 'provider_error',502);
        if (chunk.id) {
          requireValue(!id || chunk.id === id, 'generation_unconfirmed',502);
          id = chunk.id;
        }
        const choice = chunk.choices?.[0];
        const delta = choice?.delta?.content;
        if (delta != null) {
          requireValue(typeof delta === 'string','invalid_stream',502);
          if (delta) emit(JSON.stringify({id:requestId,model,choices:[{index:0,delta:{content:delta},finish_reason:null}]}));
        }
        if (choice?.finish_reason) finish = choice.finish_reason;
        if (chunk.usage) usage = chunk.usage;
      };
      try {
        while (!done) {
          const part = await reader.read();
          if (part.done) break;
          total += part.value.length;
          requireValue(total <= 2*1024*1024,'provider_output_limit',502);
          buffer += decoder.decode(part.value,{stream:true});
          let boundary;
          while ((boundary = /\r?\n\r?\n/.exec(buffer))) {
            process(buffer.slice(0,boundary.index));
            buffer = buffer.slice(boundary.index+boundary[0].length);
          }
        }
        requireValue(done && ['stop','length'].includes(finish), 'generation_unconfirmed',502);
        const measure = metering(usage,id);
        await settle('completed',measure);
        emit(JSON.stringify({id:requestId,model,choices:[{index:0,delta:{},finish_reason:finish}],
          usage:{prompt_tokens:measure.inputTokens,completion_tokens:measure.outputTokens,cost:measure.costUsd}}));
        emit('[DONE]');
      } catch {
        await recordFailure(id);
        emit(JSON.stringify({error:{code:'outcome_unknown'},requestId}));
      } finally {
        cleanup();
        await reader.cancel().catch(() => {});
        if (!downstreamClosed) output.close();
      }
    },
    cancel() { downstreamClosed = true; controller.abort(new ApiError(499,'client_disconnected')); },
  });
}
