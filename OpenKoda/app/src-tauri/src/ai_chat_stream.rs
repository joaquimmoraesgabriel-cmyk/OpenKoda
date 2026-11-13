//! Decode provider SSE frames before making any public text visible. A closed
//! socket without its protocol's terminal event never becomes a final reply.
use super::*;
use std::time::Instant;

const PUBLIC_BYTES: usize = 128 * 1024;

#[derive(Default)]
pub(in crate::ai) struct Progress {
    connected: Option<Instant>,
    text: String,
    pub(in crate::ai) generation_id: Option<String>,
}

impl Progress {
    pub fn connected(&mut self) { self.connected=Some(Instant::now()); }
    pub fn elapsed_ms(&self)->Option<u64> {self.connected.map(|at|at.elapsed().as_millis() as u64)}
    pub fn visible(&self)->&str {
        // A credential is an unbroken token. A prefix split across transport
        // frames must not escape before the ordinary full-text redactor sees it.
        let end=self.text.rfind(char::is_whitespace).unwrap_or(0);
        &self.text[..end]
    }
    pub(in crate::ai) fn append(&mut self,delta:&str)->Result<(),String>{
        if self.text.len()+delta.len()>PUBLIC_BYTES {return Err("O texto excedeu o limite da conversa.".into());}
        self.text.push_str(delta);
        Ok(())
    }
}

pub(super) struct Stream {
    provider: ProviderId,
    progress: Arc<Mutex<Progress>>,
    line: Vec<u8>,
    data: String,
    message: Value,
    end: bool,
    outcome: Option<Value>,
}

impl Stream {
    pub fn new(provider:ProviderId,progress:Arc<Mutex<Progress>>)->Self {
        Self {provider,progress,line:vec![],data:String::new(),message:json!({}),end:false,outcome:None}
    }
    pub fn push(&mut self,chunk:&[u8])->Result<(),String>{
        for &byte in chunk {
            if byte!=b'\n' {self.line.push(byte);continue;}
            let line=std::mem::take(&mut self.line);
            let line=std::str::from_utf8(&line).map_err(|_|"Evento de texto inválido.")?.trim_end_matches('\r');
            if line.is_empty(){
                if !self.data.is_empty(){let data=std::mem::take(&mut self.data);self.frame(&data)?;}
            }else if let Some(data)=line.strip_prefix("data:"){
                if !self.data.is_empty(){self.data.push('\n');}
                self.data.push_str(data.strip_prefix(' ').unwrap_or(data));
            }
        }
        Ok(())
    }
    fn frame(&mut self,raw:&str)->Result<(),String>{
        if raw.trim()=="[DONE]" {
            if matches!(self.provider,ProviderId::OpenAI|ProviderId::Anthropic) {return Ok(());}
            self.end=true;return Ok(());
        }
        let event:Value=serde_json::from_str(raw).map_err(|_|"Evento inválido na resposta da IA.")?;
        if event.get("error").is_some_and(|e|!e.is_null())||event["type"]=="error" {
            return Err("O provedor interrompeu a resposta.".into());
        }
        if self.provider == ProviderId::OpenRouter && self.message["id"].is_null() {
            if let Some(id) = event["id"].as_str().filter(|id| {
                id.len() <= 128 && id.strip_prefix("gen-").is_some_and(|suffix|
                    !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
            }) {
                self.progress.lock().map_err(failure)?.generation_id = Some(id.to_owned());
            }
        }
        let delta=match self.provider {
            ProviderId::OpenAI=>match event["type"].as_str() {
                Some("response.output_text.delta")=>event["delta"].as_str(),
                Some("response.completed"|"response.incomplete"|"response.failed")=>{
                    self.end=true;self.outcome=Some(event["response"].clone());None
                }
                _=>None,
            },
            ProviderId::Anthropic=>match event["type"].as_str() {
                Some("message_start")=>{self.message=event["message"].clone();None}
                Some("content_block_start") if event["content_block"]["type"]=="text"=>event["content_block"]["text"].as_str(),
                Some("content_block_delta") if event["delta"]["type"]=="text_delta"=>event["delta"]["text"].as_str(),
                Some("message_delta")=>{
                    self.message["stop_reason"]=event["delta"]["stop_reason"].clone();
                    if let Some(usage)=event["usage"].as_object(){
                        for (key,value) in usage {self.message["usage"][key]=value.clone();}
                    }
                    None
                }
                Some("message_stop")=>{self.end=true;None}
                _=>None,
            },
            _=>{
                for key in ["id","model","usage"] {if !event[key].is_null(){self.message[key]=event[key].clone();}}
                if let Some(usage)=event["x_groq"].get("usage"){self.message["usage"]=usage.clone();}
                let choice=event["choices"].as_array().and_then(|choices|choices.iter().find(|c|c["index"].as_u64().unwrap_or(0)==0));
                if let Some(choice)=choice {
                    if !choice["finish_reason"].is_null(){self.message["finish_reason"]=choice["finish_reason"].clone();}
                    choice["delta"]["content"].as_str()
                } else {None}
            }
        };
        if let Some(text)=delta {self.progress.lock().map_err(failure)?.append(text)?;}
        Ok(())
    }
    pub fn finish(mut self)->Result<Value,String>{
        if !self.end {return Err("O fluxo terminou antes da confirmação do provedor.".into());}
        let text=self.progress.lock().map_err(failure)?.text.clone();
        match self.provider {
            ProviderId::OpenAI=>self.outcome.ok_or_else(||"O provedor não confirmou a resposta.".into()),
            ProviderId::Anthropic=>{
                self.message["content"]=json!([{"type":"text","text":text}]);
                self.provider.normalize(self.message)
            }
            _=>{
                self.message["choices"]=json!([{"finish_reason":self.message["finish_reason"],"message":{"content":text}}]);
                self.provider.normalize(self.message)
            }
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn exercise(p:ProviderId,frames:&[Value],done:bool)->Value{
        let progress=Arc::new(Mutex::new(Progress::default()));
        let mut stream=Stream::new(p,progress.clone());
        for frame in frames {
            let packet=format!(": keepalive\r\nevent: chunk\r\ndata: {}\r\n\r\n",frame);
            for byte in packet.as_bytes().chunks(1){stream.push(byte).unwrap();}
        }
        if done {stream.push(b"data: [DONE]\n\n").unwrap();}
        assert_eq!(progress.lock().unwrap().visible(),"Olá");
        stream.finish().unwrap()
    }
    #[test]
    fn byte_splits_unicode_and_private_deltas_are_not_public(){
        let a=exercise(ProviderId::Anthropic,&[
            json!({"type":"message_start","message":{"id":"a","model":"m","usage":{"input_tokens":7}}}),
            json!({"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"PRIVATE"}}),
            json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"Olá mundo"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}),
            json!({"type":"message_stop"})],false);
        assert_eq!(a["usage"]["input_tokens"],7);assert_eq!(a["output"][0]["content"][0]["text"],"Olá mundo");
        for provider in ProviderId::ALL.into_iter().filter(|p|!matches!(p,ProviderId::OpenAI|ProviderId::Anthropic)){
            let result=exercise(provider,&[
                json!({"choices":[{"delta":{"reasoning":"PRIVATE","content":"Olá "}}]}),
                json!({"choices":[{"delta":{"content":"mundo"},"finish_reason":"stop"}]}),
                json!({"choices":[],"usage":{"prompt_tokens":4,"completion_tokens":2}})],true);
            assert_eq!(result["usage"]["input_tokens"],4);assert_eq!(result["status"],"completed");
        }
        let final_response=json!({"id":"r","status":"completed","usage":{"input_tokens":5}});
        let result=exercise(ProviderId::OpenAI,&[
            json!({"type":"response.reasoning_text.delta","delta":"PRIVATE"}),
            json!({"type":"response.output_text.delta","delta":"Olá mundo"}),
            json!({"type":"response.completed","response":final_response})],false);
        assert_eq!(result,final_response);
    }
    #[test]
    fn a_disconnection_or_in_band_error_cannot_commit_a_partial_reply(){
        let progress=Arc::new(Mutex::new(Progress::default()));
        let mut stream=Stream::new(ProviderId::OpenRouter,progress.clone());
        stream.push(b"data: {\"id\":\"gen-123-fixture\",\"choices\":[{\"delta\":{\"content\":\"hello \"}}]}\n\n").unwrap();
        assert_eq!(progress.lock().unwrap().visible(),"hello");
        assert!(stream.finish().is_err());
        assert_eq!(progress.lock().unwrap().generation_id.as_deref(),Some("gen-123-fixture"));
        let mut stream=Stream::new(ProviderId::OpenRouter,progress);
        assert!(stream.push(b"data: {\"error\":{\"message\":\"private details\"}}\n\n").is_err());
    }
}
